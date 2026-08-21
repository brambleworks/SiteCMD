//! Signed, non-blocking scan webhook delivery.

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Computes `X-SiteCMD-Signature` as lowercase
/// `sha256={HMAC-SHA256(secret, body)}`.
pub fn compute_webhook_signature(secret: &str, body: &str) -> Result<String, String> {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| format!("HMAC init: {}", e))?;
    mac.update(body.as_bytes());
    let digest = hex::encode(mac.finalize().into_bytes());
    Ok(format!("sha256={}", digest))
}

/// Send a webhook POST to the given URL.
///
/// If `secret` is set, an `X-SiteCMD-Signature` header is included with an
/// HMAC-SHA256 hex digest of the request body.
pub async fn send_webhook(
    url: &str,
    secret: Option<&str>,
    payload: &serde_json::Value,
) -> Result<(), String> {
    crate::commands::validate_external_callback_url_async(url).await?;

    let body = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    // The ExternalCallback client rejects private targets and revalidates DNS
    // at connect time to close the rebinding window.
    let client = crate::http_client::webhook_client();

    let mut req = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("User-Agent", "SiteCMD-Webhook/1.0")
        .timeout(crate::constants::API_TIMEOUT_SHORT);

    if let Some(secret) = secret {
        let signature = compute_webhook_signature(secret, &body)?;
        req = req.header("X-SiteCMD-Signature", signature);
    }

    let resp = req.body(body).send().await.map_err(|e| {
        format!(
            "Webhook delivery failed: {}",
            redact_webhook_url_from_error(&e.to_string(), url)
        )
    })?;

    if !resp.status().is_success() {
        return Err(format!("Webhook returned HTTP {}", resp.status().as_u16()));
    }

    Ok(())
}

/// Fire webhooks for a project after a scheduled scan completes.
/// Checks which webhooks are enabled for the given event type and sends payloads.
pub async fn fire_scan_webhooks(
    app: &tauri::AppHandle,
    db: &crate::db::Database,
    project_id: i64,
    event_type: &str,
    payload: serde_json::Value,
) {
    let configs = match db.get_webhook_configs(project_id) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "Failed to load webhook configs for project {}: {}",
                project_id,
                e
            );
            return;
        }
    };

    for config in configs {
        if !config.enabled {
            continue;
        }
        // Check if this webhook is subscribed to this event type
        let events: Vec<String> = serde_json::from_str(&config.events).unwrap_or_default();
        if !events.iter().any(|e| e == event_type || e == "all") {
            continue;
        }

        let url = config.url.clone();
        let secret = match crate::keyring::get_webhook_secret(
            app,
            db,
            config.project_id,
            config.id,
            &config.url,
        ) {
            Ok(secret) => secret.or_else(|| config.secret.clone()),
            Err(e) => {
                tracing::warn!("Failed to load webhook signing secret: {}", e);
                config.secret.clone()
            }
        };
        let payload = payload.clone();

        // Fire and forget - don't block on delivery
        tokio::spawn(async move {
            let target = webhook_log_target(&url);
            match send_webhook(&url, secret.as_deref(), &payload).await {
                Ok(()) => tracing::info!("Webhook delivered to {}", target),
                Err(e) => tracing::warn!("Webhook delivery failed for {}: {}", target, e),
            }
        });
    }
}

fn webhook_url_fingerprint(url: &str) -> String {
    let digest = Sha256::digest(url.trim().as_bytes());
    hex::encode(&digest[..6])
}

fn webhook_log_target(url: &str) -> String {
    let fingerprint = webhook_url_fingerprint(url);
    match reqwest::Url::parse(url) {
        Ok(parsed) => {
            let host = parsed.host_str().unwrap_or("unknown-host");
            let port = parsed
                .port()
                .map(|value| format!(":{value}"))
                .unwrap_or_default();
            format!(
                "{}://{}{} [id={}]",
                parsed.scheme(),
                host,
                port,
                fingerprint
            )
        }
        Err(_) => format!("[invalid webhook url id={}]", fingerprint),
    }
}

fn redact_webhook_url_from_error(message: &str, url: &str) -> String {
    message.replace(url, &webhook_log_target(url))
}

#[cfg(test)]
mod tests {
    use super::{compute_webhook_signature, redact_webhook_url_from_error, webhook_log_target};

    #[test]
    fn webhook_log_target_redacts_path_query_and_secret_tokens() {
        let target = webhook_log_target("https://hooks.example.com/secret/path?token=abc123");

        assert!(target.contains("https://hooks.example.com"));
        assert!(target.contains("[id="));
        assert!(!target.contains("secret/path"));
        assert!(!target.contains("token="));
        assert!(!target.contains("abc123"));
    }

    #[test]
    fn webhook_error_redaction_replaces_full_destination_urls() {
        let url = "https://hooks.example.com/secret/path?token=abc123";
        let message = format!("request to {url} timed out");
        let redacted = redact_webhook_url_from_error(&message, url);

        assert!(!redacted.contains(url));
        assert!(!redacted.contains("abc123"));
        assert!(redacted.contains("https://hooks.example.com"));
    }

    /// OpenSSL-derived reference vector for the complete signature format.
    #[test]
    fn compute_webhook_signature_matches_reference_hmac_for_simple_body() {
        let signature = compute_webhook_signature("topsecret", "hello").unwrap();
        assert_eq!(
            signature,
            "sha256=ed76fd36523b8becda5a3b36d0e3737e8ae5111f55e26c7c3a455a3ce29636d2"
        );
    }

    #[test]
    fn compute_webhook_signature_matches_reference_hmac_for_json_body() {
        let body = r#"{"event":"scan_completed","score":71}"#;
        let signature = compute_webhook_signature("wh_test_secret_456", body).unwrap();
        assert_eq!(
            signature,
            "sha256=3837794de2038cbd0f1d897761b48df052ae8d37b1ebd11048f30c205b4936c3"
        );
    }

    #[test]
    fn compute_webhook_signature_uses_lowercase_hex_and_sha256_prefix() {
        let signature = compute_webhook_signature("any-secret", "any-body").unwrap();
        assert!(
            signature.starts_with("sha256="),
            "signature must start with sha256= prefix: got {signature}"
        );
        let hex = signature.trim_start_matches("sha256=");
        assert_eq!(hex.len(), 64, "SHA-256 hex digest must be 64 chars");
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "signature hex must be lowercase: got {signature}"
        );
    }

    #[test]
    fn compute_webhook_signature_changes_with_secret() {
        let a = compute_webhook_signature("secret-a", "same-body").unwrap();
        let b = compute_webhook_signature("secret-b", "same-body").unwrap();
        assert_ne!(a, b, "different secrets must produce different signatures");
    }

    #[test]
    fn compute_webhook_signature_changes_with_body() {
        let a = compute_webhook_signature("same-secret", "body-one").unwrap();
        let b = compute_webhook_signature("same-secret", "body-two").unwrap();
        assert_ne!(a, b, "different bodies must produce different signatures");
    }
}
