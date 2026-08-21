//! Fingerprint-key rotation commands. Candidate keys stay local and complete
//! through the ordinary sync protocol.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::connected_rotation::ClaimOutcome;
use crate::db::Database;

use super::connected_setup::connected_client;
use super::sanitize_error;

/// Current fingerprint-key rotation claim state.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedKeyRotation {
    pub status: String,
    pub version: i64,
    pub expires_at: Option<String>,
    pub claimed_by: Option<String>,
}

/// Claims the next fingerprint-key epoch, then persists the candidate key and
/// binding version. This ordering keeps the current key authoritative across
/// interruptions.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn rotate_connected_fingerprint_key(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<ConnectedKeyRotation, String> {
    let (client, site) =
        connected_client(&app, &db, project_id, environment_scope_key.clone()).await?;

    let mut candidate = [0_u8; sitecmd_engine::sync::FINGERPRINT_KEY_LEN];
    getrandom::fill(&mut candidate).map_err(|error| format!("OS RNG unavailable: {error}"))?;
    let commitment =
        sitecmd_engine::sync::ProjectFingerprintKey::from_bytes(candidate).commitment();

    match client
        .claim_key_rotation(&site, &commitment)
        .await
        .map_err(sanitize_error)?
    {
        ClaimOutcome::Claimed(claim) => {
            crate::keyring::store_pending_fingerprint_key(&app, &db, project_id, &site, candidate)?;
            let db_claim = Arc::clone(&db);
            let env_claim = environment_scope_key;
            let version = claim.version;
            super::run_blocking(move || {
                db_claim.claim_pending_key_rotation(project_id, &env_claim, version)
            })
            .await?
            .map_err(sanitize_error)?;
            crate::audit_log::record(
                "connect.key_rotation_claim",
                serde_json::json!({ "site": site, "version": claim.version }),
                "ok",
            );
            Ok(ConnectedKeyRotation {
                claimed_by: None,
                expires_at: Some(claim.expires_at),
                status: "claimed".into(),
                version: claim.version,
            })
        }
        ClaimOutcome::AlreadyPending(pending) => Ok(ConnectedKeyRotation {
            claimed_by: pending.claimed_by,
            expires_at: pending.expires_at,
            status: "already_pending".into(),
            version: pending.version,
        }),
    }
}

/// Abort the service claim, then clear any local pending candidate.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn abort_connected_key_rotation(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<(), String> {
    let (client, site) =
        connected_client(&app, &db, project_id, environment_scope_key.clone()).await?;
    client
        .abort_key_rotation(&site)
        .await
        .map_err(sanitize_error)?;
    crate::keyring::delete_pending_fingerprint_key(&app, &db, project_id, &site)
        .map_err(sanitize_error)?;
    let db_clear = Arc::clone(&db);
    run_blocking_clear(db_clear, project_id, environment_scope_key).await?;
    crate::audit_log::record(
        "connect.key_rotation_abort",
        serde_json::json!({ "site": site }),
        "ok",
    );
    Ok(())
}

async fn run_blocking_clear(
    db: Arc<Database>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<(), String> {
    super::run_blocking(move || db.clear_pending_key_rotation(project_id, &environment_scope_key))
        .await?
        .map_err(sanitize_error)
}
