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
    let project_path =
        crate::project_paths::resolve_registered_project_dir(&db, project_id, project_path_hint)
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
        let (provenance, report) = tokio::task::spawn_blocking(move || {
            let before = CodeCheckoutProvenance::capture(&path_clone);
            let report = crate::core::code_scan::audit_project_with_options_and_progress(
                std::path::Path::new(&path_clone),
                crate::core::code_scan::CodeScanOptions {
                    inspect_local_databases,
                },
                move |progress| emit_code_scan_progress(&progress_app, progress),
            );
            let report = report.and_then(|report| {
                apply_source_control_suppressions(
                    Path::new(&path_clone),
                    report,
                    chrono::Utc::now().date_naive(),
                )
            });
            let provenance = before.confirm_unchanged(CodeCheckoutProvenance::capture(&path_clone));
            (provenance, report)
        })
        .await
        .map_err(|e| CodeScanError::Failed(format!("Code scan task failed: {}", e)))?;
        let source_controlled =
            report.map_err(|error| CodeScanError::Failed(sanitize_error(error)))?;
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

        let previous_history = match db.get_code_scan_history(project_id, 10) {
            Ok(history) => history,
            Err(error) => {
                tracing::warn!("Could not load prior Code Scan summary: {}", error);
                Vec::new()
            }
        };
        let previous_summary = select_relevant_previous_code_scan_summary(
            previous_history,
            environment_url.as_deref(),
        );
        let duration_ms = started_at.elapsed().as_millis() as u64;
        let domain_summaries = build_domain_summaries(&report.issues);
        let overall_score = crate::core::code_scan::score_report(&report);
        let env_url_str = environment_scope_key.as_str();
        let previous_scan =
            blame_previous_scan(previous_summary.as_ref(), environment_url.as_deref());
        let blame_snapshot = crate::core::regression_blame::capture_snapshot(
            db.as_ref(),
            project_id,
            env_url_str,
            "code_scan",
            previous_scan,
        )
        .map_err(|error| CodeScanError::Failed(sanitize_error(error)))?;
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
        let scan_id = db
            .persist_normalized_scan_run(batch)
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
        crate::core::native_alerts::emit_code_scan_alerts(
            db.as_ref(),
            project_id,
            environment_url.as_deref(),
            scan_id,
            &report.checked_at,
            overall_score,
            report.issue_count as u32,
            report.critical_count as u32,
            report.high_count as u32,
            previous_summary.as_ref(),
            notice.is_some(),
        );
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
        crate::core::native_alerts::emit_scan_failure_alert(
            db.as_ref(),
            project_id,
            environment_url.as_deref(),
            "Code Scan",
            &error.to_string(),
        );
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
mod tests {
    //! Blame helpers must compare only scans from the same environment;
    //! cross-environment fallback is valid for trends, not regression attribution.

    use super::*;

    fn summary(id: i64, environment_url: Option<&str>) -> CodeScanSummary {
        CodeScanSummary {
            id,
            project_id: 1,
            environment_url: environment_url.map(str::to_string),
            overall_score: 88,
            issue_count: 3,
            grouped_issue_count: 3,
            critical_count: 0,
            high_count: 1,
            duration_ms: 1200,
            checked_at: "2026-06-09T12:00:00Z".to_string(),
            framework: None,
            top_domain: None,
            top_domain_count: 0,
            domain_summaries: Vec::new(),
        }
    }

    #[test]
    fn blame_previous_scan_same_env_normalized_variants_match() {
        // Trailing slash and host case differences must normalize equal,
        // exactly like the work_items env key (normalize_env_url).
        let previous = summary(7, Some("https://Example.COM/app/"));
        let result = blame_previous_scan(Some(&previous), Some("https://example.com/app"))
            .expect("normalized-equal envs must produce a blame PreviousScan");
        assert_eq!(result.scan_id, 7);
        assert_eq!(result.overall_score, 88);
        assert_eq!(result.timestamp, "2026-06-09T12:00:00Z");
    }

    #[test]
    fn blame_previous_scan_differing_envs_returns_none() {
        let previous = summary(7, Some("https://staging.example.com"));
        assert!(
            blame_previous_scan(Some(&previous), Some("https://example.com")).is_none(),
            "cross-env history must not feed blame"
        );
    }

    #[test]
    fn blame_previous_scan_env_less_history_with_env_scan_returns_none() {
        // First scan under a new env key: the only history is project-wide
        // (NULL env). Blaming against it would mark every finding "new".
        let previous = summary(7, None);
        assert!(blame_previous_scan(Some(&previous), Some("https://example.com")).is_none());
    }

    #[test]
    fn blame_previous_scan_both_env_less_matches() {
        // Env-less project history is consistent with an env-less current
        // scan; blame may diff against it.
        let previous = summary(9, None);
        let result = blame_previous_scan(Some(&previous), None)
            .expect("both-None envs are the same key space");
        assert_eq!(result.scan_id, 9);
    }

    #[test]
    fn blame_previous_scan_without_history_returns_none() {
        assert!(blame_previous_scan(None, Some("https://example.com")).is_none());
        assert!(blame_previous_scan(None, None).is_none());
    }

    fn code_scan_result_fixture() -> CodeScanResult {
        CodeScanResult {
            id: 1,
            project_id: 1,
            environment_url: None,
            overall_score: 100,
            issue_count: 0,
            critical_count: 0,
            high_count: 0,
            medium_count: 0,
            low_count: 0,
            duration_ms: 10,
            checked_at: "2026-06-09T12:00:00Z".to_string(),
            framework: None,
            domain_summaries: Vec::new(),
            skipped_scopes: Default::default(),
            issues: Vec::new(),
        }
    }

    #[test]
    fn failure_alert_skips_user_cancellation() {
        let result: Result<CodeScanResult, CodeScanError> = Err(CodeScanError::Cancelled);
        let error = result.as_ref().expect_err("fixture is a cancellation");
        assert!(
            crate::core::native_alerts::is_user_cancelled_code_scan(error),
            "the typed predicate must match the Cancelled variant"
        );
        assert!(
            code_scan_failure_alert_error(&result).is_none(),
            "a user cancellation must not record a scan-failure alert"
        );
    }

    #[test]
    fn failure_alert_fires_for_real_errors() {
        let result: Result<CodeScanResult, CodeScanError> = Err(CodeScanError::Failed(
            "Code scan task failed: boom".to_string(),
        ));
        let error = code_scan_failure_alert_error(&result)
            .expect("engine/infra failures must record a scan-failure alert");
        assert!(matches!(error, CodeScanError::Failed(_)));
    }

    #[test]
    fn failure_alert_skips_success() {
        let result: Result<CodeScanResult, CodeScanError> = Ok(code_scan_result_fixture());
        assert!(code_scan_failure_alert_error(&result).is_none());
    }

    #[test]
    fn source_control_suppressions_filter_the_persisted_scan_report() {
        let project = tempfile::tempdir().expect("project");
        let sitecmd_dir = project.path().join(".sitecmd");
        std::fs::create_dir_all(&sitecmd_dir).expect("sitecmd directory");
        std::fs::write(
            sitecmd_dir.join("config.json"),
            r#"{
  "version": 1,
  "url": "https://example.com",
  "name": "Suppressed project",
  "code_scan": {
    "suppressions": [{
      "match": {
        "rule": "code_scan.cors-origin-reflection",
        "path": "content/security.ts"
      },
      "reason": "This file contains inert security guidance."
    }]
  }
}"#,
        )
        .expect("suppression config");
        let issue = crate::core::code_scan::CodeIssue {
            id: "cors-origin-reflection:content/security.ts:371".to_string(),
            check_id: "code_scan.cors-origin-reflection".to_string(),
            category: "security".to_string(),
            severity: crate::checks::Severity::High,
            title: "CORS reflects the request origin while allowing credentials".to_string(),
            description: "The source appears to reflect credentialed origins.".to_string(),
            relative_path: "content/security.ts".to_string(),
            absolute_path: project
                .path()
                .join("content/security.ts")
                .to_string_lossy()
                .to_string(),
            line: Some(371),
            source_excerpt: Some("replace origin: true with an exact allowlist".to_string()),
            evidence: None,
            why_now: None,
            likely_fix: Some("Use an exact allowlist.".to_string()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        };
        let report = crate::core::code_scan::CodeScanReport {
            checked_at: "2026-08-31T12:00:00Z".to_string(),
            framework: Some("typescript".to_string()),
            issue_count: 1,
            critical_count: 0,
            high_count: 1,
            medium_count: 0,
            low_count: 0,
            issues: vec![issue],
            skipped_scopes: Default::default(),
        };

        let filtered = apply_source_control_suppressions(
            project.path(),
            report,
            chrono::NaiveDate::from_ymd_opt(2026, 8, 31).expect("date"),
        )
        .expect("valid suppression");

        assert_eq!(filtered.report.issue_count, 0);
        assert!(filtered.report.issues.is_empty());
        assert_eq!(filtered.ignored_count, 1);
        assert_eq!(filtered.evidence_report.issue_count, 1);

        let mut batch = crate::core::normalized_scan::normalize_code_scan(
            &filtered.evidence_report,
            1,
            1,
            Some("https://example.com".to_string()),
            "https://example.com".to_string(),
            project.path().to_string_lossy().to_string(),
            100,
            10,
            1_000,
        )
        .expect("normalize evidence");
        mark_suppressed_findings(&mut batch, &filtered.suppressed_occurrence_ids)
            .expect("mark suppressed evidence");

        assert_eq!(batch.findings.len(), 1);
        assert_eq!(
            batch.findings[0].verdict,
            crate::checks::CheckStatus::Skipped
        );
    }
}
