//! Tauri commands for page summaries and integration hints.

use tauri::State;

use crate::core::types_work_items::{IssueGroup, PageSummary};
use crate::db::Database;
use std::sync::Arc;

#[tauri::command]
#[tracing::instrument(skip(state, env_url), fields(project_id))]
pub async fn get_pages_with_issues(
    state: State<'_, Arc<Database>>,
    project_id: i64,
    env_url: String,
) -> Result<Vec<PageSummary>, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    state
        .get_pages_with_issues(project_id, &env_url, now_ms)
        .map_err(String::from)
}

#[tauri::command]
#[tracing::instrument(skip(state, env_url, page_url), fields(project_id))]
pub async fn get_issues_for_page(
    state: State<'_, Arc<Database>>,
    project_id: i64,
    env_url: String,
    page_url: String,
) -> Result<Vec<IssueGroup>, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    state
        .get_work_items_grouped_for_page(project_id, &env_url, &page_url, now_ms)
        .map_err(String::from)
}

#[tauri::command]
#[tracing::instrument(skip(state), fields(project_id, check_id = %check_id, integration_type = %integration_type))]
pub async fn dismiss_integration_hint(
    state: State<'_, Arc<Database>>,
    project_id: i64,
    check_id: String,
    integration_type: String,
) -> Result<(), String> {
    state
        .dismiss_integration_hint(project_id, &check_id, &integration_type)
        .map_err(String::from)
}

#[tracing::instrument(skip(state), fields(project_id, check_id = %check_id))]
pub async fn resolve_fix_locations_for_check(
    state: State<'_, Arc<Database>>,
    check_id: String,
    project_id: i64,
) -> Result<Vec<crate::core::types_work_items::FixLocation>, String> {
    let Some(project_path) = state.get_project_path(project_id) else {
        return Ok(Vec::new());
    };
    let bounded_path =
        crate::core::code_scan::validate_project_path(std::path::Path::new(&project_path))?;
    Ok(crate::core::correlation::resolve_fix_locations(
        &check_id,
        bounded_path.to_str(),
    ))
}
