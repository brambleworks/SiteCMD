//! Verified-good baseline command tests.

use super::{describe, render_baseline, render_connected_baseline};
use crate::connected_baseline::{
    ConnectedBaselineField, ConnectedBaselineProfile, ConnectedBaselineSource,
};
use sitecmd_engine::profile::{
    CertificateIdentity, DnsPosture, FieldValue, Observation, OriginSet, RouteSet,
    SecurityHeaderProfile, VerifiedGoodProfile,
};

fn at(seconds: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_760_000_000 + seconds, 0).expect("timestamp")
}

fn headers(value: &str) -> FieldValue {
    let mut map = reqwest::header::HeaderMap::new();
    map.append(
        reqwest::header::HeaderName::from_static("content-security-policy"),
        reqwest::header::HeaderValue::from_str(value).expect("header value"),
    );
    FieldValue::SecurityHeaders(SecurityHeaderProfile::from_headers(&map))
}

fn profile_with(values: Vec<FieldValue>) -> VerifiedGoodProfile {
    VerifiedGoodProfile::default()
        .observe(
            &Observation {
                values,
                scan_id: Some(1),
            },
            at(0),
        )
        .profile
}

#[test]
fn a_site_with_no_baseline_renders_no_families() {
    let view = render_baseline(&VerifiedGoodProfile::default());

    assert_eq!(view.revision, 0);
    assert!(view.fields.is_empty());
}

#[test]
fn a_matching_site_renders_as_good_with_nothing_to_decide() {
    let view = render_baseline(&profile_with(vec![headers("default-src 'self'")]));

    let field = &view.fields[0];
    assert_eq!(field.field, "security_headers");
    assert_eq!(field.status, "good");
    assert!(field.change_digest.is_empty());
    assert!(field.changed_lines.is_empty());
    assert_eq!(
        field.good_lines,
        ["content-security-policy: default-src 'self'"]
    );
}

#[test]
fn connected_baseline_uses_good_and_drift_timestamps_for_their_actual_events() {
    let profile = ConnectedBaselineProfile {
        profile_revision: 4,
        fields: vec![ConnectedBaselineField {
            field: "security_headers".into(),
            good_digest: Some("good".into()),
            good_origin: Some("seeded".into()),
            recorded_at: Some("2026-08-19T12:00:00Z".into()),
            accepted_at: None,
            frozen: true,
            observed_digest: Some("changed".into()),
            observed_source: Some(ConnectedBaselineSource {
                source_observation_id: "scan-2".into(),
                deployment_ref: None,
                engine_release: "1.5.4".into(),
                contract_digest: "contract".into(),
                observed_at: Some("2026-08-20T14:00:00Z".into()),
            }),
            drift_first_seen_at: Some("2026-08-20T13:00:00Z".into()),
        }],
    };

    let field = &render_connected_baseline(&profile).fields[0];

    assert_eq!(
        field.recorded_at,
        chrono::DateTime::parse_from_rfc3339("2026-08-19T12:00:00Z")
            .expect("recorded timestamp")
            .timestamp_millis()
    );
    assert_eq!(
        field.change_first_seen_at,
        chrono::DateTime::parse_from_rfc3339("2026-08-20T13:00:00Z")
            .expect("drift timestamp")
            .timestamp_millis()
    );
}

#[test]
fn a_changed_family_renders_both_values_and_the_digest_to_decide_on() {
    let profile = profile_with(vec![headers("default-src 'self'")]);
    let drifted = profile
        .observe(
            &Observation {
                values: vec![headers("default-src *")],
                scan_id: Some(2),
            },
            at(60),
        )
        .profile;

    let view = render_baseline(&drifted);

    let field = &view.fields[0];
    assert_eq!(field.status, "changed");
    assert_eq!(
        field.good_lines,
        ["content-security-policy: default-src 'self'"]
    );
    assert_eq!(
        field.changed_lines,
        ["content-security-policy: default-src *"]
    );
    assert!(!field.change_digest.is_empty());
    assert_eq!(field.change_first_seen_at, at(60).timestamp_millis());
}

#[test]
fn a_silenced_change_still_renders_its_value_so_it_can_be_revisited() {
    let profile = profile_with(vec![headers("default-src 'self'")]);
    let drifted = profile
        .observe(
            &Observation {
                values: vec![headers("default-src *")],
                scan_id: Some(2),
            },
            at(60),
        )
        .profile;
    let digest = drifted.open_drift()[0].1.digest.clone();
    let silenced = drifted
        .dismiss(
            sitecmd_engine::profile::ProfileField::SecurityHeaders,
            drifted.revision,
            &digest,
        )
        .expect("dismissed")
        .profile;

    let field = &render_baseline(&silenced).fields[0];

    assert_eq!(field.status, "silenced");
    assert_eq!(
        field.changed_lines,
        ["content-security-policy: default-src *"]
    );
}

#[test]
fn families_render_in_their_declared_order_not_storage_order() {
    let view = render_baseline(&profile_with(vec![
        FieldValue::RouteSet(RouteSet::new(["/".to_string()])),
        FieldValue::ThirdPartyOrigins(OriginSet::from_origins(["https://cdn.test".to_string()])),
        headers("default-src 'self'"),
    ]));

    let order: Vec<&str> = view.fields.iter().map(|f| f.field.as_str()).collect();
    assert_eq!(
        order,
        ["security_headers", "third_party_origins", "route_set"]
    );
}

#[test]
fn a_bounded_value_never_renders_as_though_it_were_complete() {
    let origins =
        OriginSet::from_origins((0..200).map(|index| format!("https://host{index:03}.test")));

    let lines = describe(&FieldValue::ThirdPartyOrigins(origins));

    assert!(lines
        .last()
        .expect("lines")
        .starts_with("and 72 more origins"));
}

#[test]
fn a_dns_posture_says_what_it_found_and_what_it_did_not() {
    let lines = describe(&FieldValue::DnsPosture(DnsPosture::new(
        ["mail.example.test".to_string()],
        Some("example.test.cdn.test".to_string()),
        false,
        ["v=spf1 -all".to_string()],
    )));

    assert_eq!(
        lines,
        [
            "Mail exchange mail.example.test",
            "www points at example.test.cdn.test",
            "No certificate authority records",
            "SPF v=spf1 -all",
        ]
    );
}

#[test]
fn a_certificate_renders_its_issuer_before_the_names_it_covers() {
    let identity =
        CertificateIdentity::from_tls_facts(&sitecmd_engine::checks::security::tls::TlsFacts {
            not_before: None,
            not_after: None,
            issuer: Some("Example CA".into()),
            subject_names: vec!["example.test".into(), "www.example.test".into()],
            protocol: None,
            validation: sitecmd_engine::checks::security::tls::TlsValidation::valid(
                sitecmd_engine::checks::security::tls::TrustAuthority::Webpki,
            ),
            facts_observed_at: at(0),
        })
        .expect("identity");

    let lines = describe(&FieldValue::Certificate(identity));

    assert_eq!(
        lines,
        [
            "Issued by Example CA",
            "Covers example.test",
            "Covers www.example.test",
        ]
    );
}
