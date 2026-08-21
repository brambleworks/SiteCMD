use super::*;
use crate::probe::{ProbeBody, ProbeFailure, ProbeResponse};

fn page_url() -> url::Url {
    url::Url::parse("https://example.com/page").expect("static test url")
}

fn json_response(status: u16, text: &str) -> ProbeOutcome {
    ProbeOutcome::Response(ProbeResponse {
        status,
        final_url: "https://example.com/site.webmanifest".into(),
        content_type: Some("application/manifest+json".into()),
        content_length: None,
        headers: Vec::new(),
        body: (200..300).contains(&status).then(|| ProbeBody {
            text: text.to_string(),
            bytes: text.len(),
            utf8_valid: true,
        }),
    })
}

#[test]
fn extracts_manifest_href() {
    let body = r#"<link rel="manifest" href="/site.webmanifest">"#;
    assert_eq!(manifest_href(body).as_deref(), Some("/site.webmanifest"));
}

#[test]
fn extracts_href_when_attribute_order_reversed() {
    let body = r#"<link href="/manifest.json" rel="manifest" crossorigin="use-credentials">"#;
    assert_eq!(manifest_href(body).as_deref(), Some("/manifest.json"));
}

#[test]
fn no_manifest_link_returns_none() {
    assert!(manifest_href(r#"<link rel="icon" href="/favicon.ico">"#).is_none());
}

#[test]
fn extracts_unquoted_and_single_quoted_hrefs() {
    assert_eq!(
        manifest_href("<link rel=manifest href=/m.webmanifest>").as_deref(),
        Some("/m.webmanifest")
    );
    assert_eq!(
        manifest_href("<link rel='manifest' href='/m.json'>").as_deref(),
        Some("/m.json")
    );
}

#[test]
fn rel_token_list_containing_manifest_matches() {
    let body = r#"<link rel="manifest prefetch" href="/site.webmanifest">"#;
    assert_eq!(manifest_href(body).as_deref(), Some("/site.webmanifest"));
}

#[test]
fn manifest_identity_summary_requires_an_object_and_usable_values() {
    assert!(manifest_identity_summary(&serde_json::json!([])).is_none());

    let summary = manifest_identity_summary(&serde_json::json!({
        "name": "   ",
        "short_name": "Useful",
        "icons": [null, {}, {"src": ""}, {"src": " /icon.svg "}]
    }))
    .expect("object summary");
    assert!(summary.has_name);
    assert_eq!(summary.icon_source_count, 1);
}

#[test]
fn no_manifest_pass_copy_does_not_claim_a_manifest_is_the_only_theme_path() {
    let WebManifestStep::Done(results) = plan_web_manifest("<html></html>", &page_url()) else {
        panic!("a page without a manifest link must not plan a probe");
    };
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0].description.contains("installed-app experience"));
    assert!(!results[0].description.contains("only matters"));
    assert!(!results[0].description.contains("Android theme coloring"));
}

#[test]
fn a_declared_manifest_plans_a_resolved_probe() {
    let WebManifestStep::Probe { safe_href, url } = plan_web_manifest(
        r#"<link rel="manifest" href="/site.webmanifest">"#,
        &page_url(),
    ) else {
        panic!("a declared manifest must plan a probe");
    };
    assert_eq!(safe_href, "/site.webmanifest");
    assert_eq!(url.as_str(), "https://example.com/site.webmanifest");
}

#[test]
fn a_complete_manifest_passes_with_bounded_claims() {
    let results = evaluate_web_manifest(
        "/site.webmanifest",
        Ok(json_response(
            200,
            r#"{"name":"Acme","icons":[{"src":"/icon.png"}]}"#,
        )),
    );
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0]
        .description
        .contains("does not validate full browser install criteria"));
}

#[test]
fn a_manifest_without_name_or_icons_warns() {
    let results = evaluate_web_manifest("/site.webmanifest", Ok(json_response(200, r#"{}"#)));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0]
        .description
        .contains("a name or short_name and an icon entry"));
}

#[test]
fn unparseable_and_non_object_bodies_are_distinct_findings() {
    let invalid = evaluate_web_manifest("/m.json", Ok(json_response(200, "<!doctype html>")));
    assert_eq!(invalid[0].status, CheckStatus::Warn);
    assert!(invalid[0].title.contains("not valid JSON"));

    let non_object = evaluate_web_manifest("/m.json", Ok(json_response(200, "[]")));
    assert_eq!(non_object[0].status, CheckStatus::Warn);
    assert!(non_object[0].title.contains("not a JSON object"));
}

#[test]
fn a_confirmed_missing_manifest_is_high_confidence_but_a_500_is_not() {
    let missing = evaluate_web_manifest("/m.json", Ok(json_response(404, "")));
    assert_eq!(missing[0].status, CheckStatus::Warn);
    assert_eq!(missing[0].confidence, IssueConfidence::High);
    assert!(missing[0].description.contains("was not available"));

    let server_error = evaluate_web_manifest("/m.json", Ok(json_response(503, "")));
    assert_eq!(server_error[0].confidence, IssueConfidence::NeedsReview);
    assert!(server_error[0].description.contains("may be transient"));
}

#[test]
fn a_cap_overrun_reports_an_unread_body_while_a_transport_failure_reports_no_exchange() {
    let capped = evaluate_web_manifest(
        "/m.json",
        Ok(ProbeOutcome::Failure(ProbeFailure {
            class: ProbeFailureClass::BodyCapExceeded,
            detail: "body exceeded cap".into(),
        })),
    );
    assert_eq!(capped[0].status, CheckStatus::Skipped);
    assert!(capped[0]
        .description
        .contains("returned a successful status"));

    let refused = evaluate_web_manifest(
        "/m.json",
        Ok(ProbeOutcome::Failure(ProbeFailure {
            class: ProbeFailureClass::Transport,
            detail: "connection refused".into(),
        })),
    );
    assert_eq!(refused[0].status, CheckStatus::Skipped);
    assert!(refused[0]
        .description
        .contains("could not complete a request"));
}

#[test]
fn a_policy_refused_target_is_never_graded_as_available() {
    let results = evaluate_web_manifest(
        "/m.json",
        Err(WebManifestProbeSkip::Disallowed {
            safe_url: "http://169.254.169.254/m.json".into(),
        }),
    );
    assert_eq!(results[0].status, CheckStatus::Skipped);
    assert!(results[0].description.contains("network policy"));
    assert_eq!(
        results[0].raw_data.as_ref().unwrap()["reason"],
        "disallowed_page_subresource_target"
    );
}

#[test]
fn a_malformed_href_completes_without_a_probe() {
    // An unterminated IPv6 literal is one of the few hrefs that genuinely
    // fails resolution; most junk resolves as a relative path instead.
    let WebManifestStep::Done(results) = plan_web_manifest(
        r#"<link rel="manifest" href="https://[::1/m.json">"#,
        &page_url(),
    ) else {
        panic!("an unresolvable href must not plan a probe");
    };
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0].title.contains("malformed"));
}

#[test]
fn junk_that_still_resolves_is_probed_rather_than_called_malformed() {
    // `Url::join` treats most malformed-looking hrefs as relative paths, so
    // the plan must probe them; only genuine resolution failures warn.
    let WebManifestStep::Probe { url, .. } =
        plan_web_manifest(r#"<link rel="manifest" href="ht!tp://oops">"#, &page_url())
    else {
        panic!("a resolvable href must plan a probe");
    };
    assert_eq!(url.as_str(), "https://example.com/ht!tp://oops");
}
