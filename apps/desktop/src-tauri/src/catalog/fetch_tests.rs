//! Catalog request privacy-boundary tests.
//! Exact payload assertions make every newly transmitted field explicit.

use super::*;

fn request() -> CatalogRequest {
    CatalogRequest::new(
        "opaque-token",
        "1.4.0",
        Some("2026.07.20".to_string()),
        Channel::Stable,
    )
}

#[test]
fn sends_exactly_the_allowed_query_fields() {
    let pairs = request().query_pairs();
    let keys: Vec<&str> = pairs.iter().map(|(key, _)| *key).collect();
    assert_eq!(
        keys,
        vec!["client_version", "catalog_version"],
        "a catalog request must carry only the client and catalog versions"
    );
}

#[test]
fn omits_the_catalog_version_on_a_first_fetch() {
    let first = CatalogRequest::new("opaque-token", "1.4.0", None, Channel::Stable);
    let keys: Vec<&str> = first.query_pairs().iter().map(|(key, _)| *key).collect();
    assert_eq!(keys, vec!["client_version"]);
}

#[test]
fn carries_no_project_derived_value_anywhere_in_the_request() {
    let request = request();
    let emitted: String = request
        .query_pairs()
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    for forbidden in [
        "project",
        "site",
        "url",
        "path",
        "finding",
        "issue",
        "scan",
        "dependency",
        "package",
        "report",
        "prompt",
    ] {
        assert!(
            !emitted.to_lowercase().contains(forbidden),
            "catalog request must not carry {forbidden:?}: {emitted}"
        );
    }
}

#[test]
fn the_token_is_never_placed_in_the_query_string() {
    // Bearer header only. A token in a URL lands in proxy logs, browser
    // history, and Referer headers.
    let emitted: String = request()
        .query_pairs()
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    assert!(!emitted.contains("opaque-token"));
    assert!(!emitted.to_lowercase().contains("token"));
}

#[test]
fn refuses_to_build_a_url_when_no_endpoint_is_configured() {
    // Fail closed rather than defaulting to some host.
    if CATALOG_ENDPOINT.is_some() {
        return;
    }
    assert!(matches!(base_url(), Err(FetchError::NoEndpointConfigured)));
}

#[test]
fn the_manifest_carries_no_download_location() {
    let json = r#"{
        "catalog_version": "2026.07.26",
        "release_sequence": 4,
        "published_at": "2026-07-26T00:00:00Z",
        "content_hash": "abc",
        "signature": "sig",
        "minimum_engine_version": "1.0.0",
        "pack_url": "https://evil.test/pack.json"
    }"#;
    let parsed: CatalogManifest = serde_json::from_str(json).expect("extra fields are ignored");
    let round_trip = serde_json::to_string(&serde_json::json!({
        "catalog_version": parsed.catalog_version,
        "release_sequence": parsed.release_sequence,
        "published_at": parsed.published_at,
        "content_hash": parsed.content_hash,
        "signature": parsed.signature,
        "minimum_engine_version": parsed.minimum_engine_version,
    }))
    .expect("serializable");
    assert!(
        !round_trip.contains("evil.test"),
        "nothing the client parsed can carry the injected location"
    );
    let source = include_str!("fetch.rs");
    let derives = source
        .find("fn fetch_pack")
        .map(|at| &source[at..at + 600])
        .expect("fetch_pack exists");
    assert!(
        derives.contains("base_url()"),
        "fetch_pack must derive its URL from the build-time endpoint"
    );
}

#[test]
fn accepts_a_manifest_with_exactly_the_expected_fields() {
    let json = r#"{
        "catalog_version": "2026.07.26",
        "release_sequence": 4,
        "published_at": "2026-07-26T00:00:00Z",
        "content_hash": "abc",
        "signature": "sig",
        "minimum_engine_version": "1.0.0"
    }"#;
    let parsed: CatalogManifest = serde_json::from_str(json).expect("well-formed manifest");
    assert_eq!(parsed.release_sequence, 4);
}

#[test]
fn a_manifest_with_fields_this_build_does_not_know_still_parses() {
    let manifest: CatalogManifest = serde_json::from_str(
        r#"{
            "catalog_version": "2026-07-30.8",
            "release_sequence": 8,
            "published_at": "2026-07-30T00:00:00Z",
            "content_hash": "abc",
            "signature": "sig",
            "minimum_engine_version": "1.0.0",
            "field_from_the_future": { "shape": "unknown" }
        }"#,
    )
    .expect("an additive field must not brick installed clients");
    assert_eq!(manifest.release_sequence, 8);
}
