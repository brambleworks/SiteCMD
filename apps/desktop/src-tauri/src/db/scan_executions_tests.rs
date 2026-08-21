use std::sync::{Arc, Barrier};

use crate::constants::SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS;
use crate::core::normalized_scan::ScanRunKind;
use crate::core::scan_execution::{
    NewScanExecution, ScanAdmissionClass, ScanAdmissionError, ScanComponent, ScanComponentStatus,
    ScanExecutionMode, ScanExecutionStatus, ScanTrigger,
};
use crate::core::scanner::ScanType;
use crate::db::test_helpers::{temp_db, temp_db_arc};

const NOW_MS: i64 = 1_800_000_000_000;

fn request(key: &str, mode: ScanExecutionMode, trigger: ScanTrigger) -> NewScanExecution {
    let (web_status, code_status) = match mode {
        ScanExecutionMode::Full => (
            Some(ScanComponentStatus::Planned),
            Some(ScanComponentStatus::Planned),
        ),
        ScanExecutionMode::Web => (Some(ScanComponentStatus::Planned), None),
        ScanExecutionMode::Code => (None, Some(ScanComponentStatus::Planned)),
    };
    NewScanExecution {
        project_id: None,
        environment_id: None,
        environment_url: Some("https://example.com".into()),
        environment_scope_key: "https://example.com".into(),
        requested_mode: mode,
        web_focus: (mode != ScanExecutionMode::Code).then_some(ScanType::Health),
        trigger,
        admission_class: ScanAdmissionClass::GeneralScan,
        idempotency_key: key.into(),
        request_fingerprint: "v1:fixed-fingerprint".into(),
        now_ms: NOW_MS,
        web_status,
        web_detail: None,
        code_status,
        code_detail: None,
    }
}

#[test]
fn full_execution_admits_and_completes_both_children() {
    let db = temp_db();
    let admitted = db
        .admit_scan_execution(
            request("full-1", ScanExecutionMode::Full, ScanTrigger::Manual),
            SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
        )
        .expect("admit Full execution");

    assert!(!admitted.reused);

    db.start_scan_execution_component(admitted.execution.id, ScanComponent::Web)
        .expect("start Web child");
    let after_web = db
        .finish_scan_execution_component(
            admitted.execution.id,
            ScanComponent::Web,
            ScanComponentStatus::Complete,
            None,
            NOW_MS + 1_000,
        )
        .expect("finish Web child");
    assert_eq!(after_web.status, ScanExecutionStatus::Running);

    db.start_scan_execution_component(admitted.execution.id, ScanComponent::Code)
        .expect("start Code child");
    let finished = db
        .finish_scan_execution_component(
            admitted.execution.id,
            ScanComponent::Code,
            ScanComponentStatus::Complete,
            None,
            NOW_MS + 2_000,
        )
        .expect("finish Code child");
    assert_eq!(finished.status, ScanExecutionStatus::Complete);

    let history = db
        .get_scan_execution_history(None, Some("https://example.com".into()), None, 20)
        .expect("execution history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].requested_mode, ScanExecutionMode::Full);
    assert_eq!(history[0].web_status, Some(ScanComponentStatus::Complete));
    assert_eq!(history[0].code_status, Some(ScanComponentStatus::Complete));
}

fn completed_execution_with_run(
    db: &crate::db::Database,
    key: &str,
    mode: ScanExecutionMode,
    run_kind: ScanRunKind,
    started_at: i64,
) -> i64 {
    let mut admission = request(key, mode, ScanTrigger::Manual);
    admission.now_ms = started_at;
    let admitted = db
        .admit_scan_execution(admission, SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS)
        .expect("admit fixture execution");
    let component = if run_kind == ScanRunKind::Code {
        ScanComponent::Code
    } else {
        ScanComponent::Web
    };
    db.start_scan_execution_component(admitted.execution.id, component)
        .expect("start fixture component");
    let execution_id = admitted.execution.id;
    db.execute(move |conn| {
        let (source, coverage_kind) = match run_kind {
            ScanRunKind::Code => ("code_scan", "project"),
            ScanRunKind::MultiParent => ("web_scan", "page_set"),
            ScanRunKind::Single | ScanRunKind::Page => ("web_scan", "site"),
        };
        conn.execute(
            "INSERT INTO scan_runs (
                execution_id, environment_url, environment_scope_key, source,
                run_kind, status, started_at, completed_at, timestamp_text,
                duration_ms, coverage_kind, coverage_json, diagnostics_json
             ) VALUES (
                :execution_id, 'https://example.com', 'https://example.com',
                :source, :run_kind, 'complete', :started_at, :completed_at,
                '2026-07-21T00:00:00Z', 1, :coverage_kind,
                '{\"successful\":true}', '{}'
             )",
            rusqlite::named_params! {
                ":execution_id": execution_id,
                ":source": source,
                ":run_kind": run_kind.as_str(),
                ":started_at": started_at,
                ":completed_at": started_at + 1,
                ":coverage_kind": coverage_kind,
            },
        )?;
        Ok::<_, crate::db::DbError>(())
    })
    .expect("database worker")
    .expect("insert fixture run");
    db.finish_scan_execution_component(
        execution_id,
        component,
        ScanComponentStatus::Complete,
        None,
        started_at + 1,
    )
    .expect("finish fixture component");
    execution_id
}

fn mark_bounded_verification(db: &crate::db::Database, execution_id: i64) {
    db.execute(move |conn| {
        conn.execute(
            "UPDATE scan_executions
                SET trigger = 'verification', admission_class = 'bounded_verification'
              WHERE id = :execution_id",
            rusqlite::named_params! { ":execution_id": execution_id },
        )
    })
    .expect("database worker")
    .expect("mark bounded verification");
}

#[test]
fn history_run_kind_filter_is_applied_before_limit() {
    let db = temp_db();
    let session_execution = completed_execution_with_run(
        &db,
        "history-session",
        ScanExecutionMode::Web,
        ScanRunKind::MultiParent,
        NOW_MS + 100,
    );
    let web_execution = completed_execution_with_run(
        &db,
        "history-web",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 200,
    );
    let code_execution = completed_execution_with_run(
        &db,
        "history-code",
        ScanExecutionMode::Code,
        ScanRunKind::Code,
        NOW_MS + 300,
    );

    let latest_web = db
        .get_scan_execution_history(
            None,
            Some("https://example.com".into()),
            Some(ScanRunKind::Single),
            1,
        )
        .expect("latest Web history");
    let latest_code = db
        .get_scan_execution_history(
            None,
            Some("https://example.com".into()),
            Some(ScanRunKind::Code),
            1,
        )
        .expect("latest Code history");
    let latest_session = db
        .get_scan_execution_history(
            None,
            Some("https://example.com".into()),
            Some(ScanRunKind::MultiParent),
            1,
        )
        .expect("latest session history");

    assert_eq!(latest_web[0].id, web_execution);
    assert_eq!(latest_code[0].id, code_execution);
    assert_eq!(latest_session[0].id, session_execution);
}

#[test]
fn bounded_verification_is_excluded_before_the_history_limit() {
    let db = temp_db();
    let ordinary_execution = completed_execution_with_run(
        &db,
        "history-ordinary-web",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 100,
    );
    let verification_execution = completed_execution_with_run(
        &db,
        "history-bounded-verification",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 200,
    );
    mark_bounded_verification(&db, verification_execution);

    let latest_web = db
        .get_scan_execution_history(
            None,
            Some("https://example.com".into()),
            Some(ScanRunKind::Single),
            1,
        )
        .expect("latest ordinary Web history");

    assert_eq!(latest_web.len(), 1);
    assert_eq!(latest_web[0].id, ordinary_execution);
}

#[test]
fn bounded_verifications_have_a_separate_retention_window() {
    let db = temp_db();
    let old_visible = completed_execution_with_run(
        &db,
        "retention-visible-old",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 100,
    );
    let old_bounded = completed_execution_with_run(
        &db,
        "retention-bounded-old",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 200,
    );
    mark_bounded_verification(&db, old_bounded);
    let new_visible = completed_execution_with_run(
        &db,
        "retention-visible-new",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 300,
    );
    let new_bounded = completed_execution_with_run(
        &db,
        "retention-bounded-new",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 400,
    );
    mark_bounded_verification(&db, new_bounded);

    let pruned = db
        .prune_scan_executions_for_scope(
            None,
            "https://example.com",
            1,
            crate::db::ScanRetentionWindow::All,
        )
        .expect("prune separate retention windows");

    assert_eq!(pruned, 2, "one execution is pruned from each window");
    assert!(db.get_scan_execution(old_visible).unwrap().is_none());
    assert!(db.get_scan_execution(old_bounded).unwrap().is_none());
    assert!(db.get_scan_execution(new_visible).unwrap().is_some());
    assert!(db.get_scan_execution(new_bounded).unwrap().is_some());

    let history = db
        .get_scan_execution_history(
            None,
            Some("https://example.com".into()),
            Some(ScanRunKind::Single),
            10,
        )
        .expect("visible history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, new_visible);
}

#[test]
fn bounded_verification_cleanup_never_shortens_visible_history() {
    let db = temp_db();
    let old_visible = completed_execution_with_run(
        &db,
        "bounded-only-visible-old",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 100,
    );
    let old_bounded = completed_execution_with_run(
        &db,
        "bounded-only-hidden-old",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 200,
    );
    mark_bounded_verification(&db, old_bounded);
    let new_visible = completed_execution_with_run(
        &db,
        "bounded-only-visible-new",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 300,
    );
    let new_bounded = completed_execution_with_run(
        &db,
        "bounded-only-hidden-new",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 400,
    );
    mark_bounded_verification(&db, new_bounded);

    let pruned = db
        .prune_scan_executions_for_scope(
            None,
            "https://example.com",
            1,
            crate::db::ScanRetentionWindow::BoundedVerification,
        )
        .expect("prune only bounded verifications");

    assert_eq!(pruned, 1);
    assert!(db.get_scan_execution(old_visible).unwrap().is_some());
    assert!(db.get_scan_execution(new_visible).unwrap().is_some());
    assert!(db.get_scan_execution(old_bounded).unwrap().is_none());
    assert!(db.get_scan_execution(new_bounded).unwrap().is_some());
}

#[test]
fn no_action_is_blocked_by_a_daily_allowance() {
    let db = temp_db();
    for (index, mode, trigger) in [
        (1, ScanExecutionMode::Web, ScanTrigger::Manual),
        (2, ScanExecutionMode::Code, ScanTrigger::Manual),
        (3, ScanExecutionMode::Full, ScanTrigger::Scheduled),
        (4, ScanExecutionMode::Code, ScanTrigger::Scheduled),
    ] {
        db.admit_scan_execution(
            request(&format!("action-{index}"), mode, trigger),
            SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
        )
        .expect("every action admits");
    }
}

#[test]
fn admission_class_not_trigger_controls_verification_exemption() {
    let db = temp_db();
    let mut bounded = request("bounded", ScanExecutionMode::Web, ScanTrigger::Verification);
    bounded.admission_class = ScanAdmissionClass::BoundedVerification;
    let bounded = db
        .admit_scan_execution(bounded, SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS)
        .expect("bounded verification");
    assert!(!bounded.reused);

    db.admit_scan_execution(
        request(
            "full-project-verify",
            ScanExecutionMode::Code,
            ScanTrigger::Verification,
        ),
        SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
    )
    .expect("full project verification");
    // The admission-class distinction survives for verification semantics,
    // not for a meter: no usage ledger exists to consult at all.
}

#[test]
fn idempotency_is_bound_to_fingerprint_and_terminal_retry_window() {
    let db = temp_db();
    let original_request = request("retry-key", ScanExecutionMode::Web, ScanTrigger::Manual);
    let admitted = db
        .admit_scan_execution(original_request.clone(), SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS)
        .expect("initial admission");
    let planned_retry = db
        .admit_scan_execution(original_request.clone(), SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS)
        .expect("planned retry");
    assert!(planned_retry.reused);
    assert_eq!(planned_retry.execution.id, admitted.execution.id);

    let mut conflicting = original_request.clone();
    conflicting.request_fingerprint = "v1:different".into();
    assert_eq!(
        db.admit_scan_execution(conflicting, SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,)
            .expect_err("fingerprint mismatch"),
        ScanAdmissionError::IdempotencyConflict
    );

    db.start_scan_execution_component(admitted.execution.id, ScanComponent::Web)
        .expect("start");
    let completed_at = NOW_MS + 5_000;
    db.finish_scan_execution_component(
        admitted.execution.id,
        ScanComponent::Web,
        ScanComponentStatus::Complete,
        None,
        completed_at,
    )
    .expect("finish");

    let mut boundary = original_request.clone();
    boundary.now_ms = completed_at + SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS * 1_000;
    assert!(
        db.admit_scan_execution(boundary, SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,)
            .expect("boundary retry")
            .reused
    );

    let mut stale = original_request;
    stale.now_ms = completed_at + SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS * 1_000 + 1;
    assert_eq!(
        db.admit_scan_execution(stale, SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,)
            .expect_err("stale retry"),
        ScanAdmissionError::IdempotencyStale
    );
}

#[test]
fn fresh_key_for_identical_request_admits() {
    let db = temp_db();
    for key in ["fresh-a", "fresh-b"] {
        db.admit_scan_execution(
            request(key, ScanExecutionMode::Web, ScanTrigger::Manual),
            SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
        )
        .expect("fresh action");
    }
}

#[test]
fn preflight_failure_fails_the_execution() {
    let db = temp_db();
    let admitted = db
        .admit_scan_execution(
            request("release", ScanExecutionMode::Web, ScanTrigger::Manual),
            SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
        )
        .expect("admit");
    db.release_scan_execution_before_start(
        admitted.execution.id,
        "preflight failed".into(),
        NOW_MS + 1,
    )
    .expect("release");
    let stored = db
        .get_scan_execution(admitted.execution.id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, ScanExecutionStatus::Failed);
}

#[test]
fn pre_start_cancellation_records_cancelled_children() {
    let db = temp_db();
    let admitted = db
        .admit_scan_execution(
            request(
                "cancel-before-start",
                ScanExecutionMode::Full,
                ScanTrigger::Manual,
            ),
            SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
        )
        .expect("admit");

    let cancelled = db
        .cancel_scan_execution_before_start(
            admitted.execution.id,
            "cancelled_by_user".into(),
            NOW_MS + 1,
        )
        .expect("cancel planned execution");

    assert_eq!(cancelled.status, ScanExecutionStatus::Cancelled);
    assert_eq!(cancelled.web_status, Some(ScanComponentStatus::Cancelled));
    assert_eq!(cancelled.code_status, Some(ScanComponentStatus::Cancelled));
}

#[test]
fn execution_retention_is_cross_source_and_detaches_active_old_evidence() {
    let db = temp_db();
    let project_id = db
        .upsert_project("retention", "/tmp/retention", None)
        .expect("project");
    let scope = "https://example.com";
    let old_web = completed_execution_with_run(
        &db,
        "retention-web",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 100,
    );
    let new_code = completed_execution_with_run(
        &db,
        "retention-code",
        ScanExecutionMode::Code,
        ScanRunKind::Code,
        NOW_MS + 200,
    );
    let other_scope = completed_execution_with_run(
        &db,
        "retention-other-scope",
        ScanExecutionMode::Web,
        ScanRunKind::Single,
        NOW_MS + 300,
    );
    let scope_owned = scope.to_string();
    let other_scope_key = "https://other.example.com".to_string();
    let old_run = db
        .execute(move |conn| {
            for execution_id in [old_web, new_code] {
                conn.execute(
                    "UPDATE scan_executions
                     SET project_id = :project_id, environment_scope_key = :scope
                     WHERE id = :execution_id",
                    rusqlite::named_params! {
                        ":project_id": project_id,
                        ":scope": scope_owned,
                        ":execution_id": execution_id,
                    },
                )?;
                conn.execute(
                    "UPDATE scan_runs
                     SET project_id = :project_id, environment_scope_key = :scope
                     WHERE execution_id = :execution_id",
                    rusqlite::named_params! {
                        ":project_id": project_id,
                        ":scope": scope_owned,
                        ":execution_id": execution_id,
                    },
                )?;
            }
            conn.execute(
                "UPDATE scan_executions
                 SET project_id = :project_id, environment_scope_key = :scope
                 WHERE id = :execution_id",
                rusqlite::named_params! {
                    ":project_id": project_id,
                    ":scope": other_scope_key,
                    ":execution_id": other_scope,
                },
            )?;
            let old_run: i64 = conn.query_row(
                "SELECT id FROM scan_runs WHERE execution_id = :execution_id",
                rusqlite::named_params! { ":execution_id": old_web },
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO work_items (
                    project_id, env_url, source, signal_id, check_id, category,
                    severity, title, description, scan_ref, first_seen_scan_ref,
                    first_seen_at, last_seen_at
                 ) VALUES (
                    :project_id, :scope, 'web_scan', 'web_scan:active-old',
                    'security.active-old', 'security', 'high', 'Active',
                    'Still active', :scan_ref, :scan_ref, 1, 2
                 )",
                rusqlite::named_params! {
                    ":project_id": project_id,
                    ":scope": scope_owned,
                    ":scan_ref": old_run,
                },
            )?;
            Ok::<_, crate::db::DbError>(old_run)
        })
        .expect("database worker")
        .expect("seed scoped history");

    let pruned = db
        .prune_scan_executions_for_scope(
            Some(project_id),
            scope,
            1,
            crate::db::ScanRetentionWindow::All,
        )
        .expect("prune execution scope");
    assert_eq!(pruned, 1, "Web and Code share one retention window");
    assert!(db.get_scan_execution(old_web).unwrap().is_none());
    assert!(db.get_scan_execution(new_code).unwrap().is_some());
    assert!(db.get_scan_execution(other_scope).unwrap().is_some());

    let (item_count, scan_ref, first_seen_scan_ref): (i64, Option<i64>, Option<i64>) = db
        .execute(move |conn| {
            conn.query_row(
                "SELECT COUNT(*), MAX(scan_ref), MAX(first_seen_scan_ref)
                 FROM work_items
                 WHERE signal_id = 'web_scan:active-old'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
        })
        .expect("database worker")
        .expect("load active item");
    assert_eq!(item_count, 1, "active issue projection must survive");
    assert_eq!(scan_ref, None, "expired current evidence is detached");
    assert_eq!(
        first_seen_scan_ref, None,
        "expired first-seen evidence is detached"
    );
    let old_run_exists: i64 = db
        .execute(move |conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM scan_runs WHERE id = :run_id",
                rusqlite::named_params! { ":run_id": old_run },
                |row| row.get(0),
            )
        })
        .expect("database worker")
        .expect("count old run");
    assert_eq!(old_run_exists, 0);
}

#[test]
fn concurrent_admissions_all_succeed_with_no_slot_to_race_for() {
    // The retired daily limit made this a race for the last slot; with no
    // meter there is nothing to claim and both racers admit.
    let harness = temp_db_arc();
    let barrier = Arc::new(Barrier::new(3));
    let handles = ["racer-a", "racer-b"].map(|key| {
        let db = harness.db.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            db.admit_scan_execution(
                request(key, ScanExecutionMode::Code, ScanTrigger::Manual),
                SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
            )
        })
    });
    barrier.wait();
    let outcomes = handles.map(|handle| handle.join().expect("admission thread"));
    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 2);
}
