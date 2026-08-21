//! Connected scope retry-loop tests.

use super::*;
use crate::db::test_helpers::{temp_db_arc, TestDbArc};

// A bound environment holding an unacknowledged scope edit: the state the
// retry loop exists for, and the one a never-activated installation sits in
// indefinitely.
fn environment_owing_a_scope(url: &str) -> TestDbArc {
    let db = temp_db_arc();
    let project_id = db
        .upsert_project("Example", "/tmp/scope-retry-loop", None)
        .expect("project");
    db.add_environment(project_id, url, "Production", "production", "manual")
        .expect("environment");
    let site_id = db
        .get_or_create_site_for_project(project_id, url)
        .expect("site");
    db.connect_site(project_id, url, "site_remote", 1)
        .expect("connected binding");
    db.replace_scan_scope(site_id, &["/".into(), "/pricing".into()])
        .expect("scope");
    db
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn a_bound_but_unactivated_installation_holds_calmly_instead_of_retrying() {
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    crate::keyring::delete_connected_installation_token(app.handle()).expect("no stored token");
    let db = environment_owing_a_scope("https://scope-retry.example");
    let mut state = ScopeRetryState::default();

    // Credential holds skip delivery attempts and per-site keychain reads.
    assert_eq!(
        retry_pending(app.handle(), &db, &mut state).await,
        Tick::HoldingForCredential { sites: 1 }
    );
    assert!(
        state.backoff.is_empty(),
        "a hold is not a failure and must not accrue backoff"
    );
    // An unchanged hold remains silent on later ticks.
    assert!(
        !state.begin_credential_hold(),
        "the hold is announced once per episode, not once a minute"
    );
    assert_eq!(
        retry_pending(app.handle(), &db, &mut state).await,
        Tick::HoldingForCredential { sites: 1 }
    );
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn a_real_delivery_failure_keeps_its_cause_and_backs_off() {
    // A configured endpoint would make this tick a live request.
    if std::option_env!("SITECMD_CONNECTED_ENDPOINT").is_some() {
        return;
    }
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    crate::keyring::store_connected_installation_token(app.handle(), "installation-token")
        .expect("seed token");
    let db = environment_owing_a_scope("https://scope-retry-failure.example");
    let mut state = ScopeRetryState::default();

    let outcome = retry_pending(app.handle(), &db, &mut state).await;
    crate::keyring::delete_connected_installation_token(app.handle()).expect("cleanup");
    assert_eq!(
        outcome,
        Tick::Attempted {
            delivered: 0,
            failed: 1,
            skipped: 0
        }
    );
    let backoff = state
        .backoff
        .values()
        .next()
        .expect("a failure is remembered with its cause")
        .clone();
    assert!(
        backoff.cause.contains("endpoint"),
        "the warning carries the actual cause: {}",
        backoff.cause
    );
    assert_eq!(backoff.consecutive_failures, 1);
    assert_eq!(backoff.ticks_remaining, 1);
}

#[test]
fn repeated_failures_of_the_same_site_wait_longer_between_attempts() {
    let mut state = ScopeRetryState::default();
    let waits: Vec<u32> = (0..5)
        .map(|_| state.failed(4, "refused".into()).ticks_remaining)
        .collect();
    assert_eq!(waits, vec![1, 2, 4, 8, 8], "bounded, never unbounded");

    // A waiting site spends its ticks rather than hammering the service, and
    // becomes eligible again once they run out.
    let mut state = ScopeRetryState::default();
    state.failed(4, "refused".into());
    assert!(!state.ready(4));
    assert!(state.ready(4));
}

#[test]
fn a_delivered_scope_forgets_its_failure_history() {
    let mut state = ScopeRetryState::default();
    state.failed(4, "refused".into());
    state.delivered(4);
    assert!(state.ready(4), "the next edit is attempted immediately");
    assert!(state.backoff.is_empty());
}

#[test]
fn sites_that_leave_the_queue_are_forgotten() {
    // The loop runs for the life of the app, so history for a disconnected
    // site must not accumulate forever.
    let mut state = ScopeRetryState::default();
    state.failed(4, "refused".into());
    state.failed(5, "refused".into());
    state.forget_settled(&[5]);
    assert_eq!(state.backoff.keys().copied().collect::<Vec<_>>(), vec![5]);
}

#[test]
fn a_readable_credential_ends_the_hold_so_a_later_one_is_announced_again() {
    let mut state = ScopeRetryState::default();
    assert!(state.begin_credential_hold());
    assert!(!state.begin_credential_hold());
    state.end_credential_hold();
    assert!(state.begin_credential_hold());
}
