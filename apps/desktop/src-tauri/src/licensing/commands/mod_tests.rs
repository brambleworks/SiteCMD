//! Tests for the shared licensing command helpers.

use super::*;
use crate::licensing::config::OFFLINE_GRACE_PERIOD_SECS;
use chrono::{Duration, TimeZone, Utc};

fn fixed_state(status: &str, tier: Tier, last_validated_at: &str) -> LicenseState {
    LicenseState {
        license_key: "K".into(),
        instance_id: "inst-1".into(),
        variant_id: 901,
        tier,
        status: status.into(),
        last_validated_at: last_validated_at.into(),
        activated_at: "2026-01-01T00:00:00Z".into(),
        expires_at: Some("2027-01-01T00:00:00Z".into()),
    }
}

#[test]
fn validation_result_refreshes_status_but_never_rederives_the_tier() {
    let state = fixed_state("active", Tier::Pro, "2026-04-01T00:00:00Z");
    let updated = state_refreshed_from_validation_result(
        state,
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 0,
            instance_id: Some("inst-1".to_string()),
            expires_at: Some("2030-01-01T00:00:00Z".to_string()),
            error: None,
        },
        "2026-04-08T00:00:00Z".to_string(),
    );

    assert_eq!(updated.status, "active");
    assert_eq!(updated.variant_id, 901, "the stored variant stands");
    assert_eq!(updated.tier, Tier::Pro, "the stored tier stands");
    assert_eq!(updated.last_validated_at, "2026-04-08T00:00:00Z");
    assert_eq!(updated.expires_at.as_deref(), Some("2030-01-01T00:00:00Z"));

    let mut legacy = fixed_state("active", Tier::Pro, "2026-04-01T00:00:00Z");
    legacy.variant_id = 0;
    let refreshed = state_refreshed_from_validation_result(
        legacy,
        api::LicenseResult {
            valid: true,
            status: "active".to_string(),
            variant_id: 0,
            instance_id: Some("inst-1".to_string()),
            expires_at: None,
            error: None,
        },
        "2026-04-08T00:00:00Z".to_string(),
    );
    assert_eq!(
        refreshed.variant_id, 0,
        "variant is never validation's to change"
    );
    assert_eq!(
        refreshed.tier,
        Tier::Pro,
        "tier is never validation's to change"
    );
}

#[test]
fn a_deactivated_instance_outranks_staleness_warnings() {
    let state = fixed_state(
        INSTANCE_DEACTIVATED_STATUS,
        Tier::Pro,
        "2026-04-01T00:00:00Z",
    );
    for masked in [
        ValidationWarning::Stale,
        ValidationWarning::StaleFinalWarning,
    ] {
        let info = info_from_state_with_warning(&state, masked);
        assert_eq!(
            info.validation_warning,
            ValidationWarning::InstanceDeactivated
        );
    }
}

#[test]
fn a_deactivated_instance_carries_its_banner_in_every_info() {
    let state = fixed_state(
        INSTANCE_DEACTIVATED_STATUS,
        Tier::Pro,
        "2026-04-01T00:00:00Z",
    );
    let info = info_from_state(&state);
    assert_eq!(
        info.validation_warning,
        ValidationWarning::InstanceDeactivated
    );
    assert!(!info.is_active);

    let entitled = fixed_state("active", Tier::Pro, "2026-04-01T00:00:00Z");
    assert_eq!(
        info_from_state(&entitled).validation_warning,
        ValidationWarning::None
    );
}

#[test]
fn an_invalid_verdict_over_an_entitled_status_downgrades_this_machine() {
    let state = fixed_state("active", Tier::Pro, "2026-04-01T00:00:00Z");
    let updated = state_refreshed_from_validation_result(
        state,
        api::LicenseResult {
            valid: false,
            status: "active".to_string(),
            variant_id: 901,
            instance_id: None,
            expires_at: None,
            error: Some("instance_id not found".to_string()),
        },
        "2026-04-08T00:00:00Z".to_string(),
    );

    assert_eq!(updated.status, INSTANCE_DEACTIVATED_STATUS);
    assert!(
        !is_entitled_license_status(&updated.status),
        "a refused instance must not stay entitled"
    );
    // A non-entitled verdict that names its reason still passes through
    // untouched: expired is expired, not "deactivated".
    let expired = state_refreshed_from_validation_result(
        fixed_state("active", Tier::Pro, "2026-04-01T00:00:00Z"),
        api::LicenseResult {
            valid: false,
            status: "expired".to_string(),
            variant_id: 901,
            instance_id: None,
            expires_at: None,
            error: None,
        },
        "2026-04-08T00:00:00Z".to_string(),
    );
    assert_eq!(expired.status, "expired");
}

#[test]
fn validation_result_preserves_lemon_checkout_trial_status() {
    let state = fixed_state("active", Tier::Core, "2026-04-01T00:00:00Z");
    let updated = state_refreshed_from_validation_result(
        state,
        api::LicenseResult {
            valid: true,
            status: "on_trial".to_string(),
            variant_id: 901,
            instance_id: Some("inst-1".to_string()),
            expires_at: Some("2026-06-01T00:00:00Z".to_string()),
            error: None,
        },
        "2026-04-08T00:00:00Z".to_string(),
    );

    assert_eq!(updated.status, "on_trial");
    assert!(is_entitled_license_status(&updated.status));
    assert_eq!(updated.expires_at.as_deref(), Some("2026-06-01T00:00:00Z"));
}

#[test]
fn info_from_state_treats_lemon_checkout_trial_as_active() {
    let state = fixed_state("on_trial", Tier::Pro, "2026-04-01T00:00:00Z");
    let info = info_from_state(&state);

    assert_eq!(info.tier, Tier::Pro);
    assert!(info.is_active);
    assert_eq!(info.plan_name, "Pro");
}

#[test]
fn parse_iso_accepts_rfc3339_z() {
    assert!(parse_iso("2026-04-19T10:00:00Z").is_some());
}

#[test]
fn parse_iso_accepts_rfc3339_offset() {
    // Both formats parse to the same instant.
    let z = parse_iso("2026-04-19T10:00:00Z").unwrap();
    let plus = parse_iso("2026-04-19T10:00:00+00:00").unwrap();
    assert_eq!(z, plus);
    // And the offset is honored: +05:00 means 05:00 UTC.
    let offset = parse_iso("2026-04-19T10:00:00+05:00").unwrap();
    let utc_equiv = parse_iso("2026-04-19T05:00:00Z").unwrap();
    assert_eq!(offset, utc_equiv);
}

#[test]
fn parse_iso_rejects_garbage() {
    assert!(parse_iso("not a date").is_none());
    assert!(parse_iso("").is_none());
    assert!(parse_iso("2026-04-19").is_none()); // missing time
}

#[test]
fn classify_returns_fresh_within_validation_interval() {
    let last = "2026-04-19T10:00:00Z";
    let now = Utc.with_ymd_and_hms(2026, 4, 19, 11, 0, 0).unwrap();
    // 1h since validation, well within VALIDATION_INTERVAL_SECS (24h).
    assert_eq!(
        classify_offline_validation(last, now),
        OfflineValidationState::Fresh
    );
}

#[test]
fn classify_returns_stale_after_interval_but_within_offline_grace() {
    let last = "2026-04-12T10:00:00Z";
    // 2 days after the 24h interval, well within 7-day grace.
    let now = Utc.with_ymd_and_hms(2026, 4, 14, 11, 0, 0).unwrap();
    assert_eq!(
        classify_offline_validation(last, now),
        OfflineValidationState::Stale
    );
}

#[test]
fn classify_returns_final_warning_in_eighth_day_window() {
    let last = "2026-04-12T10:00:00Z";
    // 7d + 12h since validation: past OFFLINE_GRACE, inside FINAL_GRACE.
    let now = Utc.with_ymd_and_hms(2026, 4, 19, 22, 0, 0).unwrap();
    assert_eq!(
        classify_offline_validation(last, now),
        OfflineValidationState::StaleFinalWarning,
    );
}

#[test]
fn classify_returns_expired_after_both_grace_windows() {
    let last = "2026-04-12T10:00:00Z";
    // 8d + 1h: both grace windows exhausted.
    let now = Utc.with_ymd_and_hms(2026, 4, 20, 11, 0, 0).unwrap();
    assert_eq!(
        classify_offline_validation(last, now),
        OfflineValidationState::Expired
    );
}

#[test]
fn classify_returns_expired_for_unparseable_timestamp() {
    let now = Utc::now();
    assert_eq!(
        classify_offline_validation("garbage", now),
        OfflineValidationState::Expired
    );
    assert_eq!(
        classify_offline_validation("", now),
        OfflineValidationState::Expired
    );
}

#[test]
fn classify_returns_expired_for_future_timestamp_clock_tampering() {
    let last = "2030-01-01T00:00:00Z";
    let now = Utc.with_ymd_and_hms(2026, 4, 19, 10, 0, 0).unwrap();
    assert_eq!(
        classify_offline_validation(last, now),
        OfflineValidationState::Expired
    );
}

#[test]
fn classify_at_offline_grace_boundary_is_still_stale_not_final_warning() {
    // Exactly 7 days; uses `<=` boundaries so the seventh-day edge stays
    // in the Stale bucket rather than tipping into StaleFinalWarning.
    let last = "2026-04-12T10:00:00Z";
    let now = Utc.with_ymd_and_hms(2026, 4, 19, 10, 0, 0).unwrap();
    assert_eq!(
        classify_offline_validation(last, now),
        OfflineValidationState::Stale
    );
}

#[test]
fn classify_at_final_grace_boundary_is_still_final_warning_not_expired() {
    // Exactly 8 days; tips into StaleFinalWarning but not Expired.
    let last = "2026-04-12T10:00:00Z";
    let now = Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap();
    assert_eq!(
        classify_offline_validation(last, now),
        OfflineValidationState::StaleFinalWarning,
    );
}

#[test]
fn validation_warning_serialization_matches_frontend_contract() {
    assert_eq!(ValidationWarning::None.as_str(), "none");
    assert_eq!(ValidationWarning::Stale.as_str(), "stale");
    assert_eq!(
        ValidationWarning::StaleFinalWarning.as_str(),
        "stale_final_warning",
    );
    assert_eq!(ValidationWarning::KeyUnreadable.as_str(), "key_unreadable");
    // JSON shape mirrors the snake_case strings the frontend expects.
    assert_eq!(
        serde_json::to_value(ValidationWarning::Stale).unwrap(),
        serde_json::json!("stale"),
    );
    assert_eq!(
        serde_json::to_value(ValidationWarning::StaleFinalWarning).unwrap(),
        serde_json::json!("stale_final_warning"),
    );
    assert_eq!(
        serde_json::to_value(ValidationWarning::KeyUnreadable).unwrap(),
        serde_json::json!("key_unreadable"),
    );
    assert_eq!(
        ValidationWarning::InstanceDeactivated.as_str(),
        "instance_deactivated"
    );
    assert_eq!(
        serde_json::to_value(ValidationWarning::InstanceDeactivated).unwrap(),
        serde_json::json!("instance_deactivated"),
    );
}

#[test]
fn free_info_is_ungated() {
    let info = free_info();
    assert_eq!(info.tier, Tier::Free);
    assert_eq!(info.status, "none");
    assert_eq!(info.plan_name, "Free");
    assert!(info.billing_interval.is_none());
    assert!(!info.is_active);
    assert!(info.expires_at.is_none());
}

#[test]
fn billing_interval_maps_known_variant_ids() {
    let variants = config::VariantIds {
        core_monthly: 101,
        core_annual: 102,
        pro_monthly: 201,
        pro_annual: 202,
    };

    assert_eq!(
        billing_interval_from_variant_id_with_variants(101, variants),
        Some(BillingInterval::Monthly)
    );
    assert_eq!(
        billing_interval_from_variant_id_with_variants(102, variants),
        Some(BillingInterval::Yearly)
    );
    assert_eq!(
        billing_interval_from_variant_id_with_variants(201, variants),
        Some(BillingInterval::Monthly)
    );
    assert_eq!(
        billing_interval_from_variant_id_with_variants(202, variants),
        Some(BillingInterval::Yearly)
    );
    assert_eq!(
        billing_interval_from_variant_id_with_variants(0, variants),
        None
    );
    assert_eq!(
        billing_interval_from_variant_id_with_variants(999, variants),
        None
    );
}

#[test]
fn info_from_state_active_pro_unlocks_pro_tier() {
    let state = fixed_state("active", Tier::Pro, "2026-04-19T10:00:00Z");
    let info = info_from_state(&state);
    assert_eq!(info.tier, Tier::Pro);
    assert_eq!(info.status, "active");
    assert_eq!(info.plan_name, "Pro");
    assert!(info.is_active);
    assert_eq!(info.expires_at.as_deref(), Some("2027-01-01T00:00:00Z"));
}

#[test]
fn info_from_state_active_core_unlocks_core_tier() {
    let state = fixed_state("active", Tier::Core, "2026-04-19T10:00:00Z");
    let info = info_from_state(&state);
    assert_eq!(info.tier, Tier::Core);
    assert_eq!(info.plan_name, "Plus");
    assert!(info.is_active);
}

#[test]
fn info_from_state_expired_falls_back_to_free_tier() {
    // SECURITY: even if state.tier == Pro, if status != active the
    // returned tier MUST be Free - otherwise expired keys keep features.
    let state = fixed_state("expired", Tier::Pro, "2026-04-19T10:00:00Z");
    let info = info_from_state(&state);
    assert_eq!(info.tier, Tier::Free);
    assert_eq!(info.status, "expired");
    assert_eq!(info.plan_name, "Free");
    assert!(info.billing_interval.is_none());
    assert!(!info.is_active);
}

#[test]
fn info_from_state_disabled_falls_back_to_free_tier() {
    let state = fixed_state("disabled", Tier::Core, "2026-04-19T10:00:00Z");
    let info = info_from_state(&state);
    assert_eq!(info.tier, Tier::Free);
    assert_eq!(info.status, "disabled");
    assert!(!info.is_active);
}

#[test]
fn info_from_state_inactive_falls_back_to_free_tier() {
    let state = fixed_state("inactive", Tier::Pro, "2026-04-19T10:00:00Z");
    let info = info_from_state(&state);
    assert_eq!(info.tier, Tier::Free);
    assert!(!info.is_active);
}

#[test]
fn info_from_state_carries_expiry_even_when_inactive() {
    // The frontend uses expires_at to drive "renew" prompts, so it must
    // survive even when we downgrade to Free locally.
    let state = fixed_state("expired", Tier::Pro, "2026-04-19T10:00:00Z");
    let info = info_from_state(&state);
    assert_eq!(info.expires_at.as_deref(), Some("2027-01-01T00:00:00Z"));
}

#[cfg(debug_assertions)]
#[test]
fn debug_unconfigured_license_allows_dev_core_row() {
    let mut state = fixed_state("active", Tier::Core, &Utc::now().to_rfc3339());
    state.license_key = "sitecmd-dev-core".into();

    let info = dev_info_from_state_when_licensing_unconfigured(&state)
        .expect("debug dev license should be honored");

    assert_eq!(info.tier, Tier::Core);
    assert_eq!(info.plan_name, "Plus");
    assert!(info.is_active);
}

#[cfg(debug_assertions)]
#[test]
fn debug_unconfigured_license_ignores_non_dev_key() {
    let state = fixed_state("active", Tier::Core, &Utc::now().to_rfc3339());

    let info = dev_info_from_state_when_licensing_unconfigured(&state);

    assert!(info.is_none());
}

#[cfg(debug_assertions)]
#[test]
fn debug_unconfigured_license_honours_stale_active_dev_row() {
    // Dev licenses are local sentinels - there's nothing to validate them
    // against, so a stale `last_validated_at` must NOT downgrade them.
    let stale = (Utc::now() - Duration::seconds(OFFLINE_GRACE_PERIOD_SECS as i64 + 1)).to_rfc3339();
    let mut state = fixed_state("active", Tier::Core, &stale);
    state.license_key = "sitecmd-dev-core".into();

    let info = dev_info_from_state_when_licensing_unconfigured(&state)
        .expect("stale active dev row must be honoured");
    assert_eq!(info.tier, Tier::Core);
}

#[cfg(debug_assertions)]
#[test]
fn debug_unconfigured_license_rejects_inactive_dev_row() {
    // An explicitly inactive dev row should still be rejected so a manual
    // deactivate works.
    let mut state = fixed_state("inactive", Tier::Core, &Utc::now().to_rfc3339());
    state.license_key = "sitecmd-dev-core".into();

    let info = dev_info_from_state_when_licensing_unconfigured(&state);

    assert!(info.is_none());
}

#[test]
fn checkout_urls_are_derived_from_compiled_checkout_urls() {
    let urls = checkout_urls();

    assert_eq!(urls.core, config::core_checkout_url());
    assert_eq!(urls.core_monthly, urls.core);
    assert_eq!(urls.core_annual, urls.core);
    assert_eq!(urls.pro, config::pro_checkout_url());
    assert_eq!(urls.pro_monthly, urls.pro);
    assert_eq!(urls.pro_annual, urls.pro);
}

#[test]
fn now_iso_round_trips_through_parse_iso() {
    let s = now_iso();
    let parsed = parse_iso(&s).expect("now_iso() must produce valid RFC3339");
    // Timestamp should be very close to "now" (within a few seconds).
    let elapsed = (Utc::now() - parsed).num_seconds().abs();
    assert!(elapsed < 5, "now_iso() drifted by {}s", elapsed);
}
