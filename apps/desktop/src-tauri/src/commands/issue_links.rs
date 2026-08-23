//! External issue-link commands for GitHub and Jira.

use std::sync::Arc;
use tauri::{AppHandle, State};

use super::{run_blocking, CommandError, CommandResult};
use crate::checks::{CheckResult, CheckStatus};
use crate::db::{Database, IssueLink};
use crate::integrations::{github_issues, issue_tracker::IssueContext, jira, IntegrationConfig};

fn load_verified_ticket_source(
    db: &Database,
    project_id: i64,
    scan_id: i64,
    check_id: &str,
    estimated_impact: u32,
) -> CommandResult<(CheckResult, IssueContext)> {
    if check_id.trim().is_empty()
        || check_id.chars().count() > 200
        || check_id.chars().any(char::is_control)
    {
        return Err(CommandError::new("Finding identifier is invalid."));
    }
    if !db
        .scan_run_belongs_to_project(project_id, scan_id)
        .map_err(|error| CommandError::new(format!("Failed to verify scan ownership: {error}")))?
    {
        return Err(CommandError::new(
            "The selected scan does not belong to this project.",
        ));
    }

    let scan = db
        .get_scan_detail(scan_id)
        .map_err(|error| CommandError::new(format!("Failed to load the selected scan: {error}")))?
        .ok_or("The selected Web Scan no longer exists.")?;
    let issue = scan
        .issues
        .into_iter()
        .find(|issue| issue.check_id == check_id)
        .ok_or("The selected finding is not present in that scan.")?;
    if !matches!(issue.status, CheckStatus::Fail | CheckStatus::Warn) {
        return Err(CommandError::new(
            "Only an active finding can be sent to an issue tracker.",
        ));
    }
    let project_name = db
        .get_projects()
        .map_err(|error| CommandError::new(format!("Failed to load the project: {error}")))?
        .into_iter()
        .find(|project| project.id == project_id)
        .map(|project| project.name)
        .ok_or("The selected project no longer exists.")?;
    let context = IssueContext {
        project_name,
        site_url: scan.url,
        scan_timestamp: scan.timestamp,
        estimated_impact: estimated_impact.min(100),
    };
    Ok((issue, context))
}

fn provider_integration_type(
    provider: &str,
) -> Result<crate::integrations::IntegrationType, String> {
    match provider {
        "github" => Ok(crate::integrations::IntegrationType::GitHub),
        "jira" => Ok(crate::integrations::IntegrationType::Jira),
        other => Err(format!("Unsupported issue tracker provider: '{}'", other)),
    }
}

/// Resolve supported issue-tracker provider names.
#[tracing::instrument(fields(provider = %provider))]
pub(crate) fn resolve_issue_link_provider(
    provider: &str,
) -> Result<crate::integrations::IntegrationType, String> {
    provider_integration_type(provider)
}

/// Get an integration config for a given project and provider string, with keyring resolved.
fn get_resolved_config(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    provider: &str,
) -> CommandResult<IntegrationConfig> {
    get_resolved_config_with_audit(app, db, project_id, provider, &crate::keyring::audit_to_log)
}

/// `get_resolved_config` with the refusal audit sink injected, so tests can
/// verify a plaintext credential is refused without appending to the real
/// audit log.
///
/// Applies `without_unmigrated_plaintext_secrets_with` to the freshly loaded
/// configs before any `api_key` or token is read below, exactly as
/// `credentials_from_configs` in `core::integration_scheduler` does for the
/// scheduler's poll path: a plaintext SQLite credential left by a failed
/// keyring migration is dropped rather than hydrated and handed to an
/// outbound tracker request.
fn get_resolved_config_with_audit<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Database,
    project_id: i64,
    provider: &str,
    audit: crate::keyring::RefusalAudit<'_>,
) -> CommandResult<IntegrationConfig> {
    resolve_issue_link_provider(provider)?;

    let configs = db
        .get_integrations(project_id)
        .map_err(|e| CommandError::new(format!("Failed to get integrations: {}", e)))?;
    let mut configs = crate::keyring::without_unmigrated_plaintext_secrets_with(configs, audit);

    let pos = configs
        .iter()
        .position(|c| {
            let type_str = serde_json::to_string(&c.integration_type)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            type_str == provider
        })
        .ok_or_else(|| {
            CommandError::new(format!(
                "No {} integration configured for this project",
                provider
            ))
        })?;

    let config = &mut configs[pos];
    if !config.enabled {
        return Err(CommandError::new(format!(
            "{} integration is disabled for this project",
            provider
        )));
    }
    let type_str = provider.to_string();

    if config.api_key.as_deref() == Some(crate::keyring::KEYRING_PLACEHOLDER) {
        if let Ok(Some(key)) = crate::keyring::get_api_key(app, db, project_id, &type_str) {
            config.api_key = Some(key);
        }
    }

    if let Ok(Some(tokens_str)) = crate::keyring::get_tokens(app, db, project_id, &type_str) {
        if let Ok(tokens_val) = serde_json::from_str::<serde_json::Value>(&tokens_str) {
            if let Some(ref mut extra) = config.extra {
                extra["tokens"] = tokens_val;
            } else {
                config.extra = Some(serde_json::json!({ "tokens": tokens_val }));
            }
        }
    }

    Ok(configs.remove(pos))
}

/// An existing link that makes this create a retry, not a new ticket.
/// Reuse only the same check occurrence and provider.
fn reusable_existing_link(
    existing: Option<IssueLink>,
    scan_id: i64,
    provider: &str,
) -> Option<IssueLink> {
    existing.filter(|link| link.scan_id == scan_id && link.provider == provider)
}

/// Create an issue in an external tracker (GitHub or Jira) and store the link in DB.
///
/// Returns the newly created `IssueLink` record.
#[tracing::instrument(skip(app, db), fields(project_id, check_id = %check_id, scan_id, provider = %provider))]
pub async fn create_issue_link(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    check_id: String,
    scan_id: i64,
    provider: String,
    estimated_impact: u32,
) -> Result<IssueLink, CommandError> {
    // Serialize the idempotency read and write so concurrent retries cannot file twice.
    // The unique attempt index backstops writers that bypass this lock.
    static ISSUE_LINK_CREATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _creation = ISSUE_LINK_CREATION.lock().await;

    // Match the complete attempt identity before contacting the tracker.
    let existing = {
        let db = (*db).clone();
        let check_id = check_id.clone();
        let provider = provider.clone();
        run_blocking(move || {
            db.get_issue_link_for_attempt(project_id, &check_id, scan_id, &provider)
        })
        .await?
        .map_err(|e| {
            CommandError::new(format!("Failed to check for an existing issue link: {}", e))
        })?
    };
    if let Some(link) = reusable_existing_link(existing, scan_id, &provider) {
        tracing::info!(
            "An issue link for this check, scan, and provider already exists; answering with it instead of filing a duplicate"
        );
        return Ok(link);
    }

    let (config, issue, context) = {
        let db = (*db).clone();
        let provider = provider.clone();
        let check_id = check_id.clone();
        run_blocking(move || {
            let (issue, context) =
                load_verified_ticket_source(&db, project_id, scan_id, &check_id, estimated_impact)?;
            let config = get_resolved_config(&app, &db, project_id, &provider)?;
            Ok::<_, CommandError>((config, issue, context))
        })
        .await??
    };

    let ticket = match provider.as_str() {
        "github" => {
            let token = config
                .api_key
                .as_deref()
                .filter(|k| !k.is_empty() && *k != crate::keyring::KEYRING_PLACEHOLDER)
                .ok_or("GitHub token not configured")?
                .to_string();

            let repo = config
                .site_id
                .as_deref()
                .filter(|r| !r.is_empty())
                .ok_or("GitHub repository (owner/repo) not configured")?
                .to_string();

            github_issues::create_github_issue(&token, &repo, &issue, &context).await?
        }

        "jira" => {
            let extra = config
                .extra
                .as_ref()
                .ok_or("Jira configuration not found")?;

            let instance_url = extra["instance_url"]
                .as_str()
                .ok_or("Jira instance_url not configured")?
                .to_string();

            let email = extra["email"]
                .as_str()
                .ok_or("Jira email not configured")?
                .to_string();

            let project_key = extra["project_key"]
                .as_str()
                .ok_or("Jira project_key not configured")?
                .to_string();

            let issue_type = extra["issue_type"].as_str().unwrap_or("Task").to_string();

            let token = config
                .api_key
                .as_deref()
                .filter(|k| !k.is_empty() && *k != crate::keyring::KEYRING_PLACEHOLDER)
                .ok_or("Jira API token not configured")?
                .to_string();

            jira::create_jira_issue(
                &instance_url,
                &email,
                &token,
                &project_key,
                &issue_type,
                &issue,
                &context,
            )
            .await?
        }

        other => {
            return Err(CommandError::new(format!(
                "Unsupported issue tracker provider: '{}'",
                other
            )))
        }
    };

    let db = (*db).clone();
    run_blocking(move || {
        db.create_issue_link(
            project_id,
            &check_id,
            scan_id,
            &provider,
            &ticket.external_id,
            &ticket.external_url,
        )
        .map_err(|e| CommandError::new(format!("Failed to store issue link: {}", e)))?;

        // Read back by the full attempt identity so concurrent providers or
        // scans cannot return another ticket.
        db.get_issue_link_for_attempt(project_id, &check_id, scan_id, &provider)
            .map_err(|e| CommandError::new(format!("Failed to retrieve issue link: {}", e)))?
            .ok_or_else(|| CommandError::new("Issue link was created but could not be retrieved"))
    })
    .await?
}

/// Get all issue links for a project, ordered newest first.
#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id))]
pub async fn get_issue_links(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<Vec<IssueLink>, CommandError> {
    let db = (*db).clone();
    run_blocking(move || db.get_issue_links(project_id))
        .await?
        .map_err(|e| CommandError::new(format!("Failed to get issue links: {}", e)))
}

/// Get the most recent issue link for a specific check on a project.
#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id, check_id = %check_id))]
pub async fn get_issue_link_for_check(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    check_id: String,
) -> Result<Option<IssueLink>, CommandError> {
    let db = (*db).clone();
    run_blocking(move || db.get_issue_link_for_check(project_id, &check_id))
        .await?
        .map_err(|e| CommandError::new(format!("Failed to get issue link: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::{
        get_resolved_config_with_audit, load_verified_ticket_source, resolve_issue_link_provider,
        reusable_existing_link,
    };
    use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
    use crate::core::scanner::{ScanResult, ScanType};
    use crate::db::test_helpers::{temp_db, TestDb};
    use crate::db::IssueLink;
    use crate::integrations::{IntegrationConfig, IntegrationType};
    use crate::keyring::{KEYRING_PLACEHOLDER, SECRET_TEST_GUARD};
    use tauri::test::mock_app;

    fn stored_link(scan_id: i64, provider: &str) -> IssueLink {
        IssueLink {
            check_id: "meta-description".to_string(),
            created_at: "2026-08-01T00:00:00Z".to_string(),
            external_id: "77".to_string(),
            external_url: "https://github.com/acme/site/issues/77".to_string(),
            id: 1,
            project_id: 1,
            provider: provider.to_string(),
            resolved_at: None,
            scan_id,
            status: "open".to_string(),
        }
    }

    #[test]
    fn get_resolved_config_refuses_a_plaintext_github_token_left_by_a_failed_migration() {
        // Serializes with every other test that touches the process-global
        // debug secret store; the store itself stays in-memory under
        // `cfg(test)`, so this never reaches the real keychain.
        let _guard = SECRET_TEST_GUARD.lock().expect("secret test guard");

        let app = mock_app();
        let db = temp_db();
        let project_id = db
            .upsert_project("Unmigrated Tracker", "/tmp/unmigrated-tracker", None)
            .expect("project");

        // Simulate a migration that wrote the keychain but never cleaned up
        // (or never ran at all): SQLite still holds the plaintext token.
        db.save_integration(
            project_id,
            &IntegrationConfig {
                integration_type: IntegrationType::GitHub,
                api_key: Some("still-plaintext-pat".to_string()),
                site_id: Some("acme/site".to_string()),
                extra: None,
                enabled: true,
            },
        )
        .expect("seed an unmigrated plaintext github config");

        let recorded: std::sync::Mutex<Vec<(String, serde_json::Value, String)>> =
            std::sync::Mutex::new(Vec::new());
        let audit = |op: &str, detail: serde_json::Value, result: &str| {
            recorded.lock().expect("recording sink lock").push((
                op.to_string(),
                detail,
                result.to_string(),
            ));
        };

        let resolved =
            get_resolved_config_with_audit(app.handle(), &db, project_id, "github", &audit)
                .expect("a refused config still resolves; it just reports reconnect");

        assert_ne!(
            resolved.api_key.as_deref(),
            Some("still-plaintext-pat"),
            "a plaintext SQLite api_key left by a failed migration must never be used"
        );
        assert_eq!(
            resolved.api_key.as_deref(),
            Some(KEYRING_PLACEHOLDER),
            "the reconnect condition is the keychain placeholder, since nothing is stored \
             under this fresh project's keychain namespace to hydrate it from"
        );

        let entries = recorded.into_inner().expect("recording sink");
        assert_eq!(
            entries.len(),
            1,
            "exactly one refusal must be recorded for the github integration: {entries:?}"
        );
        let (op, detail, result) = &entries[0];
        assert_eq!(op, "credential_refused_unmigrated");
        assert_eq!(result, "refused");
        assert_eq!(detail, &serde_json::json!({ "integration": "github" }));
        assert!(
            !detail.to_string().contains("still-plaintext-pat"),
            "the audit detail must never carry the refused secret: {detail}"
        );
    }

    #[test]
    fn a_retry_of_a_completed_attempt_reuses_the_stored_link() {
        let reused = reusable_existing_link(Some(stored_link(9, "github")), 9, "github");
        assert_eq!(reused.map(|link| link.external_id), Some("77".to_string()));
    }

    #[test]
    fn a_new_scan_or_another_tracker_still_files_a_fresh_ticket() {
        assert!(reusable_existing_link(Some(stored_link(8, "github")), 9, "github").is_none());
        assert!(reusable_existing_link(Some(stored_link(9, "github")), 9, "jira").is_none());
        assert!(reusable_existing_link(None, 9, "github").is_none());
    }

    #[test]
    fn create_issue_link_consults_the_existing_link_before_any_tracker_call() {
        // Limit source checks to production code and preserve dispatch ordering.
        let source = include_str!("issue_links.rs");
        let code = &source[..source.find("#[cfg(test)]").expect("tests module exists")];
        let create = code
            .find("pub async fn create_issue_link")
            .expect("create_issue_link exists");
        let guard = code[create..]
            .find("reusable_existing_link(")
            .expect("create_issue_link must consult the existing link");
        let dispatch = code[create..]
            .find("match provider.as_str()")
            .expect("the provider dispatch exists");
        assert!(
            guard < dispatch,
            "the idempotency read must come before the tracker call"
        );
        let attempt_read = code[create..]
            .find("get_issue_link_for_attempt(")
            .expect("the idempotency read must query by check, scan, and provider");
        assert!(
            attempt_read < dispatch,
            "the full-identity read must come before the tracker call"
        );
        let lock = code[create..]
            .find("ISSUE_LINK_CREATION.lock().await")
            .expect("create_issue_link must serialize creations");
        assert!(
            lock < attempt_read,
            "the creation lock must be held before the idempotency read, or the read cannot see a concurrent creation's row"
        );
        let create_body_end = code[create + 1..]
            .find("\npub ")
            .map(|i| create + 1 + i)
            .unwrap_or(code.len());
        assert!(
            !code[create..create_body_end].contains("get_issue_link_for_check("),
            "create_issue_link must never read back by check alone"
        );
    }

    fn ticket_source(status: CheckStatus) -> (TestDb, i64, i64) {
        let db = temp_db();
        let project_id = db
            .upsert_project("Immutable Source", "/tmp/immutable-source", None)
            .expect("project");
        db.add_environment(
            project_id,
            "https://example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("environment");
        let site_id = db.get_or_create_site("https://example.com").expect("site");
        let scan_id = db
            .save_scan(
                site_id,
                &ScanResult {
                    page_signals: None,
                    site_facts: None,
                    url: "https://example.com".to_string(),
                    mode: "full".to_string(),
                    scan_type: ScanType::Health,
                    overall_score: 80,
                    categories: Vec::new(),
                    issues: vec![CheckResult {
                        check_id: "security.csp".to_string(),
                        category: ScanCategory::Security,
                        title: "Stored finding title".to_string(),
                        description: "Stored finding description".to_string(),
                        status,
                        severity: Severity::High,
                        fix_prompt: Some("Stored fix prompt".to_string()),
                        manual_fix: Some("Stored manual fix".to_string()),
                        raw_data: None,
                        confidence: IssueConfidence::High,
                        confidence_reason: None,
                        why_it_matters: Some("Stored impact".to_string()),
                    }],
                    detected_stack: None,
                    duration_ms: 50,
                    timestamp: "2026-07-23T12:00:00Z".to_string(),
                },
            )
            .expect("scan");
        (db, project_id, scan_id)
    }

    #[test]
    fn issue_link_provider_resolves_known_providers() {
        // Pure input validation with no tier anywhere on the path: mirroring
        // uses the user's own credentials from their own machine.
        assert_eq!(
            resolve_issue_link_provider("github").expect("github resolves"),
            IntegrationType::GitHub
        );
        assert_eq!(
            resolve_issue_link_provider("jira").expect("jira resolves"),
            IntegrationType::Jira
        );
    }

    #[test]
    fn issue_link_provider_rejects_unknown_providers() {
        let error = resolve_issue_link_provider("linear")
            .expect_err("unknown providers should be rejected");
        assert!(error.contains("Unsupported issue tracker provider"));
    }

    #[test]
    fn ticket_content_comes_from_the_verified_local_scan_snapshot() {
        let (db, project_id, scan_id) = ticket_source(CheckStatus::Fail);
        let (issue, context) =
            load_verified_ticket_source(&db, project_id, scan_id, "security.csp", 500)
                .expect("verified ticket source");

        assert_eq!(issue.title, "Stored finding title");
        assert_eq!(issue.fix_prompt.as_deref(), Some("Stored fix prompt"));
        assert_eq!(context.project_name, "Immutable Source");
        assert_eq!(context.site_url, "https://example.com");
        assert_eq!(context.scan_timestamp, "2026-07-23T12:00:00Z");
        assert_eq!(context.estimated_impact, 100);
    }

    #[test]
    fn ticket_source_rejects_cross_project_runs_and_non_active_checks() {
        let (db, project_id, scan_id) = ticket_source(CheckStatus::Pass);
        let other_project = db
            .upsert_project("Other Project", "/tmp/other-project", None)
            .expect("other project");

        assert!(
            load_verified_ticket_source(&db, other_project, scan_id, "security.csp", 4)
                .unwrap_err()
                .raw()
                .contains("does not belong")
        );
        assert!(
            load_verified_ticket_source(&db, project_id, scan_id, "security.csp", 4)
                .unwrap_err()
                .raw()
                .contains("active finding")
        );
    }
}
