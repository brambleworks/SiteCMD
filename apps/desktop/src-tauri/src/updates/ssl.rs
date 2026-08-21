//! SSL certificate expiry adapter for update work items.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslCertInfo {
    pub host: String,
    pub days_until_expiry: i64,
    pub not_after: DateTime<Utc>,
}

/// Return leaf-certificate expiry; non-HTTPS and hostless URLs return `None`.
pub async fn check_cert_expiry(url: &str) -> Result<Option<SslCertInfo>, String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid url: {}", e))?;
    if parsed.scheme() != "https" {
        return Ok(None);
    }
    let host = match parsed.host_str() {
        Some(h) => h.to_string(),
        None => return Ok(None),
    };

    let probe = crate::ssl_probe::check_ssl(url.to_string()).await?;

    if let Some(err) = probe.error {
        return Err(err);
    }

    let days_until_expiry = probe.days_remaining.unwrap_or(0);

    let not_after = probe
        .not_after_iso
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    Ok(Some(SslCertInfo {
        host,
        days_until_expiry,
        not_after,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_none_for_http_url() {
        let result = check_cert_expiry("http://example.com").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_err_for_invalid_url() {
        let result = check_cert_expiry("not a url").await;
        assert!(result.is_err());
    }
}
