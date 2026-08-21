//! Connected report links and outbound alert webhooks.
//!
//! Commands use the external-connector broker; operations targeting a
//! caller-chosen URL also require native confirmation.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::connected_delivery::ReportToggles;
use crate::db::Database;

use super::connected_setup::connected_client;
use super::sanitize_error;

/// Valid report lifetime in days.
const REPORT_TTL_DAYS_RANGE: std::ops::RangeInclusive<u32> = 1..=90;

/// Created report metadata and its one-time capability link.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedReportLink {
    pub report_id: String,
    pub link: String,
    pub expires_at: String,
    pub include_routes: bool,
}

/// One report registry row: provenance and health, never the stored
/// projection and never a reusable link.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedReportRow {
    pub report_id: String,
    pub created_at: String,
    pub created_by: String,
    pub include_routes: bool,
    pub expires_at: String,
    pub revoked: bool,
    pub view_count: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedReportRevocation {
    pub report_id: String,
    pub revoked_at: String,
}

/// Outbound alert-webhook state and secret fingerprint.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedAlertWebhook {
    pub webhook_id: String,
    pub url: String,
    pub secret_fingerprint: String,
    pub secret_generation: i64,
    pub disabled: bool,
    pub disabled_reason: Option<String>,
    pub rotation_overlap_until: Option<String>,
    pub created_at: String,
}

/// Created endpoint and one-time signing secret, never stored by the desktop.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedCreatedAlertWebhook {
    pub webhook_id: String,
    pub url: String,
    pub secret: String,
    pub secret_fingerprint: String,
}

/// A rotated endpoint and the previous generation's overlap deadline.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedRotatedAlertWebhook {
    pub webhook_id: String,
    pub secret: String,
    pub secret_fingerprint: String,
    pub rotation_overlap_until: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedWebhookTest {
    pub attempt_id: String,
    pub webhook_id: String,
}

/// Cut a frozen, expiring report link from the site's connected state.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn create_connected_report(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    include_routes: bool,
    ttl_days: u32,
) -> Result<ConnectedReportLink, String> {
    if !REPORT_TTL_DAYS_RANGE.contains(&ttl_days) {
        return Err("a report link lasts between 1 and 90 days".into());
    }
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let toggles = ReportToggles {
        include_routes,
        include_trends: true,
        trend_window_days: 30,
    };
    let created = client
        .create_report(&site, toggles, ttl_days)
        .await
        .map_err(sanitize_error)?;
    Ok(ConnectedReportLink {
        expires_at: created.expires_at,
        include_routes,
        link: created.link,
        report_id: created.report_id,
    })
}

/// The report registry with provenance: every link that exists, who cut it,
/// and how often it has been opened.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn list_connected_reports(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<Vec<ConnectedReportRow>, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let rows = client.list_reports(&site).await.map_err(sanitize_error)?;
    Ok(rows
        .into_iter()
        .map(|row| ConnectedReportRow {
            created_at: row.created_at,
            created_by: row.created_by,
            expires_at: row.expires_at,
            include_routes: row.toggles.include_routes,
            report_id: row.report_id,
            revoked: row.revoked,
            view_count: row.view_count,
        })
        .collect())
}

/// Revoke a report link immediately, ahead of its expiry.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn revoke_connected_report(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    report_id: String,
) -> Result<ConnectedReportRevocation, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let revoked = client
        .revoke_report(&site, report_id.trim())
        .await
        .map_err(sanitize_error)?;
    Ok(ConnectedReportRevocation {
        report_id: revoked.report_id,
        revoked_at: revoked.revoked_at,
    })
}

/// Register an outbound alert webhook endpoint. Behind the native
/// token-issue confirmation: the URL is caller-chosen and every alert the
/// site raises will be posted to it.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn create_connected_alert_webhook(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    url: String,
) -> Result<ConnectedCreatedAlertWebhook, String> {
    let endpoint = url.trim();
    if endpoint.is_empty() {
        return Err("a webhook endpoint URL is required".into());
    }
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let created = client
        .create_alert_webhook(&site, endpoint)
        .await
        .map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.webhook_create",
        serde_json::json!({ "site": site, "webhook": created.webhook_id }),
        "ok",
    );
    Ok(ConnectedCreatedAlertWebhook {
        secret: created.secret,
        secret_fingerprint: created.secret_fingerprint,
        url: created.url,
        webhook_id: created.webhook_id,
    })
}

#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn list_connected_alert_webhooks(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<Vec<ConnectedAlertWebhook>, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let rows = client
        .list_alert_webhooks(&site)
        .await
        .map_err(sanitize_error)?;
    Ok(rows
        .into_iter()
        .map(|row| ConnectedAlertWebhook {
            created_at: row.created_at,
            disabled: row.disabled,
            disabled_reason: row.disabled_reason,
            rotation_overlap_until: row.rotation_overlap_until,
            secret_fingerprint: row.secret_fingerprint,
            secret_generation: row.secret_generation,
            url: row.url,
            webhook_id: row.webhook_id,
        })
        .collect())
}

/// Enqueue a signed test delivery to one endpoint. Behind the native
/// token-issue confirmation for the same reason the local webhook test is:
/// it ships a SiteCMD-signed payload to an external URL on demand.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn test_connected_alert_webhook(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    webhook_id: String,
) -> Result<ConnectedWebhookTest, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let receipt = client
        .test_alert_webhook(&site, webhook_id.trim())
        .await
        .map_err(sanitize_error)?;
    Ok(ConnectedWebhookTest {
        attempt_id: receipt.attempt_id,
        webhook_id: receipt.webhook_id,
    })
}

/// Rotate one endpoint's signing secret, receiving the one copy of the next
/// generation's secret and the dual-validity deadline.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn rotate_connected_alert_webhook(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    webhook_id: String,
) -> Result<ConnectedRotatedAlertWebhook, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let rotated = client
        .rotate_alert_webhook(&site, webhook_id.trim())
        .await
        .map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.webhook_rotate",
        serde_json::json!({ "site": site, "webhook": rotated.webhook_id }),
        "ok",
    );
    Ok(ConnectedRotatedAlertWebhook {
        rotation_overlap_until: rotated.rotation_overlap_until,
        secret: rotated.secret,
        secret_fingerprint: rotated.secret_fingerprint,
        webhook_id: rotated.webhook_id,
    })
}

/// Delete one endpoint immediately. In-flight deliveries find no endpoint at
/// claim time and close with nothing owed.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn delete_connected_alert_webhook(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    webhook_id: String,
) -> Result<(), String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let deleted = client
        .delete_alert_webhook(&site, webhook_id.trim())
        .await
        .map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.webhook_delete",
        serde_json::json!({ "site": site, "webhook": deleted.webhook_id }),
        "ok",
    );
    Ok(())
}
