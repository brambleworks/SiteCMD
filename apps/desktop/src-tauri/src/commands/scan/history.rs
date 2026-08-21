use crate::core::normalized_scan::ScanRunKind;
use crate::core::scan_execution::{ScanExecutionDetail, ScanExecutionSummary};
use crate::db::{Database, ScoreTrendPoint};
use std::sync::Arc;
use tauri::State;

use super::policy::sanitize_history_limit;
use crate::commands::{run_blocking, sanitize_error};

/// Get top-level execution history. This is authoritative for requested mode,
/// trigger, quota unit, and Full-child status while collector detail remains
/// in the transitional Web and Code stores.
#[tauri::command]
#[tracing::instrument(skip(db, environment_url), fields(project_id, limit))]
pub async fn get_scan_executions(
    db: State<'_, Arc<Database>>,
    project_id: Option<i64>,
    environment_url: Option<String>,
    run_kind: Option<ScanRunKind>,
    limit: Option<u32>,
) -> Result<Vec<ScanExecutionSummary>, String> {
    let db = (*db).clone();
    run_blocking(move || {
        let max = sanitize_history_limit(limit);
        let environment_scope_key = environment_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| crate::db::normalize_env_url(Some(value)));
        db.get_scan_execution_history(project_id, environment_scope_key, run_kind, max)
    })
    .await?
    .map_err(sanitize_error)
}

/// Get one execution with all canonical collector runs and immutable findings.
#[tauri::command]
#[tracing::instrument(skip(db), fields(execution_id))]
pub async fn get_scan_execution_detail(
    db: State<'_, Arc<Database>>,
    execution_id: Option<i64>,
    run_id: Option<i64>,
) -> Result<Option<ScanExecutionDetail>, String> {
    let db = (*db).clone();
    run_blocking(move || {
        let execution_id = match (execution_id, run_id) {
            (Some(execution_id), None) => execution_id,
            (None, Some(run_id)) => db
                .get_scan_run_execution_id(run_id)?
                .ok_or_else(|| crate::db::DbError::Other("scan run was not found".into()))?,
            _ => {
                return Err(crate::db::DbError::Other(
                    "provide exactly one of execution_id or run_id".into(),
                ));
            }
        };
        db.get_scan_execution_detail(execution_id)
    })
    .await?
    .map_err(sanitize_error)
}

/// Get score trend data points for a URL (used for sparkline charts).
#[tauri::command]
#[tracing::instrument(skip(db, url), fields(project_id, limit))]
pub async fn get_score_trend(
    db: State<'_, Arc<Database>>,
    project_id: Option<i64>,
    url: String,
    limit: Option<u32>,
) -> Result<Vec<ScoreTrendPoint>, String> {
    let db = (*db).clone();
    run_blocking(move || {
        let max = sanitize_history_limit(limit);
        match project_id {
            Some(project_id) => db.get_score_trend_for_project(project_id, &url, max),
            None => db.get_score_trend(&url, max),
        }
    })
    .await?
    .map_err(sanitize_error)
}

/// Get completed Web-finding lifecycles for one project environment.
#[tauri::command]
#[tracing::instrument(skip(db, url), fields(project_id, limit))]
pub async fn get_resolved_issues(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    url: String,
    limit: Option<u32>,
) -> Result<Vec<crate::db::resolved_issues::ResolvedIssue>, String> {
    let db = (*db).clone();
    run_blocking(move || {
        let max = sanitize_history_limit(limit);
        db.get_resolved_issues(project_id, url, max)
    })
    .await?
    .map_err(sanitize_error)
}
