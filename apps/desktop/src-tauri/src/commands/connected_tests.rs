//! Connected sync command tests.

use super::*;
use crate::db::{
    test_helpers::{temp_db_arc, TestDbArc},
    GroupDecision, PendingMutation,
};

fn pending(decision: GroupDecision) -> PendingMutation {
    PendingMutation {
        id: 1,
        check_id: "security.csp".into(),
        decision,
        based_on_revision: 7,
        idempotency_key: "mut_1".into(),
        decided_at: 10,
        conflict: None,
    }
}

#[test]
fn every_local_decision_has_one_wire_transition() {
    let snooze = mutation_entry(&pending(GroupDecision::Snooze { until: 99 }));
    assert_eq!(snooze.state, ClientGroupState::Dismissed);
    assert_eq!(
        snooze.dismissal,
        Some(DismissalPolicy::Snoozed { until: 99 })
    );
    let claim = mutation_entry(&pending(GroupDecision::ClaimFixed));
    assert_eq!(claim.state, ClientGroupState::ClaimedFixed);
    assert_eq!(claim.dismissal, None);
    let reopen = mutation_entry(&pending(GroupDecision::Reopen));
    assert_eq!(reopen.state, ClientGroupState::Active);
    assert_eq!(reopen.based_on_revision, 7);
}

#[test]
fn submission_idempotency_is_stable_and_installation_scoped() {
    assert_eq!(
        sync_idempotency_key("inst_a", 4),
        sync_idempotency_key("inst_a", 4)
    );
    assert_ne!(
        sync_idempotency_key("inst_a", 4),
        sync_idempotency_key("inst_b", 4)
    );
    assert_ne!(
        sync_idempotency_key("inst_a", 4),
        sync_idempotency_key("inst_a", 5)
    );
}

// A connected environment whose local scope has been edited but never
// acknowledged: the exact state `sync_connected_site` reaches after its
// submission has applied.
fn connected_environment_owing_a_scope(url: &str) -> (TestDbArc, i64) {
    let db = temp_db_arc();
    let project_id = db
        .upsert_project("Example", "/tmp/connected-scope-delivery", None)
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
    (db, project_id)
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn a_failed_scope_delivery_reports_pending_instead_of_failing_the_sync() {
    const URL: &str = "https://scope-delivery.example";
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    crate::keyring::delete_connected_installation_token(app.handle()).expect("no stored token");
    let (db, project_id) = connected_environment_owing_a_scope(URL);

    assert!(
        deliver_pending_scope(app.handle(), &db, project_id, URL).await,
        "a delivery that could not run leaves the scope pending"
    );
    // The watermark is the retry handle, so it must still read as due.
    assert!(db
        .connected_scan_scope_pending(project_id, URL)
        .expect("pending read"));
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn an_acknowledged_scope_leaves_nothing_pending_and_touches_no_credential() {
    const URL: &str = "https://scope-settled.example";
    let _guard = crate::keyring::SECRET_TEST_GUARD
        .lock()
        .expect("secret test guard");
    let app = tauri::test::mock_app();
    crate::keyring::delete_connected_installation_token(app.handle()).expect("no stored token");
    let (db, project_id) = connected_environment_owing_a_scope(URL);
    db.mark_connected_scan_scope_synced(project_id, URL, "site_remote", 1, 1)
        .expect("acknowledge");

    // No token is stored, so reaching the delivery path at all would fail.
    // Answering false proves the settled case never leaves the database.
    assert!(!deliver_pending_scope(app.handle(), &db, project_id, URL).await);
}
