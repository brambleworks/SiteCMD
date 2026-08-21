//! Connected producer sequencing tests.

use crate::constants::SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS;
use crate::core::scan_execution::{
    NewScanExecution, ScanAdmissionClass, ScanComponentStatus, ScanExecutionMode, ScanTrigger,
};
use crate::core::scanner::ScanType;
use crate::db::test_helpers::{temp_db, TestDb};

const NOW_MS: i64 = 1_800_000_000_000;
const SITE: &str = "https://example.com";

fn web_execution(key: &str, project_id: Option<i64>, scope_key: &str) -> NewScanExecution {
    NewScanExecution {
        project_id,
        environment_id: None,
        environment_url: Some(SITE.into()),
        environment_scope_key: scope_key.into(),
        requested_mode: ScanExecutionMode::Web,
        web_focus: Some(ScanType::Health),
        trigger: ScanTrigger::Manual,
        admission_class: ScanAdmissionClass::GeneralScan,
        idempotency_key: key.into(),
        request_fingerprint: "v1:fixed-fingerprint".into(),
        now_ms: NOW_MS,
        web_status: Some(ScanComponentStatus::Planned),
        web_detail: None,
        code_status: None,
        code_detail: None,
    }
}

fn admit(db: &TestDb, key: &str, project_id: Option<i64>, scope_key: &str) -> i64 {
    db.admit_scan_execution(
        web_execution(key, project_id, scope_key),
        SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
    )
    .expect("admit execution")
    .execution
    .id
}

fn seeded_project(db: &TestDb) -> i64 {
    db.upsert_project("watermark-fixture", "", None)
        .expect("seed project")
}

#[test]
fn an_installation_that_never_submits_never_mints_an_identifier() {
    let db = temp_db();
    assert_eq!(db.get_producer_identity().expect("identity"), None);
}

#[test]
fn submission_numbers_are_allocated_strictly_increasing_under_one_identity() {
    let db = temp_db();
    let first = db.allocate_submission_sequence(NOW_MS).expect("first");
    let second = db.allocate_submission_sequence(NOW_MS + 1).expect("second");

    assert_eq!(first.sequence(), 1);
    assert_eq!(second.sequence(), 2);
    assert_eq!(
        first.installation_id(),
        second.installation_id(),
        "one installation orders itself under one identity"
    );
    assert!(
        first.installation_id().starts_with("inst_"),
        "the identity reads as what it is: {}",
        first.installation_id()
    );
}

#[test]
fn the_counter_survives_a_restart_rather_than_restarting_the_namespace() {
    // A reused number puts two different payloads under one sequence, which
    // the service reads as a replay of the first.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("producer.db");

    let (identity, first) = {
        let db = crate::db::Database::open(path.clone()).expect("open");
        let first = db.allocate_submission_sequence(NOW_MS).expect("allocate");
        db.allocate_submission_sequence(NOW_MS + 1).expect("second");
        (first.installation_id().to_string(), first.sequence())
    };

    let db = crate::db::Database::open(path).expect("reopen");
    let after_restart = db.allocate_submission_sequence(NOW_MS + 2).expect("third");
    assert_eq!(first, 1);
    assert_eq!(after_restart.sequence(), 3);
    assert_eq!(
        after_restart.installation_id(),
        identity,
        "a restart is not a new installation"
    );
}

#[test]
fn reading_the_producer_state_does_not_consume_a_submission_number() {
    let db = temp_db();
    db.allocate_submission_sequence(NOW_MS).expect("allocate");

    let state = db
        .get_producer_identity()
        .expect("identity")
        .expect("minted");
    assert_eq!(state.last_submission_sequence, 1);
    assert_eq!(state.minted_at, NOW_MS);

    assert_eq!(
        db.get_producer_identity()
            .expect("identity")
            .expect("minted")
            .last_submission_sequence,
        1,
        "reading twice must not advance the counter"
    );
    assert_eq!(
        db.allocate_submission_sequence(NOW_MS + 1)
            .expect("allocate")
            .sequence(),
        2,
        "the next real submission takes the next number, not a skipped one"
    );
}

#[test]
fn a_site_with_no_pull_reads_as_the_genesis_watermark() {
    let db = temp_db();
    let project_id = seeded_project(&db);
    let execution_id = admit(&db, "genesis-1", Some(project_id), SITE);

    assert_eq!(
        db.get_execution_event_basis(execution_id).expect("basis"),
        Some(0),
        "before the first pull there is no site event for the shield to engage against"
    );
}

#[test]
fn a_scan_declares_the_watermark_that_was_in_force_when_it_started() {
    let db = temp_db();
    let project_id = seeded_project(&db);
    db.record_pulled_event_sequence(project_id, SITE, 41, NOW_MS)
        .expect("pull");

    let execution_id = admit(&db, "basis-1", Some(project_id), SITE);
    assert_eq!(
        db.get_execution_event_basis(execution_id).expect("basis"),
        Some(41)
    );
}

#[test]
fn a_pull_during_a_scan_does_not_raise_the_basis_of_evidence_gathered_before_it() {
    let db = temp_db();
    let project_id = seeded_project(&db);
    db.record_pulled_event_sequence(project_id, SITE, 7, NOW_MS)
        .expect("pull");

    let execution_id = admit(&db, "mid-scan-1", Some(project_id), SITE);
    db.record_pulled_event_sequence(project_id, SITE, 99, NOW_MS + 5_000)
        .expect("pull mid scan");

    assert_eq!(
        db.get_execution_event_basis(execution_id).expect("basis"),
        Some(7),
        "the basis is what the producer knew when it started looking"
    );
    assert_eq!(
        db.get_execution_event_basis(admit(&db, "mid-scan-2", Some(project_id), SITE))
            .expect("basis"),
        Some(99),
        "the next scan starts from what it now knows"
    );
}

#[test]
fn the_watermark_never_moves_backwards() {
    // A reordered or replayed read must not lower what this installation has
    // genuinely seen.
    let db = temp_db();
    let project_id = seeded_project(&db);

    assert_eq!(
        db.record_pulled_event_sequence(project_id, SITE, 12, NOW_MS)
            .expect("pull"),
        12
    );
    assert_eq!(
        db.record_pulled_event_sequence(project_id, SITE, 4, NOW_MS + 1)
            .expect("older pull"),
        12
    );
    assert_eq!(
        db.record_pulled_event_sequence(project_id, SITE, 13, NOW_MS + 2)
            .expect("newer pull"),
        13
    );
    assert!(db
        .record_pulled_event_sequence(project_id, SITE, -1, NOW_MS + 3)
        .is_err());
}

#[test]
fn watermarks_are_per_site_and_keyed_the_way_lifecycle_rows_are() {
    let db = temp_db();
    let project_id = seeded_project(&db);
    db.record_pulled_event_sequence(project_id, "HTTPS://Example.com/", 30, NOW_MS)
        .expect("pull");

    assert_eq!(
        db.get_execution_event_basis(admit(&db, "key-1", Some(project_id), SITE))
            .expect("basis"),
        Some(30),
        "the same site under a differently written URL is the same site"
    );
    assert_eq!(
        db.get_execution_event_basis(admit(
            &db,
            "key-2",
            Some(project_id),
            "https://staging.example.com"
        ))
        .expect("basis"),
        Some(0),
        "another environment has its own basis"
    );
}

#[test]
fn a_code_only_projects_scope_key_is_not_run_through_url_normalization() {
    let db = temp_db();
    let project_id = seeded_project(&db);
    let scope_key = format!("project:{project_id}");
    db.record_pulled_event_sequence(project_id, &scope_key, 5, NOW_MS)
        .expect("pull");

    assert_eq!(
        db.get_execution_event_basis(admit(&db, "code-1", Some(project_id), &scope_key))
            .expect("basis"),
        Some(5)
    );
}

#[test]
fn an_unattached_scan_has_no_site_and_therefore_no_basis_to_declare() {
    let db = temp_db();
    let execution_id = admit(&db, "adhoc-1", None, SITE);
    assert_eq!(
        db.get_execution_event_basis(execution_id).expect("basis"),
        Some(0)
    );
}
