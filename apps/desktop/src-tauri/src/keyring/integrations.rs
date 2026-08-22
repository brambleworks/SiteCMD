use crate::integrations::IntegrationConfig;
use tauri::AppHandle;

use super::names::{
    key_name, legacy_key_name, legacy_token_key_name, token_key_name, KEYRING_PLACEHOLDER,
};
use super::namespace::project_secret_namespace;
use super::store::{delete_secret, durable_secret_store_enabled, get_secret, set_secret};

pub(super) fn integration_type_name(config: &IntegrationConfig) -> String {
    serde_json::to_string(&config.integration_type)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

pub(super) fn strip_tokens_from_extra(
    extra: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    match extra {
        Some(serde_json::Value::Object(mut map)) => {
            map.remove("tokens");
            if map.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(map))
            }
        }
        other => other,
    }
}

pub fn store_integration_secrets<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    config: &IntegrationConfig,
) -> Result<IntegrationConfig, String> {
    store_integration_secrets_with_durable_store(
        app,
        db,
        project_id,
        config,
        durable_secret_store_enabled(),
    )
}

pub(super) fn store_integration_secrets_with_durable_store<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    config: &IntegrationConfig,
    durable_secret_store: bool,
) -> Result<IntegrationConfig, String> {
    let integration_type = integration_type_name(config);
    let mut sanitized = config.clone();

    if durable_secret_store {
        if let Some(api_key) = config
            .api_key
            .as_deref()
            .filter(|key| !key.is_empty() && *key != KEYRING_PLACEHOLDER)
        {
            store_api_key(app, db, project_id, &integration_type, api_key)?;
            sanitized.api_key = Some(KEYRING_PLACEHOLDER.to_string());
        }

        if let Some(extra) = config.extra.as_ref() {
            if let Some(tokens) = extra.get("tokens") {
                store_tokens(app, db, project_id, &integration_type, &tokens.to_string())?;
                sanitized.extra = strip_tokens_from_extra(sanitized.extra);
            }
        }

        // Best-effort cleanup of legacy id-based entries after namespace migration.
        let _ = delete_secret(app, &legacy_key_name(project_id, &integration_type));
        let _ = delete_secret(app, &legacy_token_key_name(project_id, &integration_type));
    }

    Ok(sanitized)
}

/// A plaintext credential still in SQLite while the durable store is the
/// boundary means a migration failed. It is never used: the value is dropped
/// so the integration reports "reconnect" instead of running with a secret
/// the keychain never accepted. Returns whether anything was refused.
pub fn refuse_unmigrated_plaintext_secrets(config: &mut IntegrationConfig) -> bool {
    // Vestigial while durable_secret_store_enabled() always returns true, but
    // kept as the single switch if the durable store ever becomes conditional
    // again.
    if !durable_secret_store_enabled() {
        return false;
    }
    let mut refused = false;
    if config
        .api_key
        .as_deref()
        .is_some_and(|key| !key.is_empty() && key != KEYRING_PLACEHOLDER)
    {
        config.api_key = None;
        refused = true;
    }
    if let Some(serde_json::Value::Object(map)) = config.extra.as_mut() {
        if map.remove("tokens").is_some() {
            refused = true;
        }
    }
    if refused {
        let integration_type = integration_type_name(config);
        tracing::warn!(
            "Refusing unmigrated plaintext credential for {}; reconnect the integration",
            integration_type
        );
        crate::audit_log::record(
            "credential_refused_unmigrated",
            serde_json::json!({ "integration": integration_type }),
            "refused",
        );
    }
    refused
}

/// `refuse_unmigrated_plaintext_secrets` over a loaded config list.
pub fn without_unmigrated_plaintext_secrets(
    configs: Vec<IntegrationConfig>,
) -> Vec<IntegrationConfig> {
    configs
        .into_iter()
        .map(|mut config| {
            refuse_unmigrated_plaintext_secrets(&mut config);
            config
        })
        .collect()
}

pub fn hydrate_integration_secrets<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    config: &mut IntegrationConfig,
) {
    refuse_unmigrated_plaintext_secrets(config);
    let integration_type = integration_type_name(config);

    if config.api_key.as_deref() == Some(KEYRING_PLACEHOLDER) {
        if let Ok(Some(key)) = get_api_key(app, db, project_id, &integration_type) {
            config.api_key = Some(key);
        }
    }

    if let Ok(Some(tokens_str)) = get_tokens(app, db, project_id, &integration_type) {
        if let Ok(tokens_val) = serde_json::from_str::<serde_json::Value>(&tokens_str) {
            match config.extra.as_mut() {
                Some(serde_json::Value::Object(map)) => {
                    map.insert("tokens".to_string(), tokens_val);
                }
                _ => {
                    config.extra = Some(serde_json::json!({ "tokens": tokens_val }));
                }
            }
        }
    }
}

pub fn redact_integration_secrets(config: &mut IntegrationConfig) {
    config.api_key = None;
    config.extra = strip_tokens_from_extra(config.extra.take());
}

/// Store an API key in the OS keychain.
pub fn store_api_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    integration_type: &str,
    api_key: &str,
) -> Result<(), String> {
    let secret_namespace = project_secret_namespace(db, project_id)?;
    let user = key_name(&secret_namespace, integration_type);
    set_secret(app, &user, api_key)?;
    tracing::info!(
        "Stored API key in secure store for {}:{}",
        secret_namespace,
        integration_type
    );
    Ok(())
}

/// Retrieve an API key from the OS keychain.
pub fn get_api_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    integration_type: &str,
) -> Result<Option<String>, String> {
    let secret_namespace = project_secret_namespace(db, project_id)?;
    let user = key_name(&secret_namespace, integration_type);
    get_secret(app, &user)
}

/// Delete an API key from the OS keychain.
pub fn delete_api_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    integration_type: &str,
) -> Result<(), String> {
    if let Ok(secret_namespace) = project_secret_namespace(db, project_id) {
        let user = key_name(&secret_namespace, integration_type);
        delete_secret(app, &user)?;
    }
    let legacy_user = legacy_key_name(project_id, integration_type);
    delete_secret(app, &legacy_user)?;
    tracing::info!(
        "Deleted API key from secure store for {}:{}",
        project_id,
        integration_type
    );
    Ok(())
}

fn store_token_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    secret_namespace: &str,
    integration_type: &str,
    tokens_json: &str,
) -> Result<(), String> {
    let user = token_key_name(secret_namespace, integration_type);
    set_secret(app, &user, tokens_json)?;
    Ok(())
}

fn get_token_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    secret_namespace: &str,
    integration_type: &str,
) -> Result<Option<String>, String> {
    let user = token_key_name(secret_namespace, integration_type);
    get_secret(app, &user)
}

fn delete_token_secret<R: tauri::Runtime>(
    app: &AppHandle<R>,
    secret_namespace: &str,
    integration_type: &str,
) -> Result<(), String> {
    let user = token_key_name(secret_namespace, integration_type);
    delete_secret(app, &user)?;
    Ok(())
}

/// Store OAuth tokens (JSON string) in the OS keychain.
pub fn store_tokens<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    integration_type: &str,
    tokens_json: &str,
) -> Result<(), String> {
    let secret_namespace = project_secret_namespace(db, project_id)?;
    store_token_secret(app, &secret_namespace, integration_type, tokens_json)?;
    tracing::info!(
        "Stored OAuth tokens in secure store for {}:{}",
        secret_namespace,
        integration_type
    );
    Ok(())
}

/// Retrieve OAuth tokens (JSON string) from the OS keychain.
pub fn get_tokens<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    integration_type: &str,
) -> Result<Option<String>, String> {
    let secret_namespace = project_secret_namespace(db, project_id)?;
    get_token_secret(app, &secret_namespace, integration_type)
}

/// Delete OAuth tokens from the OS keychain.
pub fn delete_tokens<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::db::Database,
    project_id: i64,
    integration_type: &str,
) -> Result<(), String> {
    if let Ok(secret_namespace) = project_secret_namespace(db, project_id) {
        delete_token_secret(app, &secret_namespace, integration_type)?;
    }
    let legacy_user = legacy_token_key_name(project_id, integration_type);
    delete_secret(app, &legacy_user)
}
