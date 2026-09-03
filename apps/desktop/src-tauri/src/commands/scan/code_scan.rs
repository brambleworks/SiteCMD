use crate::checks::ScanCategory;
use crate::core::code_provenance::CodeCheckoutProvenance;
use crate::core::code_scan::{CodeIssueView, CodeScanAuditProgress, CodeScanError};
use crate::core::normalized_scan::normalize_code_scan_with_provenance;
use crate::core::scanner::ScanProgress;
use crate::db::{normalize_env_url, CodeScanResult, CodeScanSummary, Database};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

use super::control::ScanControlState;
use super::domain_summary::{build_domain_summaries, select_relevant_previous_code_scan_summary};
use crate::commands::sanitize_error;

struct SourceControlledCodeScanReport {
    report: crate::core::code_scan::CodeScanReport,
    evidence_report: crate::core::code_scan::CodeScanReport,
    ignored_count: usize,
    suppressed_occurrence_ids: std::collections::BTreeSet<String>,
}

fn apply_source_control_suppressions(
    project_path: &Path,
    report: crate::core::code_scan::CodeScanReport,
    today: chrono::NaiveDate,
) -> Result<SourceControlledCodeScanReport, String> {
    let evidence_report = report.clone();
    let audit =
        crate::cli::audit_suppressions::apply_project_suppressions(project_path, report, today)?;
    let suppressed_occurrence_ids = audit
        .ignored_findings
        .iter()
        .map(|finding| crate::core::normalized_scan::code_scan_occurrence_id(&finding.issue))
        .collect();
    Ok(SourceControlledCodeScanReport {
        ignored_count: audit.ignored_findings.len(),
        report: audit.report,
        evidence_report,
        suppressed_occurrence_ids,
    })
}

fn mark_suppressed_findings(
    batch: &mut crate::core::normalized_scan::NormalizedRunBatch,
    suppressed_occurrence_ids: &std::collections::BTreeSet<String>,
) -> Result<(), String> {
    let mut marked = 0;
    for finding in &mut batch.findings {
        if suppressed_occurrence_ids.contains(&finding.occurrence_id) {
            finding.verdict = crate::checks::CheckStatus::Skipped;
            marked += 1;
        }
    }
    if marked != suppressed_occurrence_ids.len() {
        return Err(
            "Could not preserve every suppressed Code Scan finding as skipped evidence".to_string(),
        );
    }
    Ok(())
}

fn emit_code_scan_progress(app: &AppHandle, progress: CodeScanAuditProgress) {
    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            check_id: progress.check_id.to_string(),
            category: ScanCategory::Config,
            status: progress.status.to_string(),
            results_count: progress.results_count,
            checks_done: progress.checks_done,
            checks_total: progress.checks_total,
        },
    );
}

fn emit_code_scan_progress_step(
    app: &AppHandle,
    check_id: &'static str,
    status: &'static str,
    results_count: usize,
    checks_done: usize,
) {
    emit_code_scan_progress(
        app,
        CodeScanAuditProgress {
            check_id,
            status,
            results_count,
            checks_done,
            checks_total: 100,
        },
    );
}

/// Build blame context only from a previous scan in the same normalized
/// environment; cross-environment history would make existing findings new.
fn blame_previous_scan(
    previous_summary: Option<&CodeScanSummary>,
    current_env: Option<&str>,
) -> Option<crate::core::regression_blame::PreviousScan> {
    let previous = previous_summary?;
    let same_env = match (previous.environment_url.as_deref(), current_env) {
        (None, None) => true,
        (Some(previous_env), Some(current_env)) => {
            normalize_env_url(Some(previous_env)) == normalize_env_url(Some(current_env))
        }
        _ => false,
    };
    same_env.then(|| crate::core::regression_blame::PreviousScan {
        scan_id: previous.id,
        overall_score: previous.overall_score as i64,
        timestamp: previous.checked_at.clone(),
    })
}

#[tracing::instrument(skip(app, db, scan_control, project_path_hint, environment_url), fields(project_id, has_project_path_hint = project_path_hint.is_some(), scan_request_id))]
pub(crate) async fn run_code_scan_internal(
    app: AppHandle,
    db: Arc<Database>,
    scan_control: &ScanControlState,
    project_id: i64,
    project_path_hint: Option<&str>,
    environment_url: Option<String>,
    environment_scope_key: String,
    inspect_local_databases: bool,
    scan_request_id: u64,
    execution_id: i64,
) -> Result<CodeScanResult, CodeScanError> {
    // Code Scan runs locally with zero marginal cost - free users can run it
    // and see summary stats. Detail gating (issue list, fix prompts, dossier)
    // lives in the frontend.
    // Resolving the registered folder is a database read; take it through the
    // async interface so it never parks an async runtime worker.
    let project_path = crate::project_paths::resolve_registered_project_dir_async(
        &db,
        project_id,
        project_path_hint,
    )
    .await
    .map_err(CodeScanError::Failed)?;
    let project_path = crate::core::code_scan::validate_project_path(&project_path)
        .map_err(CodeScanError::Failed)?;
    let project_path = project_path.to_string_lossy().to_string();

    let started_at = Instant::now();
    let is_cancelled = || scan_control.is_cancelled(scan_request_id);
    let result = async {
        if is_cancelled() {
            return Err(CodeScanError::Cancelled);
        }

        if cfg!(debug_assertions) {
            tracing::warn!("code_scan: starting audit_project");
        }
        let path_clone = project_path.clone();
        let progress_app = app.clone();
        // The audit polls this between stages and before every file, so a
        // cancel lands inside the CPU-heavy pass rather than after it.
        let audit_control = scan_control.clone();
        let (provenance, report) = tokio::task::spawn_blocking(move || {
            let before = CodeCheckoutProvenance::capture(&path_clone);
            let report = crate::core::code_scan::audit_project_with_control(
                std::path::Path::new(&path_clone),
                crate::core::code_scan::CodeScanOptions {
                    inspect_local_databases,
                },
                move |progress| emit_code_scan_progress(&progress_app, progress),
                move || audit_control.is_cancelled(scan_request_id),
            );
            let report = report.and_then(|report| {
                apply_source_control_suppressions(
                    Path::new(&path_clone),
                    report,
                    chrono::Utc::now().date_naive(),
                )
                .map_err(CodeScanError::Failed)
            });
            let provenance = before.confirm_unchanged(CodeCheckoutProvenance::capture(&path_clone));
            (provenance, report)
        })
        .await
        .map_err(|e| CodeScanError::Failed(format!("Code scan task failed: {}", e)))?;
        let source_controlled = report.map_err(|error| match error {
            CodeScanError::Cancelled => CodeScanError::Cancelled,
            CodeScanError::Failed(message) => CodeScanError::Failed(sanitize_error(message)),
        })?;
        if source_controlled.ignored_count > 0 {
            tracing::info!(
                ignored_findings = source_controlled.ignored_count,
                "code_scan: applied source-controlled suppressions"
            );
        }
        let SourceControlledCodeScanReport {
            report,
            evidence_report,
            suppressed_occurrence_ids,
            ..
        } = source_controlled;
        if cfg!(debug_assertions) {
            tracing::warn!(issues = report.issue_count, "code_scan: audit_project done");
        }

        if is_cancelled() {
            return Err(CodeScanError::Cancelled);
        }

        // The history read and the blame baseline are synchronous SQLite round
        // trips; keep them off the async worker.
        let (previous_summary, blame_snapshot) = {
            let history_db = db.clone();
            let history_env = environment_url.clone();
            let history_scope_key = environment_scope_key.clone();
            tokio::task::spawn_blocking(move || {
                let previous_history = match history_db.get_code_scan_history(project_id, 10) {
                    Ok(history) => history,
                    Err(error) => {
                        tracing::warn!("Could not load prior Code Scan summary: {}", error);
                        Vec::new()
                    }
                };
                let previous_summary = select_relevant_previous_code_scan_summary(
                    previous_history,
                    history_env.as_deref(),
                );
                let previous_scan =
                    blame_previous_scan(previous_summary.as_ref(), history_env.as_deref());
                let blame_snapshot = crate::core::regression_blame::capture_snapshot(
                    history_db.as_ref(),
                    project_id,
                    &history_scope_key,
                    "code_scan",
                    previous_scan,
                );
                (previous_summary, blame_snapshot)
            })
            .await
            .map_err(|error| {
                CodeScanError::Failed(format!("Code scan history task failed: {error}"))
            })?
        };
        let blame_snapshot =
            blame_snapshot.map_err(|error| CodeScanError::Failed(sanitize_error(error)))?;
        let duration_ms = started_at.elapsed().as_millis() as u64;
        let domain_summaries = build_domain_summaries(&report.issues);
        let overall_score = crate::core::code_scan::score_report(&report);
        // Last gate before the save stage is announced or anything is written:
        // a cancel that landed while the history read or the blame baseline
        // was in flight must leave no run behind, and no save step stuck on
        // running either. Only synchronous work separates this check from the
        // write; nothing between them can await a cancel.
        if is_cancelled() {
            return Err(CodeScanError::Cancelled);
        }
        if cfg!(debug_assertions) {
            tracing::warn!("code_scan: saving scan record");
        }
        emit_code_scan_progress_step(&app, "code-scan.save", "running", report.issue_count, 88);
        let completed_at = crate::db::timestamp_text_to_ms(&report.checked_at)
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
        let run_started_at = completed_at.saturating_sub(duration_ms as i64);
        let mut batch = normalize_code_scan_with_provenance(
            &evidence_report,
            execution_id,
            project_id,
            environment_url.clone(),
            environment_scope_key.clone(),
            project_path.clone(),
            overall_score,
            duration_ms,
            run_started_at,
            provenance,
        )
        .map_err(|error| {
            CodeScanError::Failed(format!("Could not normalize Code Scan result: {error}"))
        })?;
        mark_suppressed_findings(&mut batch, &suppressed_occurrence_ids)
            .map_err(CodeScanError::Failed)?;
        // Persist through the async database interface so the report write
        // never parks the async worker on the SQLite thread.
        let scan_id = db
            .persist_normalized_scan_run_async(batch)
            .await
            .map_err(|error| CodeScanError::Failed(sanitize_error(error)))?;
        emit_code_scan_progress_step(&app, "code-scan.save", "complete", report.issue_count, 90);
        if cfg!(debug_assertions) {
            tracing::warn!(scan_id, "code_scan: scan record saved");
        }

        emit_code_scan_progress_step(
            &app,
            "code-scan.work-items",
            "complete",
            report.issue_count,
            94,
        );

        // Build blame input from active findings when an environment is available.
        let current_issues: Vec<crate::core::regression_blame::CurrentIssue> = report
            .issues
            .iter()
            .map(|issue| crate::core::regression_blame::CurrentIssue {
                check_id: crate::core::code_scan::canonical_code_check_id(&issue.id),
                title: issue.title.clone(),
                severity: issue.severity,
            })
            .collect();
        // Keep Git blame work off the async runtime.
        let notice = match environment_url.clone() {
            Some(env) => {
                let blame_db = db.clone();
                let blame_checked_at = report.checked_at.clone();
                let blame_project_path = project_path.clone();
                tokio::task::spawn_blocking(move || {
                    crate::core::regression_blame::emit_regression_blame(
                        crate::core::regression_blame::BlameContext {
                            db: blame_db.as_ref(),
                            project_id,
                            env_url: &env,
                            scan_kind: "code",
                            scan_id,
                            current_score: overall_score as i64,
                            current_timestamp: &blame_checked_at,
                            current_issues: &current_issues,
                            project_path: Some(blame_project_path.as_str()),
                        },
                        &blame_snapshot,
                    )
                })
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("regression blame task failed: {}", e);
                    None
                })
            }
            None => None,
        };

        if cfg!(debug_assertions) {
            tracing::warn!("code_scan: emitting native alerts");
        }
        // Alert upserts are further SQLite writes; offload them too.
        {
            let alerts_db = db.clone();
            let alerts_env = environment_url.clone();
            let alerts_checked_at = report.checked_at.clone();
            let alerts_issue_count = report.issue_count as u32;
            let alerts_critical_count = report.critical_count as u32;
            let alerts_high_count = report.high_count as u32;
            let alerts_notified = notice.is_some();
            if let Err(error) = tokio::task::spawn_blocking(move || {
                crate::core::native_alerts::emit_code_scan_alerts(
                    alerts_db.as_ref(),
                    project_id,
                    alerts_env.as_deref(),
                    scan_id,
                    &alerts_checked_at,
                    overall_score,
                    alerts_issue_count,
                    alerts_critical_count,
                    alerts_high_count,
                    previous_summary.as_ref(),
                    alerts_notified,
                );
            })
            .await
            {
                tracing::error!("Code Scan alert task failed: {}", error);
            }
        }
        if let Some(notice) = notice {
            super::notify_deploy_regression(&app, &notice).await;
        }
        if cfg!(debug_assertions) {
            tracing::warn!("code_scan: native alerts done");
        }

        if cfg!(debug_assertions) {
            tracing::warn!("code_scan: building response");
        }
        emit_code_scan_progress_step(&app, "code-scan.summary", "running", report.issue_count, 96);
        let response = CodeScanResult {
            id: scan_id,
            project_id,
            environment_url: environment_url.clone(),
            overall_score,
            issue_count: report.issue_count as u32,
            critical_count: report.critical_count as u32,
            high_count: report.high_count as u32,
            medium_count: report.medium_count as u32,
            low_count: report.low_count as u32,
            duration_ms,
            checked_at: report.checked_at,
            framework: report.framework,
            domain_summaries,
            skipped_scopes: Some(report.skipped_scopes),
            issues: report.issues.into_iter().map(CodeIssueView::from).collect(),
        };

        emit_code_scan_progress_step(
            &app,
            "code-scan.complete",
            "complete",
            response.issue_count as usize,
            100,
        );
        if cfg!(debug_assertions) {
            tracing::warn!("code_scan: returning result");
        }
        Ok(response)
    }
    .await;

    if let Some(error) = code_scan_failure_alert_error(&result) {
        let failure_db = db.clone();
        let failure_env = environment_url.clone();
        let failure_text = error.to_string();
        if let Err(task_error) = tokio::task::spawn_blocking(move || {
            crate::core::native_alerts::emit_scan_failure_alert(
                failure_db.as_ref(),
                project_id,
                failure_env.as_deref(),
                "Code Scan",
                &failure_text,
            );
        })
        .await
        {
            tracing::error!("Code Scan failure-alert task failed: {}", task_error);
        }
    }

    result
}

/// Return only errors that should produce a scan-failure alert.
fn code_scan_failure_alert_error(
    result: &Result<CodeScanResult, CodeScanError>,
) -> Option<&CodeScanError> {
    match result {
        Err(error) if !crate::core::native_alerts::is_user_cancelled_code_scan(error) => {
            Some(error)
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "code_scan_tests.rs"]
mod tests;
