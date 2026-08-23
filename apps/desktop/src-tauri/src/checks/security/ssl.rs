//! Desktop `rustls` adapter for portable TLS facts.
//!
//! Plain HTTP remains the HTTPS-enforcement check's responsibility to avoid
//! duplicate findings.

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::tls::{
    evaluate_tls, parse_leaf_certificate, tls_unavailable_results, TlsFacts, TlsUnavailable,
    TlsValidation, TrustAuthority,
};
use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;

/// Validates SSL/TLS certificate validity, expiry, and chain
pub struct SslCheck;

#[async_trait::async_trait]
impl AsyncCheck for SslCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.ssl"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        if ctx.url.scheme() != "https" {
            return tls_unavailable_results(&TlsUnavailable::NotHttps);
        }
        let Some(host) = ctx.url.host_str().map(str::to_string) else {
            return tls_unavailable_results(&TlsUnavailable::NoHost);
        };

        let port = ctx.url.port().unwrap_or(443);
        let addr = format!("{}:{}", host, port);
        let probe_host = host.clone();
        // Facts carry the injected evaluation time as their observation
        // stamp, so the expiry verdict never reads an ambient clock.
        let observed_at = ctx.evaluation_time;

        // Run the TLS handshake in a blocking task to keep the async runtime free.
        let facts =
            tokio::task::spawn_blocking(move || capture_tls_facts(&addr, &probe_host, observed_at))
                .await;

        match facts {
            Ok(Ok(facts)) => {
                // The baseline needs the certificate's identity after these
                // verdicts are graded, and this handshake is the only place
                // in the scan that holds it.
                ctx.record_tls_facts(&facts);
                evaluate_tls(&host, &facts, ctx.evaluation_time)
            }
            Ok(Err(reason)) => tls_unavailable_results(&reason),
            Err(error) => tls_unavailable_results(&TlsUnavailable::ProbeFailed {
                detail: crate::log_sanitizer::bounded_issue_evidence(&error.to_string()),
            }),
        }
    }

    fn skip_in_predeploy(&self) -> bool {
        true
    }
}

/// Distinguish certificate rejection from transport failure by the rustls
/// certificate error vocabulary.
fn handshake_failure_is_certificate_rejection(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("certificate") || lower.contains("unknownissuer")
}

/// Facts for a chain the platform's trust program rejected: the rejection
/// itself is the fact, and the certificate fields stay unavailable because
/// the handshake never produced a peer certificate we can trust to parse.
fn rejected_chain_facts(detail: String, observed_at: chrono::DateTime<chrono::Utc>) -> TlsFacts {
    TlsFacts {
        not_before: None,
        not_after: None,
        issuer: None,
        subject_names: Vec::new(),
        protocol: None,
        validation: TlsValidation::invalid(
            TrustAuthority::Webpki,
            crate::log_sanitizer::bounded_issue_evidence(&detail),
        ),
        facts_observed_at: observed_at,
    }
}

/// Handshake with the platform's certificate store and read the facts off
/// the negotiated connection.
fn capture_tls_facts(
    addr: &str,
    host: &str,
    observed_at: chrono::DateTime<chrono::Utc>,
) -> Result<TlsFacts, TlsUnavailable> {
    let transport = |detail: String| TlsUnavailable::Transport {
        detail: crate::log_sanitizer::bounded_issue_evidence(&detail),
    };

    let mut stream =
        TcpStream::connect(addr).map_err(|e| transport(format!("TCP connection failed: {}", e)))?;
    stream
        .set_read_timeout(Some(crate::constants::API_TIMEOUT_SHORT))
        .ok();
    stream
        .set_write_timeout(Some(crate::constants::API_TIMEOUT_SHORT))
        .ok();

    // Same crypto stack as the async ssl_probe, in sync mode: we only need
    // the handshake's outcome and the peer certificate, not a usable stream.
    let config = crate::ssl_probe::platform_verified_client_config();
    let server_name = tokio_rustls::rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| transport(format!("Invalid SNI hostname: {}", e)))?;
    let mut conn = tokio_rustls::rustls::ClientConnection::new(Arc::new(config), server_name)
        .map_err(|e| transport(format!("TLS connector error: {}", e)))?;

    // rustls::Stream pumps reads and writes between the state machine and the
    // socket; flushing after at least one exchange completes the handshake.
    let mut tls = tokio_rustls::rustls::Stream::new(&mut conn, &mut stream);
    if let Err(error) = tls.flush() {
        let message = format!("TLS handshake failed: {}", error);
        return if handshake_failure_is_certificate_rejection(&message) {
            Ok(rejected_chain_facts(message, observed_at))
        } else {
            Err(transport(message))
        };
    }

    let protocol = conn
        .protocol_version()
        .map(|version| format!("{:?}", version))
        .map(|version| normalize_protocol_name(&version));

    let leaf = conn
        .peer_certificates()
        .and_then(|certs| certs.first().cloned())
        .ok_or_else(|| transport("No peer certificate presented".to_string()))?;
    let parsed = parse_leaf_certificate(leaf.as_ref());

    Ok(TlsFacts {
        not_before: parsed.as_ref().and_then(|facts| facts.not_before),
        not_after: parsed.as_ref().and_then(|facts| facts.not_after),
        issuer: parsed
            .as_ref()
            .and_then(|facts| facts.issuer.clone())
            .map(|issuer| crate::log_sanitizer::bounded_issue_evidence(&issuer)),
        subject_names: parsed.map(|facts| facts.subject_names).unwrap_or_default(),
        protocol,
        // The handshake completed against the operating system's trust
        // program (rustls-platform-verifier), which is exactly what that
        // authority accepting the chain means. `TrustAuthority::Webpki` is
        // the closest existing label; see the type's doc comment.
        validation: TlsValidation::valid(TrustAuthority::Webpki),
        facts_observed_at: observed_at,
    })
}

/// rustls debug-prints its version enum as `TLSv1_3`; the schema carries the
/// conventional spelling both adapters report.
fn normalize_protocol_name(raw: &str) -> String {
    raw.replace('_', ".")
}

#[cfg(test)]
#[path = "ssl_tests.rs"]
mod tests;
