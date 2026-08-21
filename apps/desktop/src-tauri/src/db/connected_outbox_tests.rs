//! Connected mutation outbox tests.

use crate::core::types_work_items::{IssueStatus, VerifiedBy};
use crate::db::test_helpers::{temp_db, TestDb};
use crate::db::{DecisionRecord, GroupDecision, IssueLifecycle, PendingMutation};

const NOW_MS: i64 = 1_800_000_000_000;
const SITE: &str = "https://example.com";
const CHECK: &str = "security.csp";

fn seeded() -> (TestDb, i64) {
    let db = temp_db();
    let project_id = db
        .upsert_project("Outbox", "/tmp/outbox", Some("nextjs"))
        .expect("upsert project");
    (db, project_id)
}

// A connected, bootstrapped environment: the state in which a decision is
// owed to the service as a mutation.
fn connected() -> (TestDb, i64) {
    let (db, project_id) = seeded();
    db.connect_site(project_id, SITE, "site_9f2c81d0a4b3", NOW_MS)
        .expect("connect");
    db.mark_site_bootstrapped(project_id, SITE, NOW_MS + 1)
        .expect("bootstrap");
    (db, project_id)
}

fn decide(db: &TestDb, project_id: i64, decision: GroupDecision, now_ms: i64) -> DecisionRecord {
    db.record_group_decision(project_id, SITE, CHECK, decision, now_ms)
        .expect("record decision")
}

fn recorded(record: DecisionRecord) -> PendingMutation {
    match record {
        DecisionRecord::Recorded(pending) => pending,
        other => panic!("expected a recorded mutation, got {other:?}"),
    }
}

fn pending(db: &TestDb, project_id: i64) -> Vec<PendingMutation> {
    db.pending_group_mutations(project_id, SITE)
        .expect("read pending")
}

#[test]
fn a_decision_on_an_unconnected_environment_stays_local() {
    let (db, project_id) = seeded();
    let record = decide(&db, project_id, GroupDecision::Ignore, NOW_MS);

    assert_eq!(record, DecisionRecord::LocalOnly);
    assert_eq!(
        db.get_issue_state(project_id, Some(SITE), CHECK)
            .expect("read state")
            .map(|row| row.0),
        Some(IssueStatus::Ignored),
        "the decision still applies locally; there is simply nobody to tell"
    );
}

#[test]
fn a_decision_made_before_bootstrap_is_carried_by_the_bootstrap_payload() {
    let (db, project_id) = seeded();
    db.connect_site(project_id, SITE, "site_9f2c81d0a4b3", NOW_MS)
        .expect("connect");

    let record = decide(&db, project_id, GroupDecision::Ignore, NOW_MS + 1);
    assert_eq!(
        record,
        DecisionRecord::CarriedByBootstrap,
        "bootstrap sends every group's current state, so recording a mutation \
         as well would submit the same decision twice"
    );
    assert!(pending(&db, project_id).is_empty());
}

#[test]
fn a_decision_records_the_revision_the_user_could_actually_have_seen() {
    let (db, project_id) = connected();
    db.record_pulled_group_revision(project_id, SITE, CHECK, 7, NOW_MS + 2)
        .expect("pull");

    let entry = recorded(decide(&db, project_id, GroupDecision::Ignore, NOW_MS + 3));
    assert_eq!(entry.based_on_revision, 7);
    assert_eq!(entry.decision, GroupDecision::Ignore);
    assert!(entry.idempotency_key.starts_with("mut_"));
    assert_eq!(entry.decided_at, NOW_MS + 3);
}

#[test]
fn a_group_the_service_never_reported_is_decided_at_the_genesis_revision() {
    let (db, project_id) = connected();
    let entry = recorded(decide(
        &db,
        project_id,
        GroupDecision::ClaimFixed,
        NOW_MS + 2,
    ));
    assert_eq!(
        entry.based_on_revision, 0,
        "a group the service does not know has no revision to have moved past"
    );
}

#[test]
fn pulling_newer_state_does_not_rebase_a_decision_already_made() {
    let (db, project_id) = connected();
    db.record_pulled_group_revision(project_id, SITE, CHECK, 7, NOW_MS + 2)
        .expect("first pull");
    let entry = recorded(decide(&db, project_id, GroupDecision::Ignore, NOW_MS + 3));
    db.record_pulled_group_revision(project_id, SITE, CHECK, 19, NOW_MS + 4)
        .expect("later pull");

    let still = pending(&db, project_id);
    assert_eq!(still.len(), 1);
    assert_eq!(
        still[0].based_on_revision, entry.based_on_revision,
        "relabeling an old decision as based on state the user never saw is \
         exactly what the revision guard exists to prevent"
    );
    assert_eq!(still[0].idempotency_key, entry.idempotency_key);
}

#[test]
fn changing_your_mind_replaces_the_pending_decision_under_a_new_key() {
    let (db, project_id) = connected();
    let first = recorded(decide(&db, project_id, GroupDecision::Ignore, NOW_MS + 2));
    let second = recorded(decide(
        &db,
        project_id,
        GroupDecision::Block {
            reason: Some("intended".to_string()),
        },
        NOW_MS + 3,
    ));

    let entries = pending(&db, project_id);
    assert_eq!(
        entries.len(),
        1,
        "two entries guarding one revision would ask the service to apply one \
         group twice inside a single atomic batch"
    );
    assert_eq!(entries[0].decision, second.decision);
    assert_ne!(
        second.idempotency_key, first.idempotency_key,
        "the request body changed, and replaying the old key with a new body \
         is an idempotency conflict rather than the new decision"
    );
}

#[test]
fn a_replacement_carries_the_basis_it_was_decided_under() {
    let (db, project_id) = connected();
    decide(&db, project_id, GroupDecision::Ignore, NOW_MS + 2);
    db.record_pulled_group_revision(project_id, SITE, CHECK, 19, NOW_MS + 3)
        .expect("pull");
    let second = recorded(decide(&db, project_id, GroupDecision::Reopen, NOW_MS + 4));

    assert_eq!(
        second.based_on_revision, 19,
        "the second decision was made against the newer picture, so that is \
         its honest basis"
    );
}

#[test]
fn settling_an_acknowledged_decision_cannot_delete_the_one_that_replaced_it() {
    let (db, project_id) = connected();
    let in_flight = recorded(decide(&db, project_id, GroupDecision::Ignore, NOW_MS + 2));
    let replacement = recorded(decide(&db, project_id, GroupDecision::Reopen, NOW_MS + 3));

    let settled = db
        .settle_group_mutation(in_flight.id, &in_flight.idempotency_key, 8, NOW_MS + 4)
        .expect("settle");
    assert!(
        !settled,
        "the row was reused by the replacement; settling by id alone would \
         drop a decision the service has never heard"
    );
    assert_eq!(pending(&db, project_id).len(), 1);

    assert!(db
        .settle_group_mutation(replacement.id, &replacement.idempotency_key, 9, NOW_MS + 5,)
        .expect("settle replacement"));
    assert!(
        pending(&db, project_id).is_empty(),
        "a delivered decision leaves the outbox; its truth lives in the \
         lifecycle row and in the service's state"
    );
    assert_eq!(
        db.record_pulled_group_revision(project_id, SITE, CHECK, 0, NOW_MS + 6)
            .expect("read revision through a stale replay"),
        9,
        "the acknowledged revision and outbox deletion must commit together"
    );
}

#[test]
fn a_conflict_keeps_what_the_service_reported_and_bases_the_next_decision_on_it() {
    let (db, project_id) = connected();
    let entry = recorded(decide(&db, project_id, GroupDecision::Ignore, NOW_MS + 2));

    assert!(db
        .record_mutation_conflict(
            entry.id,
            &entry.idempotency_key,
            "verified_fixed",
            12,
            NOW_MS + 3,
        )
        .expect("record conflict"));

    let conflicted = pending(&db, project_id);
    let conflict = conflicted[0]
        .conflict
        .clone()
        .expect("the entry carries the conflict");
    assert_eq!(conflict.state, "verified_fixed");
    assert_eq!(conflict.revision, 12);
    assert_eq!(
        conflicted[0].based_on_revision, entry.based_on_revision,
        "the recorded decision keeps its own basis; the conflict is what the \
         service said, not a new basis for an old decision"
    );

    let next = recorded(decide(&db, project_id, GroupDecision::Reopen, NOW_MS + 4));
    assert_eq!(
        next.based_on_revision, 12,
        "hearing the current revision is how the user's next decision comes to \
         be based on what the service actually holds"
    );
    assert!(
        pending(&db, project_id)[0].conflict.is_none(),
        "a fresh decision is not born conflicted"
    );
}

#[test]
fn a_conflict_for_a_superseded_decision_is_ignored() {
    let (db, project_id) = connected();
    let in_flight = recorded(decide(&db, project_id, GroupDecision::Ignore, NOW_MS + 2));
    decide(&db, project_id, GroupDecision::Reopen, NOW_MS + 3);

    assert!(!db
        .record_mutation_conflict(
            in_flight.id,
            &in_flight.idempotency_key,
            "active",
            12,
            NOW_MS + 4,
        )
        .expect("record conflict"));
    assert!(pending(&db, project_id)[0].conflict.is_none());
}

#[test]
fn every_decision_a_user_can_make_records_one_intent_and_one_lifecycle_row() {
    let (db, project_id) = connected();
    let cases = [
        (GroupDecision::Reopen, IssueStatus::New),
        (GroupDecision::Snooze { until: 9_000 }, IssueStatus::Snoozed),
        (GroupDecision::Ignore, IssueStatus::Ignored),
        (
            GroupDecision::Block {
                reason: Some("intended".to_string()),
            },
            IssueStatus::Blocked,
        ),
        (GroupDecision::ClaimFixed, IssueStatus::Verified),
    ];

    let expected_entries = cases.len();
    for (index, (decision, status)) in cases.into_iter().enumerate() {
        let check_id = format!("security.check-{index}");
        let record = db
            .record_group_decision(
                project_id,
                SITE,
                &check_id,
                decision.clone(),
                NOW_MS + index as i64,
            )
            .expect("record decision");
        let entry = recorded(record);
        assert_eq!(entry.decision, decision, "{decision:?} must round-trip");

        let state = db
            .get_issue_state(project_id, Some(SITE), &check_id)
            .expect("read state")
            .expect("row exists");
        assert_eq!(state.0, status, "{decision:?} writes the local row too");
        assert_eq!(state.0, decision.lifecycle().status());
    }

    assert_eq!(pending(&db, project_id).len(), expected_entries);
}

#[test]
fn a_user_claim_is_recorded_as_a_claim_rather_than_as_proof() {
    let (db, project_id) = connected();
    recorded(decide(
        &db,
        project_id,
        GroupDecision::ClaimFixed,
        NOW_MS + 2,
    ));

    let state = db
        .get_issue_state(project_id, Some(SITE), CHECK)
        .expect("read")
        .expect("row");
    assert_eq!(state.0, IssueStatus::Verified);
    assert_eq!(
        state.3,
        Some(VerifiedBy::UserClaim),
        "nothing looked, so a later scan that still finds the issue returns it \
         to the list instead of announcing a regression"
    );
}

#[test]
fn a_scan_result_is_evidence_and_never_becomes_a_recorded_intent() {
    let (db, project_id) = connected();
    db.set_issue_group_state(
        project_id,
        SITE,
        CHECK,
        IssueLifecycle::Verified {
            by: VerifiedBy::LocalScan,
        },
        NOW_MS + 2,
    )
    .expect("scan verification");

    assert!(
        pending(&db, project_id).is_empty(),
        "evidence is submitted as evidence; the outbox carries decisions"
    );
}

#[test]
fn a_decision_needs_a_canonical_group_and_an_environment() {
    let (db, project_id) = connected();
    let error = db
        .record_group_decision(
            project_id,
            SITE,
            "code_scan.n-plus-one-query:src/db.ts",
            GroupDecision::Ignore,
            NOW_MS + 2,
        )
        .expect_err("a path-bearing identity is not a group");
    assert!(error.to_string().contains("is not canonical"));

    let error = db
        .record_group_decision(project_id, "", CHECK, GroupDecision::Ignore, NOW_MS + 2)
        .expect_err("a decision without an environment names no site");
    assert!(error.to_string().contains("environment is required"));
}

#[test]
fn decisions_are_scoped_to_the_environment_they_were_made_in() {
    let (db, project_id) = connected();
    let staging = "https://staging.example.com";
    db.connect_site(project_id, staging, "site_staging", NOW_MS)
        .expect("connect staging");
    db.mark_site_bootstrapped(project_id, staging, NOW_MS + 1)
        .expect("bootstrap staging");

    decide(&db, project_id, GroupDecision::Ignore, NOW_MS + 2);

    assert_eq!(pending(&db, project_id).len(), 1);
    assert!(
        db.pending_group_mutations(project_id, staging)
            .expect("read staging")
            .is_empty(),
        "one environment's decision is not an instruction about another site"
    );
}
