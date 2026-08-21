use super::*;
use crate::checks::{Check, CheckStatus, PageContext, Severity};
use http::header::{HeaderMap, HeaderValue};

fn ctx_with_headers(body: &str, headers: HeaderMap) -> PageContext {
    PageContext {
        evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        url: url::Url::parse("https://example.com").unwrap(),
        response_headers: headers,
        status_code: 200,
        body: body.to_string(),
        is_localhost: false,
        is_strict_localhost: false,
        http_version: Some("HTTP/2.0".to_string()),
        body_lower_cache: std::sync::OnceLock::new(),
    }
}

#[test]
fn invalid_samesite_value_warns_instead_of_passing_on_attribute_presence() {
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        HeaderValue::from_static("session=abc; Secure; HttpOnly; SameSite=Maybe"),
    );
    let results = CookieSecurityCheck.run(&ctx_with_headers("", h));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].severity, Severity::Low);
    assert!(results[0].title.contains("invalid SameSite"));
}

#[test]
fn cookie_values_are_redacted_from_every_issue_evidence_shape() {
    for header in [
        "session=top-secret-value; Path=/",
        "__Host-session=another-secret; Domain=example.com",
        "widget=third-secret; HttpOnly; SameSite=None",
    ] {
        let mut h = HeaderMap::new();
        h.insert("set-cookie", HeaderValue::from_str(header).unwrap());
        let results = CookieSecurityCheck.run(&ctx_with_headers("", h));
        let raw = results[0].raw_data.as_ref().unwrap().to_string();
        assert!(
            !raw.contains("secret"),
            "raw evidence leaked {header}: {raw}"
        );
        assert!(raw.contains("redacted"));
    }
}

#[test]
fn unreadable_cookie_header_is_not_reported_as_no_cookies() {
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        HeaderValue::from_bytes(b"session=\xff; Secure").expect("header bytes"),
    );
    let results = CookieSecurityCheck.run(&ctx_with_headers("", h));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0].title.contains("could not be inspected"));
    assert!(!results[0].description.contains("No Set-Cookie"));
}

#[test]
fn secure_cookie_observed_on_http_fails_without_recommending_secure_removal() {
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        HeaderValue::from_static("session=abc; Secure; HttpOnly; SameSite=Lax"),
    );
    let mut ctx = ctx_with_headers("", h);
    ctx.url = url::Url::parse("http://example.com/").unwrap();
    let results = CookieSecurityCheck.run(&ctx);
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert_eq!(results[0].severity, Severity::Medium);
    assert!(results[0].title.contains("HTTP origin"));
    assert!(results[0]
        .manual_fix
        .as_deref()
        .unwrap_or_default()
        .contains("do not remove Secure"));
}

#[test]
fn partitioned_cookie_without_secure_fails_the_partitioned_contract() {
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        HeaderValue::from_static("widget=abc; Partitioned; HttpOnly; SameSite=Lax"),
    );
    let results = CookieSecurityCheck.run(&ctx_with_headers("", h));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert_eq!(results[0].severity, Severity::Medium);
    assert!(results[0].title.contains("Partitioned"));
    assert!(results[0].description.contains("supporting"));
}

#[test]
fn last_samesite_attribute_controls_the_processed_policy() {
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        HeaderValue::from_static("session=abc; HttpOnly; SameSite=None; SameSite=Lax"),
    );
    let results = CookieSecurityCheck.run(&ctx_with_headers("", h));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(!results[0].title.contains("SameSite=None"));
    assert!(results[0].description.contains("Secure"));
}

#[test]
fn last_path_attribute_controls_host_prefix_validation() {
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        HeaderValue::from_static(
            "__Host-session=abc; Secure; HttpOnly; SameSite=Lax; Path=/; Path=/app",
        ),
    );
    let results = CookieSecurityCheck.run(&ctx_with_headers("", h));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert!(results[0].description.contains("Path=/ is missing"));
}

#[test]
fn immediate_removal_cookie_does_not_create_contextual_flag_noise() {
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        HeaderValue::from_static("session=; Max-Age=0; Path=/"),
    );
    let results = CookieSecurityCheck.run(&ctx_with_headers("", h));
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0].title.contains("removal response"));
    assert_eq!(
        results[0].raw_data.as_ref().unwrap()["removal_cookie"],
        true
    );
}

#[test]
fn rejected_samesite_none_removal_cookie_still_fails() {
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        HeaderValue::from_static("session=; Max-Age=0; SameSite=None; Path=/"),
    );
    let results = CookieSecurityCheck.run(&ctx_with_headers("", h));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert!(results[0].title.contains("SameSite=None"));
}

#[test]
fn invalid_cookie_name_is_redacted_and_not_used_as_an_issue_id() {
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        HeaderValue::from_static("bad name=secret-value; Secure"),
    );
    let results = CookieSecurityCheck.run(&ctx_with_headers("", h));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert_eq!(results[0].check_id, "security.cookies.malformed_header");
    assert!(!results[0]
        .raw_data
        .as_ref()
        .unwrap()
        .to_string()
        .contains("secret-value"));
}
