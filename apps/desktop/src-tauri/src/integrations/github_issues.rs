//! GitHub Issues provider - create and close issues via the GitHub REST API.

use crate::checks::{CheckResult, ScanCategory, Severity};
use crate::integrations::issue_tracker::{
    format_issue_body_markdown, ExternalTicket, IssueContext,
};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};

const API_BASE: &str = "https://api.github.com";

fn github_headers(token: &str) -> Result<HeaderMap, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("GitHub token is empty.".to_string());
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&crate::constants::USER_AGENT)
            .unwrap_or_else(|_| HeaderValue::from_static("SiteCMD")),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        "X-GitHub-Api-Version",
        HeaderValue::from_static("2022-11-28"),
    );
    let auth_header = HeaderValue::from_str(&format!("Bearer {}", token))
        .map_err(|_| "GitHub token contains invalid header characters.".to_string())?;
    headers.insert(AUTHORIZATION, auth_header);
    Ok(headers)
}

fn severity_label(severity: &Severity) -> &'static str {
    severity.as_str()
}

fn category_label(category: &ScanCategory) -> &'static str {
    category.as_str()
}

/// Ensure a label exists in the repo. Ignores 422 (label already exists).
async fn ensure_label(
    client: &reqwest::Client,
    token: &str,
    repo: &str,
    label: &str,
) -> Result<(), String> {
    let url = format!("{}/repos/{}/labels", API_BASE, repo);
    let body = serde_json::json!({
        "name": label,
        "color": "ededed",
    });

    let resp = client
        .post(&url)
        .headers(github_headers(token)?)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("GitHub label create error: {}", e))?;

    let status = resp.status().as_u16();
    // 201 = created, 422 = already exists - both are fine
    if status != 201 && status != 422 {
        return Err(format!("GitHub label create returned {}", status));
    }

    Ok(())
}

/// Parse a GitHub issue number from an external_id like "#42".
#[tracing::instrument(fields(external_id = %external_id))]
pub fn parse_issue_number(external_id: &str) -> Option<u64> {
    external_id.trim_start_matches('#').parse::<u64>().ok()
}

/// Create a GitHub issue for a SiteCMD check result.
///
/// `repo` must be in "owner/repo" format (e.g. "acme/website").
#[tracing::instrument(skip(token, repo, issue, context))]
pub async fn create_github_issue(
    token: &str,
    repo: &str,
    issue: &CheckResult,
    context: &IssueContext,
) -> Result<ExternalTicket, String> {
    let repo = super::validation::normalize_github_repo_slug(repo)?;
    let client = crate::http_client::credentialed_service_client();

    let sev_label = severity_label(&issue.severity);
    let cat_label = category_label(&issue.category);

    // Ensure the labels we need actually exist in the repo
    for label in &["sitecmd", sev_label, cat_label] {
        ensure_label(client, token, &repo, label).await?;
    }

    let title = format!("[SiteCMD] {}", issue.title);
    let body = format_issue_body_markdown(issue, context);

    let payload = serde_json::json!({
        "title": title,
        "body": body,
        "labels": ["sitecmd", sev_label, cat_label],
    });

    let url = format!("{}/repos/{}/issues", API_BASE, repo);
    let resp = client
        .post(&url)
        .headers(github_headers(token)?)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("GitHub create issue error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!("GitHub create issue returned {}", status));
    }

    let created: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("GitHub create issue parse error: {}", e))?;

    let number = created["number"]
        .as_u64()
        .ok_or("GitHub response missing issue number")?;
    let html_url = created["html_url"].as_str().unwrap_or("").to_string();

    Ok(ExternalTicket {
        external_id: format!("#{}", number),
        external_url: html_url,
    })
}

/// Add a comment to a GitHub issue, then close it.
///
/// `issue_number` is the numeric GitHub issue number (not the "#N" string).
#[tracing::instrument(skip(token, repo, comment), fields(issue_number))]
pub async fn resolve_github_issue(
    token: &str,
    repo: &str,
    issue_number: u64,
    comment: &str,
) -> Result<(), String> {
    let repo = super::validation::normalize_github_repo_slug(repo)?;
    let client = crate::http_client::credentialed_service_client();

    let comment_url = format!(
        "{}/repos/{}/issues/{}/comments",
        API_BASE, repo, issue_number
    );
    let comment_payload = serde_json::json!({ "body": comment });

    let resp = client
        .post(&comment_url)
        .headers(github_headers(token)?)
        .json(&comment_payload)
        .send()
        .await
        .map_err(|e| format!("GitHub comment error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub comment returned {}", resp.status()));
    }

    let patch_url = format!("{}/repos/{}/issues/{}", API_BASE, repo, issue_number);
    let close_payload = serde_json::json!({ "state": "closed" });

    let resp = client
        .patch(&patch_url)
        .headers(github_headers(token)?)
        .json(&close_payload)
        .send()
        .await
        .map_err(|e| format!("GitHub close issue error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub close issue returned {}", resp.status()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_headers_rejects_control_characters_without_panicking() {
        let err = github_headers("github_pat_valid\ninjected")
            .expect_err("newline-bearing tokens cannot be valid header values");

        assert!(err.contains("invalid header characters"));
    }

    #[test]
    fn github_headers_trims_pasted_token_whitespace() {
        let headers = github_headers("  github_pat_valid  ").expect("valid token");
        let auth = headers
            .get(AUTHORIZATION)
            .expect("authorization header")
            .to_str()
            .expect("ascii header");

        assert_eq!(auth, "Bearer github_pat_valid");
    }
}
