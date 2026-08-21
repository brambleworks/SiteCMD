use super::*;
use crate::probe::{ProbeBody, ProbeFailure, ProbeFailureClass, ProbeResponse};

#[test]
fn parses_expiration_event_from_rdap_response() {
    let body = serde_json::json!({
        "objectClassName": "domain",
        "events": [
            {"eventAction": "registration", "eventDate": "2020-01-15T04:00:00Z"},
            {"eventAction": "expiration", "eventDate": "2027-01-15T04:00:00Z"},
            {"eventAction": "last changed", "eventDate": "2026-01-10T09:30:00Z"}
        ]
    });
    let expiry = parse_rdap_expiration(&body).expect("expiration event");
    assert_eq!(expiry.to_rfc3339(), "2027-01-15T04:00:00+00:00");
}

#[test]
fn parses_expiration_with_numeric_utc_offset() {
    let body = serde_json::json!({
        "events": [{"eventAction": "expiration", "eventDate": "2027-06-30T23:59:59+02:00"}]
    });
    let expiry = parse_rdap_expiration(&body).expect("expiration event");
    assert_eq!(expiry.to_rfc3339(), "2027-06-30T21:59:59+00:00");
}

#[test]
fn missing_events_array_yields_none() {
    assert!(parse_rdap_expiration(&serde_json::json!({"objectClassName": "domain"})).is_none());
}

#[test]
fn missing_expiration_action_yields_none() {
    let body = serde_json::json!({
        "events": [{"eventAction": "registration", "eventDate": "2020-01-15T04:00:00Z"}]
    });
    assert!(parse_rdap_expiration(&body).is_none());
}

#[test]
fn malformed_event_date_yields_none() {
    let body = serde_json::json!({
        "events": [{"eventAction": "expiration", "eventDate": "soon"}]
    });
    assert!(parse_rdap_expiration(&body).is_none());
}

#[test]
fn events_missing_fields_are_skipped_not_fatal() {
    let body = serde_json::json!({
        "events": [
            {"eventAction": "registration"},
            {"eventDate": "2026-01-01T00:00:00Z"},
            {"eventAction": "expiration", "eventDate": "2028-03-01T00:00:00Z"}
        ]
    });
    let expiry = parse_rdap_expiration(&body).expect("expiration event");
    assert_eq!(expiry.to_rfc3339(), "2028-03-01T00:00:00+00:00");
}

#[test]
fn past_rdap_expiration_date_warns_high_for_immediate_review() {
    for days in [-30, -1] {
        let verdict = classify_expiry(days);
        assert_eq!(verdict.status, CheckStatus::Warn, "days={}", days);
        assert_eq!(verdict.severity, Severity::High, "days={}", days);
    }
}

#[test]
fn expiry_today_through_seven_days_warns_high() {
    for days in [0, 1, 7] {
        let verdict = classify_expiry(days);
        assert_eq!(verdict.status, CheckStatus::Warn, "days={}", days);
        assert_eq!(verdict.severity, Severity::High, "days={}", days);
    }
}

#[test]
fn expiry_eight_through_thirty_days_warns_medium() {
    for days in [8, 14, 30] {
        let verdict = classify_expiry(days);
        assert_eq!(verdict.status, CheckStatus::Warn, "days={}", days);
        assert_eq!(verdict.severity, Severity::Medium, "days={}", days);
    }
}

#[test]
fn expiry_thirty_one_through_ninety_days_warns_low() {
    for days in [31, 60, 90] {
        let verdict = classify_expiry(days);
        assert_eq!(verdict.status, CheckStatus::Warn, "days={}", days);
        assert_eq!(verdict.severity, Severity::Low, "days={}", days);
    }
}

#[test]
fn distant_expiry_passes() {
    for days in [91, 365, 3650] {
        let verdict = classify_expiry(days);
        assert_eq!(verdict.status, CheckStatus::Pass, "days={}", days);
    }
}

#[test]
fn expiry_copy_handles_today_and_singular_days() {
    assert_eq!(expiry_title(0), "Domain registration expires today");
    assert_eq!(expiry_title(1), "Domain registration expires in 1 day");
    assert_eq!(expiry_title(2), "Domain registration expires in 2 days");
    assert_eq!(expiry_window_phrase(0), "today");
    assert_eq!(expiry_window_phrase(1), "in 1 day");
    assert_eq!(expiry_window_phrase(2), "in 2 days");
}

fn evaluation_time() -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
        .expect("static test time")
        .with_timezone(&Utc)
}

fn rdap_response(body: &str) -> ProbeOutcome {
    ProbeOutcome::Response(ProbeResponse {
        status: 200,
        final_url: "https://rdap.example-registry.net/domain/example.com".into(),
        content_type: Some("application/rdap+json".into()),
        content_length: None,
        headers: Vec::new(),
        body: Some(ProbeBody {
            text: body.to_string(),
            bytes: body.len(),
            utf8_valid: true,
        }),
    })
}

#[test]
fn the_planned_probe_carries_the_rdap_accept_header() {
    let request = rdap_probe("example.com");
    assert_eq!(request.url, "https://rdap.org/domain/example.com");
    assert!(request
        .headers
        .iter()
        .any(|(name, value)| name == "Accept" && value == "application/rdap+json"));
}

#[test]
fn a_distant_expiration_passes_from_the_injected_clock() {
    let body =
        r#"{"events": [{"eventAction": "expiration", "eventDate": "2027-08-05T00:00:00Z"}]}"#;
    let results = evaluate_rdap("example.com", &rdap_response(body), evaluation_time());
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert_eq!(
        results[0].raw_data.as_ref().unwrap()["days_until_expiry"],
        365
    );
}

#[test]
fn registry_infrastructure_problems_always_skip() {
    let failure = ProbeOutcome::Failure(ProbeFailure {
        class: ProbeFailureClass::Transport,
        detail: "connection refused".into(),
    });
    let not_found = ProbeOutcome::Response(ProbeResponse {
        status: 404,
        final_url: "https://rdap.org/domain/example.com".into(),
        content_type: None,
        content_length: None,
        headers: Vec::new(),
        body: None,
    });
    for outcome in [failure, not_found, rdap_response("not json")] {
        let results = evaluate_rdap("example.com", &outcome, evaluation_time());
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["reason"],
            "rdap_unavailable"
        );
    }
}

#[test]
fn a_missing_expiration_event_skips() {
    let body =
        r#"{"events": [{"eventAction": "registration", "eventDate": "2020-01-15T04:00:00Z"}]}"#;
    let results = evaluate_rdap("example.com", &rdap_response(body), evaluation_time());
    assert_eq!(results[0].status, CheckStatus::Skipped);
    assert!(results[0]
        .description
        .contains("did not publish an expiration event"));
}
