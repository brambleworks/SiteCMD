//! Commands for the canonical route scope shared by manual and scheduled scans.
//!
//! Scope semantics and bounds live in `sitecmd_engine::scope`.

use crate::connected_service::{ConnectedServiceClient, ConnectedSiteState};
use crate::db::Database;
use sitecmd_engine::scope::{build_scope, engine_check_families};
use std::collections::BTreeSet;
use std::sync::Arc;
use tauri::{AppHandle, State};
use ts_rs::TS;

use super::{run_blocking, sanitize_error};

#[derive(Debug, serde::Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanScopeWriteResult {
    pub revision: i64,
    pub routes: Vec<String>,
}

#[derive(Debug, serde::Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedScopeSyncResult {
    pub connected: bool,
    pub synced: bool,
    pub remote_scope_revision: Option<i64>,
}

fn scope_matches(state: &ConnectedSiteState, routes: &[String], check_families: &[String]) -> bool {
    let same_members = |left: &[String], right: &[String]| {
        left.iter().collect::<BTreeSet<_>>() == right.iter().collect::<BTreeSet<_>>()
    };
    state.scope.as_ref().is_some_and(|scope| {
        same_members(&scope.routes, routes) && same_members(&scope.check_families, check_families)
    })
}

/// Publish committed local scope through the external-connector broker.
pub async fn sync_connected_scan_scope(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    site_id: i64,
) -> Result<ConnectedScopeSyncResult, String> {
    sync_connected_scan_scope_for_site(&app, &db, site_id).await
}

/// Deliver scope for immediate, explicit, and durable connected-service retries.
pub(crate) async fn sync_connected_scan_scope_for_site<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Arc<Database>,
    site_id: i64,
) -> Result<ConnectedScopeSyncResult, String> {
    let db_read = Arc::clone(db);
    let (target, routes) = run_blocking(move || {
        Ok::<_, crate::db::DbError>((
            db_read.connected_scan_scope_target(site_id)?,
            db_read.get_scan_scope_routes(site_id)?,
        ))
    })
    .await?
    .map_err(sanitize_error)?;
    let Some(target) = target else {
        return Ok(ConnectedScopeSyncResult {
            connected: false,
            remote_scope_revision: None,
            synced: false,
        });
    };
    if target.local_scope_revision <= target.synced_scope_revision {
        return Ok(ConnectedScopeSyncResult {
            connected: true,
            remote_scope_revision: None,
            synced: true,
        });
    }
    let token = crate::keyring::get_connected_installation_token(app)
        .map_err(sanitize_error)?
        .ok_or_else(|| "the connected installation token is missing".to_string())?;
    let client = ConnectedServiceClient::configured(token.trim()).map_err(sanitize_error)?;
    let check_families = engine_check_families();
    let state = client
        .state(&target.remote_site_id)
        .await
        .map_err(sanitize_error)?;
    if scope_matches(&state, &routes, &check_families) {
        let remote_scope_revision = state.scope.map(|scope| scope.scope_revision);
        acknowledge_scope_delivery(db, &target).await?;
        return Ok(ConnectedScopeSyncResult {
            connected: true,
            remote_scope_revision,
            synced: true,
        });
    }
    let based_on = state
        .scope
        .as_ref()
        .map(|scope| scope.scope_revision)
        .unwrap_or(0);
    match client
        .put_scope(&target.remote_site_id, based_on, &routes, &check_families)
        .await
    {
        Ok(receipt) => {
            acknowledge_scope_delivery(db, &target).await?;
            Ok(ConnectedScopeSyncResult {
                connected: true,
                remote_scope_revision: Some(receipt.scope_revision),
                synced: true,
            })
        }
        Err(error) if error.is_stale_revision() => {
            let latest = client
                .state(&target.remote_site_id)
                .await
                .map_err(sanitize_error)?;
            if scope_matches(&latest, &routes, &check_families) {
                let remote_scope_revision = latest.scope.map(|scope| scope.scope_revision);
                acknowledge_scope_delivery(db, &target).await?;
                return Ok(ConnectedScopeSyncResult {
                    connected: true,
                    remote_scope_revision,
                    synced: true,
                });
            }
            Err(
                "the connected scan scope changed on another installation; retry this edit"
                    .to_string(),
            )
        }
        Err(error) => Err(sanitize_error(error)),
    }
}

async fn acknowledge_scope_delivery(
    db: &Arc<Database>,
    target: &crate::db::ConnectedScanScopeTarget,
) -> Result<(), String> {
    let db_write = Arc::clone(db);
    let project_id = target.project_id;
    let environment_scope_key = target.environment_scope_key.clone();
    let remote_site_id = target.remote_site_id.clone();
    let binding_connected_at = target.binding_connected_at;
    let revision = target.local_scope_revision;
    run_blocking(move || {
        db_write.mark_connected_scan_scope_synced(
            project_id,
            &environment_scope_key,
            &remote_site_id,
            binding_connected_at,
            revision,
        )
    })
    .await?
    .map_err(sanitize_error)
}

/// Return the site's authored canonical routes; empty means entry-page only.
#[tauri::command]
#[tracing::instrument(skip(db), fields(site_id))]
pub async fn get_scan_scope(
    db: State<'_, Arc<Database>>,
    site_id: i64,
) -> Result<Vec<String>, String> {
    let db = (*db).clone();
    run_blocking(move || db.get_scan_scope_routes(site_id))
        .await?
        .map_err(sanitize_error)
}

/// The revision the stored scope is at. Advances on every write, and is the
/// basis a connected replacement is guarded by.
#[tauri::command]
#[tracing::instrument(skip(db), fields(site_id))]
pub async fn get_scan_scope_revision(
    db: State<'_, Arc<Database>>,
    site_id: i64,
) -> Result<i64, String> {
    let db = (*db).clone();
    run_blocking(move || db.get_scan_scope_revision(site_id))
        .await?
        .map_err(sanitize_error)
}

/// Replace the site's scope and return its new revision.
///
/// The engine always includes the entry route for origin-scoped checks. An
/// oversized scope is rejected rather than silently truncated.
#[tauri::command]
#[tracing::instrument(skip(db, routes), fields(site_id, route_count = routes.len()))]
pub async fn set_scan_scope(
    db: State<'_, Arc<Database>>,
    site_id: i64,
    site_url: String,
    routes: Vec<String>,
) -> Result<ScanScopeWriteResult, String> {
    let entry = url::Url::parse(&site_url)
        .map_err(|error| sanitize_error(format!("Invalid site URL: {error}")))?;
    // No plan cap: the local workbench scans what its owner asks it to. The
    // connected-scope cap applies when a scope is PUT to the service, and
    // the engine evaluates it there from the entitlement.
    let scope = build_scope(&entry, &routes, engine_check_families(), None)
        .map_err(|error| error.message())?;
    let stored: Vec<String> = scope.routes.into_iter().map(|route| route.route).collect();

    let db = (*db).clone();
    let write_routes = stored.clone();
    let revision = run_blocking(move || db.replace_scan_scope(site_id, &write_routes))
        .await?
        .map_err(sanitize_error)?;
    Ok(ScanScopeWriteResult {
        revision,
        routes: stored,
    })
}

#[cfg(test)]
#[path = "scan_scope_tests.rs"]
mod tests;
