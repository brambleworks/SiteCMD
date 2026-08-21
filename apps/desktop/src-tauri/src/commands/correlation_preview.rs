//! Tauri IPC commands for pre-deploy risk preview and what-if analysis.

use std::sync::Arc;
use tauri::State;

use crate::core::correlation::preview::{
    preview_deploy_risk, whatif_resolve, DeployRiskPreview, WhatIfResult,
};
use crate::db::Database;

#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id, changed_files_count = changed_files.len()))]
pub async fn preview_deploy_risk_cmd(
    project_id: i64,
    changed_files: Vec<String>,
    db: State<'_, Arc<Database>>,
) -> Result<DeployRiskPreview, String> {
    let db = (*db).clone();
    crate::commands::run_blocking(move || {
        preview_deploy_risk(db.as_ref(), project_id, changed_files)
    })
    .await?
}

#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id, hypothetical_count = hypothetical_resolved.len()))]
pub async fn whatif_resolve_cmd(
    project_id: i64,
    hypothetical_resolved: Vec<String>,
    db: State<'_, Arc<Database>>,
) -> Result<WhatIfResult, String> {
    let db = (*db).clone();
    crate::commands::run_blocking(move || {
        whatif_resolve(db.as_ref(), project_id, hypothetical_resolved)
    })
    .await?
}
