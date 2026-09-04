use std::sync::Arc;
use tauri::{AppHandle, State};

use crate::commands::{
    confirm_sensitive_action, run_blocking, sanitize_error, SensitiveActionTone,
};
use crate::db::Database;

/// Delete all scan history (scans + issues) across all sites. Keeps projects and integrations.
#[tracing::instrument(skip(app, db))]
pub async fn clear_scan_history(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
) -> Result<u64, String> {
    if let Err(e) = confirm_sensitive_action(
        app,
        "Clear scan history?",
        SensitiveActionTone::Warning,
        "This deletes all saved scans and issue history while keeping your projects and integrations.".to_string(),
        "Clear History",
    )
    .await
    {
        crate::audit_log::record("data.delete", serde_json::json!({ "scope": "all_scan_history" }), "fail");
        return Err(e.into());
    }
    let clear_result = {
        let db = (*db).clone();
        run_blocking(move || db.clear_scan_history().map_err(String::from))
            .await
            .and_then(|inner| inner)
    };
    match clear_result.map_err(sanitize_error) {
        Ok(count) => {
            crate::audit_log::record(
                "data.delete",
                serde_json::json!({ "scope": "all_scan_history", "scans_removed": count }),
                "ok",
            );
            Ok(count)
        }
        Err(e) => {
            crate::audit_log::record(
                "data.delete",
                serde_json::json!({ "scope": "all_scan_history" }),
                "fail",
            );
            Err(e)
        }
    }
}

/// Delete a single scan and its issues by scan ID.
#[tracing::instrument(skip(app, db), fields(scan_id))]
pub async fn delete_scan(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    scan_id: i64,
) -> Result<(), String> {
    let audit_detail = serde_json::json!({ "scope": "single_scan", "scan_id": scan_id });
    if let Err(e) = confirm_sensitive_action(
        app,
        "Delete this scan?",
        SensitiveActionTone::Warning,
        "This removes the selected scan and the issues created from it.".to_string(),
        "Delete Scan",
    )
    .await
    {
        crate::audit_log::record("data.delete", audit_detail, "fail");
        return Err(e.into());
    }
    let delete_result = {
        let db = (*db).clone();
        run_blocking(move || db.delete_scan(scan_id).map_err(String::from))
            .await
            .and_then(|inner| inner)
    };
    match delete_result.map_err(sanitize_error) {
        Ok(()) => {
            crate::audit_log::record("data.delete", audit_detail, "ok");
            Ok(())
        }
        Err(e) => {
            crate::audit_log::record("data.delete", audit_detail, "fail");
            Err(e)
        }
    }
}

/// Delete all scans for a specific site by site ID. Returns count of deleted scans.
#[tracing::instrument(skip(app, db), fields(site_id))]
pub async fn delete_site_scans(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    site_id: i64,
) -> Result<u64, String> {
    let audit_detail = serde_json::json!({ "scope": "site_scans", "site_id": site_id });
    if let Err(e) = confirm_sensitive_action(
        app,
        "Delete this site's scan history?",
        SensitiveActionTone::Warning,
        "This removes every saved scan and issue tied to the selected site.".to_string(),
        "Delete Scans",
    )
    .await
    {
        crate::audit_log::record("data.delete", audit_detail, "fail");
        return Err(e.into());
    }
    let delete_result = {
        let db = (*db).clone();
        run_blocking(move || db.delete_site_scans(site_id).map_err(String::from))
            .await
            .and_then(|inner| inner)
    };
    match delete_result.map_err(sanitize_error) {
        Ok(count) => {
            crate::audit_log::record(
                "data.delete",
                serde_json::json!({
                    "scope": "site_scans",
                    "site_id": site_id,
                    "scans_removed": count,
                }),
                "ok",
            );
            Ok(count)
        }
        Err(e) => {
            crate::audit_log::record("data.delete", audit_detail, "fail");
            Err(e)
        }
    }
}
