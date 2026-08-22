//! LemonSqueezy license activation, validation, and deactivation client.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::config::{self, LICENSE_API_BASE};

/// Response from the LS license activate/validate endpoints.
#[derive(Debug, Deserialize)]
pub struct LicenseResponse {
    /// Whether a validate request was successful.
    pub valid: Option<bool>,
    /// Whether an activate request was successful.
    pub activated: Option<bool>,
    /// Whether a deactivate request was successful.
    pub deactivated: Option<bool>,
    /// Error message if valid is false.
    pub error: Option<String>,
    /// License key metadata.
    pub license_key: Option<LicenseKeyData>,
    /// Instance metadata (only on activate).
    pub instance: Option<InstanceData>,
    /// Additional metadata.
    pub meta: Option<MetaData>,
}

#[derive(Debug, Deserialize)]
pub struct LicenseKeyData {
    /// Current status: active, inactive, expired, disabled.
    pub status: Option<String>,
    /// Expiry date (ISO 8601), if applicable.
    pub expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InstanceData {
    /// Unique instance ID - save this for validate/deactivate.
    pub id: String,
    /// Instance name (we set this to a machine identifier).
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct MetaData {
    /// Store ID - verify this matches ours.
    pub store_id: Option<u64>,
    /// Product ID.
    pub product_id: Option<u64>,
    /// Variant ID - maps to a tier.
    pub variant_id: Option<u64>,
    /// Customer email.
    pub customer_email: Option<String>,
}

impl LicenseResponse {
    fn activation_succeeded(&self) -> bool {
        self.activated.or(self.valid).unwrap_or(false)
    }

    fn validation_succeeded(&self) -> bool {
        self.valid.unwrap_or(false)
    }

    fn deactivation_succeeded(&self) -> bool {
        self.deactivated.or(self.valid).unwrap_or(false)
    }
}

/// Structured result of a license operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseResult {
    pub valid: bool,
    pub status: String,
    pub variant_id: u64,
    pub instance_id: Option<String>,
    pub expires_at: Option<String>,
    pub error: Option<String>,
}

/// Reject license keys that belong to a different LemonSqueezy store.
#[tracing::instrument(skip(meta), fields(expected_store_id))]
pub fn verify_store_id(meta: Option<&MetaData>, expected_store_id: u64) -> Result<(), String> {
    if expected_store_id == 0 {
        return Err("LemonSqueezy store ID is not configured".to_string());
    }
    let meta = meta.ok_or("License response missing LemonSqueezy metadata")?;
    let store_id = meta
        .store_id
        .ok_or("License response missing LemonSqueezy store ID")?;
    if store_id != expected_store_id {
        return Err(format!(
            "License key belongs to store {} but expected {}",
            store_id, expected_store_id
        ));
    }
    Ok(())
}

/// Convert a parsed activation response into the shared result.
#[tracing::instrument(skip(parsed, http_status))]
pub fn build_activate_result(parsed: LicenseResponse, http_status: u16) -> LicenseResult {
    if !parsed.activation_succeeded() {
        let error_msg = parsed
            .error
            .unwrap_or_else(|| format!("Activation failed (HTTP {})", http_status));
        return LicenseResult {
            valid: false,
            status: parsed
                .license_key
                .as_ref()
                .and_then(|k| k.status.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            variant_id: 0,
            instance_id: None,
            expires_at: None,
            error: Some(error_msg),
        };
    }

    let variant_id = parsed.meta.as_ref().and_then(|m| m.variant_id).unwrap_or(0);
    let instance_id = parsed.instance.as_ref().map(|i| i.id.clone());
    let lk_status = parsed
        .license_key
        .as_ref()
        .and_then(|k| k.status.clone())
        .unwrap_or_else(|| "active".to_string());
    let expires_at = parsed
        .license_key
        .as_ref()
        .and_then(|k| k.expires_at.clone());

    LicenseResult {
        valid: true,
        status: lk_status,
        variant_id,
        instance_id,
        expires_at,
        error: None,
    }
}

/// Parse a raw LemonSqueezy activate-endpoint body. Pure; tested directly.
#[tracing::instrument(skip(body, http_status), fields(expected_store_id, body_len = body.len()))]
pub fn parse_activate_response(
    body: &str,
    expected_store_id: u64,
    http_status: u16,
) -> Result<LicenseResult, String> {
    // Classify provider trouble before parsing potentially non-contract bodies.
    if http_status == 429 || http_status >= 500 {
        return Err(format!(
            "License activation request failed: provider answered HTTP {http_status}"
        ));
    }
    let parsed: LicenseResponse = serde_json::from_str(body)
        .map_err(|e| format!("Failed to parse activation response: {e}"))?;
    if parsed.activated.is_none() && parsed.valid.is_none() {
        // Parseable JSON without a verdict field is not a provider answer.
        return Err(format!(
            "Activation response carried no provider verdict (status {http_status})"
        ));
    }
    if parsed.activation_succeeded() {
        if let Err(mismatch) = verify_store_id(parsed.meta.as_ref(), expected_store_id) {
            // Preserve the minted instance id so the caller can release it.
            let mut refused = build_activate_result(parsed, http_status);
            refused.valid = false;
            refused.error = Some(mismatch);
            return Ok(refused);
        }
    }
    Ok(build_activate_result(parsed, http_status))
}

/// Activate a machine instance whose id must be saved for later lifecycle calls.
#[tracing::instrument(skip(key), fields(instance_name = %instance_name))]
pub async fn activate(key: &str, instance_name: &str) -> Result<LicenseResult, String> {
    let url = format!("{}/activate", LICENSE_API_BASE);

    let resp = crate::http_client::client()
        .post(&url)
        .header("Accept", "application/json")
        .form(&[("license_key", key), ("instance_name", instance_name)])
        .send()
        .await
        .map_err(|e| format!("License activation request failed: {}", e))?;

    let status_code = resp.status().as_u16();
    let body = crate::http_client::read_text_limited(
        resp,
        crate::constants::LICENSE_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await
    .map_err(|e| format!("Failed to read activation response: {}", e))?;

    parse_activate_response(&body, config::store_id(), status_code)
}

/// Convert a parsed validation response into the shared result.
#[tracing::instrument(skip(parsed), fields(instance_id = %instance_id))]
pub fn build_validate_result(parsed: LicenseResponse, instance_id: &str) -> LicenseResult {
    let variant_id = parsed.meta.as_ref().and_then(|m| m.variant_id).unwrap_or(0);
    let lk_status = parsed
        .license_key
        .as_ref()
        .and_then(|k| k.status.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let expires_at = parsed
        .license_key
        .as_ref()
        .and_then(|k| k.expires_at.clone());

    LicenseResult {
        valid: parsed.validation_succeeded(),
        status: lk_status,
        variant_id,
        instance_id: Some(instance_id.to_string()),
        expires_at,
        error: parsed.error,
    }
}

/// Parse a raw LemonSqueezy validate-endpoint body. Pure; tested directly.
#[tracing::instrument(skip(body), fields(instance_id = %instance_id, expected_store_id, body_len = body.len()))]
pub fn parse_validate_response(
    body: &str,
    instance_id: &str,
    expected_store_id: u64,
) -> Result<LicenseResult, String> {
    let parsed: LicenseResponse = serde_json::from_str(body)
        .map_err(|e| format!("Failed to parse validation response: {e}"))?;
    if parsed.valid.is_none() {
        // Parseable JSON without `valid` is not a provider verdict.
        return Err("Validation response carried no provider verdict".to_string());
    }
    if parsed.validation_succeeded() {
        verify_store_id(parsed.meta.as_ref(), expected_store_id)?;
    }
    Ok(build_validate_result(parsed, instance_id))
}

/// Classify transient HTTP statuses before trusting a validation body.
pub fn classify_validate_response(
    status: u16,
    body: &str,
    instance_id: &str,
    expected_store_id: u64,
) -> Result<LicenseResult, String> {
    if status == 429 || status >= 500 {
        return Err(format!(
            "License validation answered HTTP {status}; provider trouble, not a license verdict"
        ));
    }
    parse_validate_response(body, instance_id, expected_store_id)
}

/// Validate a previously activated machine instance.
#[tracing::instrument(skip(key), fields(instance_id = %instance_id))]
pub async fn validate(key: &str, instance_id: &str) -> Result<LicenseResult, String> {
    // An empty local key is not a provider revocation verdict.
    if key.trim().is_empty() {
        return Err("License validation skipped: no license key available to send".to_string());
    }

    let url = format!("{}/validate", LICENSE_API_BASE);

    let resp = crate::http_client::client()
        .post(&url)
        .header("Accept", "application/json")
        .form(&[("license_key", key), ("instance_id", instance_id)])
        .send()
        .await
        .map_err(|e| format!("License validation request failed: {}", e))?;

    let status = resp.status().as_u16();
    let body = crate::http_client::read_text_limited(
        resp,
        crate::constants::LICENSE_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await
    .map_err(|e| format!("Failed to read validation response: {}", e))?;

    classify_validate_response(status, &body, instance_id, config::store_id())
}

/// Parse deactivation while preserving status for terminal classification.
#[tracing::instrument(skip(body), fields(status, body_len = body.len()))]
pub fn parse_deactivate_response(status: u16, body: &str) -> Result<(), String> {
    // Provider trouble must never become a conclusive license verdict.
    if status == 429 || status >= 500 {
        return Err(format!(
            "License deactivation answered status {status}; provider trouble, not a verdict"
        ));
    }
    // Only verdict-bearing responses use the matchable `HTTP` prefix.
    let parsed: LicenseResponse = serde_json::from_str(body)
        .map_err(|e| format!("Deactivation response unreadable (status {status}): {e}"))?;
    if parsed.deactivation_succeeded() {
        Ok(())
    } else if parsed.deactivated.is_none() && parsed.valid.is_none() {
        // Parseable JSON without a verdict field is not a provider answer.
        Err(format!(
            "Deactivation response carried no provider verdict (status {status})"
        ))
    } else {
        let reason = parsed
            .error
            .unwrap_or_else(|| "Deactivation failed".to_string());
        Err(format!("HTTP {status}: {reason}"))
    }
}

/// Whether an authoritative deactivation refusal can never succeed on retry.
/// Only errors prefixed by the verdict-bearing parser may classify as terminal.
pub fn deactivate_failure_is_terminal(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    if !error.starts_with("http ") {
        return false;
    }
    deactivate_failure_proves_absence_inner(&error)
        || error.contains("has expired")
        || error.contains("is expired")
        || error.contains("disabled")
}

/// Whether an authoritative failure proves the instance is absent.
pub fn deactivate_failure_proves_absence(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    if !error.starts_with("http ") {
        return false;
    }
    deactivate_failure_proves_absence_inner(&error)
}

fn deactivate_failure_proves_absence_inner(error: &str) -> bool {
    error.contains("http 404") || error.contains("not found")
}

/// Deactivate a license key instance (unlink this machine).
///
/// After deactivation, the activation slot is freed and can be used elsewhere.
#[tracing::instrument(skip(key), fields(instance_id = %instance_id))]
pub async fn deactivate(key: &str, instance_id: &str) -> Result<(), String> {
    let url = format!("{}/deactivate", LICENSE_API_BASE);

    let resp = crate::http_client::client()
        .post(&url)
        .header("Accept", "application/json")
        .form(&[("license_key", key), ("instance_id", instance_id)])
        .send()
        .await
        .map_err(|e| format!("License deactivation request failed: {}", e))?;

    let status = resp.status().as_u16();
    let body = crate::http_client::read_text_limited(
        resp,
        crate::constants::LICENSE_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await
    .map_err(|e| format!("Failed to read deactivation response: {}", e))?;

    parse_deactivate_response(status, &body)
}

/// Generate a stable machine-specific instance name without exposing host/user names.
#[tracing::instrument]
pub fn machine_instance_name() -> String {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let username = whoami::username().unwrap_or_else(|_| "user".to_string());
    machine_instance_name_from_parts(&hostname, &username)
}

fn machine_instance_name_from_parts(hostname: &str, username: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(hostname.trim().as_bytes());
    hasher.update([0]);
    hasher.update(username.trim().as_bytes());
    let digest = hasher.finalize();
    format!("sitecmd-{}", hex::encode(&digest[..8]))
}

#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;
