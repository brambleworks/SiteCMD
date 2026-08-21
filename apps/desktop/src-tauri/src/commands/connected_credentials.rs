//! Brokered commands for established-site credentials and reconnects. The
//! desktop does not retain shown-once secrets.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::db::Database;

use super::connected_setup::connected_client;
use super::sanitize_error;

/// One credential as the service lists it: either kind, tombstones included,
/// never a secret.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedSiteCredential {
    pub id: String,
    pub kind: String,
    pub created_at: String,
    pub created_by: String,
    pub repository: Option<String>,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
    pub secret_fingerprint: Option<String>,
    pub secret_generation: Option<i64>,
    pub rotation_overlap_until: Option<String>,
}

/// The minted webhook secret, readable exactly once here.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedWebhookSecret {
    pub id: String,
    pub secret: String,
    pub secret_fingerprint: String,
    pub secret_generation: i64,
}

/// A rotated webhook secret and the previous generation's overlap deadline.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedRotatedWebhookSecret {
    pub id: String,
    pub secret: String,
    pub secret_fingerprint: String,
    pub secret_generation: i64,
    pub rotation_overlap_until: Option<String>,
}

/// One-time webhook secret returned without a fingerprint projection.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedRemintedSecret {
    pub id: String,
    pub secret: String,
    pub secret_generation: i64,
}

/// The result of resuming a disconnected site.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedReconnection {
    pub phase: String,
    pub webhook_secret: Option<ConnectedRemintedSecret>,
    pub deploy_trigger_status: Option<String>,
    pub deploy_trigger_provider: Option<String>,
}

/// List active and revoked credentials for a connected site.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn list_connected_site_credentials(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<Vec<ConnectedSiteCredential>, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let rows = client
        .list_site_credentials(&site)
        .await
        .map_err(sanitize_error)?;
    Ok(rows
        .into_iter()
        .map(|row| ConnectedSiteCredential {
            created_at: row.created_at,
            created_by: row.created_by,
            id: row.id,
            kind: row.kind,
            last_used_at: row.last_used_at,
            repository: row.repository,
            revoked_at: row.revoked_at,
            rotation_overlap_until: row.rotation_overlap_until,
            secret_fingerprint: row.secret_fingerprint,
            secret_generation: row.secret_generation,
        })
        .collect())
}

/// Mint the site's webhook secret for the generic deploy door, shown once.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn mint_connected_webhook_secret(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<ConnectedWebhookSecret, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let minted = client
        .mint_webhook_secret(&site)
        .await
        .map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.hook_secret_mint",
        serde_json::json!({ "site": site, "token": minted.id }),
        "ok",
    );
    Ok(ConnectedWebhookSecret {
        id: minted.id,
        secret: minted.secret,
        secret_fingerprint: minted.secret_fingerprint,
        secret_generation: minted.secret_generation,
    })
}

/// Rotate the webhook secret, receiving the one copy of the next generation.
/// The service refuses CI tokens with the remint-and-revoke path named, and
/// that refusal reaches the caller in the service's own words.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn rotate_connected_site_credential(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    token_id: String,
) -> Result<ConnectedRotatedWebhookSecret, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let rotated = client
        .rotate_site_credential(&site, token_id.trim())
        .await
        .map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.hook_secret_rotate",
        serde_json::json!({ "site": site, "token": rotated.id }),
        "ok",
    );
    Ok(ConnectedRotatedWebhookSecret {
        id: rotated.id,
        rotation_overlap_until: rotated.rotation_overlap_until,
        secret: rotated.secret,
        secret_fingerprint: rotated.secret_fingerprint,
        secret_generation: rotated.secret_generation,
    })
}

/// Revoke either credential kind by its public handle. Repeating it is
/// success on the service side, so a retried revocation converges.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn revoke_connected_site_credential(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    token_id: String,
) -> Result<(), String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    client
        .revoke_site_credential(&site, token_id.trim())
        .await
        .map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.credential_revoke",
        serde_json::json!({ "site": site, "token": token_id.trim() }),
        "ok",
    );
    Ok(())
}

/// Reconnect a retained site. CI tokens stay revoked; a webhook secret is
/// reminted and returned once.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn reconnect_connected_site(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<ConnectedReconnection, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let resumed = client.reconnect_site(&site).await.map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.reconnect",
        serde_json::json!({ "site": site, "phase": resumed.phase }),
        "ok",
    );
    Ok(ConnectedReconnection {
        deploy_trigger_provider: resumed
            .deploy_trigger
            .as_ref()
            .map(|trigger| trigger.provider.clone()),
        deploy_trigger_status: resumed
            .deploy_trigger
            .as_ref()
            .map(|trigger| trigger.status.clone()),
        phase: resumed.phase,
        webhook_secret: resumed
            .webhook_secret
            .map(|secret| ConnectedRemintedSecret {
                id: secret.id,
                secret: secret.secret,
                secret_generation: secret.secret_generation,
            }),
    })
}
