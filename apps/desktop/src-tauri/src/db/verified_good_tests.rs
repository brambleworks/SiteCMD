//! Verified-good persistence tests.

use super::{BaselineDecision, BaselineDecisionOutcome};
use crate::db::test_helpers::{temp_db, TestDb};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use sitecmd_engine::profile::{
    FieldValue, Observation, OriginSet, ProfileField, RecordOrigin, SecurityHeaderProfile,
};

fn at(seconds: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_760_000_000 + seconds, 0).expect("timestamp")
}

fn headers(value: &str) -> FieldValue {
    let mut map = HeaderMap::new();
    map.append(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_str(value).expect("header value"),
    );
    FieldValue::SecurityHeaders(SecurityHeaderProfile::from_headers(&map))
}

fn observation(values: Vec<FieldValue>) -> Observation {
    Observation {
        values,
        scan_id: Some(42),
    }
}

#[test]
fn a_site_with_no_observations_has_an_empty_profile_at_revision_zero() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");

    let profile = db.get_verified_good_profile(site_id).expect("profile");

    assert_eq!(profile.revision, 0);
    assert!(profile.fields.is_empty());
}

#[test]
fn an_observation_seeds_and_survives_a_reread() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");

    db.apply_verified_good_observation(site_id, observation(vec![headers("DENY")]), at(0))
        .expect("apply");

    let profile = db.get_verified_good_profile(site_id).expect("profile");
    assert_eq!(profile.revision, 1);
    let state = &profile.fields[&ProfileField::SecurityHeaders];
    assert_eq!(state.good.origin, RecordOrigin::Seeded);
    assert_eq!(state.good.recorded_at, at(0));
    assert_eq!(state.good.source_scan_id, Some(42));
    assert!(state.drift.is_none());
}

#[test]
fn an_unchanged_observation_leaves_the_revision_alone() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    db.apply_verified_good_observation(site_id, observation(vec![headers("DENY")]), at(0))
        .expect("apply");

    db.apply_verified_good_observation(site_id, observation(vec![headers("DENY")]), at(60))
        .expect("apply");

    assert_eq!(
        db.get_verified_good_profile(site_id)
            .expect("profile")
            .revision,
        1
    );
}

#[test]
fn a_changed_value_is_stored_beside_good_rather_than_over_it() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    db.apply_verified_good_observation(site_id, observation(vec![headers("DENY")]), at(0))
        .expect("apply");

    db.apply_verified_good_observation(site_id, observation(vec![headers("SAMEORIGIN")]), at(60))
        .expect("apply");

    let profile = db.get_verified_good_profile(site_id).expect("profile");
    let state = &profile.fields[&ProfileField::SecurityHeaders];
    assert_eq!(state.good.value, headers("DENY"));
    let drift = state.drift.as_ref().expect("drift stored");
    assert_eq!(drift.value, headers("SAMEORIGIN"));
    assert_eq!(drift.first_seen_at, at(60));
    assert!(!drift.dismissed);
}

fn drifted_site() -> (TestDb, i64, String, u64) {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    db.apply_verified_good_observation(site_id, observation(vec![headers("DENY")]), at(0))
        .expect("apply");
    db.apply_verified_good_observation(site_id, observation(vec![headers("SAMEORIGIN")]), at(60))
        .expect("apply");
    let profile = db.get_verified_good_profile(site_id).expect("profile");
    let digest = profile.fields[&ProfileField::SecurityHeaders]
        .drift
        .as_ref()
        .expect("drift")
        .digest
        .clone();
    let revision = profile.revision;
    (db, site_id, digest, revision)
}

#[test]
fn accepting_moves_the_baseline_and_records_who_moved_it() {
    let (db, site_id, digest, revision) = drifted_site();

    let outcome = db
        .decide_verified_good(
            site_id,
            ProfileField::SecurityHeaders,
            revision,
            digest,
            BaselineDecision::Accept,
            at(300),
        )
        .expect("decide");

    assert!(matches!(
        outcome,
        BaselineDecisionOutcome::Applied { revision: 3 }
    ));
    let state = &db
        .get_verified_good_profile(site_id)
        .expect("profile")
        .fields[&ProfileField::SecurityHeaders];
    assert_eq!(state.good.value, headers("SAMEORIGIN"));
    assert_eq!(state.good.origin, RecordOrigin::Accepted);
    assert_eq!(state.good.recorded_at, at(300));
    assert!(state.drift.is_none());
}

#[test]
fn dismissing_silences_the_change_without_moving_the_baseline() {
    let (db, site_id, digest, revision) = drifted_site();

    db.decide_verified_good(
        site_id,
        ProfileField::SecurityHeaders,
        revision,
        digest,
        BaselineDecision::Dismiss,
        at(300),
    )
    .expect("decide");

    let profile = db.get_verified_good_profile(site_id).expect("profile");
    let state = &profile.fields[&ProfileField::SecurityHeaders];
    assert_eq!(
        state.good.value,
        headers("DENY"),
        "dismissal is not acceptance"
    );
    assert!(state.drift.as_ref().expect("drift kept").dismissed);
    assert!(profile.open_drift().is_empty());
}

#[test]
fn a_decision_taken_against_a_stale_revision_is_refused_not_applied() {
    let (db, site_id, digest, revision) = drifted_site();

    let outcome = db
        .decide_verified_good(
            site_id,
            ProfileField::SecurityHeaders,
            revision - 1,
            digest,
            BaselineDecision::Accept,
            at(300),
        )
        .expect("decide");

    match outcome {
        BaselineDecisionOutcome::Refused(error) => assert_eq!(error.code(), "stale_revision"),
        other => panic!("expected refusal, got {other:?}"),
    }
    let state = &db
        .get_verified_good_profile(site_id)
        .expect("profile")
        .fields[&ProfileField::SecurityHeaders];
    assert_eq!(state.good.value, headers("DENY"));
}

#[test]
fn a_row_this_build_cannot_read_re_seeds_instead_of_being_guessed_at() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    db.apply_verified_good_observation(site_id, observation(vec![headers("DENY")]), at(0))
        .expect("apply");
    db.execute(move |conn| {
        conn.execute(
            "UPDATE site_verified_good SET field = 'invented_family' WHERE site_id = ?1",
            rusqlite::params![site_id],
        )
        .map(|_| ())
    })
    .expect("worker")
    .expect("rename");

    let profile = db.get_verified_good_profile(site_id).expect("profile");

    assert!(profile.fields.is_empty());
}

#[test]
fn separate_families_are_stored_side_by_side() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");

    db.apply_verified_good_observation(
        site_id,
        observation(vec![
            headers("DENY"),
            FieldValue::ThirdPartyOrigins(OriginSet::from_origins(
                ["https://cdn.test".to_string()],
            )),
        ]),
        at(0),
    )
    .expect("apply");

    let profile = db.get_verified_good_profile(site_id).expect("profile");
    assert_eq!(profile.fields.len(), 2);
    assert!(profile
        .fields
        .contains_key(&ProfileField::ThirdPartyOrigins));
}
