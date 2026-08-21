use sha2::{Digest, Sha256};

/// Sentinel value stored in SQLite when the real key is in the keychain.
/// Single-sourced in `crate::constants` so the desktop and CLI builds agree.
pub use crate::constants::KEYRING_PLACEHOLDER;

/// Build a keyring key name for an integration credential.
/// Format: `shk:{secret_namespace}:{integration_type}`
pub(super) fn key_name(secret_namespace: &str, integration_type: &str) -> String {
    format!("shk:{}:{}", secret_namespace, integration_type)
}

/// Build a keyring key name for OAuth tokens (stored separately from API keys).
pub(super) fn token_key_name(secret_namespace: &str, integration_type: &str) -> String {
    format!("shk:{}:{}:tokens", secret_namespace, integration_type)
}

/// Legacy key format based on the local numeric project id.
pub(super) fn legacy_key_name(project_id: i64, integration_type: &str) -> String {
    format!("shk:{}:{}", project_id, integration_type)
}

pub(super) fn legacy_token_key_name(project_id: i64, integration_type: &str) -> String {
    format!("shk:{}:{}:tokens", project_id, integration_type)
}

/// Legacy key format for webhook signing secrets, based on local row id.
pub(super) fn legacy_webhook_key_name(secret_namespace: &str, webhook_id: i64) -> String {
    format!("shk:{}:webhook:{}", secret_namespace, webhook_id)
}

/// Stable URL-derived keyring key for webhook signing secrets.
pub(super) fn webhook_url_key_name(secret_namespace: &str, webhook_url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(webhook_url.trim().as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("shk:{}:webhook-url:{}", secret_namespace, digest)
}
