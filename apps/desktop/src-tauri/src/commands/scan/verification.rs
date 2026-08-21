//! Re-run a bounded check set against one page.
//! Verification shares execution lifecycle machinery but claims only the checks observed.

use std::sync::Arc;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::core::normalized_scan::{
    covered_routes, normalize_web_scan, CheckOutcome, ClaimBasis, ScanCoverageKind,
    ScanCoverageManifest, ScanRunKind,
};
use crate::core::scan_execution::{
    NewScanExecution, ScanAdmissionClass, ScanAdmissionError, ScanComponent, ScanComponentStatus,
    ScanExecutionMode, ScanTrigger,
};
use crate::core::scanner::{ScanError, ScanResult, ScanType, VerifyChecksResult};
use crate::db::{normalize_env_url, Database};

use super::control::ScanControlState;
use super::execution::{component_failure_status, generate_scan_action_key};
use super::execution_events::{emit_scan_execution_completed, record_scan_execution_event};

#[derive(Serialize)]
struct BoundedWebVerificationFingerprint<'a> {
    environment_scope_key: &'a str,
    target_url: &'a str,
    check_ids: &'a [String],
}

fn persist_bounded_web_verification(
    db: &Arc<Database>,
    project_id: Option<i64>,
    environment_url: &str,
    page_url: &str,
    scan_result: &ScanResult,
    coverage: ScanCoverageManifest,
    execution_id: i64,
    started_at: i64,
    completed_at: i64,
) -> Result<i64, crate::db::DbError> {
    let site_id = match project_id {
        Some(project_id) => db.get_or_create_site_for_project(project_id, environment_url)?,
        None => db.get_or_create_site(environment_url)?,
    };
    let mut batch = normalize_web_scan(
        scan_result,
        execution_id,
        None,
        project_id,
        site_id,
        ScanRunKind::Single,
        started_at,
    )?;
    batch.completed_at = completed_at;
    batch.raw_score = None;
    batch.environment_url = Some(environment_url.to_string());
    batch.environment_scope_key = normalize_env_url(Some(environment_url));
    batch.diagnostics.page_url = Some(page_url.to_string());
    batch.coverage = coverage;
    db.persist_normalized_scan_run(batch)
}

pub(crate) async fn run_bounded_web_verification(
    app: Option<&AppHandle>,
    db: Arc<Database>,
    scan_control: &ScanControlState,
    project_id: Option<i64>,
    environment_url: Option<String>,
    page_url: String,
    check_ids: Vec<String>,
    scan_request_id: Option<u64>,
    idempotency_key: Option<String>,
) -> Result<VerifyChecksResult, ScanError> {
    crate::commands::validate_url_async(&page_url)
        .await
        .map_err(ScanError::NetworkError)?;
    let mut coverage = crate::core::scanner::verify::required_web_verification_ids(&check_ids)
        .into_iter()
        .collect::<Vec<_>>();
    coverage.sort();
    if coverage.is_empty() {
        return Err(ScanError::ScanFailed(
            "Bounded verification requires at least one registered Web check".into(),
        ));
    }
    let environment_url = environment_url.unwrap_or_else(|| page_url.clone());
    let environment_scope_key = normalize_env_url(Some(&environment_url));
    let project_id = match project_id {
        Some(project_id) => Some(project_id),
        None => db.find_project_for_url(&environment_url),
    };
    let fingerprint_bytes = serde_json::to_vec(&BoundedWebVerificationFingerprint {
        environment_scope_key: &environment_scope_key,
        target_url: &page_url,
        check_ids: &coverage,
    })
    .map_err(|error| {
        ScanError::ScanFailed(format!("could not fingerprint verification: {error}"))
    })?;
    let fingerprint = format!("v1:{}", hex::encode(Sha256::digest(fingerprint_bytes)));
    let action_key = match idempotency_key {
        Some(key) => key,
        None => generate_scan_action_key("verification-web").map_err(ScanError::ScanFailed)?,
    };
    let now = chrono::Local::now();
    let admission = db
        .admit_scan_execution(
            NewScanExecution {
                project_id,
                environment_id: None,
                environment_url: Some(environment_url.clone()),
                environment_scope_key: environment_scope_key.clone(),
                requested_mode: ScanExecutionMode::Web,
                web_focus: Some(ScanType::Health),
                trigger: ScanTrigger::Verification,
                admission_class: ScanAdmissionClass::BoundedVerification,
                idempotency_key: action_key,
                request_fingerprint: fingerprint,
                now_ms: now.timestamp_millis(),
                web_status: Some(ScanComponentStatus::Planned),
                web_detail: Some(format!("check_set:{}", coverage.join(","))),
                code_status: None,
                code_detail: None,
            },
            crate::constants::SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
        )
        .map_err(scan_error_from_admission)?;
    if admission.reused {
        return Err(ScanError::ScanFailed(
            "This verification action was already collected; refresh the issue state".into(),
        ));
    }

    let execution_id = admission.execution.id;
    db.start_scan_execution_component(execution_id, ScanComponent::Web)
        .map_err(|error| ScanError::ScanFailed(error.to_string()))?;
    let started_at = chrono::Utc::now().timestamp_millis();
    let timer = std::time::Instant::now();
    let collected = super::tools::verify_scan_checks_internal(
        app,
        scan_control,
        page_url.clone(),
        check_ids,
        scan_request_id,
    )
    .await;
    let outcome = match collected {
        Ok(result) => {
            let completed_at = chrono::Utc::now().timestamp_millis();
            let duration_ms = timer.elapsed().as_millis() as u64;
            let scan_result = ScanResult {
                url: result.effective_url.clone(),
                mode: "verification".into(),
                scan_type: ScanType::Health,
                overall_score: 0,
                categories: Vec::new(),
                issues: result.results.clone(),
                detected_stack: None,
                duration_ms,
                timestamp: chrono::DateTime::from_timestamp_millis(completed_at)
                    .unwrap_or_default()
                    .to_rfc3339(),
                page_signals: None,
                site_facts: None,
            };
            // Claim authored and effective URLs, but only for checks that reached
            // a verdict; skipped checks remain coverage exceptions.
            let canonical_outcomes = result
                .results
                .iter()
                .map(|check| {
                    (
                        crate::core::correlation::resolve_check_id("web_scan", &check.check_id),
                        check.status,
                    )
                })
                .collect::<Vec<_>>();
            let routes = covered_routes(&page_url, &result.effective_url);
            let outcomes = canonical_outcomes
                .iter()
                .flat_map(|(check_id, status)| {
                    routes.iter().map(move |route| CheckOutcome {
                        route: Some(*route),
                        check_id,
                        status: *status,
                    })
                })
                .collect::<Vec<_>>();
            let coverage_manifest = ScanCoverageManifest::derive(
                ScanCoverageKind::CheckSet,
                routes.iter().map(|route| (*route).to_string()).collect(),
                &outcomes,
                ClaimBasis::PerRoute,
            );
            let db_persist = db.clone();
            let project_id_for_run = project_id;
            let environment_url_for_run = environment_url.clone();
            let page_url_for_run = page_url.clone();
            let persist = crate::commands::run_blocking(move || {
                persist_bounded_web_verification(
                    &db_persist,
                    project_id_for_run,
                    &environment_url_for_run,
                    &page_url_for_run,
                    &scan_result,
                    coverage_manifest,
                    execution_id,
                    started_at,
                    completed_at,
                )
                .map(|_| ())
            })
            .await
            .map_err(|error| {
                ScanError::ScanFailed(format!("verification persistence task failed: {error}"))
            })?
            .map_err(|error| ScanError::ScanFailed(error.to_string()));
            match persist {
                Ok(()) => Ok(result),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    };
    let (status, detail) = match &outcome {
        Ok(_) => (ScanComponentStatus::Complete, None),
        Err(error) => {
            let detail = error.to_string();
            (component_failure_status(&detail), Some(detail))
        }
    };
    let mut execution = db
        .finish_scan_execution_component(
            execution_id,
            ScanComponent::Web,
            status,
            detail,
            chrono::Utc::now().timestamp_millis(),
        )
        .map_err(|error| ScanError::ScanFailed(error.to_string()))?;
    if outcome.is_ok() {
        if let Some(project_id) = project_id {
            if crate::commands::issues::compute_and_record_current_score(
                &db,
                project_id,
                Some(&environment_scope_key),
                chrono::Utc::now().timestamp_millis(),
            )
            .is_ok()
            {
                execution = db
                    .link_scan_execution_score_snapshot(
                        execution_id,
                        project_id,
                        Some(&environment_scope_key),
                    )
                    .map_err(|error| ScanError::ScanFailed(error.to_string()))?;
            }
        }
    }
    if let Err(error) = db.prune_scan_executions_for_scope(
        project_id,
        &environment_scope_key,
        super::policy::scan_retention(None),
        crate::db::ScanRetentionWindow::BoundedVerification,
    ) {
        tracing::warn!(execution_id, "Failed to prune old verifications: {error}");
    }
    record_scan_execution_event(&db, &execution);
    if let Some(app) = app {
        if outcome.is_ok() {
            if let Some(project_id) = execution.project_id {
                crate::commands::emit_site_score_changed(app, project_id);
            }
        }
        emit_scan_execution_completed(app, &execution, None);
    }
    outcome
}

/// Admission verdict to the scanner-facing error the verification path returns.
pub(crate) fn scan_error_from_admission(error: ScanAdmissionError) -> ScanError {
    ScanError::ScanFailed(error.to_string())
}

#[cfg(test)]
#[path = "verification_tests.rs"]
mod tests;
