//! Link collection, resolution, and status verdict tests.

#![cfg(test)]

use super::*;

#[test]
fn link_sample_caps_never_shrink() {
    const {
        assert!(BROKEN_LINK_INTERNAL_SAMPLE >= 100);
        assert!(BROKEN_LINK_EXTERNAL_SAMPLE >= 30);
    }
}

fn ctx(body: &str) -> PageContext {
    PageContext {
        evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        url: url::Url::parse("https://example.com").unwrap(),
        response_headers: http::header::HeaderMap::new(),
        status_code: 200,
        body: body.to_string(),
        is_localhost: false,
        is_strict_localhost: false,
        http_version: Some("HTTP/2.0".to_string()),
        body_lower_cache: std::sync::OnceLock::new(),
    }
}

#[test]
fn head_get_retry_covers_every_error_response() {
    for status in [403, 404, 405, 410, 500] {
        assert!(head_status_needs_get_retry(status));
    }
    assert!(!head_status_needs_get_retry(200));
}

#[test]
fn collect_hrefs_handles_all_quote_forms_and_decodes_amp() {
    let html = r#"<a href="/double">a</a><a href='/single'>b</a><a href=/bare>c</a><a href="/q?a=1&amp;b=2">d</a>"#;
    assert_eq!(
        collect_hrefs(html),
        vec!["/double", "/single", "/bare", "/q?a=1&b=2"]
    );
}

#[test]
fn collect_hrefs_decodes_numeric_and_url_punctuation_character_references() {
    let html = r#"<a href="https&#58;&sol;&sol;other.example&sol;path&quest;x&equals;1&amp;y&equals;2">x</a>"#;
    assert_eq!(
        collect_hrefs(html),
        vec!["https://other.example/path?x=1&y=2"]
    );
}

#[test]
fn collect_hrefs_reads_only_real_anchors_in_initial_markup() {
    let html = r#"
        <base href="https://cdn.example/base/">
        <link rel="stylesheet" href="/app.css">
        <!-- <a href="/commented">old</a> -->
        <script>const template = '<a href="/scripted">x</a>';</script>
        <style>.x { background: url('/not-an-anchor') }</style>
        <a href="relative">real</a>
    "#;
    assert_eq!(collect_hrefs(html), vec!["relative"]);
}

#[test]
fn link_targets_resolve_relative_urls_and_compare_hosts_exactly() {
    let mut context = ctx(r##"<a href="next">next</a>
                <a href="http://example.com/plain">same host</a>
                <a href="https://example.com.evil/phish">other host</a>
                <a href="#section">fragment</a>"##);
    context.url = url::Url::parse("https://example.com/guide/page").unwrap();
    let targets = resolve_link_targets(&context, |_| true);
    assert_eq!(targets.anchor_href_count, 4);
    assert_eq!(
        targets
            .internal
            .iter()
            .map(url::Url::as_str)
            .collect::<Vec<_>>(),
        vec!["http://example.com/plain", "https://example.com/guide/next"]
    );
    assert_eq!(
        targets
            .external
            .iter()
            .map(url::Url::as_str)
            .collect::<Vec<_>>(),
        vec!["https://example.com.evil/phish"]
    );
    assert_eq!(targets.excluded_target_count, 1);
}

#[test]
fn dns_equivalent_trailing_dot_host_remains_internal() {
    let mut context = ctx(r#"<a href="https://example.com./other">other</a>"#);
    context.url = url::Url::parse("https://example.com/page").unwrap();
    let targets = resolve_link_targets(&context, |_| true);
    assert_eq!(targets.internal.len(), 1);
    assert!(targets.external.is_empty());
}

#[test]
fn first_base_href_controls_relative_anchor_resolution() {
    let mut context = ctx(r#"<base href="https://cdn.example/root/"><a href="docs">docs</a>"#);
    context.url = url::Url::parse("https://example.com/page").unwrap();
    let targets = resolve_link_targets(&context, |_| true);
    assert!(targets.internal.is_empty());
    assert_eq!(
        targets.external[0].as_str(),
        "https://cdn.example/root/docs"
    );
    assert_eq!(targets.effective_base_url, "https://cdn.example/root/");
}

#[test]
fn probe_status_classifier_confirms_only_404_and_410_as_missing() {
    assert_eq!(classify_probe_status(200), ProbeOutcomeKind::Responded);
    assert_eq!(classify_probe_status(302), ProbeOutcomeKind::Responded);
    assert_eq!(classify_probe_status(404), ProbeOutcomeKind::Missing);
    assert_eq!(classify_probe_status(410), ProbeOutcomeKind::Missing);
    for status in [401, 403, 500] {
        assert_eq!(
            classify_probe_status(status),
            ProbeOutcomeKind::Inconclusive
        );
    }
}

fn result_targets() -> LinkTargets {
    LinkTargets {
        anchor_href_count: 5,
        internal: Vec::new(),
        external: Vec::new(),
        excluded_target_count: 1,
        effective_base_url: "https://example.com/base/?token=secret".into(),
    }
}

#[test]
fn conclusive_link_sample_pass_does_not_claim_every_link_is_valid() {
    use crate::checks::CheckStatus;
    let summary = ProbeSummary {
        attempted_count: 2,
        responded_count: 2,
        ..Default::default()
    };
    let result = link_probe_result(
        "seo.broken_links",
        crate::checks::Severity::High,
        LinkScope::Internal,
        &result_targets(),
        4,
        2,
        2,
        summary,
    );
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result
        .description
        .contains("No HTTP 404 or 410 was observed"));
    assert!(result.description.contains("2 of 4 eligible"));
    assert!(!result.description.to_ascii_lowercase().contains("all"));
    assert!(!result.description.to_ascii_lowercase().contains("valid"));
    let raw = result.raw_data.as_ref().unwrap();
    assert_eq!(raw["sample_truncated"], true);
    assert_eq!(raw["soft_404_assessed"], false);
    assert_eq!(raw["rendered_dom_links_assessed"], false);
    assert!(!raw.to_string().contains("secret"));
}

#[test]
fn one_link_sample_uses_singular_grammar() {
    let summary = ProbeSummary {
        attempted_count: 1,
        responded_count: 1,
        ..Default::default()
    };
    let result = link_probe_result(
        "seo.broken_links",
        crate::checks::Severity::High,
        LinkScope::Internal,
        &result_targets(),
        1,
        1,
        100,
        summary,
    );
    assert!(result
        .description
        .contains("1 eligible destination was sampled"));
    assert!(!result.description.contains("1 eligible destinations"));
}

#[test]
fn inconclusive_link_sample_is_skipped_and_never_counted_as_broken() {
    use crate::checks::{CheckStatus, IssueConfidence};
    let summary = ProbeSummary {
        attempted_count: 2,
        responded_count: 1,
        inconclusive: vec![serde_json::json!({
            "url": "https://example.com/[path]",
            "http_status": 500,
            "method": "GET",
            "outcome": "inconclusive"
        })],
        inconclusive_labels: vec!["https://example.com/[path] (HTTP 500 via GET)".into()],
        ..Default::default()
    };
    let result = link_probe_result(
        "seo.broken_links",
        crate::checks::Severity::High,
        LinkScope::Internal,
        &result_targets(),
        2,
        2,
        100,
        summary,
    );
    assert_eq!(result.status, CheckStatus::Skipped);
    assert_eq!(result.confidence, IssueConfidence::NeedsReview);
    assert!(result.description.contains("does not establish"));
    assert_eq!(
        result.raw_data.as_ref().unwrap()["broken"],
        serde_json::json!([])
    );
    assert!(result.manual_fix.is_none());
}

#[test]
fn get_confirmed_missing_link_has_bounded_evidence_and_contextual_impact() {
    use crate::checks::CheckStatus;
    let summary = ProbeSummary {
        attempted_count: 1,
        broken: vec![serde_json::json!({
            "url": "https://example.com/[path]",
            "http_status": 404,
            "method": "GET",
            "outcome": "confirmed_404_or_410"
        })],
        broken_labels: vec!["https://example.com/[path] (HTTP 404 via GET)".into()],
        ..Default::default()
    };
    let result = link_probe_result(
        "seo.broken_links",
        crate::checks::Severity::High,
        LinkScope::Internal,
        &result_targets(),
        1,
        1,
        100,
        summary,
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.description.contains("GET confirmation"));
    assert_eq!(
        result.raw_data.as_ref().unwrap()["broken"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(result
        .why_it_matters
        .as_deref()
        .is_some_and(|why| why.contains("Actual impact depends")));
    assert!(!result
        .why_it_matters
        .as_deref()
        .unwrap_or_default()
        .contains("signal neglect"));
}

#[test]
fn link_evidence_does_not_persist_query_fragment_or_path_tokens() {
    let safe =
        evidence_url("https://user:pass@example.com/reset/secret-token?token=abc123#fragment");
    assert_eq!(safe, "https://example.com/reset/[redacted]");
    for secret in ["user", "pass", "secret-token", "abc123", "fragment"] {
        assert!(!safe.contains(secret), "evidence leaked {secret}: {safe}");
    }
}

#[test]
fn link_evidence_retains_an_actionable_ordinary_path() {
    assert_eq!(
        evidence_url("https://example.com/docs/missing-page?campaign=private#section"),
        "https://example.com/docs/missing-page"
    );
}

#[test]
fn broken_preview_truncates_long_lists_and_counts_the_rest() {
    let few: Vec<String> = (0..3).map(|i| format!("https://a.com/{i} (404)")).collect();
    assert_eq!(broken_preview(&few), few.join(", "));

    let many: Vec<String> = (0..14)
        .map(|i| format!("https://a.com/{i} (404)"))
        .collect();
    let preview = broken_preview(&many);
    assert!(preview.contains("https://a.com/9 (404)"));
    assert!(!preview.contains("https://a.com/10 (404)"));
    assert!(preview.ends_with("and 4 more (full list in the issue details)"));
}
