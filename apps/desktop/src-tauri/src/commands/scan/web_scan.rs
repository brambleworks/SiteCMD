use crate::core::normalized_scan::{
    batch_outcomes_on_routes, normalize_web_scan, ScanRunKind, ScanRunStatus,
};
use crate::core::scanner::{self, ScanType};
use crate::db::Database;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use super::control::ScanControlState;
use crate::commands::validate_url_async;

mod auto_export;
mod webview_layer;
use auto_export::auto_export_sitecmd_scan;
pub(super) use webview_layer::{apply_webview_layer, BrowserRuntime};

pub(crate) struct WebScanOutput {
    pub result: scanner::ScanResult,
    pub incomplete_detail: Option<String>,
}

pub(crate) async fn scan_url_for_execution(
    app: AppHandle,
    db: Arc<Database>,
    scan_control: &ScanControlState,
    page_url: String,
    environment_url: Option<String>,
    project_id: Option<i64>,
    enabled_categories: Option<Vec<String>>,
    timeout_secs: Option<u64>,
    axe_enabled: Option<bool>,
    scan_type: Option<ScanType>,
    scan_request_id: u64,
    execution_id: i64,
) -> Result<WebScanOutput, scanner::ScanError> {
    validate_url_async(&page_url)
        .await
        .map_err(scanner::ScanError::NetworkError)?;
    let environment_url = environment_url.unwrap_or_else(|| page_url.clone());

    let scan_type = scan_type.unwrap_or_default();
    let scan_control_for_run = scan_control.clone();
    let result = run_and_persist_scan(
        &app,
        db.clone(),
        scan_control_for_run,
        scan_request_id,
        page_url.clone(),
        environment_url.clone(),
        project_id,
        enabled_categories,
        timeout_secs,
        axe_enabled,
        scan_type,
        execution_id,
    )
    .await;

    if let Err(error) = &result {
        if !crate::core::native_alerts::is_user_cancelled_scan(error) {
            let db = db.clone();
            let environment_url = environment_url.clone();
            let error_text = error.to_string();
            let _ = crate::commands::run_blocking(move || {
                let project_id = project_id.or_else(|| db.find_project_for_url(&environment_url));
                if let Some(project_id) = project_id {
                    crate::core::native_alerts::emit_scan_failure_alert(
                        db.as_ref(),
                        project_id,
                        Some(&environment_url),
                        "Web Scan",
                        &error_text,
                    );
                }
            })
            .await;
        }
    }

    result
}

/// Run the scanner and persist its output.
async fn run_and_persist_scan(
    app: &AppHandle,
    db: Arc<Database>,
    scan_control: ScanControlState,
    scan_request_id: u64,
    page_url: String,
    environment_url: String,
    project_id: Option<i64>,
    enabled_categories: Option<Vec<String>>,
    timeout_secs: Option<u64>,
    axe_enabled: Option<bool>,
    scan_type: ScanType,
    execution_id: i64,
) -> Result<WebScanOutput, scanner::ScanError> {
    tracing::info!(
        "Starting {} scan for: {}",
        scan_type,
        crate::log_sanitizer::log_safe_url_target(&page_url)
    );

    // One probe covers the network scanner and the browser layer, so a cancel
    // stops both and the run is never persisted.
    let cancel_fn: std::sync::Arc<crate::scan_runtime::CancelFn> =
        std::sync::Arc::new(move || scan_control.is_cancelled(scan_request_id));

    let mut result = run_scan_with_progress(
        app,
        cancel_fn.clone(),
        page_url.clone(),
        enabled_categories,
        timeout_secs,
        scan_type,
    )
    .await?;

    let browser_runtime = apply_webview_layer(
        app,
        &mut result,
        &page_url,
        scan_type,
        axe_enabled,
        cancel_fn.as_ref(),
    )
    .await?;

    let safe_url = crate::log_sanitizer::log_safe_url_target(&page_url);
    tracing::info!(
        "Scan complete for {}: score={}, issues={}, duration={}ms",
        safe_url,
        result.overall_score,
        result.issues.len(),
        result.duration_ms
    );

    let incomplete_detail = browser_runtime.incomplete_detail();
    // Last gate before anything is written: a cancel that landed during the
    // browser layer must not leave a saved run reported as complete.
    if cancel_fn() {
        return Err(scanner::ScanError::Cancelled);
    }
    let outcome = post_scan_persist(
        app,
        &db,
        &environment_url,
        &page_url,
        scan_type,
        project_id,
        execution_id,
        axe_enabled.unwrap_or(false),
        &browser_runtime,
        &mut result,
    )
    .await;
    // A scan that ran but was never saved must not report success: history,
    // work items, and the score would silently miss it.
    outcome
        .scan_id
        .map_err(|error| persist_failure_error(&error))?;

    Ok(WebScanOutput {
        result,
        incomplete_detail,
    })
}

/// Command-path mapping for a persistence failure. The scan itself ran, so
/// the frontend message must say the save failed rather than the scan.
fn persist_failure_error(error: &str) -> scanner::ScanError {
    scanner::ScanError::ScanFailed(format!(
        "scan completed but could not be fully persisted: {}",
        error
    ))
}

// Accessibility gating belongs to the webview layer, not the network scanner.
async fn run_scan_with_progress(
    app: &AppHandle,
    cancel_fn: std::sync::Arc<crate::scan_runtime::CancelFn>,
    url: String,
    enabled_categories: Option<Vec<String>>,
    timeout_secs: Option<u64>,
    scan_type: ScanType,
) -> Result<scanner::ScanResult, scanner::ScanError> {
    let progress_app = app.clone();
    let progress_fn: std::sync::Arc<scanner::ProgressFn> = std::sync::Arc::new(move |p| {
        let _ = progress_app.emit("scan-progress", p);
    });
    crate::scan_runtime::run_scan_low_priority(
        url,
        Some(progress_fn),
        enabled_categories,
        timeout_secs,
        scan_type,
        false,
        Some(cancel_fn),
        // Single-page scan: no other page to share stylesheets with.
        None,
    )
    .await
}

/// Outcome of the shared post-scan persistence flow, for callers that layer
/// extra notifications on top of it (the scan scheduler).
pub(crate) struct PostScanOutcome {
    /// `Ok(scan_id)` only after the immutable scan row and canonical issue
    /// state were both saved. `Err` carries the failed persistence stage.
    pub scan_id: Result<i64, String>,
    /// True when the deploy-regression blame path sent its own desktop
    /// notification for this scan; schedulers must not ping a second time.
    pub blame_notified: bool,
}

/// Persist canonical scan state and trigger post-scan side effects.
/// Prefer `known_project_id` because multiple projects may share a URL; ad-hoc
/// scans may fall back to normalized URL lookup.
pub(crate) async fn post_scan_persist(
    app: &AppHandle,
    db: &Arc<Database>,
    environment_url: &str,
    page_url: &str,
    scan_type: ScanType,
    known_project_id: Option<i64>,
    execution_id: i64,
    axe_enabled: bool,
    browser_runtime: &BrowserRuntime,
    result: &mut scanner::ScanResult,
) -> PostScanOutcome {
    let complete_browser_coverage = browser_runtime.failure.is_none();
    let resolved_project_id = match known_project_id {
        Some(project_id) => Some(project_id),
        None => match db.find_project_for_url_result(environment_url) {
            Ok(project_id) => project_id,
            Err(error) => {
                return PostScanOutcome {
                    scan_id: Err(format!(
                        "could not resolve project for issue persistence: {error}"
                    )),
                    blame_notified: false,
                };
            }
        },
    };

    // SQLite persistence blocks on the DB worker, so keep it off async runtime
    // threads with `run_blocking`.
    let (outcome, regression_notice) = {
        let db = db.clone();
        let environment_url = environment_url.to_string();
        let page_url = page_url.to_string();
        let result = result.clone();
        let browser_runtime = browser_runtime.clone();
        crate::commands::run_blocking(move || {
            persist_scan_blocking(
                &db,
                &environment_url,
                &page_url,
                scan_type,
                resolved_project_id,
                execution_id,
                axe_enabled,
                browser_runtime.ran,
                browser_runtime.axe_ran,
                browser_runtime.build.as_deref(),
                browser_runtime.failure.as_deref(),
                &result,
            )
        })
        .await
        .unwrap_or_else(|error| {
            tracing::error!("Scan persistence task failed: {}", error);
            (
                PostScanOutcome {
                    scan_id: Err(format!("scan persistence task failed: {}", error)),
                    blame_notified: false,
                },
                None,
            )
        })
    };

    // Run notification and link-resolution work on the async tail.
    if let Some(notice) = regression_notice {
        super::notify_deploy_regression(app, &notice).await;
    }
    if complete_browser_coverage {
        if let Some(project_id) = resolved_project_id {
            super::issue_link_resolve::spawn_issue_link_auto_resolves(app, db, project_id, result);
        }
    }

    outcome
}

/// Blocking persistence half of [`post_scan_persist`], returning any deploy
/// regression notice for asynchronous delivery. Core persistence failures are
/// returned through `scan_id`.
fn persist_scan_blocking(
    db: &Arc<Database>,
    environment_url: &str,
    page_url: &str,
    scan_type: ScanType,
    resolved_project_id: Option<i64>,
    execution_id: i64,
    axe_enabled: bool,
    browser_ran: bool,
    axe_ran: bool,
    browser_build: Option<&str>,
    browser_failure: Option<&str>,
    result: &scanner::ScanResult,
) -> (
    PostScanOutcome,
    Option<crate::core::regression_blame::RegressionNotice>,
) {
    let project_path_for_export = match resolved_project_id {
        Some(project_id) => match db.get_project_path_result(project_id) {
            Ok(path) => path,
            Err(error) => {
                return (
                    PostScanOutcome {
                        scan_id: Err(format!(
                            "could not read project scope for issue persistence: {error}"
                        )),
                        blame_notified: false,
                    },
                    None,
                );
            }
        },
        None => None,
    };
    let site = match resolved_project_id {
        Some(project_id) => db.get_or_create_site_for_project(project_id, environment_url),
        None => db.get_or_create_site(environment_url),
    };
    let site_id = match site {
        Ok(site_id) => site_id,
        Err(error) => {
            let error = format!("Failed to get/create site: {error}");
            tracing::error!("{}", error);
            return (
                PostScanOutcome {
                    scan_id: Err(error),
                    blame_notified: false,
                },
                None,
            );
        }
    };

    // Capture the pre-run active set before the canonical persistence
    // transaction reconciles the current projection.
    let blame_snapshot = if should_capture_regression_snapshot(scan_type, browser_failure) {
        resolved_project_id.and_then(|project_id| {
            let previous = db
                .get_scan_history_for_project(project_id, environment_url, 5)
                .ok()?
                .into_iter()
                .find(|entry| entry.scan_type == ScanType::Health)
                .map(|entry| crate::core::regression_blame::PreviousScan {
                    scan_id: entry.id,
                    overall_score: entry.overall_score as i64,
                    timestamp: entry.timestamp,
                });
            match crate::core::regression_blame::capture_snapshot(
                db.as_ref(),
                project_id,
                environment_url,
                "web_scan",
                previous,
            ) {
                Ok(snapshot) => Some(snapshot),
                Err(error) => {
                    tracing::warn!("Could not capture pre-scan regression state: {error}");
                    None
                }
            }
        })
    } else {
        None
    };

    let completed_at = crate::db::timestamp_text_to_ms(&result.timestamp)
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let started_at = completed_at.saturating_sub(result.duration_ms as i64);
    let mut batch = match normalize_web_scan(
        result,
        execution_id,
        None,
        resolved_project_id,
        site_id,
        ScanRunKind::Single,
        started_at,
    ) {
        Ok(batch) => batch,
        Err(error) => {
            let error = format!("Failed to normalize Web Scan result: {error}");
            return (
                PostScanOutcome {
                    scan_id: Err(error),
                    blame_notified: false,
                },
                None,
            );
        }
    };
    batch.diagnostics.axe_enabled = Some(axe_enabled);
    batch.diagnostics.browser_ran = Some(browser_ran);
    batch.diagnostics.axe_ran = Some(axe_ran);
    batch.diagnostics.browser_build = browser_build.map(str::to_owned);
    batch.environment_url = Some(environment_url.to_string());
    batch.environment_scope_key = crate::db::normalize_env_url(Some(environment_url));
    batch.diagnostics.page_url = Some(page_url.to_string());
    // Claim both authored and effective URLs so redirect findings remain inside
    // the scan coverage that produced them.
    let routes = crate::core::normalized_scan::covered_routes(page_url, &result.url);
    let outcomes = batch_outcomes_on_routes(&batch, &routes);
    batch.coverage = crate::core::normalized_scan::ScanCoverageManifest::derive(
        crate::core::normalized_scan::ScanCoverageKind::Page,
        routes.iter().map(|route| (*route).to_string()).collect(),
        &outcomes,
        crate::core::normalized_scan::ClaimBasis::PerRoute,
    );
    if let Some(failure) = browser_failure {
        batch.status = ScanRunStatus::Failed;
        batch.raw_score = None;
        batch.coverage.successful = false;
        batch.status_detail = Some(format!("Browser analysis failed: {failure}"));
    }
    let scan_id = match db.persist_normalized_scan_run(batch) {
        Ok(run_id) => run_id,
        Err(error) => {
            let error = format!("Failed to save canonical Web Scan run: {error}");
            tracing::error!("{}", error);
            return (
                PostScanOutcome {
                    scan_id: Err(error),
                    blame_notified: false,
                },
                None,
            );
        }
    };
    tracing::info!("Canonical Web Scan saved: run_id={}", scan_id);
    if browser_failure.is_none() {
        super::baseline::record_baseline_observation(db, site_id, Some(scan_id), result);
    }

    let mut outcome = PostScanOutcome {
        scan_id: Ok(scan_id),
        blame_notified: false,
    };
    let mut regression_notice = None;

    if let Some(project_id) = resolved_project_id {
        let notice = blame_snapshot.as_ref().and_then(|blame_snapshot| {
            let current_issues: Vec<crate::core::regression_blame::CurrentIssue> = result
                .issues
                .iter()
                .filter(|issue| {
                    matches!(
                        issue.status,
                        crate::checks::CheckStatus::Fail | crate::checks::CheckStatus::Warn
                    )
                })
                .map(|issue| crate::core::regression_blame::CurrentIssue {
                    check_id: crate::core::correlation::resolve_check_id(
                        "web_scan",
                        &issue.check_id,
                    ),
                    title: issue.title.clone(),
                    severity: issue.severity,
                })
                .collect();
            crate::core::regression_blame::emit_regression_blame(
                crate::core::regression_blame::BlameContext {
                    db: db.as_ref(),
                    project_id,
                    env_url: environment_url,
                    scan_kind: "web",
                    scan_id,
                    current_score: result.overall_score as i64,
                    current_timestamp: &result.timestamp,
                    current_issues: &current_issues,
                    project_path: project_path_for_export.as_deref(),
                },
                blame_snapshot,
            )
        });
        if browser_failure.is_none() {
            crate::core::native_alerts::emit_web_scan_alerts(
                db.as_ref(),
                project_id,
                environment_url,
                scan_id,
                result,
                notice.is_some(),
            );
        }
        outcome.blame_notified = notice.is_some();
        regression_notice = notice;
    }
    if browser_failure.is_none() {
        auto_export_sitecmd_scan(project_path_for_export.as_deref(), result);
    }
    (outcome, regression_notice)
}

fn should_capture_regression_snapshot(scan_type: ScanType, browser_failure: Option<&str>) -> bool {
    scan_type == ScanType::Health && browser_failure.is_none()
}

#[cfg(test)]
#[path = "web_scan_issue_count_tests.rs"]
mod issue_count_tests;

#[cfg(test)]
#[path = "web_scan_coverage_tests.rs"]
mod coverage_tests;

#[cfg(test)]
mod tests {
    //! Behavior-focused tests for the post-scan persistence helpers.
    //!
    //! Full end-to-end persistence is exercised via integration tests against
    //! a real `Database`.

    use super::*;

    fn scan_result_fixture(url: &str) -> scanner::ScanResult {
        scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: url.to_string(),
            mode: "live".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 85,
            categories: vec![],
            issues: vec![],
            detected_stack: None,
            duration_ms: 1234,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn execution_fixture(db: &Database, key: &str) -> i64 {
        let key = key.to_string();
        db.execute(move |conn| {
            conn.execute(
                "INSERT INTO scan_executions (
                    environment_url, environment_scope_key, requested_mode,
                    web_focus, trigger, admission_class, status,
                    idempotency_key, request_fingerprint, started_at, web_status
                 ) VALUES (
                    'https://example.com', 'https://example.com', 'web',
                    'health', 'manual', 'general_scan', 'running',
                    ?1, ?2, 1, 'running'
                 )",
                rusqlite::params![key, format!("v1:{key}")],
            )
            .expect("insert execution");
            conn.last_insert_rowid()
        })
        .expect("execution dispatch")
    }

    fn project_execution_fixture(db: &Database, key: &str, project_id: i64, url: &str) -> i64 {
        let key = key.to_string();
        let url = url.to_string();
        db.execute(move |conn| {
            conn.execute(
                "INSERT INTO scan_executions (
                    project_id, environment_url, environment_scope_key, requested_mode,
                    web_focus, trigger, admission_class, status,
                    idempotency_key, request_fingerprint, started_at, web_status
                 ) VALUES (
                    ?1, ?2, ?2, 'web', 'health', 'manual', 'general_scan', 'running',
                    ?3, ?4, 1, 'running'
                 )",
                rusqlite::params![project_id, url, key, format!("v1:{key}")],
            )
            .expect("insert execution");
            conn.last_insert_rowid()
        })
        .expect("execution dispatch")
    }

    #[test]
    fn persist_scan_blocking_returns_scan_id_when_save_succeeds() {
        let db = crate::db::test_helpers::temp_db_arc();
        let result = scan_result_fixture("https://example.com");
        let execution_id = execution_fixture(&db.db, "web-persist-success");

        let (outcome, notice) = persist_scan_blocking(
            &db.db,
            "https://example.com",
            "https://example.com",
            ScanType::Health,
            None,
            execution_id,
            true,
            true,
            true,
            Some("test-browser"),
            None,
            &result,
        );

        let scan_id = outcome.scan_id.expect("scan persistence should succeed");
        let diagnostics_json: String = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT diagnostics_json FROM scan_runs WHERE id = ?1",
                    [scan_id],
                    |row| row.get(0),
                )
                .expect("stored diagnostics")
            })
            .expect("database worker");
        let diagnostics: crate::core::normalized_scan::NormalizedRunDiagnostics =
            serde_json::from_str(&diagnostics_json).expect("run diagnostics");
        assert!(scan_id > 0);
        assert_eq!(diagnostics.axe_enabled, Some(true));
        assert_eq!(diagnostics.browser_ran, Some(true));
        assert_eq!(diagnostics.axe_ran, Some(true));
        assert_eq!(diagnostics.browser_build.as_deref(), Some("test-browser"));
        assert!(!outcome.blame_notified);
        assert!(notice.is_none());
    }

    #[test]
    fn persist_scan_blocking_marks_browser_failure_as_incomplete() {
        let db = crate::db::test_helpers::temp_db_arc();
        let result = scan_result_fixture("https://example.com");
        let execution_id = execution_fixture(&db.db, "web-browser-failure");

        let (outcome, notice) = persist_scan_blocking(
            &db.db,
            "https://example.com",
            "https://example.com",
            ScanType::Health,
            None,
            execution_id,
            true,
            false,
            false,
            None,
            Some("Failed to create webview: unavailable"),
            &result,
        );

        let scan_id = outcome
            .scan_id
            .expect("partial scan persistence should succeed");
        let stored: (String, Option<i64>, String, Option<String>) = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT status, raw_score, coverage_json, status_detail
                     FROM scan_runs WHERE id = ?1",
                    [scan_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("stored partial run")
            })
            .expect("database worker");
        let coverage: crate::core::normalized_scan::ScanCoverageManifest =
            serde_json::from_str(&stored.2).expect("stored coverage");

        assert_eq!(stored.0, "failed");
        assert_eq!(stored.1, None);
        assert!(!coverage.successful);
        assert_eq!(
            stored.3.as_deref(),
            Some("Browser analysis failed: Failed to create webview: unavailable")
        );
        assert!(!outcome.blame_notified);
        assert!(notice.is_none());
    }

    #[test]
    fn persist_scan_blocking_uses_the_selected_project_site_for_a_shared_url() {
        const URL: &str = "https://shared-persist.example";
        let db = crate::db::test_helpers::temp_db_arc();
        let first = db
            .upsert_project("First", "/tmp/shared-persist-a", None)
            .expect("project a");
        let second = db
            .upsert_project("Second", "/tmp/shared-persist-b", None)
            .expect("project b");
        for project_id in [first, second] {
            db.add_environment(project_id, URL, "Production", "production", "manual")
                .expect("environment");
        }
        db.get_or_create_site_for_project(first, URL)
            .expect("first site");
        let selected_site = db
            .get_or_create_site_for_project(second, URL)
            .expect("selected site");
        let execution_id = project_execution_fixture(&db.db, "web-persist-shared", second, URL);

        let (outcome, _) = persist_scan_blocking(
            &db.db,
            URL,
            URL,
            ScanType::Health,
            Some(second),
            execution_id,
            false,
            false,
            false,
            None,
            None,
            &scan_result_fixture(URL),
        );
        let scan_id = outcome.scan_id.expect("scan persistence");
        let stored: (Option<i64>, Option<i64>) = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT project_id, site_id FROM scan_runs WHERE id = ?1",
                    [scan_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("stored run")
            })
            .expect("database worker");

        assert_eq!(stored, (Some(second), Some(selected_site)));
    }

    #[test]
    fn persist_scan_blocking_surfaces_save_failure_and_command_error_mentions_it() {
        let db = crate::db::test_helpers::temp_db_arc();
        // Break canonical run persistence while leaving site creation intact.
        db.execute(|conn| {
            conn.execute("DROP TABLE scan_findings", [])
                .map_err(|e| e.to_string())
        })
        .expect("db dispatch")
        .expect("drop scan_findings table");

        let result = scan_result_fixture("https://example.com");
        let execution_id = execution_fixture(&db.db, "web-persist-failure");
        let (outcome, notice) = persist_scan_blocking(
            &db.db,
            "https://example.com",
            "https://example.com",
            ScanType::Health,
            None,
            execution_id,
            false,
            false,
            false,
            None,
            None,
            &result,
        );

        let error = outcome
            .scan_id
            .expect_err("saving into a dropped scan_findings table must fail");
        assert!(
            error.contains("Failed to save canonical Web Scan run"),
            "got: {error}"
        );
        assert!(!outcome.blame_notified);
        assert!(notice.is_none());

        // The manual-command wrapper (run_and_persist_scan) returns this to
        // the frontend; it must say the scan ran but persistence failed.
        let command_error = persist_failure_error(&error).to_string();
        assert!(
            command_error.contains("scan completed but could not be fully persisted"),
            "got: {command_error}"
        );
        assert!(
            command_error.contains("Failed to save canonical Web Scan run"),
            "got: {command_error}"
        );
    }

    #[test]
    fn browser_failure_disables_regression_comparison() {
        assert!(should_capture_regression_snapshot(ScanType::Health, None));
        assert!(!should_capture_regression_snapshot(
            ScanType::Health,
            Some("unavailable")
        ));
        assert!(!should_capture_regression_snapshot(
            ScanType::Security,
            None
        ));
    }
}
