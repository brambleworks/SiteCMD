use super::*;

#[test]
fn activate_request_carries_exactly_three_fields() {
    let body = activate_body_json("key-1", "inst-1", "nonce-1");
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    let object = value.as_object().unwrap();
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["installationId", "licenseKey", "nonce"]);
    assert_eq!(object["licenseKey"], "key-1");
    assert_eq!(object["installationId"], "inst-1");
    assert_eq!(object["nonce"], "nonce-1");
}

#[test]
fn parses_an_issued_credential() {
    // Verbatim shape from the first live activation.
    let outcome = parse_activate_response(200, r#"{"tier":"core","token":"sitecmd_cat_abc"}"#);
    assert_eq!(
        outcome.unwrap(),
        ActivationOutcome::Issued {
            token: "sitecmd_cat_abc".to_string(),
            tier: "core".to_string(),
        }
    );
}

#[test]
fn parses_a_replay_as_already_activated() {
    // The replay response deliberately carries no token: the service cannot
    // recover one. The caller's remedy is release-and-retry, not parsing.
    let outcome = parse_activate_response(200, r#"{"replayed":true,"status":"already_activated"}"#);
    assert_eq!(outcome.unwrap(), ActivationOutcome::AlreadyActivated);
}

#[test]
fn a_success_without_a_token_is_malformed_not_issued() {
    let outcome = parse_activate_response(200, r#"{"tier":"core"}"#);
    assert!(matches!(
        outcome,
        Err(ActivationError::MalformedResponse(_))
    ));
}

#[test]
fn refusals_carry_the_service_reason_and_are_not_retryable() {
    for reason in [
        "invalid_license",
        "wrong_store",
        "unknown_variant",
        "subscription_inactive",
    ] {
        let body = format!(r#"{{"error":"{reason}"}}"#);
        match parse_activate_response(403, &body) {
            Err(error @ ActivationError::Refused { .. }) => {
                assert_eq!(error.to_string(), format!("activation refused: {reason}"));
                assert!(!error.is_retryable(), "{reason} must not be retried");
            }
            other => panic!("expected refusal for {reason}, got {other:?}"),
        }
    }
}

#[test]
fn the_credential_cap_carries_its_numbers() {
    // Live 409 shape, including the remedy field this parser ignores.
    let body = r#"{"activeCredentials":3,"cap":3,"error":"credential_cap_reached","remedy":"Deactivate an existing installation, then activate this one."}"#;
    match parse_activate_response(409, body) {
        Err(ActivationError::CredentialCapReached { active, cap }) => {
            assert_eq!((active, cap), (3, 3));
        }
        other => panic!("expected cap error, got {other:?}"),
    }
}

#[test]
fn a_409_without_the_services_cap_shape_is_not_a_cap_verdict() {
    for body in [
        r#"{"message":"conflict"}"#,
        r#"{"error":"conflict"}"#,
        "{}",
        r#"{"error":"credential_cap_reached"}"#,
        r#"{"error":"credential_cap_reached","cap":3}"#,
        r#"{"error":"credential_cap_reached","activeCredentials":3}"#,
        r#"{"error":"invalid_license"}"#,
    ] {
        let outcome = parse_activate_response(409, body);
        match &outcome {
            Err(ActivationError::Refused { reason }) => {
                assert_eq!(reason, "refused", "body {body} must degrade");
            }
            other => panic!("expected degraded refusal for {body}, got {other:?}"),
        }
        assert_eq!(
            nonce_disposition(&outcome),
            NonceAction::Keep,
            "a shapeless 409 must keep the reclaim handle"
        );
    }
}

#[test]
fn upstream_trouble_is_retryable() {
    for status in [429, 500, 502, 503] {
        let error = parse_activate_response(status, r#"{"error":"upstream_unavailable"}"#)
            .expect_err("must be an error");
        assert!(matches!(error, ActivationError::ServiceUnavailable));
        assert!(error.is_retryable());
    }
}

#[test]
fn garbage_is_malformed_never_issued() {
    assert!(matches!(
        parse_activate_response(200, "not json"),
        Err(ActivationError::MalformedResponse(_))
    ));
    assert!(matches!(
        parse_activate_response(204, "{}"),
        Err(ActivationError::MalformedResponse(_))
    ));
}

#[test]
fn deactivation_reports_released_slots() {
    // Verbatim from the live deactivation, and the already-gone case, which
    // the service reports as success with zero released so retries are safe.
    assert_eq!(
        parse_deactivate_response(200, r#"{"released":1,"status":"deactivated"}"#).unwrap(),
        1
    );
    assert_eq!(
        parse_deactivate_response(200, r#"{"released":0,"status":"deactivated"}"#).unwrap(),
        0
    );
}

#[test]
fn a_deactivation_200_with_no_released_count_is_not_a_release() {
    // A missing count is ambiguous; treating it as success can lose the only
    // retry handle for an unreleased credential slot.
    for body in [
        "{}",
        r#"{"status":"ok"}"#,
        r#"{"released":null}"#,
        r#"{"error":"blocked by security policy"}"#,
    ] {
        let error = parse_deactivate_response(200, body).unwrap_err();
        assert!(
            matches!(error, ActivationError::MalformedResponse(_)),
            "body {body} produced {error:?} instead of a malformed-response verdict"
        );
        assert!(
            !matches!(error, ActivationError::Refused { .. }),
            "body {body} must not settle as a refusal"
        );
    }
}

#[test]
fn deactivation_refusal_and_unavailability_map_like_activation() {
    assert!(matches!(
        parse_deactivate_response(400, r#"{"error":"malformed_request"}"#),
        Err(ActivationError::Refused { .. })
    ));
    assert!(matches!(
        parse_deactivate_response(503, r#"{"error":"unavailable"}"#),
        Err(ActivationError::ServiceUnavailable)
    ));
}

#[test]
fn nonces_are_fresh_and_opaque() {
    let a = fresh_nonce();
    let b = fresh_nonce();
    assert_ne!(a, b, "two attempts must never share a nonce");
    assert_eq!(a.len(), 32);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn no_endpoint_refuses_without_touching_the_network() {
    // Development builds have no activation endpoint, and the refusal must be
    // the configuration error, not a transport attempt.
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_none() {
        let result = base_url();
        assert!(matches!(result, Err(ActivationError::NoEndpointConfigured)));
    }
}

#[test]
fn a_pending_attempt_knows_its_identity() {
    let pending = PendingActivation::mint("key-1", "inst-1");
    assert!(pending.is_for("key-1", "inst-1"));
    assert!(!pending.is_for("key-2", "inst-1"), "different key");
    assert!(!pending.is_for("key-1", "inst-2"), "different installation");
    let encoded = serde_json::to_string(&pending).unwrap();
    assert!(!encoded.contains("key-1"));
}

// In-memory store for driving `obtain_token`'s nonce lifecycle without a
// keychain.
struct MemoryNonceStore(std::sync::Mutex<Option<PendingActivation>>);

impl PendingNonceStore for MemoryNonceStore {
    fn load(&self) -> Result<Option<PendingActivation>, String> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, pending: &PendingActivation) -> Result<(), String> {
        *self.0.lock().unwrap() = Some(pending.clone());
        Ok(())
    }
    fn clear(&self) {
        *self.0.lock().unwrap() = None;
    }
}

// A store whose reads fail, as a locked or ACL-denied keychain's do.
struct UnreadableNonceStore;

impl PendingNonceStore for UnreadableNonceStore {
    fn load(&self) -> Result<Option<PendingActivation>, String> {
        Err("keychain locked".to_string())
    }
    fn save(&self, _pending: &PendingActivation) -> Result<(), String> {
        panic!("an unreadable store must refuse before anything is written");
    }
    fn clear(&self) {
        panic!("an unreadable store must refuse before anything is cleared");
    }
}

#[tokio::test]
async fn an_unreadable_nonce_store_refuses_before_minting() {
    let outcome = obtain_token("key-1", "inst-1", &UnreadableNonceStore).await;

    let error = outcome.expect_err("an unreadable store must refuse");
    assert!(matches!(error, ActivationError::NonceStoreUnreadable(_)));
    assert!(error.is_retryable(), "keychains unlock; this must retry");
}

#[tokio::test]
async fn a_matching_pending_attempt_is_replayed_not_replaced() {
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_some() {
        return;
    }
    let pending = PendingActivation::mint("key-1", "inst-1");
    let original_nonce = pending.nonce.clone();
    let store = MemoryNonceStore(std::sync::Mutex::new(Some(pending)));

    let outcome = obtain_token("key-1", "inst-1", &store).await;

    assert!(matches!(
        outcome,
        Err(ActivationError::NoEndpointConfigured)
    ));
    let kept = store
        .load()
        .unwrap()
        .expect("nonce must survive a retryable failure");
    assert_eq!(
        kept.nonce, original_nonce,
        "a retry must replay, not remint"
    );
}

#[tokio::test]
async fn a_pending_attempt_for_another_identity_is_discarded() {
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_some() {
        return;
    }
    let stale = PendingActivation::mint("old-key", "inst-1");
    let stale_nonce = stale.nonce.clone();
    let store = MemoryNonceStore(std::sync::Mutex::new(Some(stale)));

    let outcome = obtain_token("new-key", "inst-1", &store).await;

    assert!(matches!(
        outcome,
        Err(ActivationError::NoEndpointConfigured)
    ));
    let fresh = store
        .load()
        .unwrap()
        .expect("a fresh attempt must be persisted");
    assert_ne!(fresh.nonce, stale_nonce);
    assert!(fresh.is_for("new-key", "inst-1"));
}

#[test]
fn retryable_failures_keep_the_pending_nonce() {
    for outcome in [
        Err(ActivationError::Transport("connection reset".into())),
        Err(ActivationError::ServiceUnavailable),
        Err(ActivationError::NoEndpointConfigured),
    ] {
        assert_eq!(nonce_disposition(&outcome), NonceAction::Keep);
    }
}

#[test]
fn only_a_proven_absence_of_credential_clears_the_pending_nonce() {
    // Refused and cap-reached are the two answers that prove the service holds
    // nothing for this attempt, so the nonce is genuinely spent.
    for outcome in [
        Err(ActivationError::Refused {
            reason: "invalid_license".into(),
        }),
        Err(ActivationError::CredentialCapReached { active: 3, cap: 3 }),
    ] {
        assert_eq!(nonce_disposition(&outcome), NonceAction::Clear);
    }
}

#[test]
fn a_degraded_refusal_keeps_the_pending_nonce() {
    let degraded: Result<ActivationOutcome, ActivationError> = Err(ActivationError::Refused {
        reason: "refused".into(),
    });
    assert_eq!(nonce_disposition(&degraded), NonceAction::Keep);
}

#[test]
fn an_issued_credential_keeps_its_nonce_until_the_caller_stores_the_token() {
    let issued: Result<ActivationOutcome, ActivationError> = Ok(ActivationOutcome::Issued {
        token: "sitecmd_cat_x".into(),
        tier: "core".into(),
    });
    assert_eq!(nonce_disposition(&issued), NonceAction::Keep);
}

#[test]
fn ambiguous_answers_keep_the_pending_nonce() {
    for outcome in [
        Err(ActivationError::MalformedResponse("truncated".into())),
        Err(ActivationError::NonceNotPersisted("keychain locked".into())),
        Ok(ActivationOutcome::AlreadyActivated),
    ] {
        assert_eq!(nonce_disposition(&outcome), NonceAction::Keep);
    }
}

#[test]
fn every_server_error_status_is_retryable_not_malformed() {
    for status in [500, 502, 503, 504, 507, 599] {
        let parsed = parse_activate_response(status, "{}");
        assert!(
            matches!(parsed, Err(ActivationError::ServiceUnavailable)),
            "HTTP {status} must be retryable, got {parsed:?}"
        );
        assert!(parsed.unwrap_err().is_retryable());
    }
}

#[test]
fn a_server_error_with_a_non_json_body_is_still_retryable() {
    for status in [429, 500, 502, 504] {
        let parsed = parse_activate_response(status, "<html>gateway error</html>");
        assert!(
            matches!(parsed, Err(ActivationError::ServiceUnavailable)),
            "HTTP {status} with an HTML body must be retryable, got {parsed:?}"
        );
    }
}

#[test]
fn a_deactivate_outage_with_a_non_json_body_is_still_retryable() {
    for status in [429, 500, 502, 504] {
        let parsed = parse_deactivate_response(status, "<html>gateway error</html>");
        assert!(
            matches!(parsed, Err(ActivationError::ServiceUnavailable)),
            "HTTP {status} with an HTML body must be retryable, got {parsed:?}"
        );
    }
}

#[test]
fn only_the_services_own_refusal_codes_are_known() {
    for known in KNOWN_REFUSALS {
        assert!(known_refusal(known), "{known}");
    }
    for unknown in ["refused", "blocked", "forbidden", ""] {
        assert!(!known_refusal(unknown), "{unknown}");
    }
}

#[test]
fn a_middlebox_refusal_degrades_and_stays_unknown_end_to_end() {
    let parsed = parse_deactivate_response(400, r#"{ "error": "blocked by security policy" }"#);
    match parsed {
        Err(ActivationError::Refused { reason }) => {
            assert_eq!(reason, "refused");
            assert!(!known_refusal(&reason));
        }
        other => panic!("expected a degraded refusal, got {other:?}"),
    }
    // The service's own answer still settles.
    let parsed = parse_deactivate_response(400, r#"{ "error": "malformed_request" }"#);
    match parsed {
        Err(ActivationError::Refused { reason }) => assert!(known_refusal(&reason)),
        other => panic!("expected a known refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn the_connect_exchange_replays_its_own_pending_nonce() {
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_some() {
        return;
    }
    let pending = PendingActivation::mint("key-1", "inst-1");
    let original_nonce = pending.nonce.clone();
    let store = MemoryNonceStore(std::sync::Mutex::new(Some(pending)));

    let outcome = obtain_connect_token("key-1", "inst-1", &store).await;

    assert!(matches!(
        outcome,
        Err(ActivationError::NoEndpointConfigured)
    ));
    let kept = store
        .load()
        .unwrap()
        .expect("nonce must survive a retryable failure");
    assert_eq!(
        kept.nonce, original_nonce,
        "a connect retry must replay, not remint"
    );
}
