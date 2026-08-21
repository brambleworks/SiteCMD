use super::*;

// Catalog refresh pipeline inspected by the guardrail tests.
const PIPELINE: &str = include_str!("catalog_refresh_tick.rs");

// Fail loudly when endpoint configuration would skip the credential suite.
#[test]
fn the_credential_suite_is_live_in_this_build() {
    assert!(
        std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_none(),
        "SITECMD_ACTIVATION_ENDPOINT is compiled into this test build, so every \
         ensure_credential and retry_pending_release test in this file is skipping \
         itself. Run the tests without the endpoint configured."
    );
}

#[test]
fn ensure_credential_reads_the_license_generation_as_one() {
    let source = include_str!("catalog_refresh.rs");
    let body = &source[source
        .find("pub(crate) async fn ensure_credential")
        .expect("ensure_credential exists")..];
    let credential_lock = body
        .find("CREDENTIAL_LOCK.lock()")
        .expect("ensure_credential serializes credential transitions");
    let generation_lock = body
        .find("license_mutation().lock()")
        .expect("ensure_credential takes the license-generation lock for its reads");
    let mint = body
        .find("obtain_token(")
        .expect("ensure_credential mints through obtain_token");
    assert!(
        credential_lock < generation_lock && generation_lock < mint,
        "the generation read sits inside the credential lock and before the mint"
    );
    let key_read = body
        .find("get_license_key(app)")
        .expect("ensure_credential re-reads the key");
    let row_read = body
        .find("crate::licensing::store::load")
        .expect("ensure_credential re-reads the row");
    assert!(
        generation_lock < key_read && key_read < row_read && row_read < mint,
        "key and row are both read under the generation lock"
    );
}

#[test]
fn the_stored_license_key_is_read_through_usable_key() {
    let source = PIPELINE;
    let body = &source[source
        .find("async fn current_or_new_token")
        .expect("current_or_new_token exists")..];
    let read = body
        .find("crate::licensing::commands::usable_key(")
        .expect("the key read is filtered through usable_key");
    let mint = body
        .find("ensure_credential(")
        .expect("current_or_new_token mints through ensure_credential");
    assert!(
        read < mint,
        "the filter sits on the read that feeds the mint"
    );
}

#[test]
fn every_release_refusal_arm_is_gated_on_the_known_vocabulary() {
    let source = include_str!("catalog_refresh.rs");
    for (function, next) in [
        (
            "pub(crate) async fn release_credential",
            "async fn record_pending_provider_release",
        ),
        ("async fn retry_pending_release", "pub async fn run"),
    ] {
        let start = source.find(function).expect(function);
        let end = source[start..].find(next).expect(next) + start;
        let region = &source[start..end];
        let refusals = region.matches("ActivationError::Refused").count();
        let gates = region.matches("known_refusal(&reason)").count();
        assert!(refusals >= 1, "{function} matches on Refused");
        assert_eq!(
            refusals, gates,
            "{function}: every Refused arm carries the known_refusal gate"
        );
    }
}

#[test]
fn an_unrecorded_tombstone_is_audited_as_lost_not_pending() {
    let source = include_str!("catalog_refresh.rs");
    let start = source
        .find("pub(crate) async fn release_credential")
        .expect("release_credential exists");
    let end = source[start..]
        .find("async fn retry_pending_release")
        .expect("retry_pending_release follows")
        + start;
    let region = &source[start..end];
    assert!(
        region.contains(r#""unrecorded""#),
        "a failed tombstone write downgrades the audit outcome"
    );
    let error_logs = region.matches("tracing::error!").count();
    assert!(
        error_logs >= 2,
        "both recorders report the lost seat at error level"
    );
}

// Deactivation fails if the released seat's token remains in the keyring.
#[test]
fn a_token_left_in_the_keyring_is_not_a_clean_release() {
    let source = include_str!("catalog_refresh.rs");
    let start = source
        .find("pub(crate) async fn release_credential")
        .expect("release_credential exists");
    let body = &source[start..];
    assert!(
        body.contains("let local_token_cleared ="),
        "the local deletion's result must be captured, not only logged"
    );
    assert!(
        body.contains("\"local_token_cleared\": local_token_cleared"),
        "the audit row must record whether the credential actually left this machine"
    );
    // Captured is not enough; it has to change the ending the caller sees.
    let decision = body
        .find("if !local_token_cleared")
        .expect("the cleanup result must gate the outcome");
    let mapping = body
        .find("match outcome {")
        .expect("outcome mapping exists");
    assert!(
        decision < mapping,
        "a failed local cleanup must be decided before the outcome is mapped to a clean ending"
    );
}

#[test]
fn the_stranded_outcome_is_visible_in_settings() {
    let source = include_str!("catalog_refresh.rs");
    let arm_start = source
        .find("Ok(ActivationOutcome::AlreadyActivated)")
        .expect("the stranded arm exists");
    let arm = &source[arm_start..arm_start + 600];
    assert!(
        arm.contains("record_credential_block(Some"),
        "the stranded outcome must record a credential block"
    );
}

#[test]
fn a_fresh_install_downloads_whatever_the_service_offers() {
    assert!(needs_download(1, None, false));
}

#[test]
fn a_machine_needing_repair_sends_no_conditional_version() {
    assert_eq!(
        conditional_version(true, Some("2026-07-28.7".to_string())),
        None
    );
    assert_eq!(
        conditional_version(false, Some("2026-07-28.7".to_string())),
        Some("2026-07-28.7".to_string())
    );
    assert_eq!(conditional_version(false, None), None);
}

#[test]
fn the_tick_feeds_its_conditional_through_the_repair_check() {
    let source = PIPELINE;
    let start = source.find("async fn tick").expect("tick exists");
    let end = source[start..]
        .find("async fn current_or_new_token")
        .expect("current_or_new_token follows tick")
        + start;
    assert!(
        source[start..end].contains("conditional_version("),
        "tick must build its request version through conditional_version"
    );
}

// Unauthorized manifest and pack fetches both clear the rejected credential.
#[test]
fn either_rejection_clears_the_token_it_presented() {
    let source = PIPELINE;
    let start = source.find("async fn tick").expect("tick exists");
    let end = source[start..]
        .find("async fn clear_rejected_token")
        .expect("clear_rejected_token follows tick")
        + start;
    let body = &source[start..end];
    assert_eq!(
        body.matches("clear_rejected_token(app, &token)").count(),
        2,
        "both the manifest and the pack fetch must clear a token the service refused"
    );
    // And neither may reach the generic failure arm with an Unauthorized.
    assert_eq!(
        body.matches("Err(FetchError::Unauthorized)").count(),
        2,
        "each fetch must name Unauthorized before its catch-all error arm"
    );
}

#[test]
fn a_newer_sequence_downloads_and_an_equal_or_older_one_does_not() {
    assert!(needs_download(2, Some(1), false));
    // Equal means current; the service should have said 304, and a service
    // that did not must not be able to force a re-download.
    assert!(!needs_download(1, Some(1), false));
    // Older is the rollback direction. Verification would refuse it anyway;
    // refusing here means the bytes are never even fetched.
    assert!(!needs_download(1, Some(2), false));
}

#[test]
fn a_corrupt_active_pack_repairs_at_exactly_its_own_sequence() {
    assert!(needs_download(5, Some(5), true));
    // Nothing lower, corrupt or not.
    assert!(!needs_download(4, Some(5), true));
}

// These tests hold the debug keychain guard across awaits so no peer can clear
// the shared store during the credential operation.

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn ensure_credential_reuses_a_token_stored_while_it_waited() {
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_some() {
        return;
    }
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    let handle = app.handle();

    crate::keyring::store_catalog_token(handle, "token-from-interactive-path").expect("seed token");

    let db = crate::db::test_helpers::temp_db_arc();
    let outcome = ensure_credential(handle, &db, "key-1", "inst-1").await;

    assert_eq!(
        outcome,
        Ok(true),
        "an existing token is the job already done"
    );
    assert_eq!(
        crate::keyring::get_catalog_token(handle).expect("token readable"),
        Some("token-from-interactive-path".to_string()),
        "the stored token must survive untouched"
    );
    crate::keyring::delete_catalog_token(handle).expect("cleanup");
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn ensure_credential_refuses_to_mint_for_a_license_that_changed() {
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_some() {
        return;
    }
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    let handle = app.handle();

    crate::keyring::store_license_key(handle, "key-2-replaced-it").expect("seed license");

    let db = crate::db::test_helpers::temp_db_arc();
    let outcome = ensure_credential(handle, &db, "key-1", "inst-1").await;

    assert_eq!(
        outcome,
        Ok(false),
        "a changed license is nothing-to-do, not an error and never a mint"
    );
    assert_eq!(
        crate::keyring::get_pending_activation(handle).expect("nonce readable"),
        None,
        "refusing must happen before a nonce is minted or persisted"
    );
    crate::keyring::delete_license_key(handle).expect("cleanup");
}

// Seed the singleton license row with the given instance id.
fn seed_license_row(db: &std::sync::Arc<crate::db::Database>, instance_id: &str) {
    let state = crate::licensing::store::LicenseState {
        license_key: "key-1".to_string(),
        instance_id: instance_id.to_string(),
        variant_id: 1,
        tier: crate::licensing::config::Tier::Core,
        status: "active".to_string(),
        last_validated_at: "2026-07-29T00:00:00Z".to_string(),
        activated_at: "2026-07-29T00:00:00Z".to_string(),
        expires_at: None,
    };
    db.execute(move |conn| crate::licensing::store::save(conn, &state))
        .expect("db reachable")
        .expect("seed license row");
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn ensure_credential_refuses_a_stale_installation_id() {
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_some() {
        return;
    }
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    let handle = app.handle();

    crate::keyring::store_license_key(handle, "key-1").expect("seed license");
    let db = crate::db::test_helpers::temp_db_arc();
    seed_license_row(&db, "inst-current");

    let outcome = ensure_credential(handle, &db, "key-1", "inst-stale").await;

    assert_eq!(
        outcome,
        Ok(false),
        "a stale installation id is nothing-to-do, never a mint"
    );
    assert_eq!(
        crate::keyring::get_pending_activation(handle).expect("nonce readable"),
        None,
        "refusing must happen before a nonce is minted or persisted"
    );
    crate::keyring::delete_license_key(handle).expect("cleanup");
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn ensure_credential_refuses_when_the_license_row_is_gone() {
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_some() {
        return;
    }
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    let handle = app.handle();

    crate::keyring::store_license_key(handle, "key-1").expect("seed license");
    let db = crate::db::test_helpers::temp_db_arc();

    let outcome = ensure_credential(handle, &db, "key-1", "inst-1").await;

    assert_eq!(
        outcome,
        Ok(false),
        "a cleared license row is nothing-to-do, never a mint"
    );
    assert_eq!(
        crate::keyring::get_pending_activation(handle).expect("nonce readable"),
        None,
        "refusing must happen before a nonce is minted or persisted"
    );
    crate::keyring::delete_license_key(handle).expect("cleanup");
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn a_completed_pending_release_clears_its_own_tombstone() {
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_some() {
        return;
    }
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    let handle = app.handle();

    crate::keyring::store_pending_release(handle, pending("key-1", "inst-1", true, false))
        .expect("seed tombstone");

    retry_pending_release(handle).await;

    assert_eq!(
        crate::keyring::get_pending_releases(handle).expect("tombstones readable"),
        vec![],
        "a completed release must not be retried forever"
    );
}

// Shorthand for a pending-release record.
fn pending(
    key: &str,
    installation: &str,
    catalog: bool,
    lemonsqueezy: bool,
) -> crate::keyring::PendingRelease {
    crate::keyring::PendingRelease {
        license_key: key.to_string(),
        installation_id: installation.to_string(),
        catalog,
        lemonsqueezy,
    }
}

#[test]
fn retry_pending_release_drops_the_lock_across_the_network() {
    let source = include_str!("catalog_refresh.rs");
    let body = &source[source
        .find("async fn retry_pending_release")
        .expect("retry_pending_release exists")..];
    let first_lock = body
        .find("CREDENTIAL_LOCK.lock()")
        .expect("the snapshot takes the lock");
    let release_call = body
        .find("activation::deactivate(")
        .expect("the drain releases over the network");
    let second_lock = body[first_lock + 1..]
        .find("CREDENTIAL_LOCK.lock()")
        .map(|at| at + first_lock + 1)
        .expect("the subtraction takes the lock again");
    assert!(
        first_lock < release_call && release_call < second_lock,
        "network calls must sit between the two lock scopes, not inside one"
    );
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn a_second_offline_release_preserves_the_first_and_both_drain() {
    if std::option_env!("SITECMD_ACTIVATION_ENDPOINT").is_some() {
        return;
    }
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    let handle = app.handle();

    crate::keyring::store_pending_release(handle, pending("key-1", "inst-1", true, false))
        .expect("first tombstone");
    crate::keyring::store_pending_release(handle, pending("key-2", "inst-2", true, false))
        .expect("second tombstone");
    // A repeat for the same slot must not grow the list (side-merge
    // semantics are pinned by the pure merge tests in keyring::tests).
    crate::keyring::store_pending_release(handle, pending("key-2", "inst-2", true, false))
        .expect("repeat tombstone");

    let recorded = crate::keyring::get_pending_releases(handle).expect("tombstones readable");
    assert_eq!(
        recorded.len(),
        2,
        "both releases recorded once each: {recorded:?}"
    );
    assert_eq!(recorded[0].license_key, "key-1");
    assert_eq!(recorded[1].license_key, "key-2");

    retry_pending_release(handle).await;

    assert_eq!(
        crate::keyring::get_pending_releases(handle).expect("tombstones readable"),
        vec![],
        "every completed release clears; none is retried forever"
    );
}
