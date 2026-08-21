//! Site-fact observation tests.

use super::observe_site_facts;
use crate::checks::{CheckContext, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use sitecmd_engine::checks::security::tls::{TlsFacts, TlsValidation, TrustAuthority};
use sitecmd_engine::profile::FieldValue;

fn at(seconds: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_760_000_000 + seconds, 0).expect("timestamp")
}

const BODY: &str = r#"<html><head><script src="https://cdn.test/app.js"></script></head>
<body><a href="https://elsewhere.test/post">link</a></body></html>"#;

fn context(url: &str) -> CheckContext {
    let mut response_headers = reqwest::header::HeaderMap::new();
    response_headers.append(
        reqwest::header::HeaderName::from_static("x-frame-options"),
        reqwest::header::HeaderValue::from_static("DENY"),
    );
    response_headers.append(
        reqwest::header::HeaderName::from_static("set-cookie"),
        reqwest::header::HeaderValue::from_static("session=secret"),
    );
    CheckContext::new(
        PageContext {
            evaluation_time: at(0),
            url: url::Url::parse(url).expect("url"),
            response_headers,
            status_code: 200,
            body: BODY.to_string(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".into()),
            body_lower_cache: std::sync::OnceLock::new(),
        },
        crate::http_client::for_url(false).clone(),
    )
}

fn check_result(check_id: &str) -> CheckResult {
    CheckResult {
        check_id: check_id.to_string(),
        category: ScanCategory::Security,
        title: "t".into(),
        description: "d".into(),
        status: CheckStatus::Pass,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: Default::default(),
        confidence_reason: None,
        why_it_matters: None,
    }
}

fn families(observation: &sitecmd_engine::profile::Observation) -> Vec<&'static str> {
    observation
        .values
        .iter()
        .map(|value| value.field().as_str())
        .collect()
}

#[tokio::test]
async fn a_page_run_contributes_the_origins_it_loads_from() {
    let ctx = context("https://example.test/page");

    let observation = observe_site_facts(&ctx, &[], true).await;

    let origins = observation
        .values
        .iter()
        .find_map(|value| match value {
            FieldValue::ThirdPartyOrigins(set) => Some(set),
            _ => None,
        })
        .expect("origins observed");
    assert_eq!(origins.origins.values, ["https://cdn.test"]);
}

#[tokio::test]
async fn a_follow_on_page_contributes_only_what_it_actually_covered() {
    let ctx = context("https://example.test/second");

    let observation = observe_site_facts(&ctx, &[], false).await;

    assert_eq!(families(&observation), ["third_party_origins"]);
}

#[tokio::test]
async fn an_entry_page_run_contributes_the_response_header_profile() {
    let ctx = context("https://example.test/");

    let observation = observe_site_facts(&ctx, &[], true).await;

    let headers = observation
        .values
        .iter()
        .find_map(|value| match value {
            FieldValue::SecurityHeaders(profile) => Some(profile),
            _ => None,
        })
        .expect("headers observed");
    assert!(headers.headers.contains_key("x-frame-options"));
    assert!(
        !headers.headers.contains_key("set-cookie"),
        "an unlisted header cannot ride a baseline"
    );
}

#[tokio::test]
async fn the_certificate_rides_only_when_the_probe_captured_it() {
    let ctx = context("https://example.test/");
    assert!(!families(&observe_site_facts(&ctx, &[], true).await).contains(&"certificate"));

    ctx.record_tls_facts(&TlsFacts {
        not_before: None,
        not_after: None,
        issuer: Some("Example CA".into()),
        subject_names: vec!["example.test".into()],
        protocol: None,
        validation: TlsValidation::valid(TrustAuthority::Webpki),
        facts_observed_at: at(0),
    });

    assert!(families(&observe_site_facts(&ctx, &[], true).await).contains(&"certificate"));
}

#[tokio::test]
async fn dns_posture_is_absent_when_the_scan_never_asked_a_dns_question() {
    let ctx = context("https://example.test/");

    let observation = observe_site_facts(&ctx, &[check_result("security.ssl.expiry")], true).await;

    assert!(
        !families(&observation).contains(&"dns_posture"),
        "a baseline must not widen a scan's egress"
    );
}
