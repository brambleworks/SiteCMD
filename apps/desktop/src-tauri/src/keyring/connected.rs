//! Connected-service secrets kept out of SQLite backups.

use sha2::{Digest, Sha256};
use tauri::AppHandle;

use super::namespace::project_secret_namespace;
use super::store::{delete_secret, get_secret_strict, set_secret};

const INSTALLATION_TOKEN_USER: &str = "app:connected_installation_token";
const FINGERPRINT_KEY_SUFFIX: &str = "fingerprint-key-v1";
/// A rotation's candidate key, held beside the current one until the
/// completing snapshot commits. One slot, because the service admits one
/// pending claim per site.
const PENDING_FINGERPRINT_KEY_SUFFIX: &str = "fingerprint-key-pending";

fn connected_secret_name(
    db: &crate::db::Database,
    project_id: i64,
    site_id: &str,
    suffix: &str,
) -> Result<String, String> {
    if site_id.trim().is_empty() {
        return Err("a connected secret needs a site id".into());
    }
    let namespace = project_secret_namespace(db, project_id)?;
    let site_digest = hex::encode(Sha256::digest(site_id.as_bytes()));
    Ok(format!("shk:{namespace}:connected:{site_digest}:{suffix}"))
}

pub fn store_connected_installation_token<R: tauri::Runtime>(
    app: &AppHandle<R>,
    token: &str,
) -> Result<(), String> {
    if token.trim().is_empty() {
        return Err("an installation token cannot be empty".into());
    }
    set_secret(app, INSTALLATION_TOKEN_USER, token)
}

/// Strict bearer read that distinguishes an inaccessible keychain entry from
/// an absent credential.
pub fn get_connected_installation_token<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<String>, String> {
    get_secret_strict(app, INSTALLATION_TOKEN_USER)
}

pub fn delete_connected_installation_token<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<(), String> {
    delete_secret(app, INSTALLATION_TOKEN_USER)
}

pub fn store_project_fingerprint_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    site_id: &str,
    bytes: [u8; sitecmd_engine::sync::FINGERPRINT_KEY_LEN],
) -> Result<sitecmd_engine::sync::ProjectFingerprintKey, String> {
    let name = connected_secret_name(db, project_id, site_id, FINGERPRINT_KEY_SUFFIX)?;
    set_secret(app, &name, &hex::encode(bytes))?;
    Ok(sitecmd_engine::sync::ProjectFingerprintKey::from_bytes(
        bytes,
    ))
}

pub(crate) fn get_project_fingerprint_key_bytes<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    site_id: &str,
) -> Result<Option<[u8; sitecmd_engine::sync::FINGERPRINT_KEY_LEN]>, String> {
    let name = connected_secret_name(db, project_id, site_id, FINGERPRINT_KEY_SUFFIX)?;
    let Some(encoded) = get_secret_strict(app, &name)? else {
        return Ok(None);
    };
    let bytes = hex::decode(encoded)
        .map_err(|error| format!("stored project fingerprint key is not valid hex: {error}"))?;
    bytes.try_into().map(Some).map_err(|bytes: Vec<u8>| {
        format!("stored project fingerprint key has {} bytes", bytes.len())
    })
}

/// Strict read for the same reason as the installation bearer. A corrupt key
/// fails loudly so it cannot mint stable-looking hashes that match nothing.
pub fn get_project_fingerprint_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    site_id: &str,
) -> Result<Option<sitecmd_engine::sync::ProjectFingerprintKey>, String> {
    Ok(
        get_project_fingerprint_key_bytes(app, db, project_id, site_id)?
            .map(sitecmd_engine::sync::ProjectFingerprintKey::from_bytes),
    )
}

/// Hold a rotation's candidate key until its completing snapshot commits.
pub fn store_pending_fingerprint_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    site_id: &str,
    bytes: [u8; sitecmd_engine::sync::FINGERPRINT_KEY_LEN],
) -> Result<(), String> {
    let name = connected_secret_name(db, project_id, site_id, PENDING_FINGERPRINT_KEY_SUFFIX)?;
    set_secret(app, &name, &hex::encode(bytes))
}

pub fn get_pending_fingerprint_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    site_id: &str,
) -> Result<Option<sitecmd_engine::sync::ProjectFingerprintKey>, String> {
    let name = connected_secret_name(db, project_id, site_id, PENDING_FINGERPRINT_KEY_SUFFIX)?;
    let Some(encoded) = get_secret_strict(app, &name)? else {
        return Ok(None);
    };
    let bytes = hex::decode(encoded)
        .map_err(|error| format!("stored pending fingerprint key is not valid hex: {error}"))?;
    let bytes: [u8; sitecmd_engine::sync::FINGERPRINT_KEY_LEN] =
        bytes.try_into().map_err(|bytes: Vec<u8>| {
            format!("stored pending fingerprint key has {} bytes", bytes.len())
        })?;
    Ok(Some(
        sitecmd_engine::sync::ProjectFingerprintKey::from_bytes(bytes),
    ))
}

pub fn delete_pending_fingerprint_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    site_id: &str,
) -> Result<(), String> {
    let name = connected_secret_name(db, project_id, site_id, PENDING_FINGERPRINT_KEY_SUFFIX)?;
    delete_secret(app, &name)
}

/// Promote a pending key before clearing its staging slot to preserve crash safety.
pub fn promote_pending_fingerprint_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    site_id: &str,
) -> Result<(), String> {
    let pending_name =
        connected_secret_name(db, project_id, site_id, PENDING_FINGERPRINT_KEY_SUFFIX)?;
    let encoded = get_secret_strict(app, &pending_name)?
        .ok_or_else(|| "no pending fingerprint key to promote".to_string())?;
    let current_name = connected_secret_name(db, project_id, site_id, FINGERPRINT_KEY_SUFFIX)?;
    set_secret(app, &current_name, &encoded)?;
    delete_secret(app, &pending_name)
}

/// Delete site-scoped secrets before disconnecting the SQLite binding.
pub fn delete_connected_site_secrets<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    site_id: &str,
) -> Result<(), String> {
    let key = connected_secret_name(db, project_id, site_id, FINGERPRINT_KEY_SUFFIX)?;
    delete_secret(app, &key)?;
    let pending = connected_secret_name(db, project_id, site_id, PENDING_FINGERPRINT_KEY_SUFFIX)?;
    delete_secret(app, &pending)
}
