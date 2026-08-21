//! Background execution for saved scan schedules.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::{
    checks::{CheckStatus, Severity},
    commands,
    core::scan_control::ScanControlState,
    core::scan_execution::{ScanExecutionMode, ScanTrigger},
    core::scanner::{ScanResult, ScanType, ScheduledScanType},
    db::{Database, ScanSchedule, MAX_SCAN_RETENTION},
    webhooks,
};

fn build_scheduled_execution_request(
    schedule: &ScanSchedule,
    url: &str,
    scope_urls: Vec<String>,
    project_path: Option<String>,
) -> Result<commands::scan::execution::RunScanExecutionRequest, String> {
    let occurrence = schedule
        .next_run_at
        .as_deref()
        .ok_or_else(|| "scheduled execution is missing its occurrence timestamp".to_string())?;
    let (web_focus, _) = schedule.scan_type.scheduled_run_plan();
    let requested_mode = match schedule.scan_type {
        ScheduledScanType::Full => ScanExecutionMode::Full,
        ScheduledScanType::Code => ScanExecutionMode::Code,
        _ => ScanExecutionMode::Web,
    };
    Ok(commands::scan::execution::RunScanExecutionRequest {
        project_id: Some(schedule.project_id),
        environment_id: Some(schedule.environment_id),
        environment_url: Some(url.to_string()),
        requested_mode,
        web_focus,
        // The site's scan scope, not the entry URL alone. A scheduled run is
        // the one nobody watches, so it is exactly where watching a single
        // page while the owner checks twelve went unnoticed.
        urls: web_focus.map(|_| scope_urls).unwrap_or_default(),
        enabled_categories: None,
        timeout_secs: None,
        axe_enabled: Some(false),
        inspect_local_databases: false,
        project_path,
        scan_request_id: None,
        retention: Some(MAX_SCAN_RETENTION),
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
    let previous_web_score = web_focus.and_then(|_| {
        db.get_scan_history_for_project(schedule.project_id, &url, 1)
            .ok()
            .and_then(|history| history.first().map(|scan| scan.overall_score))
    });
    let previous_code_summary = wants_code
        .then(|| {
            commands::scan::select_relevant_previous_code_scan_summary(
                db.get_code_scan_history(schedule.project_id, 10)
                    .unwrap_or_default(),
                Some(&url),
            )
        })
        .flatten();
    let request = build_scheduled_execution_request(
        &schedule,
        &url,
        crate::db::scan_scope_urls_for_project(db, schedule.project_id, &url),
        db.get_project_path(schedule.project_id),
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
                    previous_web_score,
                    previous_code_summary.as_ref(),
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
    previous_web_score: Option<u32>,
    previous_code_summary: Option<&crate::db::CodeScanSummary>,
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
    let mut should_notify_full = false;
    if let Some(web_result) = result.web_result.as_ref() {
        let counts = web_scan_issue_counts(web_result);
        let scan_id = web_scan_id;
        let blame_notified = scan_id
            .and_then(|id| db.get_regression_by_scan("web", id).ok().flatten())
            .is_some();
        let should_notify = should_notify_score_change(
            previous_web_score,
            web_result.overall_score,
            counts.critical,
        );
        let notify_gate = should_send_scheduler_notification(
            blame_notified,
            previous_web_score,
            web_result.overall_score,
            counts.critical,
        );
        should_notify_full |= notify_gate;

        if schedule.scan_type != ScheduledScanType::Full {
            let _ = app_handle.emit(
                "scheduled-scan-complete",
                serde_json::json!({
                    "executionId": result.execution.id,
                    "projectId": schedule.project_id,
                    "url": url,
                    "scanId": scan_id,
                    "score": web_result.overall_score,
                    "issues": counts.total,
                    "scanType": schedule.scan_type.as_str(),
                    "timestamp": web_result.timestamp,
                }),
            );
            if notify_gate {
                send_web_scan_notification(
                    app_handle,
                    url,
                    result.execution.web_focus.unwrap_or(ScanType::Health),
                    previous_web_score,
                    web_result.overall_score,
                    counts.critical,
                    counts.total,
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
                    "score": web_result.overall_score,
                    "previous_score": previous_web_score,
                    "issues_total": counts.total,
                    "critical_issues": counts.critical,
                    "scan_type": result.execution.web_focus.unwrap_or(ScanType::Health).as_str(),
                }
            }),
        )
        .await;
    }

    if let Some(code_result) = result.code_result.as_ref() {
        let previous_score = previous_code_summary.map(|entry| entry.overall_score);
        let leading_domain =
            commands::scan::top_code_scan_domain_from_summaries(&code_result.domain_summaries);
        let domain_trend_label = previous_code_summary.and_then(|previous| {
            commands::scan::describe_code_scan_domain_trend(
                &code_result.domain_summaries,
                &previous.domain_summaries,
                leading_domain.map(|(domain, _)| domain),
                previous.top_domain,
            )
        });
        let blame_notified = db
            .get_regression_by_scan("code", code_result.id)
            .ok()
            .flatten()
            .is_some();
        let should_notify = should_notify_score_change(
            previous_score,
            code_result.overall_score,
            code_result.critical_count as usize,
        );
        let notify_gate = should_send_scheduler_notification(
            blame_notified,
            previous_score,
            code_result.overall_score,
            code_result.critical_count as usize,
        );
        should_notify_full |= notify_gate;

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
                    "scan_type": "code",
                }
            }),
        )
        .await;
    }

    if schedule.scan_type == ScheduledScanType::Full
        && (result.web_result.is_some() || result.code_result.is_some())
    {
        emit_full_scan_completion(
            db,
            app_handle,
            schedule,
            url,
            result.execution.id,
            web_scan_id,
            should_notify_full,
        )
        .await;
    }
}

/// Emit one score, event, and optional notification for a full scheduled scan.
async fn emit_full_scan_completion(
    db: &Arc<Database>,
    app_handle: &AppHandle,
    schedule: &ScanSchedule,
    url: &str,
    execution_id: i64,
    web_scan_id: Option<i64>,
    should_notify: bool,
) {
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
        }),
    );

    if should_notify {
        send_full_scan_notification(app_handle, url, score, issue_count, snapshot.critical_count);
    }
}

fn send_full_scan_notification(
    app_handle: &AppHandle,
    url: &str,
    score: u32,
    issue_count: usize,
    critical: usize,
) {
    use tauri_plugin_notification::NotificationExt;

    let hostname = hostname_for_url(url);
    let body = if critical > 0 {
        format!(
            "{} scheduled full scan complete. SiteCMD Score {}/100 - {} critical issue{} among {} tracked.",
            hostname,
            score,
            critical,
            plural_suffix(critical),
            issue_count
        )
    } else {
        format!(
            "{} scheduled full scan complete. SiteCMD Score {}/100 - {} issue{} tracked.",
            hostname,
            score,
            issue_count,
            plural_suffix(issue_count)
        )
    };

    if let Err(error) = app_handle
        .notification()
        .builder()
        .title("SiteCMD - Scheduled Full Scan")
        .body(&body)
        .show()
    {
        tracing::warn!("Failed to send notification: {:?}", error);
    } else {
        tracing::info!("Notification sent: {}", body);
    }
}

fn send_code_scan_notification(
    app_handle: &AppHandle,
    url: &str,
    prev_score: Option<u32>,
    new_score: u32,
    new_critical: usize,
    issue_count: usize,
    domain_trend_label: Option<&str>,
) {
    use tauri_plugin_notification::NotificationExt;

    let hostname = hostname_for_url(url);
    let domain_trend_suffix = domain_trend_label
        .map(|label| format!(" {label}."))
        .unwrap_or_default();
    let body = match prev_score {
        Some(prev) if new_critical > 0 => format!(
            "{} scheduled Code Scan diagnostic dropped from {} to {}. {} critical code issue{} found.{}",
            hostname,
            prev,
            new_score,
            new_critical,
            plural_suffix(new_critical),
            domain_trend_suffix
        ),
        Some(prev) => format!(
            "{} scheduled Code Scan diagnostic dropped from {} to {}. {} code issue{} detected.{}",
            hostname,
            prev,
            new_score,
            issue_count,
            plural_suffix(issue_count),
            domain_trend_suffix
        ),
        None => format!(
            "{} scheduled Code Scan found {} critical code issue{} (diagnostic: {}/100).{}",
            hostname,
            new_critical,
            plural_suffix(new_critical),
            new_score,
            domain_trend_suffix
        ),
    };

    if let Err(error) = app_handle
        .notification()
        .builder()
        .title("SiteCMD - Scheduled Code Alert")
        .body(&body)
        .show()
    {
        tracing::warn!("Failed to send notification: {:?}", error);
    } else {
        tracing::info!("Notification sent: {}", body);
    }
}

fn send_web_scan_notification(
    app_handle: &AppHandle,
    url: &str,
    scan_type: ScanType,
    prev_score: Option<u32>,
    new_score: u32,
    new_critical: usize,
    issue_count: usize,
) {
    use tauri_plugin_notification::NotificationExt;

    let hostname = hostname_for_url(url);
    let scan_label = if scan_type == ScanType::Security {
        "Security Scan"
    } else {
        "Web Scan"
    };
    let body = if let Some(prev) = prev_score {
        if new_critical > 0 {
            format!(
                "{} scheduled {} diagnostic dropped from {} to {}. {} critical issue{} found.",
                hostname,
                scan_label,
                prev,
                new_score,
                new_critical,
                plural_suffix(new_critical)
            )
        } else {
            format!(
                "{} scheduled {} diagnostic dropped from {} to {}. {} actionable issue{} detected.",
                hostname,
                scan_label,
                prev,
                new_score,
                issue_count,
                plural_suffix(issue_count)
            )
        }
    } else {
        format!(
            "{} scheduled {} found {} critical issue{} (diagnostic: {}/100).",
            hostname,
            scan_label,
            new_critical,
            plural_suffix(new_critical),
            new_score
        )
    };
    let title = if scan_type == ScanType::Security {
        "SiteCMD - Scheduled Security Alert"
    } else {
        "SiteCMD - Scheduled Web Alert"
    };

    if let Err(error) = app_handle
        .notification()
        .builder()
        .title(title)
        .body(&body)
        .show()
    {
        tracing::warn!("Failed to send notification: {:?}", error);
    } else {
        tracing::info!("Notification sent: {}", body);
    }
}

#[derive(Debug, Clone, Copy)]
struct WebScanIssueCounts {
    total: usize,
    critical: usize,
    high: usize,
}

fn web_scan_issue_counts(result: &ScanResult) -> WebScanIssueCounts {
    let actionable = result
        .issues
        .iter()
        .filter(|issue| !matches!(issue.status, CheckStatus::Pass));

    actionable.fold(
        WebScanIssueCounts {
            total: 0,
            critical: 0,
            high: 0,
        },
        |mut counts, issue| {
            counts.total += 1;
            if matches!(issue.severity, Severity::Critical) {
                counts.critical += 1;
            }
            if matches!(issue.severity, Severity::High) {
                counts.high += 1;
            }
            counts
        },
    )
}

fn should_notify_score_change(
    prev_score: Option<u32>,
    new_score: u32,
    new_critical: usize,
) -> bool {
    if let Some(prev) = prev_score {
        let drop = prev as i32 - new_score as i32;
        drop >= 10 || (new_critical > 0 && drop > 0)
    } else {
        new_critical > 0
    }
}

/// Scheduler OS-notification gate: the score-change threshold, suppressed
/// when the shared persist path already sent a deploy-regression
/// notification for the same scan. One ping per event.
fn should_send_scheduler_notification(
    blame_notified: bool,
    prev_score: Option<u32>,
    new_score: u32,
    new_critical: usize,
) -> bool {
    !blame_notified && should_notify_score_change(prev_score, new_score, new_critical)
}

fn scan_completion_event_type(should_notify: bool) -> &'static str {
    if should_notify {
        "score_drop"
    } else {
        "scan_complete"
    }
}

fn hostname_for_url(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(String::from))
        .unwrap_or_else(|| url.to_string())
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
#[path = "scan_scheduler_tests.rs"]
mod tests;
