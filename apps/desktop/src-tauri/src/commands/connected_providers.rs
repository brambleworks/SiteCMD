//! Brokered deploy-provider connection and ownership-verification commands.
//! Provider callbacks and credentials remain service-side.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::connected_service::ConnectedServiceClient;
use crate::db::Database;

use super::connected_setup::connected_client;
use super::sanitize_error;

/// The account-level client: the stored installation credential and nothing
/// else, for the commands that name no site.
pub(super) fn installation_client(app: &AppHandle) -> Result<ConnectedServiceClient, String> {
    let token = crate::keyring::get_connected_installation_token(app)
        .map_err(sanitize_error)?
        .ok_or_else(|| "no installation token is stored for this machine".to_string())?;
    ConnectedServiceClient::configured(token.trim()).map_err(sanitize_error)
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedProviderAccount {
    pub id: String,
    pub name: Option<String>,
}

/// One provider connection as listed: status and identity, never a
/// credential.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedProviderConnection {
    pub id: String,
    pub provider: String,
    pub status: String,
    pub created_at: String,
    pub activated_at: Option<String>,
    pub external_account: Option<ConnectedProviderAccount>,
    pub granted_scopes: Option<String>,
    pub failed_reason: Option<String>,
    pub revoked_at: Option<String>,
    pub revoked_reason: Option<String>,
}

/// A started connection round: what to show, then what to open. The scopes
/// are in the answer so consent renders BEFORE the browser leaves this app.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedCreatedProviderConnection {
    pub authorize_url: String,
    pub connection: ConnectedProviderConnection,
    pub requested_scopes: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedProviderProject {
    pub external_project_id: String,
    pub name: String,
}

/// A provider-attested verification's answer, deploy trigger included: a
/// trigger that could not provision is a visible degraded state on a
/// verified site, never a failed verification.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedProviderVerification {
    pub phase: String,
    pub verified: bool,
    pub deploy_trigger_status: Option<String>,
    pub deploy_trigger_provider: Option<String>,
}

fn present_connection(
    row: crate::connected_providers::ProviderConnectionRow,
) -> ConnectedProviderConnection {
    ConnectedProviderConnection {
        activated_at: row.activated_at,
        created_at: row.created_at,
        external_account: row
            .external_account
            .map(|account| ConnectedProviderAccount {
                id: account.id,
                name: account.name,
            }),
        failed_reason: row.failed_reason,
        granted_scopes: row.granted_scopes,
        id: row.id,
        provider: row.provider,
        revoked_at: row.revoked_at,
        revoked_reason: row.revoked_reason,
        status: row.status,
    }
}

/// Start an OAuth round; the caller opens the returned authorization URL.
#[tracing::instrument(skip(app))]
pub async fn create_connected_provider_connection(
    app: AppHandle,
    provider: String,
) -> Result<ConnectedCreatedProviderConnection, String> {
    let provider = provider.trim().to_lowercase();
    if provider != "vercel" && provider != "netlify" {
        return Err("choose a supported provider: Vercel or Netlify".into());
    }
    let client = installation_client(&app)?;
    let created = client
        .create_provider_connection(&provider)
        .await
        .map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.provider_connect",
        serde_json::json!({ "connection": created.connection.id, "provider": provider }),
        "ok",
    );
    Ok(ConnectedCreatedProviderConnection {
        authorize_url: created.authorize_url,
        connection: present_connection(created.connection),
        requested_scopes: created.requested_scopes,
    })
}

/// Every provider connection the account holds, terminal states included, so
/// a failed or revoked link is a fact on screen rather than a silent gap.
#[tracing::instrument(skip(app))]
pub async fn list_connected_provider_connections(
    app: AppHandle,
) -> Result<Vec<ConnectedProviderConnection>, String> {
    let client = installation_client(&app)?;
    let rows = client
        .list_provider_connections()
        .await
        .map_err(sanitize_error)?;
    Ok(rows.into_iter().map(present_connection).collect())
}

/// The projects one active connection can see at the provider.
#[tracing::instrument(skip(app))]
pub async fn list_connected_provider_projects(
    app: AppHandle,
    connection_id: String,
) -> Result<Vec<ConnectedProviderProject>, String> {
    let client = installation_client(&app)?;
    let projects = client
        .list_provider_projects(connection_id.trim())
        .await
        .map_err(sanitize_error)?;
    Ok(projects
        .into_iter()
        .map(|project| ConnectedProviderProject {
            external_project_id: project.external_project_id,
            name: project.name,
        })
        .collect())
}

/// Revoke a connection. Deploy triggers it provisioned are retired first,
/// while the credential can still act at the provider.
#[tracing::instrument(skip(app))]
pub async fn revoke_connected_provider_connection(
    app: AppHandle,
    connection_id: String,
) -> Result<(), String> {
    let client = installation_client(&app)?;
    let revoked = client
        .revoke_provider_connection(connection_id.trim())
        .await
        .map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.provider_revoke",
        serde_json::json!({ "connection": revoked.connection_id }),
        "ok",
    );
    Ok(())
}

/// Prove this environment's site through a provider project's own domain
/// records. Verification held also means the project is bound to the site
/// and its deploy trigger provisioned through the same open credential.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn verify_connected_site_provider(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    connection_id: String,
    external_project_id: String,
) -> Result<ConnectedProviderVerification, String> {
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let verified = client
        .verify_site_provider(&site, connection_id.trim(), external_project_id.trim())
        .await
        .map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.provider_verify",
        serde_json::json!({ "connection": connection_id.trim(), "site": site }),
        "ok",
    );
    Ok(ConnectedProviderVerification {
        deploy_trigger_provider: verified
            .deploy_trigger
            .as_ref()
            .map(|trigger| trigger.provider.clone()),
        deploy_trigger_status: verified
            .deploy_trigger
            .as_ref()
            .map(|trigger| trigger.status.clone()),
        phase: verified.phase,
        verified: verified.verified,
    })
}
