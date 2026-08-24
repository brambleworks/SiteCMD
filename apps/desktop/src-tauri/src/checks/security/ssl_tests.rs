//! Desktop TLS adapter tests for handshake classification and fact normalization.

use super::*;
use crate::checks::{CheckStatus, IssueConfidence};
use sitecmd_engine::checks::security::tls::{ValidationResult, TLS_CHECK_IDS};

fn ctx_for(url: &str) -> CheckContext {
    CheckContext {
        page: crate::checks::PageContext {
            evaluation_time: chrono::Utc::now(),
            url: url::Url::parse(url).expect("static test url"),
            response_headers: reqwest::header::HeaderMap::new(),
            status_code: 200,
            body: String::new(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        },
        client: crate::http_client::for_url(false).clone(),
        probe_cache: Default::default(),
    }
}

#[tokio::test]
async fn http_url_skips_tls_probe_in_favor_of_https_enforcement_finding() {
    let results = SslCheck.run(&ctx_for("http://example.com/page")).await;
    let ids: Vec<&str> = results.iter().map(|row| row.check_id.as_str()).collect();
    assert_eq!(ids, TLS_CHECK_IDS);
    assert!(results.iter().all(|row| row.status == CheckStatus::Skipped));
    assert!(results[0].description.contains("HTTPS enforcement check"));
}

#[tokio::test]
async fn an_unreachable_host_is_a_transport_skip_not_a_certificate_finding() {
    let results = SslCheck.run(&ctx_for("https://127.0.0.1:1/")).await;
    assert!(results.iter().all(|row| row.status == CheckStatus::Skipped));
    assert!(results
        .iter()
        .all(|row| row.confidence == IssueConfidence::NeedsReview));
    assert_eq!(
        results[0].raw_data.as_ref().unwrap()["reason"],
        "transport_failure"
    );
}

#[test]
fn a_platform_verifier_build_failure_reports_transport_not_a_panic() {
    // with_platform_verifier() eagerly builds the verifier and can fail
    // (e.g. no native CA certificates load on some Linux configurations);
    // this proves that failure reports a Transport outcome through the
    // build_config seam instead of unwinding, the sync counterpart to
    // ssl_probe's a_platform_verifier_build_failure_reports_unavailable_not_a_panic.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind stub listener");
    let addr = listener.local_addr().expect("stub listener address");
    let accepted = std::thread::spawn(move || {
        // The handshake fails before any bytes are exchanged; just keep the
        // accepted socket alive long enough for the connect to succeed.
        let _ = listener.accept();
    });

    let result =
        capture_tls_facts_with_config(&addr.to_string(), "example.com", chrono::Utc::now(), || {
            Err("no native CA certificates loaded".to_string())
        });

    assert!(
        matches!(
            result,
            Err(TlsUnavailable::Transport { ref detail }) if detail.contains("TLS configuration error")
        ),
        "expected a Transport outcome naming the configuration error, got {result:?}"
    );
    accepted.join().expect("stub listener thread");
}

#[test]
fn certificate_errors_classify_as_chain_rejections() {
    for message in [
        "TLS handshake failed: invalid peer certificate: Expired",
        "TLS handshake failed: invalid peer certificate: UnknownIssuer",
        "TLS handshake failed: invalid peer certificate: NotValidForName",
    ] {
        assert!(
            handshake_failure_is_certificate_rejection(message),
            "must classify as a chain rejection: {message}"
        );
    }
}

#[test]
fn network_errors_classify_as_transport() {
    for message in [
        "TLS handshake failed: connection reset by peer",
        "TLS handshake failed: Connection closed",
        "TLS handshake failed: timed out",
        "TLS handshake failed: broken pipe",
    ] {
        assert!(
            !handshake_failure_is_certificate_rejection(message),
            "must stay transport: {message}"
        );
    }
}

#[test]
fn a_rejected_chain_records_the_rejection_without_inventing_certificate_facts() {
    let observed_at = chrono::Utc::now();
    let facts = rejected_chain_facts(
        "TLS handshake failed: invalid peer certificate: Expired".into(),
        observed_at,
    );
    assert_eq!(facts.validation.authority, TrustAuthority::Webpki);
    assert_eq!(facts.validation.result, ValidationResult::Invalid);
    assert!(facts
        .validation
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("Expired")));
    // No completed handshake means no trustworthy certificate to read.
    assert!(facts.not_after.is_none());
    assert!(facts.subject_names.is_empty());
    assert!(facts.protocol.is_none());
    assert_eq!(facts.facts_observed_at, observed_at);
}

#[test]
fn rustls_protocol_names_normalize_to_the_schema_spelling() {
    assert_eq!(normalize_protocol_name("TLSv1_3"), "TLSv1.3");
    assert_eq!(normalize_protocol_name("TLSv1_2"), "TLSv1.2");
}
