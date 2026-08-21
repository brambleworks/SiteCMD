use std::sync::Arc;

use tauri::State;

use crate::db::Database;

use super::{run_blocking, sanitize_error};

/// Audit a linked project folder for issues unavailable to live-site scans.
#[tracing::instrument(skip(db), fields(project_id))]
pub async fn run_code_scan_audit(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    project_path: Option<String>,
    inspect_local_databases: Option<bool>,
) -> Result<crate::db::CodeScanReportPayload, String> {
    let db = (*db).clone();
    let path = {
        let db = db.clone();
        run_blocking(move || -> Result<std::path::PathBuf, String> {
            let path = crate::project_paths::resolve_registered_project_dir(
                &db,
                project_id,
                project_path.as_deref(),
            )?;
            crate::core::code_scan::validate_project_path(&path)
        })
        .await??
    };
    let report = run_blocking(move || {
        crate::core::code_scan::audit_project_with_options(
            &path,
            crate::core::code_scan::CodeScanOptions {
                inspect_local_databases: inspect_local_databases.unwrap_or(false),
            },
        )
    })
    .await?
    .map_err(sanitize_error)?;
    Ok(crate::db::CodeScanReportPayload::from(
        crate::core::code_scan::CodeScanReportView::from(&report),
    ))
}
