//! Tests for the LemonSqueezy API client.

use super::*;

fn meta_with_store(store_id: u64) -> MetaData {
    MetaData {
        store_id: Some(store_id),
        product_id: None,
        variant_id: None,
        customer_email: None,
    }
}

#[tokio::test]
async fn validate_refuses_empty_and_whitespace_keys_without_a_network_call() {
    for key in ["", "   ", "\t\n"] {
        let err = validate(key, "inst-1").await.unwrap_err();
        assert!(
            err.contains("no license key available"),
            "expected the empty-key refusal, got: {err}"
        );
    }
}

#[test]
fn verify_store_id_rejects_placeholder_expected_store() {
    let meta = meta_with_store(99);
    let err = verify_store_id(Some(&meta), 0).unwrap_err();
    assert!(err.contains("not configured"));
}

#[test]
fn verify_store_id_rejects_when_meta_absent() {
    let err = verify_store_id(None, 42).unwrap_err();
    assert!(err.contains("missing LemonSqueezy metadata"));
}

#[test]
fn verify_store_id_rejects_when_store_id_field_absent() {
    let meta = MetaData {
        store_id: None,
        product_id: None,
        variant_id: None,
        customer_email: None,
    };
    let err = verify_store_id(Some(&meta), 42).unwrap_err();
    assert!(err.contains("missing LemonSqueezy store ID"));
}

#[test]
fn verify_store_id_passes_on_match() {
    let meta = meta_with_store(42);
    assert!(verify_store_id(Some(&meta), 42).is_ok());
}

#[test]
fn verify_store_id_rejects_cross_store_keys() {
    // The security-critical case: a key from store 99 must not unlock
    // tier features in our store 42.
    let meta = meta_with_store(99);
    let err = verify_store_id(Some(&meta), 42).unwrap_err();
    assert!(err.contains("99"));
    assert!(err.contains("42"));
}

#[test]
fn parse_activate_response_routes_provider_trouble_to_the_network_bucket() {
    use crate::licensing::activation_errors::{
        classify_activation_error, LicenseActivationErrorCode,
    };
    for status in [429u16, 500, 502, 503] {
        let err = parse_activate_response("<html>outage</html>", 123, status).unwrap_err();
        assert_eq!(
            classify_activation_error(&err),
            LicenseActivationErrorCode::Network,
            "{err}"
        );
    }
    let err =
        parse_activate_response(r#"{"activated": false, "error": "busy"}"#, 123, 503).unwrap_err();
    assert_eq!(
        classify_activation_error(&err),
        LicenseActivationErrorCode::Network
    );
}

fn activate_success_body() -> &'static str {
    r#"{
            "valid": true,
            "license_key": {
                "id": 12345,
                "status": "active",
                "key": "REDACTED-KEY",
                "activation_limit": 3,
                "activation_usage": 1,
                "expires_at": "2027-01-01T00:00:00Z"
            },
            "instance": { "id": "inst-abc-123", "name": "macbook-deadbeef" },
            "meta": {
                "store_id": 42,
                "product_id": 100,
                "variant_id": 901,
                "customer_email": "user@example.com"
            }
        }"#
}

#[test]
fn parse_activate_response_returns_full_result_on_success() {
    let result = parse_activate_response(activate_success_body(), 42, 200).unwrap();
    assert!(result.valid);
    assert_eq!(result.status, "active");
    assert_eq!(result.variant_id, 901);
    assert_eq!(result.instance_id.as_deref(), Some("inst-abc-123"));
    assert_eq!(result.expires_at.as_deref(), Some("2027-01-01T00:00:00Z"));
    assert!(result.error.is_none());
}

#[test]
fn parse_activate_response_accepts_lemon_checkout_trial_status() {
    let body = r#"{
            "activated": true,
            "license_key": { "status": "on_trial", "expires_at": "2026-06-01T00:00:00Z" },
            "instance": { "id": "inst-trial", "name": "macbook-deadbeef" },
            "meta": { "store_id": 42, "variant_id": 901 }
        }"#;
    let result = parse_activate_response(body, 42, 200).unwrap();

    assert!(result.valid);
    assert_eq!(result.status, "on_trial");
    assert_eq!(result.variant_id, 901);
    assert_eq!(result.instance_id.as_deref(), Some("inst-trial"));
}

#[test]
fn parse_activate_response_accepts_lemon_activate_shape() {
    let body = r#"{
            "activated": true,
            "error": null,
            "license_key": {
                "id": 12345,
                "status": "active",
                "key": "REDACTED-KEY",
                "activation_limit": 5,
                "activation_usage": 1,
                "created_at": "2026-05-12T16:02:02.000000Z"
            },
            "instance": { "id": "inst-live-123", "name": "macbook-deadbeef" },
            "meta": {
                "store_id": 42,
                "product_id": 100,
                "variant_id": 901,
                "customer_email": "user@example.com"
            }
        }"#;
    let result = parse_activate_response(body, 42, 200).unwrap();
    assert!(result.valid);
    assert_eq!(result.status, "active");
    assert_eq!(result.variant_id, 901);
    assert_eq!(result.instance_id.as_deref(), Some("inst-live-123"));
    assert!(result.error.is_none());
}

#[test]
fn parse_activate_response_does_not_require_echoed_license_key() {
    let body = r#"{
            "activated": true,
            "error": null,
            "license_key": {
                "status": "active",
                "expires_at": "2027-01-01T00:00:00Z"
            },
            "instance": { "id": "inst-no-key", "name": "macbook-deadbeef" },
            "meta": { "store_id": 42, "variant_id": 901 }
        }"#;
    let result = parse_activate_response(body, 42, 200).unwrap();
    assert!(result.valid);
    assert_eq!(result.instance_id.as_deref(), Some("inst-no-key"));
    assert_eq!(result.expires_at.as_deref(), Some("2027-01-01T00:00:00Z"));
}

#[test]
fn parse_activate_response_enforces_store_id() {
    let refused = parse_activate_response(activate_success_body(), 99, 200)
        .expect("a store mismatch is a refusal, not a parse failure");
    assert!(!refused.valid, "a store mismatch must not read as valid");
    let error = refused.error.expect("the mismatch names both stores");
    assert!(error.contains("42"), "expected store-id mismatch: {error}");
    assert!(
        refused.instance_id.is_some(),
        "the minted instance id must survive the refusal, or nothing can release it"
    );
    assert_eq!(
        crate::licensing::activation_errors::classify_activation_error(&error),
        crate::licensing::activation_errors::LicenseActivationErrorCode::StoreMismatch,
        "the refusal must still classify as StoreMismatch for the user-facing copy"
    );
}

#[test]
fn an_activate_body_with_no_verdict_field_is_not_a_verdict() {
    for body in [
        r#"{"error":"Not Found"}"#,
        r#"{"error":"limit reached"}"#,
        r#"{"message":"Forbidden"}"#,
        "{}",
    ] {
        let err = parse_activate_response(body, 42, 404)
            .expect_err("a body with no verdict field must not be a verdict");
        assert!(
            err.contains("carried no provider verdict"),
            "body {body} produced {err}"
        );
        // The classifier must keep it out of every conclusive bucket -
        // above all out of LimitReached, which frees a live instance.
        assert_eq!(
            crate::licensing::activation_errors::classify_activation_error(&err),
            crate::licensing::activation_errors::LicenseActivationErrorCode::ServerError,
            "body {body} classified conclusively"
        );
    }
}

#[test]
fn parse_activate_response_handles_invalid_response() {
    let body = r#"{
            "valid": false,
            "error": "License key not found",
            "license_key": { "id": 0, "status": "inactive", "key": "BAD" }
        }"#;
    let result = parse_activate_response(body, 0, 404).unwrap();
    assert!(!result.valid);
    assert_eq!(result.status, "inactive");
    assert_eq!(result.variant_id, 0);
    assert!(result.instance_id.is_none());
    assert_eq!(result.error.as_deref(), Some("License key not found"));
}

#[test]
fn parse_activate_response_tolerates_partial_failure_license_key() {
    let body = r#"{
            "activated": false,
            "error": "License key not found",
            "license_key": { "key": "REDACTED-KEY" }
        }"#;
    let result = parse_activate_response(body, 0, 404).unwrap();
    assert!(!result.valid);
    assert_eq!(result.status, "unknown");
    assert_eq!(result.error.as_deref(), Some("License key not found"));
}

#[test]
fn parse_activate_response_synthesizes_error_when_missing() {
    let body = r#"{ "valid": false }"#;
    let result = parse_activate_response(body, 0, 422).unwrap();
    assert!(!result.valid);
    assert_eq!(result.status, "unknown");
    let err = result.error.unwrap();
    assert!(
        err.contains("422"),
        "expected HTTP status in error: {}",
        err
    );
}

#[test]
fn parse_activate_response_defaults_status_to_active_when_field_absent() {
    // valid=true with no license_key block shouldn't crash; default to active.
    let body = r#"{ "valid": true, "instance": { "id": "x", "name": "y" }, "meta": { "store_id": 42, "variant_id": 7 } }"#;
    let result = parse_activate_response(body, 42, 200).unwrap();
    assert!(result.valid);
    assert_eq!(result.status, "active");
    assert_eq!(result.variant_id, 7);
}

#[test]
fn parse_activate_response_propagates_json_error() {
    let err = parse_activate_response(
        r#"{ "activated": true, "license_key": { "key": "REDACTED-KEY" }, "#,
        0,
        200,
    )
    .unwrap_err();
    assert!(err.contains("Failed to parse activation response"));
    assert!(!err.contains("REDACTED-KEY"));
}

#[test]
fn parse_validate_response_returns_active_state() {
    let body = r#"{
            "valid": true,
            "license_key": { "id": 1, "status": "active", "key": "K", "expires_at": "2030-01-01T00:00:00Z" },
            "meta": { "store_id": 42, "variant_id": 901 }
        }"#;
    let result = parse_validate_response(body, "inst-xyz", 42).unwrap();
    assert!(result.valid);
    assert_eq!(result.status, "active");
    assert_eq!(result.variant_id, 901);
    assert_eq!(result.instance_id.as_deref(), Some("inst-xyz"));
    assert_eq!(result.expires_at.as_deref(), Some("2030-01-01T00:00:00Z"));
}

#[test]
fn parse_validate_response_accepts_lemon_checkout_trial_status() {
    let body = r#"{
            "valid": true,
            "license_key": { "status": "on_trial", "expires_at": "2026-06-01T00:00:00Z" },
            "meta": { "store_id": 42, "variant_id": 901 }
        }"#;
    let result = parse_validate_response(body, "inst-trial", 42).unwrap();

    assert!(result.valid);
    assert_eq!(result.status, "on_trial");
    assert_eq!(result.variant_id, 901);
    assert_eq!(result.instance_id.as_deref(), Some("inst-trial"));
}

#[test]
fn parse_validate_response_does_not_require_echoed_license_key() {
    let body = r#"{
            "valid": true,
            "license_key": { "status": "active" },
            "meta": { "store_id": 42, "variant_id": 901 }
        }"#;
    let result = parse_validate_response(body, "inst-xyz", 42).unwrap();
    assert!(result.valid);
    assert_eq!(result.status, "active");
    assert_eq!(result.variant_id, 901);
}

#[test]
fn a_throttle_or_outage_status_is_trouble_and_never_a_verdict() {
    let throttle = r#"{ "message": "Too Many Requests" }"#;
    let err = classify_validate_response(429, throttle, "inst-1", 42).unwrap_err();
    assert!(err.contains("429"), "{err}");

    let outage = r#"{ "error": "internal" }"#;
    let err = classify_validate_response(500, outage, "inst-1", 42).unwrap_err();
    assert!(err.contains("500"), "{err}");
    assert!(classify_validate_response(503, "{}", "inst-1", 42).is_err());
}

#[test]
fn a_validate_body_with_no_verdict_field_is_not_a_verdict() {
    for body in [
        r#"{"error":"forbidden"}"#,
        r#"{"message":"Not Found"}"#,
        "{}",
        r#"{"license_key":{"status":"active"}}"#,
    ] {
        let err = classify_validate_response(403, body, "inst-1", 42)
            .expect_err("a body with no verdict field must not be a verdict");
        assert!(
            err.contains("carried no provider verdict"),
            "body {body} produced {err}"
        );
    }
}

#[test]
fn a_conclusive_refusal_on_a_4xx_still_lands_as_a_verdict() {
    let body = r#"{
            "valid": false,
            "error": "license_key has expired",
            "license_key": { "status": "expired" }
        }"#;
    let result = classify_validate_response(400, body, "inst-1", 42).unwrap();
    assert!(!result.valid);
    assert_eq!(result.status, "expired");

    let ok = r#"{
            "valid": true,
            "license_key": { "status": "active" },
            "meta": { "store_id": 42, "variant_id": 901 }
        }"#;
    let result = classify_validate_response(200, ok, "inst-1", 42).unwrap();
    assert!(result.valid);
}

#[test]
fn parse_validate_response_returns_expired_state() {
    let body = r#"{
            "valid": false,
            "error": "license_key has expired",
            "license_key": { "id": 1, "status": "expired", "key": "K" },
            "meta": { "variant_id": 901 }
        }"#;
    let result = parse_validate_response(body, "inst-xyz", 42).unwrap();
    assert!(!result.valid);
    assert_eq!(result.status, "expired");
    assert_eq!(result.error.as_deref(), Some("license_key has expired"));
    // Critical: instance_id is preserved so the caller can still address
    // the slot for a deactivate even after the key expires.
    assert_eq!(result.instance_id.as_deref(), Some("inst-xyz"));
}

#[test]
fn parse_validate_response_defaults_status_when_license_key_missing() {
    let body = r#"{ "valid": false }"#;
    let result = parse_validate_response(body, "inst-1", 42).unwrap();
    assert!(!result.valid);
    assert_eq!(result.status, "unknown");
    assert_eq!(result.variant_id, 0);
    assert_eq!(result.instance_id.as_deref(), Some("inst-1"));
}

#[test]
fn parse_validate_response_tolerates_partial_failure_license_key() {
    let body = r#"{
            "valid": false,
            "error": "license_key not found",
            "license_key": { "key": "REDACTED-KEY" }
        }"#;
    let result = parse_validate_response(body, "inst-1", 42).unwrap();
    assert!(!result.valid);
    assert_eq!(result.status, "unknown");
    assert_eq!(result.error.as_deref(), Some("license_key not found"));
}

#[test]
fn parse_validate_response_propagates_json_error() {
    let err = parse_validate_response(
        r#"{ "valid": true, "license_key": { "key": "REDACTED-KEY" }, "#,
        "inst",
        42,
    )
    .unwrap_err();
    assert!(err.contains("Failed to parse validation response"));
    assert!(!err.contains("REDACTED-KEY"));
}

#[test]
fn parse_validate_response_enforces_store_id() {
    let body = r#"{
            "valid": true,
            "license_key": { "id": 1, "status": "active", "key": "K" },
            "meta": { "store_id": 99, "variant_id": 901 }
        }"#;
    let err = parse_validate_response(body, "inst-xyz", 42).unwrap_err();
    assert!(err.contains("99"));
    assert!(err.contains("42"));
}

#[test]
fn a_gone_instance_or_dead_key_is_terminal_and_trouble_is_not() {
    for terminal in [
        "HTTP 404: Deactivation failed",
        "HTTP 400: instance not found",
        "HTTP 400: This license key has expired.",
        "HTTP 400: This license key has been disabled.",
    ] {
        assert!(deactivate_failure_is_terminal(terminal), "{terminal}");
    }
    for owed in [
        "License deactivation request failed: connection reset by peer",
        "License deactivation answered status 500; provider trouble, not a verdict",
        "License deactivation answered status 429; provider trouble, not a verdict",
        "Deactivation response unreadable (status 502): expected value",
        "Deactivation response unreadable (status 404): expected value at line 1 column 1",
        r#"Deactivation response unreadable (status 404): invalid type: string "not found", expected a boolean at line 1 column 21"#,
        "Deactivation response carried no provider verdict (status 404)",
    ] {
        assert!(!deactivate_failure_is_terminal(owed), "{owed}");
    }
}

// Reject terminal vocabulary echoed through a malformed typed field.
#[test]
fn a_serde_echo_of_terminal_vocabulary_stays_owed() {
    for body in [
        r#"{ "valid": "not found" }"#,
        r#"{ "deactivated": "has expired" }"#,
    ] {
        let err = parse_deactivate_response(404, body).unwrap_err();
        assert!(
            err.to_ascii_lowercase().contains("not found")
                || err.to_ascii_lowercase().contains("has expired"),
            "the fixture must actually echo, or this test pins nothing: {err}"
        );
        assert!(
            !deactivate_failure_is_terminal(&err),
            "an echoed body string is not the provider's verdict: {err}"
        );
    }
}

// End-to-end polarity through the real parse path, not hand-built
// strings: an unparseable 404 stays owed, the provider's parsed 404
// verdict settles.
#[test]
fn only_a_parsed_404_verdict_is_terminal() {
    let edge_html = parse_deactivate_response(404, "<html>Not Found</html>").unwrap_err();
    assert!(
        !deactivate_failure_is_terminal(&edge_html),
        "an unparsed 404 must stay owed: {edge_html}"
    );
    let provider_verdict =
        parse_deactivate_response(404, r#"{ "valid": false, "error": "Instance not found." }"#)
            .unwrap_err();
    assert!(
        deactivate_failure_is_terminal(&provider_verdict),
        "the provider's parsed 404 settles: {provider_verdict}"
    );
}

// Vacuous JSON from an intermediary must not settle a provider release.
#[test]
fn a_vacuous_json_body_is_not_a_provider_verdict() {
    for middlebox in [
        r#"{}"#,
        r#"{ "message": "Not Found" }"#,
        r#"{ "error": "Not Found" }"#,
    ] {
        let err = parse_deactivate_response(404, middlebox).unwrap_err();
        assert!(
            !deactivate_failure_is_terminal(&err),
            "a verdict-free JSON 404 must stay owed: {err}"
        );
    }
    for verdict_body in [
        r#"{ "deactivated": false }"#,
        r#"{ "valid": false, "error": "Instance not found." }"#,
    ] {
        let verdict = parse_deactivate_response(404, verdict_body).unwrap_err();
        assert!(
            deactivate_failure_is_terminal(&verdict),
            "a verdict-bearing 404 settles: {verdict}"
        );
    }
}

#[test]
fn parse_deactivate_response_succeeds_when_valid() {
    assert!(parse_deactivate_response(200, r#"{ "valid": true }"#).is_ok());
}

#[test]
fn parse_deactivate_response_succeeds_when_deactivated() {
    assert!(parse_deactivate_response(200, r#"{ "deactivated": true }"#).is_ok());
}

#[test]
fn parse_deactivate_response_returns_error_string() {
    let err =
        parse_deactivate_response(400, r#"{ "valid": false, "error": "instance not found" }"#)
            .unwrap_err();
    assert_eq!(err, "HTTP 400: instance not found");
}

#[test]
fn parse_deactivate_response_defaults_error_when_missing() {
    let err = parse_deactivate_response(400, r#"{ "valid": false }"#).unwrap_err();
    assert_eq!(err, "HTTP 400: Deactivation failed");
}

#[test]
fn parse_deactivate_response_classifies_provider_trouble_before_parsing() {
    // A 5xx or 429 answers transient regardless of body - JSON or HTML -
    // and its message can never match the terminal set.
    for status in [500u16, 502, 503, 429] {
        let err = parse_deactivate_response(status, "<html>outage</html>").unwrap_err();
        assert!(err.contains("provider trouble"), "{err}");
        assert!(!deactivate_failure_is_terminal(&err), "{err}");
    }
    let json_5xx =
        parse_deactivate_response(503, r#"{ "valid": false, "error": "not found" }"#).unwrap_err();
    assert!(
        !deactivate_failure_is_terminal(&json_5xx),
        "a 5xx body's wording never reaches the terminal matcher: {json_5xx}"
    );
}

#[test]
fn parse_deactivate_response_propagates_json_error() {
    let err = parse_deactivate_response(400, "not json").unwrap_err();
    assert!(err.contains("Deactivation response unreadable"));
}

#[test]
fn machine_instance_name_is_deterministic() {
    // Same machine/user → same name across calls.
    let a = machine_instance_name();
    let b = machine_instance_name();
    assert_eq!(a, b);
}

#[test]
fn machine_instance_name_format_is_sitecmd_dash_hex16() {
    let name = machine_instance_name();
    let hex = name
        .strip_prefix("sitecmd-")
        .expect("expected `sitecmd-hex` format");
    assert_eq!(
        hex.len(),
        16,
        "hash segment should be 16 hex chars: {}",
        name
    );
    assert!(
        hex.chars().all(|c| c.is_ascii_hexdigit()),
        "hash must be hex: {}",
        name
    );
}

#[test]
fn machine_instance_name_does_not_leak_host_or_username() {
    let name = machine_instance_name_from_parts("Zephyrs-MacBook-Pro", "zephyr");
    assert!(!name.to_lowercase().contains("zephyr"));
    assert!(!name.to_lowercase().contains("macbook"));
    assert!(!name.to_lowercase().contains("pro"));
}

#[test]
fn machine_instance_name_does_not_leak_current_username() {
    let name = machine_instance_name();
    let username = whoami::username().unwrap_or_else(|_| "user".to_string());
    if username.is_empty() || username == "unknown" {
        return;
    }
    assert!(
        !name.to_lowercase().contains(&username.to_lowercase()),
        "instance name `{}` leaked username `{}`",
        name,
        username,
    );
}
