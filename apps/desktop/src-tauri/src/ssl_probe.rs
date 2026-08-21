//! TLS certificate expiry probe.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;
use ts_rs::TS;

use crate::constants::CHECK_LINK_TIMEOUT as PROBE_TIMEOUT;

/// Serialized result returned to the frontend.
#[derive(Debug, Serialize, Clone, TS)]
#[ts(export_to = "ipc-bindings.ts")]
pub struct SslProbeResult {
    pub days_remaining: Option<i64>,
    pub auto_renew_hint: bool,
    pub not_after_iso: Option<String>,
    pub error: Option<String>,
}

impl SslProbeResult {
    fn err(msg: impl Into<String>) -> Self {
        Self {
            days_remaining: None,
            auto_renew_hint: false,
            not_after_iso: None,
            error: Some(msg.into()),
        }
    }
}

/// Build a webpki-rooted rustls config with an explicit ring provider.
///
/// Headless entry points do not install a process-default provider.
pub(crate) fn webpki_roots_client_config() -> ClientConfig {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    ClientConfig::builder_with_provider(Arc::new(
        tokio_rustls::rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("ring crypto provider supports the safe default TLS protocol versions")
    .with_root_certificates(root_store)
    .with_no_client_auth()
}

pub(crate) fn host_from_url(url: &str) -> Option<String> {
    if url.trim().is_empty() {
        return None;
    }
    if let Ok(u) = url::Url::parse(url) {
        return u.host_str().map(|s| s.to_string());
    }
    let candidate = url.trim().trim_end_matches('/');
    if candidate.contains(' ') || candidate.contains('\n') {
        return None;
    }
    if !candidate.contains('.') {
        return None;
    }
    Some(candidate.to_string())
}

pub(crate) fn days_between(now: DateTime<Utc>, not_after: DateTime<Utc>) -> Option<i64> {
    Some((not_after - now).num_days().max(0))
}

/// Probe certificate expiry, returning failures through `SslProbeResult::error`.
#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn check_ssl(url: String) -> Result<SslProbeResult, String> {
    let host = match host_from_url(&url) {
        Some(h) => h,
        None => return Ok(SslProbeResult::err("Invalid URL")),
    };

    let connector = TlsConnector::from(Arc::new(webpki_roots_client_config()));

    let server_name = match ServerName::try_from(host.clone()) {
        Ok(n) => n,
        Err(_) => return Ok(SslProbeResult::err("Invalid hostname for SNI")),
    };

    let tcp =
        match tokio::time::timeout(PROBE_TIMEOUT, TcpStream::connect((host.as_str(), 443))).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Ok(SslProbeResult::err(format!("TCP connect failed: {}", e))),
            Err(_) => return Ok(SslProbeResult::err("TCP connect timed out")),
        };

    let tls_stream =
        match tokio::time::timeout(PROBE_TIMEOUT, connector.connect(server_name, tcp)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Ok(SslProbeResult::err(format!("TLS handshake failed: {}", e))),
            Err(_) => return Ok(SslProbeResult::err("TLS handshake timed out")),
        };

    let (_io, session) = tls_stream.into_inner();
    let peer_certs = match session.peer_certificates() {
        Some(c) if !c.is_empty() => c,
        _ => return Ok(SslProbeResult::err("No peer certificate")),
    };

    let leaf = &peer_certs[0];
    let parsed = match x509_parser::parse_x509_certificate(leaf.as_ref()) {
        Ok((_, cert)) => cert,
        Err(e) => return Ok(SslProbeResult::err(format!("Parse cert failed: {}", e))),
    };
    let not_after_ts = parsed.validity().not_after.timestamp();
    let not_after = match DateTime::<Utc>::from_timestamp(not_after_ts, 0) {
        Some(t) => t,
        None => return Ok(SslProbeResult::err("Cert NotAfter out of range")),
    };
    let days = days_between(Utc::now(), not_after);
    Ok(SslProbeResult {
        days_remaining: days,
        auto_renew_hint: is_likely_auto_renew(&host, &parsed),
        not_after_iso: Some(not_after.to_rfc3339()),
        error: None,
    })
}

fn is_likely_auto_renew(host: &str, cert: &x509_parser::certificate::X509Certificate<'_>) -> bool {
    let issuer = cert.issuer().to_string().to_lowercase();
    let host_lc = host.to_lowercase();
    issuer.contains("let's encrypt")
        || issuer.contains("google trust services")
        || issuer.contains("cloudflare")
        || host_lc.ends_with(".vercel.app")
        || host_lc.ends_with(".netlify.app")
        || host_lc.ends_with(".fly.dev")
        || host_lc.ends_with(".pages.dev")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_from_url_extracts_hostname() {
        assert_eq!(
            host_from_url("https://example.com").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            host_from_url("https://example.com:443/path").as_deref(),
            Some("example.com")
        );
        assert_eq!(host_from_url("example.com").as_deref(), Some("example.com"));
        assert_eq!(host_from_url("").as_deref(), None);
        assert_eq!(host_from_url("not a url").as_deref(), None);
    }

    #[test]
    fn days_between_counts_days_remaining() {
        let now = Utc::now();
        assert_eq!(
            days_between(now, now + chrono::Duration::days(30)).unwrap(),
            30
        );
    }

    #[test]
    fn client_config_binds_provider_without_process_default() {
        // Headless entry points may not install a process-default CryptoProvider.
        let config = webpki_roots_client_config();
        assert!(
            !config.crypto_provider().cipher_suites.is_empty(),
            "ring provider must expose cipher suites",
        );
    }

    #[test]
    fn days_between_clamps_past_to_zero() {
        let now = Utc::now();
        assert_eq!(
            days_between(now, now - chrono::Duration::days(5)).unwrap(),
            0
        );
    }
}
