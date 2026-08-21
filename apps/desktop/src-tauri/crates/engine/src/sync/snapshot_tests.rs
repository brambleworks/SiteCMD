use super::*;
use crate::release::ExecutionProfile;
use serde_json::json;

#[test]
fn the_engine_makes_the_browser_fact_and_the_build_rides_along() {
    let both = ExecutionProfile {
        browser_engine: Some("webkit".into()),
        browser_build: Some("621.1.15".into()),
        ..ExecutionProfile::default()
    };
    assert_eq!(
        WireExecutionProfile::from_execution(&both).browser,
        Some(BrowserProfile {
            engine: "webkit".into(),
            build: Some("621.1.15".into()),
        })
    );

    let engine_only = ExecutionProfile {
        browser_engine: Some("webkit".into()),
        ..ExecutionProfile::default()
    };
    assert_eq!(
        WireExecutionProfile::from_execution(&engine_only).browser,
        Some(BrowserProfile {
            engine: "webkit".into(),
            build: None,
        })
    );

    // A build with no engine names nothing.
    let build_only = ExecutionProfile {
        browser_build: Some("621.1.15".into()),
        ..ExecutionProfile::default()
    };
    assert_eq!(
        WireExecutionProfile::from_execution(&build_only).browser,
        None
    );
}

#[test]
fn the_wire_profile_never_carries_a_locally_invented_epoch() {
    let profile = ExecutionProfile {
        browser_engine: Some("webkit".into()),
        browser_build: Some("621.1.15".into()),
        browser_epoch: Some("invented-locally".into()),
        ..ExecutionProfile::default()
    };
    let wire =
        serde_json::to_string(&WireExecutionProfile::from_execution(&profile)).expect("serialize");
    assert!(!wire.contains("invented-locally"), "{wire}");
    assert!(!wire.contains("epoch"), "{wire}");
}

#[test]
fn the_wire_profile_has_no_place_for_an_instance_or_a_locality() {
    let wire = serde_json::to_value(WireExecutionProfile::default()).expect("serialize");
    let object = wire.as_object().expect("object");
    assert!(!object.contains_key("producer_instance"));
    assert!(!object.contains_key("locality"));
    assert!(!object.contains_key("vantage"));
}

#[test]
fn the_wire_profile_preserves_the_tls_trust_authority() {
    let profile = ExecutionProfile {
        tls_client: Some("rustls".into()),
        trust_authority: Some("webpki_roots".into()),
        ..ExecutionProfile::default()
    };

    let wire = WireExecutionProfile::from_execution(&profile);

    assert_eq!(wire.trust_authority.as_deref(), Some("webpki_roots"));
}

#[test]
fn only_a_checkout_or_compatible_basis_may_resolve_absence() {
    assert!(CodeBasisKind::ExactCheckout.may_resolve_absence());
    assert!(CodeBasisKind::Compatible.may_resolve_absence());
    assert!(!CodeBasisKind::Stale.may_resolve_absence());
    assert!(!CodeBasisKind::Unknown.may_resolve_absence());
}

#[test]
fn the_desktop_basis_is_named_apart_from_the_attested_one() {
    assert_eq!(
        serde_json::to_value(CodeBasisKind::ExactCheckout).expect("serialize"),
        json!("exact_checkout")
    );
    let attested: Result<CodeBasisKind, _> = serde_json::from_value(json!("exact"));
    assert!(attested.is_err());
}

#[test]
fn a_web_occurrence_carries_its_route_and_query_flag_flat() {
    let occurrence = WebOccurrence {
        check: "security.csp".into(),
        route: Some(crate::route::CanonicalRoute::new("/product", true)),
        scope_route: Some("/catalog".into()),
        severity: crate::vocab::Severity::High,
        confidence: None,
    };
    assert_eq!(
        serde_json::to_value(&occurrence).expect("serialize"),
        json!({
            "check": "security.csp",
            "route": "/product",
            "query_dependent": true,
            "scope_route": "/catalog",
            "severity": "high",
        })
    );
}

#[test]
fn a_site_scoped_occurrence_does_not_invent_a_route() {
    let occurrence = WebOccurrence {
        check: "seo.duplicate_title_across_pages".into(),
        route: None,
        scope_route: None,
        severity: crate::vocab::Severity::Medium,
        confidence: Some(crate::vocab::IssueConfidence::Confirmed),
    };

    let value = serde_json::to_value(&occurrence).expect("serialize");
    assert!(value.get("route").is_none());
    assert!(value.get("query_dependent").is_none());
}

#[test]
fn coverage_states_a_claim_and_the_pairs_it_does_not_reach() {
    let coverage = WireCoverage {
        kind: crate::coverage::ScanCoverageKind::PageSet,
        complete: true,
        routes: vec!["/".into(), "/docs".into()],
        checks: vec!["security.csp".into(), "performance.lcp".into()],
        exceptions: vec![WireCoverageException {
            route: Some("/docs".into()),
            checks_not_run: vec!["performance.lcp".into()],
            reason: crate::coverage::CoverageExceptionReason::CheckSkipped,
        }],
    };
    let value = serde_json::to_value(&coverage).expect("serialize");
    assert_eq!(value["kind"], json!("page_set"));
    assert_eq!(value["complete"], json!(true));
    assert_eq!(value["exceptions"][0]["reason"], json!("check_skipped"));
    assert_eq!(
        value["exceptions"][0]["checks_not_run"],
        json!(["performance.lcp"])
    );
}

#[test]
fn a_code_snapshot_states_no_crawl_profile() {
    // A code scan walks a project tree and crawls nothing. Carrying the field
    // would state a fact about an activity that did not occur.
    let versions = CodeVersions {
        engine_release: "1.5.4".into(),
        fingerprint_schema: 1,
        fingerprint_key_version: 1,
        canonicalizer: 1,
    };
    let value = serde_json::to_value(&versions).expect("serialize");
    assert!(value.get("crawl_profile").is_none(), "{value:#}");
    assert_eq!(value["fingerprint_key_version"], json!(1));
}

#[test]
fn an_empty_collection_is_omitted_rather_than_sent_as_noise() {
    // The payload is meant to be read by a human in the sync inspector. Empty
    // arrays for every optional collection would bury the facts that matter.
    let coverage = WireCoverage {
        kind: crate::coverage::ScanCoverageKind::Project,
        complete: true,
        routes: vec![],
        checks: vec![],
        exceptions: vec![],
    };
    let value = serde_json::to_value(&coverage).expect("serialize");
    assert_eq!(value, json!({"kind": "project", "complete": true}));
}
