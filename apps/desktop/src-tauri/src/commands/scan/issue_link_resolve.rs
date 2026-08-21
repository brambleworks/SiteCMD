//! Resolves linked GitHub and Jira tickets after checks begin passing.
//! Local links are resolved only after their tracker tickets close.

use std::sync::Arc;

use tauri::AppHandle;

use super::policy::validate_issue_link_provider;
use crate::checks::CheckStatus;
use crate::core::scanner;
use crate::db::Database;

pub(super) fn spawn_issue_link_auto_resolves(
    app: &AppHandle,
    db: &Arc<Database>,
    project_id: i64,
    result: &scanner::ScanResult,
) {
    let passing_check_ids: Vec<String> = result
        .issues
        .iter()
        .filter(|i| matches!(i.status, CheckStatus::Pass))
        .map(|i| i.check_id.clone())
        .collect();
    if passing_check_ids.is_empty() {
        return;
    }
    let resolvable = match db.find_resolvable_issue_links(project_id, passing_check_ids) {
        Ok(resolvable) => resolvable,
        Err(error) => {
            // Nothing external happened yet; skipping is safe, but silence
            // here left tracker tickets open with no trace of why.
            tracing::warn!("Issue-link auto-resolve lookup failed; skipping: {}", error);
            return;
        }
    };
    for (link_id, check_id, provider, external_id) in resolvable {
        let db_clone = db.clone();
        let app_clone = app.clone();
        let timestamp = result.timestamp.clone();
        tokio::spawn(async move {
            auto_resolve_one_link(
                &app_clone,
                db_clone,
                project_id,
                link_id,
                check_id,
                provider,
                external_id,
                timestamp,
            )
            .await;
        });
    }
}

async fn auto_resolve_one_link(
    app: &AppHandle,
    db: Arc<Database>,
    project_id: i64,
    link_id: i64,
    check_id: String,
    provider: String,
    external_id: String,
    timestamp: String,
) {
    if let Err(error) = validate_issue_link_provider(&provider) {
        tracing::info!(
            "Skipping auto-resolve for {} {}: {}",
            provider,
            external_id,
            error
        );
        return;
    }

    let comment =
        crate::integrations::issue_tracker::format_resolution_comment(&check_id, &timestamp);

    let resolve_result = match provider.as_str() {
        "github" => resolve_github_link(app, &db, project_id, &external_id, &comment).await,
        "jira" => resolve_jira_link(app, &db, project_id, &external_id, &comment).await,
        other => Err(format!("Unknown provider: {}", other)),
    };

    match resolve_result {
        Ok(()) => {
            // Persist off the async runtime; log local failure because the
            // external tracker has already accepted the resolution.
            let resolve_db = db.clone();
            let resolve_outcome = tauri::async_runtime::spawn_blocking(move || {
                resolve_db.resolve_issue_link(link_id)
            })
            .await;
            match resolve_outcome {
                Ok(Ok(())) => {
                    tracing::info!(
                        "Auto-resolved issue link {} ({} {})",
                        link_id,
                        provider,
                        external_id
                    );
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Auto-resolved {} {} on tracker but failed to mark link {} resolved locally: {}",
                        provider,
                        external_id,
                        link_id,
                        e
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Auto-resolve link-update task panicked for link {}: {}",
                        link_id,
                        e
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to auto-resolve {} {}: {}", provider, external_id, e);
        }
    }
}

// Generic over the Tauri runtime so tests can drive the guard chain with a
// mock app handle.
async fn resolve_github_link<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    db: &std::sync::Arc<Database>,
    project_id: i64,
    external_id: &str,
    comment: &str,
) -> Result<(), String> {
    let configs = db.get_integrations(project_id)?;
    let github_config = configs
        .iter()
        .find(|c| c.integration_type == crate::integrations::IntegrationType::GitHub && c.enabled)
        .ok_or("GitHub not configured")?;

    let token = crate::keyring::get_api_key(app, db, project_id, "github")
        .ok()
        .flatten()
        .or_else(|| github_config.api_key.clone())
        .ok_or("No GitHub token")?;

    let repo = github_config
        .site_id
        .clone()
        .ok_or("No GitHub repo configured")?;

    let issue_number = crate::integrations::github_issues::parse_issue_number(external_id)
        .ok_or("Invalid issue number")?;

    crate::integrations::github_issues::resolve_github_issue(&token, &repo, issue_number, comment)
        .await
}

async fn resolve_jira_link<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    db: &std::sync::Arc<Database>,
    project_id: i64,
    external_id: &str,
    comment: &str,
) -> Result<(), String> {
    let configs = db.get_integrations(project_id)?;
    let jira_config = configs
        .iter()
        .find(|c| c.integration_type == crate::integrations::IntegrationType::Jira && c.enabled)
        .ok_or("Jira not configured")?;

    let extra = jira_config
        .extra
        .as_ref()
        .ok_or("Jira config missing extra fields")?;
    let instance_url = extra
        .get("instance_url")
        .and_then(|v| v.as_str())
        .ok_or("Missing Jira instance URL")?;
    let email = extra
        .get("email")
        .and_then(|v| v.as_str())
        .ok_or("Missing Jira email")?;

    let api_token = crate::keyring::get_api_key(app, db, project_id, "jira")
        .ok()
        .flatten()
        .or_else(|| jira_config.api_key.clone())
        .ok_or("No Jira API token")?;

    crate::integrations::jira::resolve_jira_issue(
        instance_url,
        email,
        &api_token,
        external_id,
        comment,
    )
    .await
}

#[cfg(test)]
mod tests {
    //! Pre-network guards for unconfigured or disabled tracker integrations.

    use super::*;

    #[tokio::test]
    async fn github_resolve_stops_when_the_integration_is_not_configured() {
        let app = tauri::test::mock_app();
        let harness = crate::db::test_helpers::temp_db_arc();

        let error = resolve_github_link(app.handle(), &harness.db, 1, "42", "resolved")
            .await
            .expect_err("no GitHub integration configured");
        assert_eq!(error, "GitHub not configured");
    }

    #[tokio::test]
    async fn jira_resolve_stops_when_the_integration_is_not_configured() {
        let app = tauri::test::mock_app();
        let harness = crate::db::test_helpers::temp_db_arc();

        let error = resolve_jira_link(app.handle(), &harness.db, 1, "PROJ-7", "resolved")
            .await
            .expect_err("no Jira integration configured");
        assert_eq!(error, "Jira not configured");
    }
}
