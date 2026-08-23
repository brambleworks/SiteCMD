//! Fulfils MCP agent requests with the desktop's own fix-attempt and scan
//! paths, and publishes the heartbeat the MCP server reads for liveness.

use std::sync::Arc;

use tauri::AppHandle;

use crate::commands::scan::execution::{run_scan_execution_internal, RunScanExecutionRequest};
use crate::commands::{create_fix_attempt_inner, emit_event, CreateFixAttemptArgs};
use crate::constants::{AGENT_REQUEST_EXPIRY_MS, AGENT_REQUEST_POLL_INTERVAL};
use crate::core::agent_tools::AgentTool;
use crate::core::fix_brief::BriefLocation;
use crate::core::scan_control::ScanControlState;
use crate::core::scan_execution::{ScanExecutionMode, ScanTrigger};
use crate::core::scanner::ScanType;
use crate::db::{AgentRequestRow, Database};

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub async fn run(db: Arc<Database>, app: AppHandle, scan_control: ScanControlState) {
    loop {
        tick(&db, &app, &scan_control).await;
        tokio::time::sleep(AGENT_REQUEST_POLL_INTERVAL).await;
    }
}

async fn tick(db: &Arc<Database>, app: &AppHandle, scan_control: &ScanControlState) {
    let now = now_ms();
    if let Err(error) = crate::core::desktop_heartbeat::write_heartbeat(now) {
        tracing::warn!("agent request watcher: heartbeat: {error}");
    }
    if let Err(error) = db.expire_stale_agent_requests(now - AGENT_REQUEST_EXPIRY_MS, now) {
        tracing::warn!("agent request watcher: expire: {error}");
    }
    let requests = db
        .list_agent_requests_in_status("requested")
        .unwrap_or_else(|error| {
            tracing::warn!("agent request watcher: list: {error}");
            Vec::new()
        });
    for request in requests {
        match db.claim_agent_request(request.id, now_ms()) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => {
                tracing::warn!("agent request watcher: claim {}: {error}", request.id);
                continue;
            }
        }
        match request.kind.as_str() {
            "start_fix" => settle(
                db,
                app,
                request.id,
                fulfil_start_fix(db, &request, now_ms()),
            ),
            "run_scan" => {
                // Scans take minutes; run each on its own task so the heartbeat keeps beating.
                let db = db.clone();
                let app = app.clone();
                let scan_control = scan_control.clone();
                tauri::async_runtime::spawn(async move {
                    let outcome = fulfil_run_scan(&db, &app, &scan_control, &request).await;
                    settle(&db, &app, request.id, outcome);
                });
            }
            other => settle(
                db,
                app,
                request.id,
                Err(format!("unknown agent request kind {other}")),
            ),
        }
    }
}

fn settle(db: &Database, app: &AppHandle, request_id: i64, outcome: Result<String, String>) {
    let written = match outcome {
        Ok(result_json) => db.fulfil_agent_request(request_id, &result_json, now_ms()),
        Err(detail) => db.fail_agent_request(request_id, &detail, now_ms()),
    };
    if let Err(error) = written {
        tracing::warn!("agent request watcher: settle {request_id}: {error}");
    }
    emit_event(app, "fix-attempt-updated", ());
}

fn agent_tool_from_token(token: &str) -> AgentTool {
    serde_json::from_value(serde_json::Value::String(token.to_string()))
        .unwrap_or(AgentTool::ClaudeCode)
}

/// Create the attempt exactly as the desktop button does, from the stored issue.
pub(crate) fn fulfil_start_fix(
    db: &Database,
    request: &AgentRequestRow,
    now: i64,
) -> Result<String, String> {
    let check_id = request
        .check_id
        .clone()
        .ok_or_else(|| "start_fix needs a check_id".to_string())?;
    let items = db
        .get_active_work_items(request.project_id, Some(&request.env_url))
        .map_err(|error| error.to_string())?;
    let item = items
        .iter()
        .filter(|item| item.check_id == check_id)
        .min_by_key(|item| item.severity.sort_rank())
        .ok_or_else(|| format!("no open issue {check_id} for {}", request.env_url))?;
    let code_locations = item.metadata.relative_path.as_ref().map(|path| {
        vec![BriefLocation {
            label: match item.metadata.line {
                Some(line) => format!("{path}:{line}"),
                None => path.clone(),
            },
            path: path.clone(),
            line: item.metadata.line,
            reason: "Code Scan occurrence".to_string(),
        }]
    });
    let previous_failure = db
        .get_latest_fix_attempt(request.project_id, &request.env_url, &check_id)
        .map_err(|error| error.to_string())?
        .filter(|attempt| attempt.status == "verify_failed")
        .and_then(|attempt| attempt.failure_detail);
    let args = CreateFixAttemptArgs {
        project_id: request.project_id,
        env_url: Some(request.env_url.clone()),
        check_id,
        agent_tool: agent_tool_from_token(&request.agent_tool),
        title: item.title.clone(),
        severity: item.severity,
        description: item.description.clone(),
        why_it_matters: item.why_it_matters.clone(),
        evidence: None,
        manual_fix: item.manual_fix.clone(),
        url: request.env_url.clone(),
        detected_stack: None,
        code_locations,
        previous_failure,
    };
    let dto = create_fix_attempt_inner(db, args, now)?;
    Ok(serde_json::json!({ "attempt_id": dto.id, "status": dto.status }).to_string())
}

pub(crate) fn scan_plan_for_scope(
    scope: &str,
) -> Result<(ScanExecutionMode, Option<ScanType>), String> {
    match scope {
        "web" => Ok((ScanExecutionMode::Web, Some(ScanType::Health))),
        "code" => Ok((ScanExecutionMode::Code, None)),
        "full" => Ok((ScanExecutionMode::Full, Some(ScanType::Health))),
        other => Err(format!(
            "unknown scan scope {other}; use web, code, or full"
        )),
    }
}

async fn fulfil_run_scan(
    db: &Arc<Database>,
    app: &AppHandle,
    scan_control: &ScanControlState,
    request: &AgentRequestRow,
) -> Result<String, String> {
    let (requested_mode, web_focus) =
        scan_plan_for_scope(request.scope.as_deref().unwrap_or("web"))?;
    let environment_id = db
        .get_projects()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|project| project.id == request.project_id)
        .and_then(|project| {
            project.environments.into_iter().find(|environment| {
                crate::db::normalize_env_url(Some(&environment.url)) == request.env_url
            })
        })
        .map(|environment| environment.id);
    let execution_request = RunScanExecutionRequest {
        project_id: Some(request.project_id),
        environment_id,
        environment_url: Some(request.env_url.clone()),
        requested_mode,
        web_focus,
        urls: web_focus
            .map(|_| {
                crate::db::scan_scope_urls_for_project(db, request.project_id, &request.env_url)
            })
            .unwrap_or_default(),
        enabled_categories: None,
        timeout_secs: None,
        axe_enabled: Some(false),
        inspect_local_databases: false,
        project_path: db.get_project_path(request.project_id),
        scan_request_id: None,
        retention: Some(crate::db::MAX_SCAN_RETENTION),
        trigger: ScanTrigger::Manual,
        idempotency_key: format!("agent_request:{}", request.id),
    };
    let result =
        run_scan_execution_internal(app.clone(), db.clone(), scan_control, execution_request)
            .await
            .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "execution_id": result.execution.id,
        "reused": result.reused,
        "status": result.execution.status,
    })
    .to_string())
}

#[cfg(test)]
#[path = "agent_request_watcher_tests.rs"]
mod tests;
