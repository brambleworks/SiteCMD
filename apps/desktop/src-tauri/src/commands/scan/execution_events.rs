//! Emits timeline and frontend completion events after scan executions settle.

use crate::core::scan_execution::{ScanExecutionMode, ScanExecutionRecord, ScanTrigger};
use crate::db::{Database, EventSeverity, EventSource, EventType, SiteEvent};

pub(super) fn emit_scan_execution_completed<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    execution: &ScanExecutionRecord,
    code_run_id: Option<i64>,
) {
    crate::commands::emit_event(
        app,
        "scan-execution-completed",
        serde_json::json!({
            "executionId": execution.id,
            "projectId": execution.project_id,
            "requestedMode": execution.requested_mode.as_str(),
            "status": execution.status.as_str(),
            "webStatus": execution.web_status.map(|status| status.as_str()),
            "codeStatus": execution.code_status.map(|status| status.as_str()),
            "codeRunId": code_run_id,
        }),
    );
}

pub(super) fn record_scan_execution_event(db: &Database, execution: &ScanExecutionRecord) {
    let Some(project_id) = execution.project_id else {
        return;
    };
    let (score, critical_count, high_count, finding_count) =
        match db.get_scan_execution_event_stats(execution.id) {
            Ok(stats) => stats,
            Err(error) => {
                tracing::warn!(
                    execution_id = execution.id,
                    "Could not load execution event stats: {error}"
                );
                return;
            }
        };
    let affected_check_ids = db
        .get_scan_execution_affected_check_ids(execution.id)
        .map_err(|error| {
            tracing::warn!(
                execution_id = execution.id,
                "Could not load execution event check ids: {error}"
            );
        })
        .ok();
    let label = match execution.requested_mode {
        ScanExecutionMode::Full => "Full scan",
        ScanExecutionMode::Web => "Web Scan",
        ScanExecutionMode::Code => "Code Scan",
    };
    let event_type = if execution.trigger == ScanTrigger::Verification {
        EventType::Verification
    } else {
        EventType::Scan
    };
    let severity = score.map_or_else(
        || EventSeverity::from_issue_counts(critical_count as usize, high_count as usize),
        |value| EventSeverity::from_scan_score(value.round() as u32),
    );
    let score_suffix = score
        .map(|value| format!(" · SiteCMD Score {}", value.round() as i64))
        .unwrap_or_default();
    let title = format!(
        "{label}: {}{score_suffix}",
        execution.status.as_str().replace('_', " ")
    );
    if let Err(error) = db.insert_event(&SiteEvent {
        id: 0,
        project_id,
        event_type,
        severity,
        occurred_at_ms: execution.completed_at.unwrap_or(execution.started_at),
        title,
        summary: format!(
            "{finding_count} collector findings ({} critical, {} high)",
            critical_count, high_count
        ),
        detail: Some(
            serde_json::json!({
                "execution_id": execution.id,
                "requested_mode": execution.requested_mode.as_str(),
                "status": execution.status.as_str(),
                "web_status": execution.web_status.map(|status| status.as_str()),
                "code_status": execution.code_status.map(|status| status.as_str()),
                "sitecmd_score": score,
                "url": execution.environment_url,
            })
            .to_string(),
        ),
        source: EventSource::Internal,
        source_id: Some(format!("scan_execution_{}", execution.id)),
        metadata: None,
        affected_check_ids,
    }) {
        tracing::warn!(
            execution_id = execution.id,
            "Could not persist scan execution event: {error}"
        );
    }
}
