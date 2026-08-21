use std::path::{Path, PathBuf};
use tauri::AppHandle;

use super::integrations::{integration_type_name, store_integration_secrets};
use super::names::{
    key_name, legacy_key_name, legacy_token_key_name, token_key_name, KEYRING_PLACEHOLDER,
};
use super::namespace::project_secret_namespace;
use super::store::{delete_secret, get_secret, secure_store_available_for_migration, set_secret};
use super::webhooks::migrate_webhook_secrets;

const LEGACY_KEY_MIGRATION_MARKER_FILE: &str = "legacy-keyring-upgrade-v1.marker";

fn migrate_legacy_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    current_key: &str,
    legacy_key: &str,
) -> Result<bool, String> {
    if get_secret(app, current_key)?.is_some() {
        return Ok(false);
    }
    let Some(value) = get_secret(app, legacy_key)? else {
        return Ok(false);
    };
    set_secret(app, current_key, &value)?;
    delete_secret(app, legacy_key)?;
    Ok(true)
}

fn legacy_key_migration_marker_path<R: tauri::Runtime>(
    _app: &AppHandle<R>,
) -> Result<PathBuf, String> {
    let app_data_dir = crate::app_identity::default_storage_dir()
        .ok_or_else(|| "Failed to resolve app data directory".to_string())?;
    Ok(app_data_dir.join(LEGACY_KEY_MIGRATION_MARKER_FILE))
}

pub(super) fn has_legacy_key_migration_marker(path: &Path) -> bool {
    path.exists()
}

pub(super) fn write_legacy_key_migration_marker(path: &Path) -> Result<(), String> {
    crate::app_identity::write_private_file(path, b"complete")
        .map_err(|e| format!("Failed to persist keyring migration marker: {}", e))
}

pub fn mark_legacy_key_migration_complete<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<(), String> {
    let path = legacy_key_migration_marker_path(app)?;
    write_legacy_key_migration_marker(&path)
}

pub(super) struct CredentialMigrationOutcome {
    pub(super) migrated: usize,
    pub(super) had_failures: bool,
    pub(super) legacy_key_migration_attempted: bool,
}

pub(super) fn should_write_legacy_key_migration_marker(
    allow_legacy_key_migration: bool,
    had_failures: bool,
    legacy_key_migration_attempted: bool,
) -> bool {
    allow_legacy_key_migration && legacy_key_migration_attempted && !had_failures
}

pub(super) fn migrate_credentials_impl<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    allow_legacy_key_migration: bool,
) -> Result<CredentialMigrationOutcome, String> {
    if !secure_store_available_for_migration() {
        return Ok(CredentialMigrationOutcome {
            migrated: 0,
            had_failures: false,
            legacy_key_migration_attempted: false,
        });
    }

    let projects = db
        .get_projects()
        .map_err(|e| format!("Failed to get projects: {}", e))?;
    let mut migrated = 0;
    let mut had_failures = false;

    for project in &projects {
        let secret_namespace = project_secret_namespace(db, project.id)?;
        let configs = db
            .get_integrations(project.id)
            .map_err(|e| format!("Failed to get configs for project {}: {}", project.id, e))?;

        for config in &configs {
            let integration_type = integration_type_name(config);

            if allow_legacy_key_migration {
                if migrate_legacy_secret(
                    app,
                    &key_name(&secret_namespace, &integration_type),
                    &legacy_key_name(project.id, &integration_type),
                )? {
                    migrated += 1;
                }
                if migrate_legacy_secret(
                    app,
                    &token_key_name(&secret_namespace, &integration_type),
                    &legacy_token_key_name(project.id, &integration_type),
                )? {
                    migrated += 1;
                }
            }

            let has_plaintext_api_key = config
                .api_key
                .as_deref()
                .is_some_and(|key| !key.is_empty() && key != KEYRING_PLACEHOLDER);
            let has_embedded_tokens = config
                .extra
                .as_ref()
                .and_then(|extra| extra.get("tokens"))
                .is_some();

            if !has_plaintext_api_key && !has_embedded_tokens {
                continue;
            }

            match store_integration_secrets(app, db, project.id, config) {
                Ok(sanitized) => {
                    if let Err(e) = db.save_integration(project.id, &sanitized) {
                        tracing::error!(
                            "Failed to sanitize stored integration for {}:{}: {}",
                            project.id,
                            integration_type,
                            e
                        );
                        had_failures = true;
                        continue;
                    }
                    migrated += 1;
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to migrate secrets for {}:{}: {}",
                        project.id,
                        integration_type,
                        e
                    );
                    had_failures = true;
                }
            }
        }
    }

    match migrate_webhook_secrets(app, db) {
        Ok(count) => migrated += count,
        Err(e) => {
            tracing::error!("Failed to migrate webhook signing secrets: {}", e);
            had_failures = true;
        }
    }

    if migrated > 0 {
        tracing::info!(
            "Migrated {} credential(s) from SQLite or legacy keychain entries",
            migrated
        );
    }

    if had_failures {
        tracing::warn!("Credential migration finished with partial failures");
    }

    Ok(CredentialMigrationOutcome {
        migrated,
        had_failures,
        legacy_key_migration_attempted: allow_legacy_key_migration,
    })
}

/// Idempotently migrate plaintext SQLite credentials to the keychain.
pub fn migrate_credentials<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
) -> Result<usize, String> {
    let marker_path = legacy_key_migration_marker_path(app)?;
    let allow_legacy_key_migration = !has_legacy_key_migration_marker(&marker_path);
    let outcome = migrate_credentials_impl(app, db, allow_legacy_key_migration)?;
    if should_write_legacy_key_migration_marker(
        allow_legacy_key_migration,
        outcome.had_failures,
        outcome.legacy_key_migration_attempted,
    ) {
        write_legacy_key_migration_marker(&marker_path)?;
    } else if allow_legacy_key_migration && outcome.had_failures {
        tracing::warn!(
            "Leaving legacy keyring migration marker unset so startup can retry after partial failures"
        );
    }
    Ok(outcome.migrated)
}

/// Sanitize restored credentials without consulting legacy ID-based keyring keys.
pub fn migrate_restored_credentials<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
) -> Result<usize, String> {
    Ok(migrate_credentials_impl(app, db, false)?.migrated)
}
