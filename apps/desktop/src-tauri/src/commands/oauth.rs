use crate::db::Database;
use crate::integrations::{self, IntegrationConfig, IntegrationType};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

use super::sanitize_error;
use super::TokioMutex;

/// Type alias for pending OAuth state.
struct OAuthFlow<T> {
    project_id: i64,
    payload: T,
}

struct GooglePendingFlow {
    redirect_port: u16,
    receiver: tokio::sync::oneshot::Receiver<String>,
    code_verifier: String,
}

type OAuthPending = HashMap<String, OAuthFlow<GooglePendingFlow>>;
type GoogleTokenStore = HashMap<String, OAuthFlow<integrations::google_oauth::GoogleTokens>>;

#[allow(clippy::type_complexity)]
static OAUTH_PENDING: LazyLock<TokioMutex<OAuthPending>> =
    LazyLock::new(|| TokioMutex::new(HashMap::new()));
static OAUTH_TOKENS: LazyLock<TokioMutex<GoogleTokenStore>> =
    LazyLock::new(|| TokioMutex::new(HashMap::new()));

type GitHubPendingStore = HashMap<String, OAuthFlow<integrations::github_oauth::GitHubDeviceFlow>>;
type GitHubTokenStore = HashMap<String, OAuthFlow<integrations::github_oauth::GitHubTokens>>;

#[allow(clippy::type_complexity)]
static GH_OAUTH_PENDING: LazyLock<TokioMutex<GitHubPendingStore>> =
    LazyLock::new(|| TokioMutex::new(HashMap::new()));
static GH_OAUTH_TOKENS: LazyLock<TokioMutex<GitHubTokenStore>> =
    LazyLock::new(|| TokioMutex::new(HashMap::new()));

// allow-inline-duration: OAuth-flow TTL is intrinsic to this module and
// has no equivalent in `constants.rs`.
const OAUTH_FLOW_TTL: Duration = Duration::from_secs(10 * 60);

fn generate_flow_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|e| sanitize_error(format!("Failed to start OAuth flow: {}", e)))?;
    Ok(hex::encode(bytes))
}

fn take_project_bound_flow<T>(
    store: &mut HashMap<String, OAuthFlow<T>>,
    flow_id: &str,
    project_id: i64,
    missing_message: &str,
    mismatch_message: &str,
) -> Result<T, String> {
    let flow_project_id = store.get(flow_id).ok_or(missing_message)?.project_id;
    if flow_project_id != project_id {
        return Err(mismatch_message.to_string());
    }
    store
        .remove(flow_id)
        .map(|flow| flow.payload)
        .ok_or_else(|| missing_message.to_string())
}

/// Read a project-bound OAuth payload without consuming the shared flow.
fn peek_project_bound_flow<T: Clone>(
    store: &HashMap<String, OAuthFlow<T>>,
    flow_id: &str,
    project_id: i64,
    missing_message: &str,
    mismatch_message: &str,
) -> Result<T, String> {
    let flow = store.get(flow_id).ok_or(missing_message)?;
    if flow.project_id != project_id {
        return Err(mismatch_message.to_string());
    }
    Ok(flow.payload.clone())
}

fn spawn_google_flow_cleanup(flow_id: String) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(OAUTH_FLOW_TTL).await;
        OAUTH_PENDING.lock().await.remove(&flow_id);
        OAUTH_TOKENS.lock().await.remove(&flow_id);
    });
}

fn spawn_github_flow_cleanup(flow_id: String) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(OAUTH_FLOW_TTL).await;
        GH_OAUTH_PENDING.lock().await.remove(&flow_id);
        GH_OAUTH_TOKENS.lock().await.remove(&flow_id);
    });
}

/// Start Google OAuth and open the system browser.
#[tracing::instrument(fields(project_id))]
pub async fn connect_google(project_id: i64) -> Result<serde_json::Value, String> {
    let client_id = integrations::google_oauth::client_id();
    if client_id.is_empty() {
        return Err("Google OAuth not configured - GOOGLE_CLIENT_ID is not set".into());
    }

    // Generate CSRF state and start callback server with state verification
    let state = integrations::google_oauth::generate_state();
    let pkce = integrations::google_oauth::generate_pkce_pair()?;
    let (port, rx) = integrations::google_oauth::start_callback_server(state.clone()).await?;
    let auth_url =
        integrations::google_oauth::build_auth_url(client_id, port, &state, &pkce.challenge);

    open::that(&auth_url).map_err(|e| sanitize_error(format!("Failed to open browser: {}", e)))?;

    let flow_id = generate_flow_id()?;

    {
        let mut lock = OAUTH_PENDING.lock().await;
        lock.insert(
            flow_id.clone(),
            OAuthFlow {
                project_id,
                payload: GooglePendingFlow {
                    redirect_port: port,
                    receiver: rx,
                    code_verifier: pkce.verifier,
                },
            },
        );
    }
    spawn_google_flow_cleanup(flow_id.clone());

    Ok(serde_json::json!({ "flow_id": flow_id }))
}

/// Decide which already-configured Google services should have their tokens
/// refreshed after a reconnect. Pure (no IO) so it is unit-testable: returns
/// each configured Google integration paired with its saved site id.
fn google_services_to_resave(configs: &[IntegrationConfig]) -> Vec<(IntegrationType, String)> {
    configs
        .iter()
        .filter_map(|config| {
            if !matches!(
                config.integration_type,
                IntegrationType::GoogleAnalytics | IntegrationType::GoogleSearchConsole
            ) {
                return None;
            }
            let site_id = config.site_id.clone().filter(|site| !site.is_empty())?;
            Some((config.integration_type.clone(), site_id))
        })
        .collect()
}

/// Persist fresh Google tokens across existing Google integrations.
fn persist_google_reconnect(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    tokens: &integrations::google_oauth::GoogleTokens,
) -> Vec<String> {
    let configs = match db.get_integrations(project_id) {
        Ok(configs) => configs,
        Err(_) => return Vec::new(),
    };
    let extra = serde_json::json!({ "tokens": tokens });
    let mut saved = Vec::new();
    for (integration_type, site_id) in google_services_to_resave(&configs) {
        let type_name = match integration_type {
            IntegrationType::GoogleSearchConsole => "googlesearchconsole",
            _ => "googleanalytics",
        };
        let config = IntegrationConfig {
            integration_type,
            api_key: None,
            site_id: Some(site_id),
            extra: Some(extra.clone()),
            enabled: true,
        };
        if super::integrations::persist_integration_config_securely(app, db, project_id, &config)
            .is_ok()
        {
            saved.push(type_name.to_string());
        }
    }
    saved
}

#[cfg(test)]
mod reconnect_tests {
    use super::*;

    fn cfg(integration_type: IntegrationType, site: Option<&str>) -> IntegrationConfig {
        IntegrationConfig {
            integration_type,
            api_key: None,
            site_id: site.map(|s| s.to_string()),
            extra: None,
            enabled: true,
        }
    }

    #[test]
    fn resaves_only_configured_google_services_with_a_site() {
        let configs = vec![
            cfg(
                IntegrationType::GoogleSearchConsole,
                Some("sc-domain:example.com"),
            ),
            cfg(IntegrationType::GoogleAnalytics, Some("properties/123")),
            cfg(IntegrationType::Plausible, Some("example.com")),
            cfg(IntegrationType::GoogleAnalytics, None),
            cfg(IntegrationType::GoogleSearchConsole, Some("")),
        ];
        let sites: Vec<String> = google_services_to_resave(&configs)
            .into_iter()
            .map(|(_, site)| site)
            .collect();
        assert_eq!(sites, vec!["sc-domain:example.com", "properties/123"]);
    }
}

/// Complete Google OAuth and return available properties or sites.
#[tracing::instrument(skip(app, db), fields(project_id, flow_id = %flow_id))]
pub async fn complete_google_oauth(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    flow_id: String,
) -> Result<serde_json::Value, String> {
    let audit_detail = serde_json::json!({ "provider": "google", "project_id": project_id });
    let client_id = integrations::google_oauth::client_id();
    if client_id.is_empty() {
        crate::audit_log::record("oauth.complete", audit_detail, "fail");
        return Err("Google OAuth not configured - GOOGLE_CLIENT_ID is not set".into());
    }
    let inner = async {
        let pending = {
            let mut lock = OAUTH_PENDING.lock().await;
            take_project_bound_flow(
                &mut lock,
                &flow_id,
                project_id,
                "No pending OAuth flow",
                "OAuth flow belongs to a different project - reconnect and try again",
            )?
        };

        // Wait for callback (up to 120 seconds)
        let code = tokio::time::timeout(crate::constants::OAUTH_TIMEOUT, pending.receiver)
            .await
            .map_err(|_| "OAuth timed out - no response within 2 minutes".to_string())?
            .map_err(|_| "OAuth callback channel closed".to_string())?;

        let tokens = integrations::google_oauth::exchange_code(
            client_id,
            &pending.code_verifier,
            &code,
            pending.redirect_port,
        )
        .await?;

        // Fetch available properties and sites using the new tokens
        let (ga4_properties, gsc_sites) = tokio::join!(
            integrations::google_analytics::list_properties(&tokens.access_token),
            integrations::search_console::list_sites(&tokens.access_token),
        );

        let auto_saved = persist_google_reconnect(&app, &db, project_id, &tokens);

        // Store tokens for step 3 (the property picker / fresh-connect path)
        {
            let mut lock = OAUTH_TOKENS.lock().await;
            lock.insert(
                flow_id,
                OAuthFlow {
                    project_id,
                    payload: tokens,
                },
            );
        }

        if !auto_saved.is_empty() {
            let _ = app.emit(
                "google-integration-updated",
                serde_json::json!({ "projectId": project_id }),
            );
        }

        let ga4_error = ga4_properties
            .as_ref()
            .err()
            .map(|e| sanitize_error(e.clone()));
        let gsc_error = gsc_sites.as_ref().err().map(|e| sanitize_error(e.clone()));

        Ok::<_, String>(serde_json::json!({
            "ga4_properties": ga4_properties.unwrap_or_default(),
            "gsc_sites": gsc_sites.unwrap_or_default(),
            "ga4_error": ga4_error,
            "gsc_error": gsc_error,
            "auto_saved": auto_saved,
        }))
    };

    match inner.await {
        Ok(value) => {
            crate::audit_log::record("oauth.complete", audit_detail, "ok");
            Ok(value)
        }
        Err(e) => {
            crate::audit_log::record("oauth.complete", audit_detail, "fail");
            let error = sanitize_error(e);
            tracing::warn!("Google OAuth completion failed: {}", error);
            Err(error)
        }
    }
}

/// Save the selected Google integration and OAuth tokens.
#[tracing::instrument(skip(app, db), fields(project_id, flow_id = %flow_id, integration_type = %integration_type, site_id = %site_id))]
pub async fn save_google_integration(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    flow_id: String,
    integration_type: String,
    site_id: String,
) -> Result<String, String> {
    let resolved_integration_type = match integration_type.as_str() {
        "googlesearchconsole" => IntegrationType::GoogleSearchConsole,
        _ => IntegrationType::GoogleAnalytics,
    };
    let tokens = {
        let lock = OAUTH_TOKENS.lock().await;
        peek_project_bound_flow(
            &lock,
            &flow_id,
            project_id,
            "No OAuth tokens - please reconnect",
            "OAuth flow belongs to a different project - reconnect and try again",
        )?
    };

    let extra = serde_json::json!({ "tokens": tokens });

    let config = IntegrationConfig {
        integration_type: resolved_integration_type,
        api_key: None,
        site_id: Some(site_id),
        extra: Some(extra),
        enabled: true,
    };
    super::integrations::persist_integration_config_securely(&app, &db, project_id, &config)?;

    Ok("Connected".into())
}

/// Start GitHub OAuth and open the system browser.
#[tracing::instrument(fields(project_id))]
pub async fn connect_github(project_id: i64) -> Result<serde_json::Value, String> {
    let client_id = integrations::github_oauth::client_id();
    if client_id.is_empty() {
        return Err("GitHub OAuth not configured - GITHUB_CLIENT_ID is not set".into());
    }
    let device_flow = integrations::github_oauth::start_device_flow(client_id).await?;

    open::that(&device_flow.verification_uri)
        .map_err(|e| sanitize_error(format!("Failed to open browser: {}", e)))?;

    let flow_id = generate_flow_id()?;
    let user_code = device_flow.user_code.clone();
    let verification_uri = device_flow.verification_uri.clone();
    let mut lock = GH_OAUTH_PENDING.lock().await;
    lock.insert(
        flow_id.clone(),
        OAuthFlow {
            project_id,
            payload: device_flow,
        },
    );
    drop(lock);
    spawn_github_flow_cleanup(flow_id.clone());
    Ok(serde_json::json!({
        "flow_id": flow_id,
        "user_code": user_code,
        "verification_uri": verification_uri,
    }))
}

/// Complete GitHub OAuth and list repositories.
#[tracing::instrument(fields(project_id, flow_id = %flow_id))]
pub async fn complete_github_oauth(
    project_id: i64,
    flow_id: String,
) -> Result<serde_json::Value, String> {
    let audit_detail = serde_json::json!({ "provider": "github", "project_id": project_id });
    let client_id = integrations::github_oauth::client_id();
    if client_id.is_empty() {
        crate::audit_log::record("oauth.complete", audit_detail, "fail");
        return Err("GitHub OAuth not configured - GITHUB_CLIENT_ID is not set".into());
    }

    let inner = async {
        let device_flow = {
            let mut lock = GH_OAUTH_PENDING.lock().await;
            take_project_bound_flow(
                &mut lock,
                &flow_id,
                project_id,
                "No pending GitHub OAuth flow",
                "GitHub OAuth flow belongs to a different project - reconnect and try again",
            )?
        };

        let tokens = tokio::time::timeout(
            crate::constants::OAUTH_TIMEOUT,
            integrations::github_oauth::poll_device_flow(
                client_id,
                &device_flow.device_code,
                device_flow.expires_in,
                device_flow.interval,
            ),
        )
        .await
        .map_err(|_| "GitHub OAuth timed out - no response within 2 minutes".to_string())??;

        let repos = integrations::github_oauth::list_repos(&tokens.access_token)
            .await
            .unwrap_or_default();

        {
            let mut lock = GH_OAUTH_TOKENS.lock().await;
            lock.insert(
                flow_id,
                OAuthFlow {
                    project_id,
                    payload: tokens,
                },
            );
        }

        Ok::<_, String>(serde_json::json!({ "repos": repos }))
    };

    match inner.await {
        Ok(value) => {
            crate::audit_log::record("oauth.complete", audit_detail, "ok");
            Ok(value)
        }
        Err(e) => {
            crate::audit_log::record("oauth.complete", audit_detail, "fail");
            Err(e)
        }
    }
}

/// Save the selected GitHub repository and OAuth token.
#[tracing::instrument(skip(app, db), fields(project_id, flow_id = %flow_id, repo = %repo))]
pub async fn save_github_integration(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    flow_id: String,
    repo: String,
) -> Result<String, String> {
    let tokens = {
        let mut lock = GH_OAUTH_TOKENS.lock().await;
        take_project_bound_flow(
            &mut lock,
            &flow_id,
            project_id,
            "No GitHub OAuth tokens - please reconnect",
            "GitHub OAuth flow belongs to a different project - reconnect and try again",
        )?
    };

    let extra = serde_json::json!({ "tokens": tokens });
    let config = IntegrationConfig {
        integration_type: IntegrationType::GitHub,
        api_key: None,
        site_id: Some(repo),
        extra: Some(extra),
        enabled: true,
    };
    super::integrations::persist_integration_config_securely(&app, &db, project_id, &config)?;

    Ok("Connected".into())
}

#[cfg(test)]
mod tests {
    use super::{peek_project_bound_flow, take_project_bound_flow, OAuthFlow};
    use std::collections::HashMap;

    #[test]
    fn take_project_bound_flow_requires_matching_project_without_consuming_flow() {
        let mut store = HashMap::new();
        store.insert(
            "flow-1".to_string(),
            OAuthFlow {
                project_id: 7,
                payload: "secret".to_string(),
            },
        );

        let err = take_project_bound_flow(&mut store, "flow-1", 8, "missing", "wrong project")
            .expect_err("mismatched project should fail");

        assert_eq!(err, "wrong project");
        assert_eq!(store.len(), 1);

        let payload = take_project_bound_flow(&mut store, "flow-1", 7, "missing", "wrong project")
            .expect("original flow should still be available");
        assert_eq!(payload, "secret");
        assert!(store.is_empty());
    }

    #[test]
    fn take_project_bound_flow_returns_payload_for_matching_project() {
        let mut store = HashMap::new();
        store.insert(
            "flow-1".to_string(),
            OAuthFlow {
                project_id: 7,
                payload: "secret".to_string(),
            },
        );

        let payload = take_project_bound_flow(&mut store, "flow-1", 7, "missing", "wrong project")
            .expect("matching project should succeed");

        assert_eq!(payload, "secret");
        assert!(store.is_empty());
    }

    #[test]
    fn peek_project_bound_flow_returns_clone_and_leaves_entry_intact() {
        let mut store = std::collections::HashMap::new();
        store.insert(
            "flow-1".to_string(),
            OAuthFlow {
                project_id: 7,
                payload: "token-data".to_string(),
            },
        );

        let first = peek_project_bound_flow(&store, "flow-1", 7, "missing", "wrong project")
            .expect("should succeed");
        assert_eq!(first, "token-data");
        assert_eq!(store.len(), 1, "entry should still be in store after peek");

        let second = peek_project_bound_flow(&store, "flow-1", 7, "missing", "wrong project")
            .expect("second peek should succeed");
        assert_eq!(second, "token-data");
        assert_eq!(store.len(), 1);

        let err = peek_project_bound_flow(&store, "flow-1", 99, "missing", "wrong project")
            .expect_err("mismatched project should fail");
        assert_eq!(err, "wrong project");
    }
}
