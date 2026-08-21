#![cfg(test)]

use super::*;
use crate::checks::Check;
use crate::page::PageContext;
use crate::vocab::CheckStatus;
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

fn all_security_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
            "content-security-policy",
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
            ),
        );
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    h.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=()"),
    );
    h
}

#[test]
fn test_headers_all_present_pass() {
    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers("", all_security_headers());
    let results = check.run(&ctx);
    assert_eq!(results.len(), 6);
    for r in &results {
        assert_eq!(
            r.status,
            CheckStatus::Pass,
            "expected pass for {}",
            r.check_id
        );
    }
}

#[test]
fn test_headers_none_present_fail() {
    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers("", HeaderMap::new());
    let results = check.run(&ctx);
    assert_eq!(results.len(), 6);
    // CSP and HSTS are direct policy failures; framing/MIME/referrer/feature
    // headers are contextual hardening advisories on an otherwise unknown page.
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();
    assert_eq!(csp.status, CheckStatus::Fail);
    let hsts = results
        .iter()
        .find(|r| r.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(hsts.status, CheckStatus::Fail);
    let xfo = results
        .iter()
        .find(|r| r.check_id == "security.headers.x_frame_options")
        .unwrap();
    assert_eq!(xfo.status, CheckStatus::Warn);
    let xcto = results
        .iter()
        .find(|r| r.check_id == "security.headers.x_content_type_options")
        .unwrap();
    assert_eq!(xcto.status, CheckStatus::Warn);
    let referrer = results
        .iter()
        .find(|r| r.check_id == "security.headers.referrer_policy")
        .unwrap();
    assert_eq!(referrer.status, CheckStatus::Warn);
    let perms = results
        .iter()
        .find(|r| r.check_id == "security.headers.permissions_policy")
        .unwrap();
    assert_eq!(perms.status, CheckStatus::Warn);
}

#[test]
fn test_headers_csp_frame_ancestors_covers_xfo() {
    let mut h = HeaderMap::new();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'self'; frame-ancestors 'self'"),
    );
    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers("", h);
    let results = check.run(&ctx);
    let xfo = results
        .iter()
        .find(|r| r.check_id == "security.headers.x_frame_options")
        .unwrap();
    assert_eq!(
        xfo.status,
        CheckStatus::Pass,
        "CSP frame-ancestors should satisfy clickjacking protection"
    );
}

#[test]
fn test_headers_weak_csp_fails_even_when_header_exists() {
    let mut h = HeaderMap::new();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src *; script-src 'self' 'unsafe-inline' 'unsafe-eval' https:",
        ),
    );

    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers("", h);
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();

    assert_eq!(csp.status, CheckStatus::Fail);
    assert!(
        csp.description.contains("unsafe inline")
            || csp.description.contains("unsafe")
            || csp.description.contains("broad script sources"),
        "weak CSP failure should explain the unsafe policy: {}",
        csp.description
    );
    let policy_issues = csp
        .raw_data
        .as_ref()
        .and_then(|raw| raw.get("policy_issues"))
        .and_then(|issues| issues.as_array())
        .unwrap();
    assert!(
        policy_issues.len() >= 2,
        "expected unsafe and broad source issues, got {:?}",
        policy_issues
    );
}

#[test]
fn test_headers_csp_fails_for_data_and_filesystem_but_not_blob_alone() {
    let mut h = all_security_headers();
    h.insert(
            "content-security-policy",
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self' data: filesystem:; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
            ),
        );

    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers("", h);
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();

    assert_eq!(csp.status, CheckStatus::Fail);
    assert!(
        csp.description.contains("data") && csp.description.contains("filesystem"),
        "CSP should fail with clear executable URL-source guidance: {}",
        csp.description
    );
}

#[test]
fn test_headers_csp_blob_alone_is_warning_not_blocker() {
    let mut h = all_security_headers();
    h.insert(
            "content-security-policy",
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self' blob:; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
            ),
        );

    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers("", h);
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();

    assert_ne!(
        csp.status,
        CheckStatus::Fail,
        "blob: alone in script-src must not be a blocker: {}",
        csp.description
    );
}

#[test]
fn test_headers_csp_unsafe_inline_with_nonce_is_warning_not_blocker() {
    let mut h = all_security_headers();
    h.insert(
            "content-security-policy",
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self' 'nonce-abc123' 'unsafe-inline'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
            ),
        );

    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers("", h);
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();

    assert_ne!(
            csp.status,
            CheckStatus::Fail,
            "unsafe-inline + nonce must not be a blocker (CSP3 ignores unsafe-inline when nonce present): {}",
            csp.description
        );
}

#[test]
fn test_headers_partial_csp_warns_for_missing_hardening_directives() {
    let mut h = HeaderMap::new();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'self'; script-src 'self'"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));

    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers("", h);
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();

    assert_eq!(csp.status, CheckStatus::Warn);
    assert!(
        csp.description.contains("object-src") && csp.description.contains("base-uri"),
        "partial CSP should name missing hardening directives: {}",
        csp.description
    );
}

#[test]
fn test_headers_nonce_csp_passes_without_self_script_source() {
    let mut h = all_security_headers();
    h.insert(
            "content-security-policy",
            HeaderValue::from_static(
                "default-src 'self'; script-src 'nonce-abc123'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
            ),
        );

    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers("", h);
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();

    assert_eq!(csp.status, CheckStatus::Pass);
}

#[test]
fn hsts_max_age_zero_is_treated_as_disabled() {
    let mut h = all_security_headers();
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=0; includeSubDomains"),
    );
    let check = SecurityHeadersCheck;
    let results = check.run(&ctx_with_headers("", h));
    let hsts = results
        .iter()
        .find(|r| r.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(
        hsts.status,
        CheckStatus::Fail,
        "max-age=0 must not Pass: {}",
        hsts.description
    );
    assert!(hsts.description.to_ascii_lowercase().contains("disables"));
}

#[test]
fn hsts_max_age_below_one_year_warns() {
    let mut h = all_security_headers();
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=86400; includeSubDomains"),
    );
    let check = SecurityHeadersCheck;
    let results = check.run(&ctx_with_headers("", h));
    let hsts = results
        .iter()
        .find(|r| r.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(
        hsts.status,
        CheckStatus::Warn,
        "max-age below 1 year must Warn: {}",
        hsts.description
    );
    assert!(
        hsts.description.contains("one-year"),
        "warning must identify the one-year hardening baseline: {}",
        hsts.description
    );
}

#[test]
fn hsts_max_age_one_year_or_more_passes() {
    let mut h = all_security_headers();
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    let check = SecurityHeadersCheck;
    let results = check.run(&ctx_with_headers("", h));
    let hsts = results
        .iter()
        .find(|r| r.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(hsts.status, CheckStatus::Pass);
}

#[test]
fn referrer_policy_unsafe_url_is_warning_not_pass() {
    let mut h = all_security_headers();
    h.insert("referrer-policy", HeaderValue::from_static("unsafe-url"));
    let check = SecurityHeadersCheck;
    let results = check.run(&ctx_with_headers("", h));
    let rp = results
        .iter()
        .find(|r| r.check_id == "security.headers.referrer_policy")
        .unwrap();
    assert_eq!(
        rp.status,
        CheckStatus::Warn,
        "unsafe-url must Warn, not Pass: {}",
        rp.description
    );
    assert!(rp.description.contains("path and query"));
}

#[test]
fn referrer_policy_strict_origin_when_cross_origin_passes() {
    let mut h = all_security_headers();
    h.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    let check = SecurityHeadersCheck;
    let results = check.run(&ctx_with_headers("", h));
    let rp = results
        .iter()
        .find(|r| r.check_id == "security.headers.referrer_policy")
        .unwrap();
    assert_eq!(rp.status, CheckStatus::Pass);
}

#[test]
fn referrer_policy_comma_list_honors_last_policy() {
    let mut h = all_security_headers();
    h.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin, unsafe-url"),
    );
    let check = SecurityHeadersCheck;
    let results = check.run(&ctx_with_headers("", h));
    let rp = results
        .iter()
        .find(|r| r.check_id == "security.headers.referrer_policy")
        .unwrap();
    assert_eq!(rp.status, CheckStatus::Warn);
}

#[test]
fn test_headers_localhost_preview_are_skipped() {
    let check = SecurityHeadersCheck;
    let mut ctx = ctx_with_headers("", HeaderMap::new());
    ctx.url = url::Url::parse("http://127.0.0.1:4324").unwrap();
    ctx.is_localhost = true;
    ctx.is_strict_localhost = true;

    let results = check.run(&ctx);
    assert_eq!(results.len(), 6);
    assert!(results
        .iter()
        .all(|result| result.status == CheckStatus::Skipped));
}

#[test]
fn hsts_without_includesubdomains_is_informational_warn() {
    let mut h = all_security_headers();
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000"),
    );
    let check = SecurityHeadersCheck;
    let results = check.run(&ctx_with_headers("", h));
    let hsts = results
        .iter()
        .find(|r| r.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(
        hsts.status,
        CheckStatus::Warn,
        "missing includeSubDomains must Warn: {}",
        hsts.description
    );
    assert_eq!(hsts.severity, crate::vocab::Severity::Low);
    assert!(hsts.description.contains("includeSubDomains"));
}

#[test]
fn csp_report_only_without_enforced_policy_is_called_out() {
    let mut h = all_security_headers();
    h.remove("content-security-policy");
    h.insert(
        "content-security-policy-report-only",
        HeaderValue::from_static("default-src 'self'; script-src 'self'"),
    );
    let check = SecurityHeadersCheck;
    let results = check.run(&ctx_with_headers("", h));
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();
    assert_eq!(csp.status, CheckStatus::Warn);
    assert_eq!(csp.confidence, crate::vocab::IssueConfidence::NeedsReview);
    assert!(
        csp.title.contains("Report-Only"),
        "title must name the Report-Only posture: {}",
        csp.title
    );
    assert!(csp.description.contains("does not enforce"));
    assert!(csp.description.contains("may be an intentional rollout"));
    assert!(!csp.description.contains("never finished"));
}

#[test]
fn xfo_allow_from_is_not_protection() {
    let mut h = all_security_headers();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'",
        ),
    );
    h.insert(
        "x-frame-options",
        HeaderValue::from_static("ALLOW-FROM https://partner.example"),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let xfo = results
        .iter()
        .find(|r| r.check_id == "security.headers.x_frame_options")
        .unwrap();
    assert_eq!(
        xfo.status,
        CheckStatus::Warn,
        "ALLOW-FROM must not count as protection: {}",
        xfo.description
    );
    assert!(xfo.title.contains("ignored"), "title: {}", xfo.title);
    assert!(xfo.description.contains("ALLOW-FROM"));
}

#[test]
fn xfo_garbage_value_is_not_protection() {
    let mut h = all_security_headers();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'",
        ),
    );
    h.insert("x-frame-options", HeaderValue::from_static("ALLOWALL"));
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let xfo = results
        .iter()
        .find(|r| r.check_id == "security.headers.x_frame_options")
        .unwrap();
    assert_eq!(xfo.status, CheckStatus::Warn);
}

#[test]
fn xfo_lowercase_sameorigin_passes() {
    let mut h = all_security_headers();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'",
        ),
    );
    h.insert("x-frame-options", HeaderValue::from_static("sameorigin"));
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let xfo = results
        .iter()
        .find(|r| r.check_id == "security.headers.x_frame_options")
        .unwrap();
    assert_eq!(xfo.status, CheckStatus::Pass);
}

#[test]
fn hsts_max_age_zero_with_whitespace_is_parsed() {
    let mut h = all_security_headers();
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age = 0; includeSubDomains"),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let hsts = results
        .iter()
        .find(|r| r.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(
        hsts.status,
        CheckStatus::Fail,
        "whitespace max-age = 0 must Fail: {}",
        hsts.description
    );
    assert!(hsts.description.to_ascii_lowercase().contains("disables"));
}

#[test]
fn hsts_without_max_age_fails() {
    // RFC 6797 requires max-age; browsers ignore the header without it.
    let mut h = all_security_headers();
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("includeSubDomains; preload"),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let hsts = results
        .iter()
        .find(|r| r.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(
        hsts.status,
        CheckStatus::Fail,
        "HSTS without max-age must Fail: {}",
        hsts.description
    );
    assert!(hsts.description.contains("required max-age"));
}

#[test]
fn hsts_quoted_max_age_is_parsed() {
    assert_eq!(parse_hsts_max_age("max-age=\"31536000\""), Some(31536000));
    assert_eq!(parse_hsts_max_age("max-age = 600"), Some(600));
    assert_eq!(parse_hsts_max_age("includeSubDomains"), None);
}

#[test]
fn meta_delivered_csp_is_not_reported_as_missing() {
    let html = r#"<html><head>
        <meta http-equiv="Content-Security-Policy"
              content="default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'">
    </head><body></body></html>"#;
    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers(html, HeaderMap::new());
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .expect("csp result");
    assert_eq!(csp.status, CheckStatus::Pass);
    assert!(csp.description.contains("<meta http-equiv>"));
    let raw = csp.raw_data.as_ref().expect("raw data");
    assert_eq!(raw["delivered_via"], "meta");
}

#[test]
fn commented_out_meta_csp_does_not_suppress_the_missing_finding() {
    let html = r#"<html><head>
        <!-- <meta http-equiv="Content-Security-Policy" content="default-src 'self'"> -->
    </head><body></body></html>"#;
    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers(html, HeaderMap::new());
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .expect("csp result");
    assert_eq!(csp.status, CheckStatus::Fail, "{}", csp.description);
    assert_ne!(
        csp.raw_data
            .as_ref()
            .and_then(|raw| raw.get("delivered_via"))
            .and_then(|value| value.as_str()),
        Some("meta"),
    );
}

#[test]
fn weak_meta_delivered_csp_is_still_graded_as_weak() {
    // A meta-delivered policy is evaluated like a header policy: unsafe
    // sources still fail. Delivery form must not launder a weak policy.
    let html =
        r#"<meta http-equiv="content-security-policy" content="script-src * 'unsafe-inline'">"#;
    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers(html, HeaderMap::new());
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .expect("csp result");
    assert_eq!(csp.status, CheckStatus::Fail);
    assert!(csp
        .description
        .contains("script-src allows unsafe inline script execution"));
}

#[test]
fn meta_report_only_variant_does_not_count_as_enforced() {
    let html =
        r#"<meta http-equiv="Content-Security-Policy-Report-Only" content="default-src 'self'">"#;
    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers(html, HeaderMap::new());
    let results = check.run(&ctx);
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .expect("csp result");
    assert_eq!(csp.status, CheckStatus::Fail);
    assert_eq!(csp.title, "No enforced Content-Security-Policy");
}

#[test]
fn meta_referrer_tag_is_not_reported_as_missing_policy() {
    let html = r#"<meta name="referrer" content="strict-origin-when-cross-origin">"#;
    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers(html, HeaderMap::new());
    let results = check.run(&ctx);
    let referrer = results
        .iter()
        .find(|r| r.check_id == "security.headers.referrer_policy")
        .expect("referrer result");
    assert_eq!(referrer.status, CheckStatus::Pass);
    assert!(referrer.description.contains("meta"));
}

#[test]
fn duplicate_csp_directives_grade_the_first_occurrence() {
    let mut h = all_security_headers();
    h.insert(
            "content-security-policy",
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; script-src * 'unsafe-inline'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
            ),
        );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();
    assert_eq!(
        csp.status,
        CheckStatus::Pass,
        "the first script-src ('self') is what browsers enforce; the ignored duplicate must not grade the policy: {}",
        csp.description
    );

    let mut h = all_security_headers();
    h.insert(
            "content-security-policy",
            HeaderValue::from_static(
                "default-src 'self'; script-src *; script-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
            ),
        );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();
    assert_eq!(
        csp.status,
        CheckStatus::Fail,
        "the first script-src (*) is what browsers enforce; the safe duplicate must not launder it: {}",
        csp.description
    );
}

#[test]
fn host_allowlist_only_script_src_names_the_allowlist_gap() {
    let mut h = all_security_headers();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "script-src https://cdn.example.com; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
        ),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let csp = results
        .iter()
        .find(|r| r.check_id == "security.headers.csp")
        .unwrap();
    assert_eq!(csp.status, CheckStatus::Warn);
    assert_eq!(csp.confidence, crate::vocab::IssueConfidence::NeedsReview);
    assert!(
        csp.description.contains("host allowlist"),
        "allowlist-only policy should be described as such, not as missing: {}",
        csp.description
    );
    assert!(
        !csp.description.contains("script-src is missing"),
        "must not claim script-src is missing when it exists: {}",
        csp.description
    );
}

#[test]
fn frame_ancestors_wildcard_is_not_clickjacking_protection() {
    for broad in ["frame-ancestors *", "frame-ancestors https:"] {
        let mut h = all_security_headers();
        h.remove("x-frame-options");
        h.insert(
            "content-security-policy",
            HeaderValue::from_str(&format!(
                "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'; {}",
                broad
            ))
            .unwrap(),
        );
        let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
        let xfo = results
            .iter()
            .find(|r| r.check_id == "security.headers.x_frame_options")
            .unwrap();
        assert_eq!(
            xfo.status,
            CheckStatus::Warn,
            "{} must not count as clickjacking protection: {}",
            broad,
            xfo.description
        );
        assert!(
            xfo.description.contains("frame-ancestors"),
            "description should name the broad frame-ancestors: {}",
            xfo.description
        );
    }
}

#[test]
fn frame_ancestors_explicit_origins_pass_with_accurate_copy() {
    let mut h = all_security_headers();
    h.remove("x-frame-options");
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors https://partner.example",
        ),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let xfo = results
        .iter()
        .find(|r| r.check_id == "security.headers.x_frame_options")
        .unwrap();
    assert_eq!(xfo.status, CheckStatus::Pass);
    assert!(
        xfo.description.contains("https://partner.example"),
        "explicit-origin pass copy should name the allowed ancestors instead of claiming no domain can embed: {}",
        xfo.description
    );
    assert!(
        !xfo.description.contains("cannot be embedded"),
        "explicit origins CAN embed the page; copy must not deny it: {}",
        xfo.description
    );
}

#[test]
fn frame_ancestors_none_passes_as_unembeddable() {
    let mut h = all_security_headers();
    h.remove("x-frame-options");
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
        ),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let xfo = results
        .iter()
        .find(|r| r.check_id == "security.headers.x_frame_options")
        .unwrap();
    assert_eq!(xfo.status, CheckStatus::Pass);
    assert!(xfo.description.contains("reject every framing ancestor"));
}

#[test]
fn missing_referrer_policy_copy_reflects_modern_defaults() {
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", HeaderMap::new()));
    let rp = results
        .iter()
        .find(|r| r.check_id == "security.headers.referrer_policy")
        .unwrap();
    assert_eq!(rp.status, CheckStatus::Warn);
    assert!(
        rp.description.contains("strict-origin-when-cross-origin"),
        "missing-header copy should describe the modern browser default: {}",
        rp.description
    );
    assert!(
        !rp.description.contains("session tokens"),
        "must not claim full URLs leak by default: {}",
        rp.description
    );
    assert!(!rp.description.contains("guarantees that behavior"));
}

#[test]
fn missing_hsts_copy_does_not_promise_first_visit_protection() {
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", HeaderMap::new()));
    let hsts = results
        .iter()
        .find(|r| r.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(hsts.status, CheckStatus::Fail);
    assert!(
        hsts.description
            .contains("before it has a cached HSTS policy")
            && hsts
                .description
                .contains("Direct `https://` navigation remains encrypted"),
        "copy must distinguish uncached HTTP navigation from direct HTTPS: {}",
        hsts.description
    );
}

#[test]
fn missing_permissions_policy_does_not_claim_silent_access() {
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", HeaderMap::new()));
    let perms = results
        .iter()
        .find(|r| r.check_id == "security.headers.permissions_policy")
        .unwrap();
    assert_eq!(perms.status, CheckStatus::Warn);
    let why = perms.why_it_matters.as_deref().unwrap_or_default();
    assert!(
        !why.contains("silently"),
        "permission prompts gate these features; access is not silent: {}",
        why
    );
    assert!(
        why.contains("request"),
        "why_it_matters should describe limiting what embedded code may request: {}",
        why
    );
}

#[test]
fn leaky_meta_referrer_value_still_warns() {
    let html = r#"<meta name="referrer" content="unsafe-url">"#;
    let check = SecurityHeadersCheck;
    let ctx = ctx_with_headers(html, HeaderMap::new());
    let results = check.run(&ctx);
    let referrer = results
        .iter()
        .find(|r| r.check_id == "security.headers.referrer_policy")
        .expect("referrer result");
    assert_eq!(referrer.status, CheckStatus::Warn);
    assert!(referrer.description.contains("unsafe-url"));
}

#[test]
fn csp_evidence_masks_volatile_tokens_and_report_url_secrets() {
    let mut headers = all_security_headers();
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'nonce-privateNonce123456789' 'sha256-privateHashPayload123456789'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'; report-uri https://reports.example.com/csp?api_key=privateReportKey",
        ),
    );

    let results = SecurityHeadersCheck.run(&ctx_with_headers("", headers));
    let csp = results
        .iter()
        .find(|result| result.check_id == "security.headers.csp")
        .expect("CSP result");
    let evidence = csp.raw_data.as_ref().expect("CSP evidence").to_string();

    assert!(evidence.contains("nonce-[redacted]"), "{evidence}");
    assert!(evidence.contains("sha256-[redacted]"), "{evidence}");
    assert!(
        evidence.contains("https://reports.example.com/csp"),
        "{evidence}"
    );
    for secret in ["privateNonce", "privateHash", "privateReportKey", "api_key"] {
        assert!(
            !evidence.contains(secret),
            "unsafe CSP evidence: {evidence}"
        );
    }
}

#[test]
fn permissive_frame_ancestors_overrides_xfo_in_supporting_browsers() {
    let mut h = all_security_headers();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors *",
        ),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let framing = results
        .iter()
        .find(|result| result.check_id == "security.headers.x_frame_options")
        .unwrap();
    assert_eq!(framing.status, CheckStatus::Warn);
    assert!(framing.description.contains("takes precedence"));
    assert_eq!(
        framing.confidence,
        crate::vocab::IssueConfidence::NeedsReview
    );
}

#[test]
fn x_content_type_options_requires_the_exact_nosniff_value() {
    let mut h = all_security_headers();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("notnosniff"),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let result = results
        .iter()
        .find(|result| result.check_id == "security.headers.x_content_type_options")
        .unwrap();
    assert_eq!(result.status, CheckStatus::Warn);
    assert!(result.description.contains("notnosniff"));
}

#[test]
fn referrer_policy_falls_back_to_the_last_recognized_token() {
    let mut h = all_security_headers();
    h.insert(
        "referrer-policy",
        HeaderValue::from_static("unsafe-url, future-policy"),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let result = results
        .iter()
        .find(|result| result.check_id == "security.headers.referrer_policy")
        .unwrap();
    assert_eq!(result.status, CheckStatus::Warn);
    assert_eq!(
        result.raw_data.as_ref().unwrap()["effective_policy"],
        "unsafe-url"
    );
}

#[test]
fn unrecognized_referrer_policy_is_not_reported_as_protection() {
    let mut h = all_security_headers();
    h.insert("referrer-policy", HeaderValue::from_static("future-policy"));
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let result = results
        .iter()
        .find(|result| result.check_id == "security.headers.referrer_policy")
        .unwrap();
    assert_eq!(result.status, CheckStatus::Warn);
    assert!(result.title.contains("recognized"));
}

#[test]
fn duplicate_hsts_directive_invalidates_the_policy() {
    let mut h = all_security_headers();
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; max-age=0; includeSubDomains"),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let hsts = results
        .iter()
        .find(|result| result.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(hsts.status, CheckStatus::Fail);
    assert!(hsts.description.contains("repeated"));
}

#[test]
fn include_subdomains_detection_is_token_aware() {
    let mut h = all_security_headers();
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; x-includeSubDomains-marker"),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let hsts = results
        .iter()
        .find(|result| result.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(hsts.status, CheckStatus::Warn);
    assert!(hsts.title.contains("includeSubDomains"));
}

#[test]
fn permissions_policy_pass_copy_does_not_claim_the_policy_is_valid_or_used() {
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", all_security_headers()));
    let result = results
        .iter()
        .find(|result| result.check_id == "security.headers.permissions_policy")
        .unwrap();
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result.description.contains("presence check"));
    assert!(!result.description.contains("features it uses"));
}

#[test]
fn empty_nonce_does_not_launder_unsafe_inline() {
    let mut h = all_security_headers();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "script-src 'nonce-' 'unsafe-inline'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
        ),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let csp = results
        .iter()
        .find(|result| result.check_id == "security.headers.csp")
        .unwrap();
    assert_eq!(csp.status, CheckStatus::Fail);
    assert!(csp.description.contains("unsafe inline"));
}

#[test]
fn none_combined_with_a_host_is_not_treated_as_a_none_only_script_policy() {
    let mut h = all_security_headers();
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "script-src 'none' https://cdn.example; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
        ),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", h));
    let csp = results
        .iter()
        .find(|result| result.check_id == "security.headers.csp")
        .unwrap();
    assert_eq!(csp.status, CheckStatus::Warn);
    assert!(csp.description.contains("host allowlist"));
}

#[test]
fn hsts_on_an_http_response_is_not_reported_as_active() {
    let mut ctx = ctx_with_headers("", all_security_headers());
    ctx.url = url::Url::parse("http://example.com/").unwrap();
    let results = SecurityHeadersCheck.run(&ctx);
    let hsts = results
        .iter()
        .find(|result| result.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(hsts.status, CheckStatus::Warn);
    assert!(hsts.title.contains("delivered over HTTP"));
    assert!(hsts.description.contains("ignore"));
}

#[test]
fn missing_hsts_on_http_is_skipped_in_favor_of_https_enforcement() {
    let mut headers = all_security_headers();
    headers.remove("strict-transport-security");
    let mut ctx = ctx_with_headers("", headers);
    ctx.url = url::Url::parse("http://example.com/").unwrap();
    let results = SecurityHeadersCheck.run(&ctx);
    let hsts = results
        .iter()
        .find(|result| result.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(hsts.status, CheckStatus::Skipped);
    assert!(hsts.description.contains("HTTPS-enforcement"));
}

#[test]
fn repeated_hsts_fields_warn_and_name_first_field_semantics() {
    let mut headers = all_security_headers();
    headers.append(
        "strict-transport-security",
        HeaderValue::from_static("max-age=0"),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", headers));
    let hsts = results
        .iter()
        .find(|result| result.check_id == "security.headers.hsts")
        .unwrap();
    assert_eq!(hsts.status, CheckStatus::Warn);
    assert!(hsts.description.contains("only the first"));
    assert_eq!(hsts.raw_data.as_ref().unwrap()["header_count"], 2);
}

#[test]
fn multiple_enforced_csp_fields_are_reviewed_as_an_intersection() {
    let mut headers = all_security_headers();
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("script-src *; frame-ancestors *"),
    );
    headers.append(
        "content-security-policy",
        HeaderValue::from_static(
            "script-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
        ),
    );
    let results = SecurityHeadersCheck.run(&ctx_with_headers("", headers));
    let csp = results
        .iter()
        .find(|result| result.check_id == "security.headers.csp")
        .unwrap();
    assert_eq!(csp.status, CheckStatus::Warn);
    assert!(csp.title.contains("Multiple enforced CSP"));
    assert!(csp.description.contains("intersect"));
    assert_eq!(csp.raw_data.as_ref().unwrap()["policy_count"], 2);

    let framing = results
        .iter()
        .find(|result| result.check_id == "security.headers.x_frame_options")
        .unwrap();
    assert_eq!(framing.status, CheckStatus::Pass);
    assert!(framing.description.contains("intersection"));
}
