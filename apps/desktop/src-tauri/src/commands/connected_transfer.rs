//! Transfer and unlink operations for site connections.
//! Exports preserve fingerprint continuity but never include the installation bearer.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::connected_service::ConnectedServiceClient;
use crate::db::Database;

use super::{run_blocking, sanitize_error};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedImportResult {
    pub site_id: String,
    pub environment_scope_key: String,
}

/// Import grants fingerprint continuity only when paired with this
/// installation's own bearer. The bearer is never part of the export file.
#[tracing::instrument(
    skip(app, db, encrypted_export, passphrase, installation_token),
    fields(project_id)
)]
pub async fn import_connected_connection(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    encrypted_export: String,
    passphrase: String,
    installation_token: String,
) -> Result<ConnectedImportResult, String> {
    let passphrase = zeroize::Zeroizing::new(passphrase);
    let installation_token =
        super::connected_setup::resolve_installation_token(&app, &installation_token)?;
    let imported =
        crate::connected_export::decrypt_site_connection(&encrypted_export, passphrase.as_str())
            .map_err(sanitize_error)?;
    // Any version the service coordinated is importable: the export carries
    // the site's CURRENT epoch, and this desktop adopts it wholesale. Zero
    // is the one value no epoch ever had.
    if imported.fingerprint_key_version < 1 {
        return Err("the connection export carries an invalid fingerprint key version".into());
    }
    let expected = crate::db::normalize_env_url(Some(&environment_scope_key));
    let imported_environment = crate::db::normalize_env_url(Some(&imported.environment_scope_key));
    if expected.is_empty() || imported_environment != expected {
        return Err("the connection export belongs to a different environment".into());
    }
    let import_client =
        ConnectedServiceClient::configured(installation_token.trim()).map_err(sanitize_error)?;
    let remote_state = import_client
        .state(&imported.site_id)
        .await
        .map_err(sanitize_error)?;
    if remote_state.phase.trim().is_empty()
        || remote_state.event_sequence < 0
        || remote_state.state_revision < 0
    {
        return Err("connected service returned an invalid site state".into());
    }

    let db_read = Arc::clone(&db);
    let env_read = environment_scope_key.clone();
    let existing = run_blocking(move || db_read.get_connected_site(project_id, &env_read))
        .await?
        .map_err(sanitize_error)?;
    if existing
        .as_ref()
        .is_some_and(|site| site.site_id != imported.site_id)
    {
        return Err("this environment is already connected to another site".into());
    }
    let prior_token =
        crate::keyring::get_connected_installation_token(&app).map_err(sanitize_error)?;
    let prior_key = match existing.as_ref() {
        Some(site) => {
            crate::keyring::get_project_fingerprint_key_bytes(&app, &db, project_id, &site.site_id)
                .map_err(sanitize_error)?
        }
        None => None,
    };
    let newly_bound = existing.is_none();
    if newly_bound {
        let db_connect = Arc::clone(&db);
        let env_connect = environment_scope_key.clone();
        let site_connect = imported.site_id.clone();
        let now_ms = chrono::Utc::now().timestamp_millis();
        run_blocking(move || {
            db_connect.connect_site(project_id, &env_connect, &site_connect, now_ms)
        })
        .await?
        .map_err(sanitize_error)?;
    }

    let restore = |error: String| -> String {
        if let Some(token) = prior_token.as_deref() {
            let _ = crate::keyring::store_connected_installation_token(&app, token);
        } else {
            let _ = crate::keyring::delete_connected_installation_token(&app);
        }
        if let Some(key) = prior_key {
            let _ = crate::keyring::store_project_fingerprint_key(
                &app,
                &db,
                project_id,
                &imported.site_id,
                key,
            );
        } else {
            let _ = crate::keyring::delete_connected_site_secrets(
                &app,
                &db,
                project_id,
                &imported.site_id,
            );
        }
        if newly_bound {
            let _ = db.disconnect_site(project_id, &environment_scope_key);
        }
        error
    };
    if let Err(error) =
        crate::keyring::store_connected_installation_token(&app, installation_token.as_str())
    {
        return Err(restore(sanitize_error(error)));
    }
    if let Err(error) = crate::keyring::store_project_fingerprint_key(
        &app,
        &db,
        project_id,
        &imported.site_id,
        imported.fingerprint_key,
    ) {
        return Err(restore(sanitize_error(error)));
    }
    // The binding adopts the epoch the key belongs to, or every code
    // fingerprint this desktop submits would claim a version it is not.
    let db_version = Arc::clone(&db);
    let env_version = environment_scope_key.clone();
    let version = i64::from(imported.fingerprint_key_version);
    if let Err(error) = run_blocking(move || {
        db_version.set_fingerprint_key_version(project_id, &env_version, version)
    })
    .await?
    .map_err(sanitize_error)
    {
        return Err(restore(error));
    }
    Ok(ConnectedImportResult {
        site_id: imported.site_id.clone(),
        environment_scope_key: imported.environment_scope_key.clone(),
    })
}

#[tracing::instrument(skip(app, db, passphrase), fields(project_id))]
pub async fn export_connected_connection(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    passphrase: String,
) -> Result<String, String> {
    let passphrase = zeroize::Zeroizing::new(passphrase);
    let db_read = Arc::clone(&db);
    let env_read = environment_scope_key.clone();
    let site = run_blocking(move || db_read.get_connected_site(project_id, &env_read))
        .await?
        .map_err(sanitize_error)?
        .ok_or_else(|| "this environment is not connected".to_string())?;
    let mut key =
        crate::keyring::get_project_fingerprint_key_bytes(&app, &db, project_id, &site.site_id)
            .map_err(sanitize_error)?
            .ok_or_else(|| "the project fingerprint key is missing".to_string())?;
    let result = crate::connected_export::encrypt_site_connection(
        &site.site_id,
        &environment_scope_key,
        // The site's current epoch, not a constant: a second desktop
        // importing this must stamp the version this key actually is.
        u16::try_from(site.fingerprint_key_version)
            .map_err(|_| "the fingerprint key version is out of range".to_string())?,
        key,
        passphrase.as_str(),
    )
    .map_err(sanitize_error);
    zeroize::Zeroize::zeroize(&mut key);
    result
}

/// Remove this desktop's local binding and fingerprint key. It deliberately
/// leaves the installation bearer alone because the bearer may authorize
/// other sites. Remote site deletion is a separate account-level operation.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn unlink_connected_site(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<(), String> {
    let db_read = Arc::clone(&db);
    let env_read = environment_scope_key.clone();
    let Some(site) = run_blocking(move || db_read.get_connected_site(project_id, &env_read))
        .await?
        .map_err(sanitize_error)?
    else {
        return Ok(());
    };
    let key =
        crate::keyring::get_project_fingerprint_key_bytes(&app, &db, project_id, &site.site_id)
            .map_err(sanitize_error)?;
    crate::keyring::delete_connected_site_secrets(&app, &db, project_id, &site.site_id)
        .map_err(sanitize_error)?;
    let db_unlink = Arc::clone(&db);
    let env_unlink = environment_scope_key;
    if let Err(error) = run_blocking(move || db_unlink.disconnect_site(project_id, &env_unlink))
        .await?
        .map_err(sanitize_error)
    {
        if let Some(key) = key {
            let _ = crate::keyring::store_project_fingerprint_key(
                &app,
                &db,
                project_id,
                &site.site_id,
                key,
            );
        }
        return Err(error);
    }
    Ok(())
}
