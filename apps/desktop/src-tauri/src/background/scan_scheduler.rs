//! Background execution for saved scan schedules.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::{
    commands,
    core::scan_control::ScanControlState,
    core::scan_execution::{ScanExecutionMode, ScanTrigger},
    core::scanner::{ScanType, ScheduledScanType},
    db::{Database, ScanSchedule, WebRunComparisonProfile},
    webhooks,
};

#[path = "scan_scheduler_comparison.rs"]
mod comparison;
use comparison::{
    has_complete_full_comparison_baseline, load_full_score_baseline, scan_completion_event_type,
    scan_provenance_matches_previous, scheduled_completion_status, should_notify_score_change,
    should_send_full_scheduler_notification, should_send_scheduler_notification, FullScoreBaseline,
};
#[path = "scan_scheduler_notifications.rs"]
mod notifications;
use notifications::{
    send_code_scan_notification, send_full_scan_notification, send_web_scan_notification,
};
#[path = "scan_scheduler_reporting.rs"]
mod reporting;
use reporting::{scheduled_web_run_kind, summarize_scheduled_web_result, PreviousWebCompletion};

fn scheduled_execution_mode(scan_type: ScheduledScanType) -> ScanExecutionMode {
    match scan_type {
        ScheduledScanType::Full => ScanExecutionMode::Full,
        ScheduledScanType::Code => ScanExecutionMode::Code,
        _ => ScanExecutionMode::Web,
    }
}

fn scheduled_axe_enabled(scan_type: ScheduledScanType) -> bool {
    matches!(
        scan_type,
        ScheduledScanType::Accessibility | ScheduledScanType::Full
    )
}

fn scheduled_web_comparison_profile(
    web_focus: ScanType,
    axe_enabled: bool,
    url: &str,
) -> Option<WebRunComparisonProfile> {
    let parsed = url::Url::parse(url).ok()?;
    let (browser_ran, axe_ran) =
        commands::scan::webview_analysis_profile(web_focus, Some(axe_enabled), &parsed);
    Some(WebRunComparisonProfile {
        axe_enabled,
        browser_ran,
        axe_ran,
    })
}

fn build_scheduled_execution_request(
    schedule: &ScanSchedule,
    url: &str,
    scope_urls: Vec<String>,
    project_path: Option<String>,
    retention: u32,
) -> Result<commands::scan::execution::RunScanExecutionRequest, String> {
    let occurrence = schedule
        .next_run_at
        .as_deref()
        .ok_or_else(|| "scheduled execution is missing its occurrence timestamp".to_string())?;
    let (web_focus, _) = schedule.scan_type.scheduled_run_plan();
    let requested_mode = scheduled_execution_mode(schedule.scan_type);
    Ok(commands::scan::execution::RunScanExecutionRequest {
        project_id: Some(schedule.project_id),
        environment_id: Some(schedule.environment_id),
        environment_url: Some(url.to_string()),
        requested_mode,
        web_focus,
        // Scheduled web runs cover the saved environment scope.
        urls: web_focus.map(|_| scope_urls).unwrap_or_default(),
        enabled_categories: None,
        timeout_secs: None,
        axe_enabled: Some(scheduled_axe_enabled(schedule.scan_type)),
        inspect_local_databases: false,
        project_path,
        scan_request_id: None,
        retention: Some(retention),
        trigger: ScanTrigger::Scheduled,
        idempotency_key: format!("schedule:{}:{occurrence}", schedule.id.unwrap_or_default()),
    })
}

/// Poll for due schedules and register them with shared scan cancellation.
#[tracing::instrument(skip_all)]
pub async fn run(db: Arc<Database>, app_handle: AppHandle, scan_control: ScanControlState) {
    tokio::time::sleep(crate::constants::INITIAL_SCHEDULE_DELAY).await;

    loop {
        run_due_schedules(&db, &app_handle, &scan_control).await;
        tokio::time::sleep(crate::constants::SCHEDULE_POLL_INTERVAL).await;
    }
}

async fn run_due_schedules(
    db: &Arc<Database>,
    app_handle: &AppHandle,
    scan_control: &ScanControlState,
) {
    match commands::scan::get_due_schedules(db) {
        Ok(schedules) => {
            for (schedule, url) in schedules {
                run_due_schedule(db, app_handle, scan_control, schedule, url).await;
            }
        }
        Err(error) => tracing::error!("Schedule check failed: {}", error),
    }
}

async fn run_due_schedule(
    db: &Arc<Database>,
    app_handle: &AppHandle,
    scan_control: &ScanControlState,
    schedule: ScanSchedule,
    url: String,
) {
    let schedule_id = match schedule.id {
        Some(id) => id,
        None => return,
    };
    let safe_url = crate::log_sanitizer::log_safe_url_target(&url);

    tracing::info!(
        schedule_id,
        project_id = schedule.project_id,
        url = %safe_url,
        scan_type = %schedule.scan_type,
        "Scheduled {} scan triggered for {}",
        schedule.scan_type,
        safe_url,
    );

    let (web_focus, wants_code) = schedule.scan_type.scheduled_run_plan();
    let requested_mode = scheduled_execution_mode(schedule.scan_type);
    let axe_enabled = scheduled_axe_enabled(schedule.scan_type);
    let scope_urls = crate::db::scan_scope_urls_for_project(db, schedule.project_id, &url);
    let web_comparison_profile =
        web_focus.and_then(|focus| scheduled_web_comparison_profile(focus, axe_enabled, &url));
    let previous_web_completion =
        if let (Some(web_focus), Some(profile)) = (web_focus, web_comparison_profile) {
            let run_kind = scheduled_web_run_kind(&scope_urls);
            match db.get_latest_web_run_baseline_for_project(
                schedule.project_id,
                &url,
                run_kind,
                web_focus,
                requested_mode,
                profile,
                &scope_urls,
            ) {
                Ok(baseline) => baseline.map(|(run_id, score, critical)| PreviousWebCompletion {
                    run_id,
                    score,
                    critical: critical as usize,
                }),
                Err(error) => {
                    tracing::warn!(
                        "Scheduled scan for {} could not read its previous Web result: {}",
                        safe_url,
                        error
                    );
                    None
                }
            }
        } else {
            None
        };
    let previous_code_result = if wants_code {
        match db.get_latest_scheduled_code_run_baseline_for_project(
            schedule.project_id,
            &url,
            requested_mode,
        ) {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(
                    "Scheduled scan for {} could not read its previous Code result: {}",
                    safe_url,
                    error
                );
                None
            }
        }
    } else {
        None
    };
    let request = build_scheduled_execution_request(
        &schedule,
        &url,
        scope_urls.clone(),
        db.get_project_path(schedule.project_id),
        commands::scan::configured_scan_retention(app_handle),
    );

    match request {
        Ok(request) => match commands::scan::execution::run_scan_execution_internal(
            app_handle.clone(),
            db.clone(),
            scan_control,
            request,
        )
        .await
        {
            Ok(result) => {
                report_scheduled_execution(
                    db,
                    app_handle,
                    &schedule,
                    &url,
                    previous_web_completion.as_ref(),
                    previous_code_result.as_ref(),
                    web_comparison_profile,
                    &scope_urls,
                    &result,
                )
                .await;
            }
            Err(error) => tracing::error!(
                schedule_id,
                "Scheduled execution for {} failed before completion: {}",
                safe_url,
                error
            ),
        },
        Err(error) => tracing::error!(schedule_id, "Scheduled execution plan failed: {}", error),
    }

    if let Err(error) = commands::scan::mark_schedule_run(
        db,
        schedule_id,
        &schedule.frequency,
        &schedule.time_of_day,
        schedule.day_of_week,
    ) {
        tracing::error!("Failed to update schedule: {}", error);
    }
}

async fn report_scheduled_execution(
    db: &Arc<Database>,
    app_handle: &AppHandle,
    schedule: &ScanSchedule,
    url: &str,
    previous_web_completion: Option<&PreviousWebCompletion>,
    previous_code_result: Option<&crate::db::CodeScanResult>,
    web_comparison_profile: Option<WebRunComparisonProfile>,
    scope_urls: &[String],
    result: &commands::scan::execution::RunScanExecutionResult,
) {
    if result.reused {
        tracing::info!(
            execution_id = result.execution.id,
            "Scheduled occurrence already has an execution; no collection restarted"
        );
        return;
    }

    let execution_summary = db
        .get_scan_execution_detail(result.execution.id)
        .ok()
        .flatten()
        .map(|detail| detail.summary);
    let web_scan_id = execution_summary
        .as_ref()
        .and_then(|execution| execution.web_scan_id);
    let web_session_id = execution_summary
        .as_ref()
        .and_then(|execution| execution.web_session_id);
    let current_web_profile_matches = execution_summary.as_ref().is_some_and(|execution| {
        web_comparison_profile.is_some_and(|profile| {
            crate::db::web_execution_matches_comparison_profile(
                execution,
                scheduled_web_run_kind(scope_urls),
                profile,
                scope_urls,
            )
        })
    });
    let mut web_completion = summarize_scheduled_web_result(
        result.web_result.as_ref(),
        result.multi_result.as_ref(),
        web_scan_id,
        web_session_id,
        &chrono::Utc::now().to_rfc3339(),
    );
    if let Some(completion) = web_completion.as_mut() {
        completion.comparison_eligible &= current_web_profile_matches
            && scan_provenance_matches_previous(
                db,
                previous_web_completion.map(|previous| previous.run_id),
                completion.scan_id,
            );
    }
    let completion_status = scheduled_completion_status(
        result.execution.status,
        web_completion
            .as_ref()
            .map(|completion| completion.scope_complete),
    );
    let mut full_blame_notified = false;
    let mut full_uncovered_regression = false;
    if let Some(web_result) = web_completion.as_ref() {
        let previous_web_score = web_result
            .comparison_eligible
            .then(|| previous_web_completion.map(|scan| scan.score))
            .flatten();
        let previous_web_critical = web_result
            .comparison_eligible
            .then(|| previous_web_completion.map(|scan| scan.critical))
            .flatten();
        let blame_notified = web_result.comparison_eligible
            && web_result.regression_scan_ids.iter().any(|scan_id| {
                db.get_regression_by_scan("web", *scan_id)
                    .ok()
                    .flatten()
                    .is_some()
            });
        let should_notify = web_result.comparison_eligible
            && should_notify_score_change(
                previous_web_score,
                web_result.score,
                previous_web_critical,
                web_result.counts.critical,
            );
        let notify_gate = web_result.comparison_eligible
            && should_send_scheduler_notification(
                blame_notified,
                previous_web_score,
                web_result.score,
                previous_web_critical,
                web_result.counts.critical,
            );
        full_blame_notified |= blame_notified;
        full_uncovered_regression |= notify_gate;

        if schedule.scan_type != ScheduledScanType::Full {
            let _ = app_handle.emit(
                "scheduled-scan-complete",
                serde_json::json!({
                    "executionId": result.execution.id,
                    "projectId": schedule.project_id,
                    "url": url,
                    "scanId": web_result.scan_id,
                    "score": web_result.score,
                    "issues": web_result.counts.total,
                    "scanType": schedule.scan_type.as_str(),
                    "timestamp": web_result.timestamp,
                    "status": completion_status,
                    "completedPages": web_result.completed_pages,
                    "totalPages": web_result.total_pages,
                }),
            );
            if notify_gate {
                send_web_scan_notification(
                    app_handle,
                    url,
                    result.execution.web_focus.unwrap_or(ScanType::Health),
                    previous_web_score,
                    web_result.score,
                    web_result.counts.critical,
                    web_result.counts.total,
                );
            }
        }

        let event_type = scan_completion_event_type(should_notify);
        webhooks::fire_scan_webhooks(
            app_handle,
            db.as_ref(),
            schedule.project_id,
            event_type,
            serde_json::json!({
                "event": event_type,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "data": {
                    "execution_id": result.execution.id,
                    "url": url,
                    "score": web_result.score,
                    "previous_score": previous_web_score,
                    "issues_total": web_result.counts.total,
                    "critical_issues": web_result.counts.critical,
                    "comparison_eligible": web_result.comparison_eligible,
                    "status": completion_status,
                    "completed_pages": web_result.completed_pages,
                    "total_pages": web_result.total_pages,
                    "scan_type": result.execution.web_focus.unwrap_or(ScanType::Health).as_str(),
                }
            }),
        )
        .await;
    }

    let mut code_comparison_eligible = true;
    if let Some(code_result) = result.code_result.as_ref() {
        code_comparison_eligible = scan_provenance_matches_previous(
            db,
            previous_code_result.map(|previous| previous.id),
            Some(code_result.id),
        );
        let previous_score = code_comparison_eligible
            .then(|| previous_code_result.map(|entry| entry.overall_score))
            .flatten();
        let previous_critical = code_comparison_eligible
            .then(|| previous_code_result.map(|entry| entry.critical_count as usize))
            .flatten();
        let leading_domain =
            commands::scan::top_code_scan_domain_from_summaries(&code_result.domain_summaries);
        let previous_top_domain = previous_code_result.and_then(|previous| {
            commands::scan::top_code_scan_domain_from_summaries(&previous.domain_summaries)
                .map(|(domain, _)| domain)
        });
        let domain_trend_label = code_comparison_eligible
            .then_some(previous_code_result)
            .flatten()
            .and_then(|previous| {
                commands::scan::describe_code_scan_domain_trend(
                    &code_result.domain_summaries,
                    &previous.domain_summaries,
                    leading_domain.map(|(domain, _)| domain),
                    previous_top_domain,
                )
            });
        let blame_notified = code_comparison_eligible
            && db
                .get_regression_by_scan("code", code_result.id)
                .ok()
                .flatten()
                .is_some();
        let should_notify = code_comparison_eligible
            && should_notify_score_change(
                previous_score,
                code_result.overall_score,
                previous_critical,
                code_result.critical_count as usize,
            );
        let notify_gate = code_comparison_eligible
            && should_send_scheduler_notification(
                blame_notified,
                previous_score,
                code_result.overall_score,
                previous_critical,
                code_result.critical_count as usize,
            );
        full_blame_notified |= blame_notified;
        full_uncovered_regression |= notify_gate;

        if schedule.scan_type != ScheduledScanType::Full {
            let _ = app_handle.emit(
                "scheduled-scan-complete",
                serde_json::json!({
                    "executionId": result.execution.id,
                    "projectId": schedule.project_id,
                    "url": url,
                    "scanId": code_result.id,
                    "score": code_result.overall_score,
                    "issues": code_result.issue_count,
                    "scanType": "code",
                    "timestamp": code_result.checked_at,
                    "topDomain": leading_domain.map(|(domain, _)| domain.as_str()),
                    "topDomainCount": leading_domain.map(|(_, count)| count).unwrap_or(0),
                    "domainTrendLabel": domain_trend_label,
                    "status": completion_status,
                }),
            );
            if notify_gate {
                send_code_scan_notification(
                    app_handle,
                    url,
                    previous_score,
                    code_result.overall_score,
                    code_result.critical_count as usize,
                    code_result.issue_count as usize,
                    domain_trend_label.as_deref(),
                );
            }
        }

        let event_type = scan_completion_event_type(should_notify);
        webhooks::fire_scan_webhooks(
            app_handle,
            db.as_ref(),
            schedule.project_id,
            event_type,
            serde_json::json!({
                "event": event_type,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "data": {
                    "execution_id": result.execution.id,
                    "url": url,
                    "score": code_result.overall_score,
                    "previous_score": previous_score,
                    "issues_total": code_result.issue_count,
                    "critical_issues": code_result.critical_count,
                    "comparison_eligible": code_comparison_eligible,
                    "status": completion_status,
                    "scan_type": "code",
                }
            }),
        )
        .await;
    }

    if schedule.scan_type == ScheduledScanType::Full
        && (web_completion.is_some() || result.code_result.is_some())
    {
        let web_completed = web_completion.is_some();
        let code_completed = result.code_result.is_some();
        let has_comparable_baseline = has_complete_full_comparison_baseline(
            web_completed,
            web_completion.as_ref().is_some_and(|completion| {
                completion.comparison_eligible && previous_web_completion.is_some()
            }),
            code_completed,
            code_comparison_eligible && previous_code_result.is_some(),
        );
        let current_components_comparable = completion_status == "complete"
            && web_completion
                .as_ref()
                .is_none_or(|completion| completion.comparison_eligible)
            && (!code_completed || code_comparison_eligible);
        let completed_pages = web_completion
            .as_ref()
            .map_or(0, |completion| completion.completed_pages);
        let total_pages = web_completion
            .as_ref()
            .map_or(scope_urls.len(), |completion| completion.total_pages);
        let previous_full_baseline = has_comparable_baseline
            .then(|| {
                load_full_score_baseline(
                    db,
                    web_completed
                        .then_some(previous_web_completion.map(|previous| previous.run_id))
                        .flatten(),
                    code_completed
                        .then_some(previous_code_result.map(|previous| previous.id))
                        .flatten(),
                )
            })
            .flatten();
        emit_full_scan_completion(
            db,
            app_handle,
            schedule,
            url,
            FullCompletionReport {
                execution_id: result.execution.id,
                web_scan_id: web_completion
                    .as_ref()
                    .and_then(|completion| completion.scan_id),
                previous_snapshot: previous_full_baseline.as_ref(),
                blame_notified: full_blame_notified,
                uncovered_component_regression: full_uncovered_regression,
                comparison_eligible: current_components_comparable,
                completion_status,
                completed_pages,
                total_pages,
            },
        )
        .await;
    }
}

struct FullCompletionReport<'a> {
    execution_id: i64,
    web_scan_id: Option<i64>,
    previous_snapshot: Option<&'a FullScoreBaseline>,
    blame_notified: bool,
    uncovered_component_regression: bool,
    comparison_eligible: bool,
    completion_status: &'a str,
    completed_pages: usize,
    total_pages: usize,
}

/// Emit one score, event, and optional notification for a full scheduled scan.
async fn emit_full_scan_completion(
    db: &Arc<Database>,
    app_handle: &AppHandle,
    schedule: &ScanSchedule,
    url: &str,
    report: FullCompletionReport<'_>,
) {
    let FullCompletionReport {
        execution_id,
        web_scan_id,
        previous_snapshot,
        blame_notified,
        uncovered_component_regression,
        comparison_eligible,
        completion_status,
        completed_pages,
        total_pages,
    } = report;
    let snapshot = match crate::commands::issues::compute_and_record_current_score(
        db,
        schedule.project_id,
        Some(url),
        chrono::Utc::now().timestamp_millis(),
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::warn!(
                "Scheduled full scan for {} finished but the unified score was unavailable: {}",
                crate::log_sanitizer::log_safe_url_target(url),
                error
            );
            return;
        }
    };
    let score = snapshot.overall.round() as u32;
    let issue_count =
        snapshot.critical_count + snapshot.high_count + snapshot.medium_count + snapshot.low_count;
    let should_notify = should_send_full_scheduler_notification(
        comparison_eligible,
        blame_notified,
        uncovered_component_regression,
        previous_snapshot,
        &snapshot,
    );
    let _ = app_handle.emit(
        "scheduled-scan-complete",
        serde_json::json!({
            "executionId": execution_id,
            "projectId": schedule.project_id,
            "url": url,
            "scanId": web_scan_id,
            "score": score,
            "issues": issue_count,
            "scanType": "full",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": completion_status,
            "completedPages": completed_pages,
            "totalPages": total_pages,
        }),
    );

    if should_notify {
        send_full_scan_notification(app_handle, url, score, issue_count, snapshot.critical_count);
    }
}

#[cfg(test)]
#[path = "scan_scheduler_tests.rs"]
mod tests;
