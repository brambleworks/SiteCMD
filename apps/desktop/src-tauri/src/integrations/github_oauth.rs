//! GitHub's public-client device authorization flow.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

/// Bundled OAuth client identifier - loaded from environment variables at runtime.
/// Set GITHUB_CLIENT_ID before building, or the flow will return an error when initiated.
#[tracing::instrument]
pub fn client_id() -> &'static str {
    static ID: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        std::env::var("GITHUB_CLIENT_ID")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                option_env!("GITHUB_CLIENT_ID")
                    .unwrap_or_default()
                    .to_string()
            })
    });
    &ID
}

/// Stored as JSON in `integration_configs.extra`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubTokens {
    pub access_token: String,
    /// Token creation timestamp.
    pub created_at: i64,
}

/// Classic OAuth scope required to read private repositories.
pub const SCOPES: &[&str] = &["repo"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubDeviceFlow {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

fn validate_device_flow(flow: GitHubDeviceFlow) -> Result<GitHubDeviceFlow, String> {
    let verification_uri = url::Url::parse(&flow.verification_uri)
        .map_err(|_| "GitHub returned an invalid verification URL".to_string())?;
    let expected_origin = verification_uri.scheme() == "https"
        && verification_uri.host_str() == Some("github.com")
        && verification_uri.port_or_known_default() == Some(443)
        && verification_uri.username().is_empty()
        && verification_uri.password().is_none()
        && verification_uri.path() == "/login/device"
        && verification_uri.query().is_none()
        && verification_uri.fragment().is_none();
    if !expected_origin {
        return Err("GitHub returned an unexpected verification URL".to_string());
    }
    if flow.device_code.is_empty()
        || flow.device_code.len() > 1024
        || flow.user_code.is_empty()
        || flow.user_code.len() > 128
        || flow
            .user_code
            .chars()
            .any(|character| character.is_control())
    {
        return Err("GitHub returned invalid device authorization data".to_string());
    }
    Ok(flow)
}

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    interval: Option<u64>,
}

/// Start GitHub's device authorization flow and return the user-entered code.
#[tracing::instrument(fields(client_id = %client_id))]
pub async fn start_device_flow(client_id: &str) -> Result<GitHubDeviceFlow, String> {
    let client = crate::http_client::credentialed_service_client();
    let scope = SCOPES.join(" ");

    let resp = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", &scope)])
        .timeout(crate::constants::API_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("GitHub device flow failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let _ = crate::http_client::read_text_limited(
            resp,
            crate::constants::OAUTH_RESPONSE_MAX_BYTES,
            crate::constants::API_TIMEOUT_SHORT,
        )
        .await;
        return Err(format!("GitHub device flow returned {status}"));
    }

    let flow = crate::http_client::read_json_limited(
        resp,
        crate::constants::OAUTH_RESPONSE_MAX_BYTES,
        crate::constants::API_TIMEOUT_SHORT,
    )
    .await
    .map_err(|e| format!("Failed to parse GitHub device flow response: {}", e))?;
    validate_device_flow(flow)
}

/// Poll GitHub until the user approves the device code or the flow expires.
#[tracing::instrument(skip(device_code), fields(client_id = %client_id))]
pub async fn poll_device_flow(
    client_id: &str,
    device_code: &str,
    expires_in: u64,
    interval: u64,
) -> Result<GitHubTokens, String> {
    let client = crate::http_client::credentialed_service_client();
    // Clamp the server-provided lifetime so a malformed or hostile response
    // cannot overflow Instant + Duration (a panic). GitHub device codes expire
    // well within an hour, so an hour ceiling is a safe upper bound.
    let deadline = Instant::now() + Duration::from_secs(expires_in.clamp(1, 3600));
    let mut poll_interval = Duration::from_secs(interval.max(5));

    loop {
        if Instant::now() >= deadline {
            return Err("GitHub OAuth device flow expired - please reconnect".into());
        }

        let resp = client
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .timeout(crate::constants::API_TIMEOUT)
            .send()
            .await
            .map_err(|e| format!("GitHub token polling failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let _ = crate::http_client::read_text_limited(
                resp,
                crate::constants::OAUTH_RESPONSE_MAX_BYTES,
                crate::constants::API_TIMEOUT_SHORT,
            )
            .await;
            return Err(format!("GitHub token polling returned {status}"));
        }

        let body: GitHubTokenResponse = crate::http_client::read_json_limited(
            resp,
            crate::constants::OAUTH_RESPONSE_MAX_BYTES,
            crate::constants::API_TIMEOUT_SHORT,
        )
        .await
        .map_err(|e| format!("Failed to parse GitHub token response: {}", e))?;

        if let Some(access_token) = body.access_token {
            return Ok(GitHubTokens {
                access_token,
                created_at: chrono::Utc::now().timestamp(),
            });
        }

        match body.error.as_deref() {
            Some("authorization_pending") => {
                tokio::time::sleep(poll_interval).await;
            }
            Some("slow_down") => {
                poll_interval += Duration::from_secs(body.interval.unwrap_or(5).max(5));
                tokio::time::sleep(poll_interval).await;
            }
            Some("expired_token") => {
                return Err("GitHub OAuth device flow expired - please reconnect".into());
            }
            Some("access_denied") => {
                return Err("GitHub OAuth was cancelled in the browser".into());
            }
            Some(_) => {
                return Err("GitHub OAuth was rejected. Reconnect and try again.".to_string());
            }
            None => {
                return Err("GitHub OAuth response did not include an access token".into());
            }
        }
    }
}

/// Fetch the list of repos the user has access to (for picking one)
#[tracing::instrument(skip(token))]
pub async fn list_repos(token: &str) -> Result<Vec<RepoInfo>, String> {
    let client = crate::http_client::credentialed_service_client();
    let mut repos = Vec::new();
    let mut page = 1;

    // Fetch up to 3 pages (300 repos)
    loop {
        let url = format!(
            "https://api.github.com/user/repos?per_page=100&sort=pushed&direction=desc&page={}",
            page
        );

        let resp = client
            .get(&url)
            .header("User-Agent", crate::constants::USER_AGENT.as_str())
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {}", token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| format!("GitHub API error: {}", e))?;

        if !resp.status().is_success() {
            break;
        }

        let page_repos: Vec<GhRepo> = crate::http_client::read_json_limited(
            resp,
            crate::constants::MAX_BODY_SIZE,
            crate::constants::API_TIMEOUT_SHORT,
        )
        .await
        .unwrap_or_default();
        if page_repos.is_empty() {
            break;
        }

        for r in &page_repos {
            repos.push(RepoInfo {
                full_name: r.full_name.clone(),
                description: r.description.clone(),
                private: r.private,
                default_branch: r.default_branch.clone(),
                pushed_at: r.pushed_at.clone(),
            });
        }

        if page_repos.len() < 100 || page >= 3 {
            break;
        }
        page += 1;
    }

    Ok(repos)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
    pub default_branch: String,
    pub pushed_at: Option<String>,
}

#[derive(Deserialize)]
struct GhRepo {
    full_name: String,
    description: Option<String>,
    private: bool,
    default_branch: String,
    pushed_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{validate_device_flow, GitHubDeviceFlow, SCOPES};

    #[test]
    fn classic_oauth_requests_only_the_repository_scope() {
        assert_eq!(SCOPES, &["repo"]);
    }

    fn device_flow(verification_uri: &str) -> GitHubDeviceFlow {
        GitHubDeviceFlow {
            device_code: "device-code".to_string(),
            user_code: "ABCD-1234".to_string(),
            verification_uri: verification_uri.to_string(),
            expires_in: 900,
            interval: 5,
        }
    }

    #[test]
    fn device_flow_accepts_only_the_expected_github_verification_url() {
        assert!(validate_device_flow(device_flow("https://github.com/login/device")).is_ok());

        for unexpected in [
            "http://github.com/login/device",
            "https://evil.example/login/device",
            "https://github.com.evil.example/login/device",
            "https://github.com/login/device?next=https://evil.example",
            "file:///tmp/device",
        ] {
            assert!(
                validate_device_flow(device_flow(unexpected)).is_err(),
                "{unexpected} should be rejected"
            );
        }
    }
}
