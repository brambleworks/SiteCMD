use super::*;
use crate::checks::security::tls::TlsFacts;

fn at(seconds: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_760_000_000 + seconds, 0).expect("timestamp")
}

fn headers(pairs: &[(&str, &str)]) -> http::HeaderMap {
    let mut map = http::HeaderMap::new();
    for (name, value) in pairs {
        map.append(
            http::header::HeaderName::from_bytes(name.as_bytes()).expect("header name"),
            http::HeaderValue::from_str(value).expect("header value"),
        );
    }
    map
}

fn header_value(pairs: &[(&str, &str)]) -> FieldValue {
    FieldValue::SecurityHeaders(SecurityHeaderProfile::from_headers(&headers(pairs)))
}

fn origins(values: &[&str]) -> FieldValue {
    FieldValue::ThirdPartyOrigins(OriginSet::from_origins(
        values.iter().map(|value| (*value).to_string()),
    ))
}

fn observation(values: Vec<FieldValue>, scan_id: i64) -> Observation {
    Observation {
        values,
        scan_id: Some(scan_id),
    }
}

fn seeded(value: FieldValue) -> VerifiedGoodProfile {
    VerifiedGoodProfile::default()
        .observe(&observation(vec![value], 1), at(0))
        .profile
}

#[test]
fn the_first_observation_of_a_field_seeds_it() {
    let update = VerifiedGoodProfile::default()
        .observe(&observation(vec![origins(&["https://cdn.test"])], 7), at(0));

    assert!(update.changed);
    assert_eq!(update.profile.revision, 1);
    assert_eq!(
        update.transitions,
        vec![(ProfileField::ThirdPartyOrigins, FieldTransition::Seeded)]
    );
    let state = &update.profile.fields[&ProfileField::ThirdPartyOrigins];
    assert_eq!(state.good.origin, RecordOrigin::Seeded);
    assert_eq!(state.good.source_scan_id, Some(7));
    assert!(state.drift.is_none());
}

#[test]
fn an_unchanged_observation_does_not_burn_a_revision() {
    let profile = seeded(header_value(&[("x-frame-options", "DENY")]));

    let update = profile.observe(
        &observation(vec![header_value(&[("x-frame-options", "DENY")])], 2),
        at(60),
    );

    assert!(!update.changed);
    assert_eq!(update.profile.revision, profile.revision);
    assert_eq!(
        update.transitions,
        vec![(ProfileField::SecurityHeaders, FieldTransition::Unchanged)]
    );
}

#[test]
fn a_changed_value_freezes_good_and_becomes_the_finding() {
    let profile = seeded(header_value(&[("x-frame-options", "DENY")]));
    let good_digest = profile.fields[&ProfileField::SecurityHeaders]
        .good
        .digest
        .clone();

    let update = profile.observe(
        &observation(vec![header_value(&[("x-frame-options", "SAMEORIGIN")])], 2),
        at(60),
    );

    let state = &update.profile.fields[&ProfileField::SecurityHeaders];
    assert_eq!(
        state.good.digest, good_digest,
        "drift must never overwrite good"
    );
    let drift = state.drift.as_ref().expect("drift recorded");
    assert_eq!(drift.first_seen_at, at(60));
    assert!(!drift.dismissed);
    assert_eq!(
        update.transitions,
        vec![(ProfileField::SecurityHeaders, FieldTransition::DriftOpened)]
    );
}

#[test]
fn the_same_difference_seen_again_only_moves_last_seen() {
    let profile = seeded(header_value(&[("x-frame-options", "DENY")]));
    let drifted = profile
        .observe(
            &observation(vec![header_value(&[("x-frame-options", "SAMEORIGIN")])], 2),
            at(60),
        )
        .profile;

    let update = drifted.observe(
        &observation(vec![header_value(&[("x-frame-options", "SAMEORIGIN")])], 3),
        at(120),
    );

    let drift = update.profile.fields[&ProfileField::SecurityHeaders]
        .drift
        .as_ref()
        .expect("drift still open");
    assert_eq!(drift.first_seen_at, at(60));
    assert_eq!(drift.last_seen_at, at(120));
    assert_eq!(
        update.transitions,
        vec![(
            ProfileField::SecurityHeaders,
            FieldTransition::DriftPersisted
        )]
    );
}

#[test]
fn a_dismissal_does_not_carry_to_a_different_difference() {
    let profile = seeded(header_value(&[("x-frame-options", "DENY")]));
    let drifted = profile
        .observe(
            &observation(vec![header_value(&[("x-frame-options", "SAMEORIGIN")])], 2),
            at(60),
        )
        .profile;
    let digest = drifted.fields[&ProfileField::SecurityHeaders]
        .drift
        .as_ref()
        .expect("drift")
        .digest
        .clone();
    let dismissed = drifted
        .dismiss(ProfileField::SecurityHeaders, drifted.revision, &digest)
        .expect("dismiss")
        .profile;

    let update = dismissed.observe(&observation(vec![header_value(&[])], 4), at(180));

    let drift = update.profile.fields[&ProfileField::SecurityHeaders]
        .drift
        .as_ref()
        .expect("new drift");
    assert!(!drift.dismissed, "a new difference is not pre-silenced");
    assert_eq!(
        update.transitions,
        vec![(ProfileField::SecurityHeaders, FieldTransition::DriftMoved)]
    );
}

#[test]
fn a_value_that_comes_back_re_establishes_good_without_acceptance() {
    let profile = seeded(header_value(&[("x-frame-options", "DENY")]));
    let drifted = profile
        .observe(
            &observation(vec![header_value(&[("x-frame-options", "SAMEORIGIN")])], 2),
            at(60),
        )
        .profile;

    let update = drifted.observe(
        &observation(vec![header_value(&[("x-frame-options", "DENY")])], 3),
        at(120),
    );

    let state = &update.profile.fields[&ProfileField::SecurityHeaders];
    assert!(state.drift.is_none());
    assert_eq!(state.good.origin, RecordOrigin::Promoted);
    assert_eq!(state.good.source_scan_id, Some(3));
    assert_eq!(
        update.transitions,
        vec![(ProfileField::SecurityHeaders, FieldTransition::Recovered)]
    );
}

#[test]
fn a_projection_change_re_seeds_instead_of_inventing_drift() {
    let mut profile = seeded(header_value(&[("x-frame-options", "DENY")]));
    profile
        .fields
        .get_mut(&ProfileField::SecurityHeaders)
        .expect("field")
        .good
        .profile_version = PROFILE_VERSION - 1;

    let update = profile.observe(
        &observation(vec![header_value(&[("x-frame-options", "SAMEORIGIN")])], 2),
        at(60),
    );

    let state = &update.profile.fields[&ProfileField::SecurityHeaders];
    assert_eq!(state.good.origin, RecordOrigin::Reseeded);
    assert!(state.drift.is_none(), "a detector change is not site drift");
    assert_eq!(
        update.transitions,
        vec![(ProfileField::SecurityHeaders, FieldTransition::Reseeded)]
    );
}

#[test]
fn a_family_the_run_did_not_observe_is_left_alone() {
    let profile = VerifiedGoodProfile::default()
        .observe(
            &observation(
                vec![
                    header_value(&[("x-frame-options", "DENY")]),
                    origins(&["https://cdn.test"]),
                ],
                1,
            ),
            at(0),
        )
        .profile;

    let update = profile.observe(
        &observation(vec![header_value(&[("x-frame-options", "DENY")])], 2),
        at(60),
    );

    assert!(!update.changed);
    assert!(update
        .profile
        .fields
        .contains_key(&ProfileField::ThirdPartyOrigins));
}

fn drifted_profile() -> (VerifiedGoodProfile, String) {
    let profile = seeded(header_value(&[("x-frame-options", "DENY")]));
    let drifted = profile
        .observe(
            &observation(vec![header_value(&[("x-frame-options", "SAMEORIGIN")])], 2),
            at(60),
        )
        .profile;
    let digest = drifted.fields[&ProfileField::SecurityHeaders]
        .drift
        .as_ref()
        .expect("drift")
        .digest
        .clone();
    (drifted, digest)
}

#[test]
fn accepting_moves_good_with_acceptance_provenance() {
    let (profile, digest) = drifted_profile();

    let update = profile
        .accept(
            ProfileField::SecurityHeaders,
            profile.revision,
            &digest,
            at(300),
        )
        .expect("accepted");

    let state = &update.profile.fields[&ProfileField::SecurityHeaders];
    assert_eq!(state.good.digest, digest);
    assert_eq!(state.good.origin, RecordOrigin::Accepted);
    assert_eq!(state.good.recorded_at, at(300));
    assert!(state.drift.is_none());
    assert_eq!(update.profile.revision, profile.revision + 1);
}

#[test]
fn dismissing_silences_without_moving_good() {
    let (profile, digest) = drifted_profile();
    let good_digest = profile.fields[&ProfileField::SecurityHeaders]
        .good
        .digest
        .clone();

    let update = profile
        .dismiss(ProfileField::SecurityHeaders, profile.revision, &digest)
        .expect("dismissed");

    let state = &update.profile.fields[&ProfileField::SecurityHeaders];
    assert_eq!(
        state.good.digest, good_digest,
        "dismissal is not acceptance"
    );
    assert!(state.drift.as_ref().expect("drift kept").dismissed);
    assert!(update.profile.open_drift().is_empty());
}

#[test]
fn a_stale_revision_refuses_the_acceptance() {
    let (profile, digest) = drifted_profile();

    let error = profile
        .accept(
            ProfileField::SecurityHeaders,
            profile.revision - 1,
            &digest,
            at(300),
        )
        .expect_err("stale");

    assert_eq!(error.code(), "stale_revision");
    assert_eq!(
        error,
        DecisionError::StaleRevision {
            current_revision: profile.revision,
            current_digest: Some(digest),
        }
    );
}

#[test]
fn a_digest_the_person_never_saw_refuses_the_acceptance() {
    let (profile, _) = drifted_profile();

    let error = profile
        .accept(
            ProfileField::SecurityHeaders,
            profile.revision,
            "0000000000000000",
            at(300),
        )
        .expect_err("stale");

    assert_eq!(error.code(), "stale_revision");
}

#[test]
fn a_field_with_nothing_to_decide_refuses_both_decisions() {
    let profile = seeded(header_value(&[("x-frame-options", "DENY")]));

    assert_eq!(
        profile
            .accept(ProfileField::SecurityHeaders, profile.revision, "x", at(1))
            .expect_err("no drift"),
        DecisionError::NoDrift
    );
    assert_eq!(
        profile
            .dismiss(ProfileField::DnsPosture, profile.revision, "x")
            .expect_err("no field"),
        DecisionError::NoDrift
    );
}

#[test]
fn only_allowlisted_headers_can_ride_a_profile() {
    let profile = SecurityHeaderProfile::from_headers(&headers(&[
        ("set-cookie", "session=secret; HttpOnly"),
        ("authorization", "Bearer token"),
        ("x-frame-options", "DENY"),
        ("server", "nginx"),
    ]));

    assert_eq!(
        profile.headers.keys().collect::<Vec<_>>(),
        ["x-frame-options"]
    );
    let serialized = serde_json::to_string(&profile).expect("serializes");
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("Bearer"));
}

#[test]
fn multiple_field_lines_stay_separate() {
    let profile = SecurityHeaderProfile::from_headers(&headers(&[
        ("content-security-policy", "default-src 'self'"),
        ("content-security-policy", "frame-ancestors 'none'"),
    ]));

    let lines = &profile.headers["content-security-policy"];
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].value, "default-src 'self'");
    assert_eq!(lines[1].value, "frame-ancestors 'none'");
}

#[test]
fn whitespace_mangling_canonicalizes_to_one_value() {
    let spaced = SecurityHeaderProfile::from_headers(&headers(&[(
        "content-security-policy",
        "  default-src   'self';   img-src *  ",
    )]));
    let tight = SecurityHeaderProfile::from_headers(&headers(&[(
        "content-security-policy",
        "default-src 'self'; img-src *",
    )]));

    assert_eq!(spaced, tight);
}

#[test]
fn an_oversize_field_line_truncates_with_its_marker() {
    let long = "a".repeat(MAX_HEADER_VALUE_BYTES + 50);
    let profile =
        SecurityHeaderProfile::from_headers(&headers(&[("permissions-policy", long.as_str())]));

    let line = &profile.headers["permissions-policy"][0];
    assert_eq!(line.value.len(), MAX_HEADER_VALUE_BYTES);
    assert!(line.truncated);
}

#[test]
fn an_absent_header_is_an_absent_key() {
    let profile = SecurityHeaderProfile::from_headers(&headers(&[("x-frame-options", "DENY")]));

    assert!(!profile.headers.contains_key("content-security-policy"));
}

#[test]
fn a_thinner_observation_is_not_a_removal() {
    let profile = seeded(origins(&["https://a.test", "https://b.test"]));

    let update = profile.observe(&observation(vec![origins(&["https://a.test"])], 2), at(60));

    assert!(!update.changed, "partial coverage is not drift");
}

#[test]
fn a_thinner_observation_does_not_clear_an_open_growth_only_drift() {
    let profile = seeded(origins(&["https://a.test"]));
    let drifted = profile
        .observe(
            &observation(vec![origins(&["https://a.test", "https://b.test"])], 2),
            at(60),
        )
        .profile;

    let update = drifted.observe(&observation(vec![origins(&["https://a.test"])], 3), at(120));

    assert!(
        update.profile.fields[&ProfileField::ThirdPartyOrigins]
            .drift
            .is_some(),
        "thinner coverage cannot prove the added origin recovered"
    );
    assert!(!update.changed);
}

#[test]
fn a_new_origin_drifts_and_records_the_union() {
    let profile = seeded(origins(&["https://a.test"]));

    let update = profile.observe(&observation(vec![origins(&["https://b.test"])], 2), at(60));

    let drift = update.profile.fields[&ProfileField::ThirdPartyOrigins]
        .drift
        .as_ref()
        .expect("drift");
    let FieldValue::ThirdPartyOrigins(set) = &drift.value else {
        panic!("origin drift");
    };
    assert_eq!(set.origins.values, ["https://a.test", "https://b.test"]);
}

#[test]
fn a_new_route_drifts_and_an_unchanged_route_set_does_not() {
    let profile = seeded(FieldValue::RouteSet(RouteSet::new([
        "/".to_string(),
        "/about".to_string(),
    ])));

    let quiet = profile.observe(
        &observation(
            vec![FieldValue::RouteSet(RouteSet::new(["/".to_string()]))],
            2,
        ),
        at(60),
    );
    let noisy = profile.observe(
        &observation(
            vec![FieldValue::RouteSet(RouteSet::new([
                "/".to_string(),
                "/admin".to_string(),
            ]))],
            3,
        ),
        at(120),
    );

    assert!(!quiet.changed);
    assert_eq!(
        noisy.transitions,
        vec![(ProfileField::RouteSet, FieldTransition::DriftOpened)]
    );
}

#[test]
fn an_overflowed_set_never_claims_containment() {
    let set = BoundedSet::new(
        (0..MAX_ORIGINS + 5).map(|i| format!("https://{i}.test")),
        MAX_ORIGINS,
    );

    assert_eq!(set.overflow, 5);
    assert!(
        !set.is_subset_of(&set),
        "a bound narrows what a record proves"
    );
}

fn tls_facts(names: &[&str], issuer: Option<&str>, not_after_days: i64) -> TlsFacts {
    TlsFacts {
        not_before: Some(at(0)),
        not_after: Some(at(not_after_days * 86_400)),
        issuer: issuer.map(str::to_string),
        subject_names: names.iter().map(|name| (*name).to_string()).collect(),
        protocol: Some("TLSv1.3".into()),
        validation: crate::checks::security::tls::TlsValidation::valid(
            crate::checks::security::tls::TrustAuthority::Webpki,
        ),
        facts_observed_at: at(0),
    }
}

#[test]
fn a_renewal_is_not_a_certificate_change() {
    let before = CertificateIdentity::from_tls_facts(&tls_facts(
        &["example.test", "www.example.test"],
        Some("Let's Encrypt R3"),
        90,
    ))
    .expect("identity");
    let after = CertificateIdentity::from_tls_facts(&tls_facts(
        &["www.example.test", "example.test"],
        Some("Let's Encrypt R3"),
        180,
    ))
    .expect("identity");

    assert_eq!(before, after);
}

#[test]
fn a_new_issuer_is_a_certificate_change() {
    let profile = seeded(FieldValue::Certificate(
        CertificateIdentity::from_tls_facts(&tls_facts(&["example.test"], Some("Issuer A"), 90))
            .expect("identity"),
    ));

    let update = profile.observe(
        &observation(
            vec![FieldValue::Certificate(
                CertificateIdentity::from_tls_facts(&tls_facts(
                    &["example.test"],
                    Some("Issuer B"),
                    90,
                ))
                .expect("identity"),
            )],
            2,
        ),
        at(60),
    );

    assert_eq!(
        update.transitions,
        vec![(ProfileField::Certificate, FieldTransition::DriftOpened)]
    );
}

#[test]
fn facts_without_any_identity_produce_no_certificate_field() {
    assert!(CertificateIdentity::from_tls_facts(&tls_facts(&[], None, 90)).is_none());
}

#[test]
fn arbitrary_txt_records_never_ride_a_dns_posture() {
    let posture = DnsPosture::new(
        ["mail.example.test".to_string()],
        None,
        true,
        [
            "google-site-verification=SUPERSECRET".to_string(),
            "v=spf1 include:_spf.example.test ~all".to_string(),
            "v=DMARC1; p=reject".to_string(),
        ],
    );

    assert_eq!(
        posture.spf.as_deref(),
        Some("v=spf1 include:_spf.example.test ~all")
    );
    assert_eq!(posture.dmarc.as_deref(), Some("v=DMARC1; p=reject"));
    let serialized = serde_json::to_string(&posture).expect("serializes");
    assert!(!serialized.contains("SUPERSECRET"));
}

#[test]
fn a_document_contributes_loaded_origins_and_not_outbound_links() {
    let body = r#"<html><head>
        <script src="https://cdn.test/app.js"></script>
        <link rel="stylesheet" href="https://fonts.test/style.css">
        </head><body>
        <a href="https://news.test/story">read</a>
        <img src="/local.png">
        <iframe src="https://embed.test/frame"></iframe>
        </body></html>"#;
    let page = url::Url::parse("https://example.test/page").expect("url");

    let set = OriginSet::from_document(&page, body, &body.to_ascii_lowercase());

    assert_eq!(
        set.origins.values,
        [
            "https://cdn.test",
            "https://embed.test",
            "https://fonts.test"
        ]
    );
}

#[test]
fn a_digest_is_stable_and_distinguishes_values() {
    let value = header_value(&[("x-frame-options", "DENY")]);
    let other = header_value(&[("x-frame-options", "SAMEORIGIN")]);

    assert_eq!(value.digest(), value.clone().digest());
    assert_ne!(value.digest(), other.digest());
    assert_eq!(value.digest().len(), 16);
}

#[test]
fn every_field_round_trips_through_its_stored_key() {
    for field in ProfileField::ALL {
        assert_eq!(ProfileField::parse(field.as_str()), Some(*field));
    }
    assert_eq!(ProfileField::parse("invented"), None);
    for origin in [
        RecordOrigin::Seeded,
        RecordOrigin::Promoted,
        RecordOrigin::Accepted,
        RecordOrigin::Reseeded,
    ] {
        assert_eq!(RecordOrigin::parse(origin.as_str()), Some(origin));
    }
}

#[test]
fn a_union_of_overflowed_sets_reports_a_lower_bound_not_a_sum() {
    let left = BoundedSet::new(
        (0..MAX_ORIGINS + 5).map(|i| format!("https://a{i}.test")),
        MAX_ORIGINS,
    );
    let right = BoundedSet::new(
        (0..MAX_ORIGINS + 9).map(|i| format!("https://b{i}.test")),
        MAX_ORIGINS,
    );

    let union = left.union(&right, MAX_ORIGINS);

    assert_eq!(union.values.len(), MAX_ORIGINS);
    assert_eq!(union.overflow, MAX_ORIGINS + 9);
}
