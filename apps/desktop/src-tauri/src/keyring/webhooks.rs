use tauri::AppHandle;

use super::names::{legacy_webhook_key_name, webhook_url_key_name};
use super::namespace::project_secret_namespace;
use super::store::{delete_secret, get_secret, secure_store_available_for_migration, set_secret};

/// Store a webhook signing secret in the OS keychain.
pub fn store_webhook_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    webhook_url: &str,
    secret: &str,
) -> Result<(), String> {
    let secret_namespace = project_secret_namespace(db, project_id)?;
    let user = webhook_url_key_name(&secret_namespace, webhook_url);
    set_secret(app, &user, secret)?;
    tracing::info!(
        "Stored webhook signing secret in secure store for {}",
        secret_namespace
    );
    Ok(())
}

/// Retrieve a webhook signing secret from the OS keychain.
pub fn get_webhook_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    webhook_id: i64,
    webhook_url: &str,
) -> Result<Option<String>, String> {
    let secret_namespace = project_secret_namespace(db, project_id)?;
    let user = webhook_url_key_name(&secret_namespace, webhook_url);
    if let Some(secret) = get_secret(app, &user)? {
        return Ok(Some(secret));
    }

    let legacy_user = legacy_webhook_key_name(&secret_namespace, webhook_id);
    let Some(secret) = get_secret(app, &legacy_user)? else {
        return Ok(None);
    };

    if let Err(error) = set_secret(app, &user, &secret) {
        tracing::warn!("Failed to migrate webhook signing secret key: {}", error);
    } else if let Err(error) = delete_secret(app, &legacy_user) {
        tracing::warn!("Failed to delete legacy webhook signing secret: {}", error);
    }

    Ok(Some(secret))
}

/// Delete a webhook signing secret from the OS keychain.
pub fn delete_webhook_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    webhook_id: i64,
    webhook_url: &str,
) -> Result<(), String> {
    delete_webhook_secret_for_url(app, db, project_id, webhook_url)?;
    if let Ok(secret_namespace) = project_secret_namespace(db, project_id) {
        let legacy_user = legacy_webhook_key_name(&secret_namespace, webhook_id);
        delete_secret(app, &legacy_user)?;
    }
    Ok(())
}

pub fn delete_webhook_secret_for_url<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    webhook_url: &str,
) -> Result<(), String> {
    if let Ok(secret_namespace) = project_secret_namespace(db, project_id) {
        let user = webhook_url_key_name(&secret_namespace, webhook_url);
        delete_secret(app, &user)?;
    }
    Ok(())
}

/// Migrate plaintext webhook signing secrets from SQLite to the OS keychain.
pub fn migrate_webhook_secrets<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
) -> Result<usize, String> {
    if !secure_store_available_for_migration() {
        return Ok(0);
    }

    let configs = db
        .get_all_webhook_configs()
        .map_err(|e| format!("Failed to get webhook configs: {}", e))?;
    let mut migrated = 0;

    for config in configs {
        let Some(secret) = config.secret.as_deref().filter(|secret| !secret.is_empty()) else {
            continue;
        };
        store_webhook_secret(app, db, config.project_id, &config.url, secret)?;
        db.clear_webhook_secret(config.id)?;
        migrated += 1;
    }

    if migrated > 0 {
        tracing::info!(
            "Migrated {} webhook signing secret(s) from SQLite",
            migrated
        );
    }

    Ok(migrated)
}
