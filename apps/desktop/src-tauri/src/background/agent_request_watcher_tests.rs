//! Tests for `agent_request_watcher`.

use super::*;
use crate::db::test_helpers::{insert_test_work_item, temp_db};

fn project_with_issue(db: &crate::db::Database, check_id: &str) -> i64 {
    let project_id = db
        .upsert_project("Agent Loop", "/tmp/agent-loop", Some("astro"))
        .expect("upsert");
    db.add_environment(
        project_id,
        "https://example.com",
        "Production",
        "production",
        "test",
    )
    .expect("environment");
    insert_test_work_item(db, project_id, "https://example.com", check_id).expect("work item");
    project_id
}

#[test]
fn start_fix_request_creates_a_briefed_attempt_through_the_shared_path() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    let request_id = db
        .insert_agent_request(
            "start_fix",
            project_id,
            "https://example.com",
            Some("security.csp"),
            None,
            "claude-code",
            1_000,
        )
        .expect("queue request");

    let request = db
        .list_agent_requests_in_status("requested")
        .expect("list")
        .remove(0);
    let result = fulfil_start_fix(&db, &request, 2_000).expect("fulfilled");
    let payload: serde_json::Value = serde_json::from_str(&result).expect("json");
    let attempt_id = payload["attempt_id"].as_i64().expect("attempt id");
    assert_eq!(payload["status"], "briefed");

    let attempt = db.get_fix_attempt(attempt_id).expect("get").expect("row");
    assert_eq!(attempt.check_id, "security.csp");
    assert_eq!(attempt.agent_tool, "claude-code");
    assert!(attempt.brief_md.contains("## Acceptance criteria"));
    assert_eq!(request.id, request_id);
}

#[test]
fn start_fix_request_without_an_open_issue_fails_with_detail() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Agent Loop", "/tmp/agent-loop", Some("astro"))
        .expect("upsert");
    db.insert_agent_request(
        "start_fix",
        project_id,
        "https://example.com",
        Some("security.csp"),
        None,
        "cursor",
        1_000,
    )
    .expect("queue request");
    let request = db
        .list_agent_requests_in_status("requested")
        .expect("list")
        .remove(0);
    let error = fulfil_start_fix(&db, &request, 2_000).expect_err("no open issue");
    assert!(error.contains("no open issue security.csp"), "{error}");
}

#[test]
fn scan_scope_maps_to_the_execution_plan_the_scheduler_uses() {
    assert_eq!(
        scan_plan_for_scope("web").expect("web"),
        (ScanExecutionMode::Web, Some(ScanType::Health))
    );
    assert_eq!(
        scan_plan_for_scope("code").expect("code"),
        (ScanExecutionMode::Code, None)
    );
    assert_eq!(
        scan_plan_for_scope("full").expect("full"),
        (ScanExecutionMode::Full, Some(ScanType::Health))
    );
    assert!(scan_plan_for_scope("everything").is_err());
}

#[test]
fn stale_requests_expire_and_claimed_requests_cannot_be_claimed_twice() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    let old = db
        .insert_agent_request(
            "run_scan",
            project_id,
            "https://example.com",
            None,
            Some("web"),
            "codex",
            1_000,
        )
        .expect("old request");
    let fresh = db
        .insert_agent_request(
            "run_scan",
            project_id,
            "https://example.com",
            None,
            Some("web"),
            "codex",
            50_000,
        )
        .expect("fresh request");
    assert_eq!(
        db.expire_stale_agent_requests(10_000, 60_000)
            .expect("expire"),
        1
    );
    assert!(db.claim_agent_request(fresh, 60_001).expect("claim"));
    assert!(!db.claim_agent_request(fresh, 60_002).expect("second claim"));
    assert!(!db.claim_agent_request(old, 60_003).expect("expired claim"));
}

fn queue_scan(db: &crate::db::Database, project_id: i64, now: i64) -> i64 {
    db.insert_agent_request(
        "run_scan",
        project_id,
        "https://example.com",
        None,
        Some("web"),
        "codex",
        now,
    )
    .expect("queue scan")
}

fn row(db: &crate::db::Database, status: &str, id: i64) -> Option<AgentRequestRow> {
    db.list_agent_requests_in_status(status)
        .expect("list")
        .into_iter()
        .find(|request| request.id == id)
}

#[test]
fn only_one_queued_scan_is_claimed_per_tick() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    let queued: Vec<i64> = (0..3)
        .map(|n| queue_scan(&db, project_id, 1_000 + n))
        .collect();

    let requests = db.list_agent_requests_in_status("requested").expect("list");
    let claimed = claim_due_requests(&db, requests, 2_000);

    assert_eq!(
        claimed.len(),
        1,
        "a backlog must not start every scan at once"
    );
    assert_eq!(claimed[0].id, queued[0], "the oldest scan goes first");
    assert_eq!(
        db.list_agent_requests_in_status("running")
            .expect("running")
            .len(),
        1
    );
    assert_eq!(
        db.list_agent_requests_in_status("requested")
            .expect("requested")
            .len(),
        2,
        "the rest stay queued for later ticks"
    );
}

#[test]
fn the_one_scan_limit_does_not_hold_back_fix_requests() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    queue_scan(&db, project_id, 1_000);
    queue_scan(&db, project_id, 1_001);
    for now in [1_002, 1_003] {
        db.insert_agent_request(
            "start_fix",
            project_id,
            "https://example.com",
            Some("security.csp"),
            None,
            "claude-code",
            now,
        )
        .expect("queue fix");
    }

    let requests = db.list_agent_requests_in_status("requested").expect("list");
    let claimed = claim_due_requests(&db, requests, 2_000);

    assert_eq!(claimed.len(), 3);
    assert_eq!(
        claimed.iter().filter(|r| r.kind == "run_scan").count(),
        1,
        "one scan"
    );
    assert_eq!(
        claimed.iter().filter(|r| r.kind == "start_fix").count(),
        2,
        "every fix request"
    );
}

#[test]
fn settling_writes_a_terminal_row_that_a_second_settle_cannot_rewrite() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    let fulfilled = queue_scan(&db, project_id, 1_000);
    let failed = queue_scan(&db, project_id, 1_001);
    assert!(db.claim_agent_request(fulfilled, 2_000).expect("claim"));
    assert!(db.claim_agent_request(failed, 2_000).expect("claim"));

    settle(&db, fulfilled, Ok(r#"{"execution_id":7}"#.to_string()));
    settle(&db, failed, Err("scan_admission_refused".to_string()));

    let settled = row(&db, "fulfilled", fulfilled).expect("fulfilled row");
    assert_eq!(
        settled.result_json.as_deref(),
        Some(r#"{"execution_id":7}"#)
    );
    assert!(settled.failure_detail.is_none());
    let broken = row(&db, "failed", failed).expect("failed row");
    assert_eq!(
        broken.failure_detail.as_deref(),
        Some("scan_admission_refused")
    );
    assert!(broken.result_json.is_none());

    settle(&db, fulfilled, Err("late failure".to_string()));
    settle(&db, failed, Ok(r#"{"execution_id":9}"#.to_string()));
    assert_eq!(
        row(&db, "fulfilled", fulfilled)
            .expect("still fulfilled")
            .result_json
            .as_deref(),
        Some(r#"{"execution_id":7}"#)
    );
    assert_eq!(
        row(&db, "failed", failed)
            .expect("still failed")
            .failure_detail
            .as_deref(),
        Some("scan_admission_refused")
    );
}

#[test]
fn a_failure_detail_is_sanitized_before_it_is_stored() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    let request_id = queue_scan(&db, project_id, 1_000);
    assert!(db.claim_agent_request(request_id, 2_000).expect("claim"));

    settle(
        &db,
        request_id,
        Err("could not read /Users/dev/projects/site/astro.config.mjs".to_string()),
    );

    let detail = row(&db, "failed", request_id)
        .expect("failed row")
        .failure_detail
        .expect("detail");
    assert!(!detail.contains("/Users/dev"), "{detail}");
    assert!(detail.contains("[internal path]"), "{detail}");
}

#[test]
fn an_unknown_agent_tool_fails_the_request_instead_of_briefing_claude_code() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    db.insert_agent_request(
        "start_fix",
        project_id,
        "https://example.com",
        Some("security.csp"),
        None,
        "emacs-agent",
        1_000,
    )
    .expect("queue request");
    let request = db
        .list_agent_requests_in_status("requested")
        .expect("list")
        .remove(0);

    let error = fulfil_start_fix(&db, &request, 2_000).expect_err("unsupported tool");
    assert_eq!(error, "unknown_agent_tool");
    assert!(
        db.get_latest_fix_attempt(project_id, "https://example.com", "security.csp")
            .expect("lookup")
            .is_none(),
        "no attempt may be created for an unsupported agent"
    );
}

#[test]
fn queued_scans_run_one_at_a_time_across_ticks() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    let queued: Vec<i64> = (0..3)
        .map(|n| queue_scan(&db, project_id, 1_000 + n))
        .collect();

    for (index, expected) in queued.iter().enumerate() {
        let now = 2_000 + (index as i64) * 100;
        let requests = db.list_agent_requests_in_status("requested").expect("list");
        let claimed = claim_due_requests(&db, requests, now);
        assert_eq!(claimed.len(), 1, "tick {index} took more than one scan");
        assert_eq!(claimed[0].id, *expected, "scans run oldest first");
        assert_eq!(
            db.list_agent_requests_in_status("requested")
                .expect("requested")
                .len(),
            queued.len() - index - 1,
            "the rest of the backlog stays queued"
        );

        let requests = db.list_agent_requests_in_status("requested").expect("list");
        assert!(
            claim_due_requests(&db, requests, now + 10).is_empty(),
            "a scan still running from tick {index} must block the next tick"
        );

        settle(&db, *expected, Ok(r#"{"execution_id":1}"#.to_string()));
        assert!(!db.has_running_scan().expect("probe"));
    }

    assert_eq!(
        db.list_agent_requests_in_status("fulfilled")
            .expect("fulfilled")
            .len(),
        3
    );
    assert!(db
        .list_agent_requests_in_status("requested")
        .expect("requested")
        .is_empty());
    assert!(db
        .list_agent_requests_in_status("running")
        .expect("running")
        .is_empty());
}

#[test]
fn a_restart_fails_the_requests_the_previous_process_had_claimed() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    let abandoned = queue_scan(&db, project_id, 1_000);
    let waiting = queue_scan(&db, project_id, 1_001);
    assert!(db.claim_agent_request(abandoned, 2_000).expect("claim"));
    assert!(db.has_running_scan().expect("probe"));

    reconcile_orphaned_requests(&db);

    let failed = row(&db, "failed", abandoned).expect("failed row");
    assert_eq!(
        failed.failure_detail.as_deref(),
        Some(ORPHANED_REQUEST_DETAIL)
    );
    assert!(
        row(&db, "requested", waiting).is_some(),
        "queued work the old process never claimed is untouched"
    );
    assert!(
        !db.has_running_scan().expect("probe"),
        "a fresh watcher starts with no scan in flight"
    );

    let requests = db.list_agent_requests_in_status("requested").expect("list");
    let claimed = claim_due_requests(&db, requests, 3_000);
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, waiting);
}

#[test]
fn a_running_fix_request_does_not_block_a_queued_scan() {
    let db = temp_db();
    let project_id = project_with_issue(&db, "security.csp");
    let fix = db
        .insert_agent_request(
            "start_fix",
            project_id,
            "https://example.com",
            Some("security.csp"),
            None,
            "claude-code",
            1_000,
        )
        .expect("queue fix");
    let scan = queue_scan(&db, project_id, 1_001);
    assert!(db.claim_agent_request(fix, 2_000).expect("claim"));
    assert!(!db.has_running_scan().expect("probe"));

    let requests = db.list_agent_requests_in_status("requested").expect("list");
    let claimed = claim_due_requests(&db, requests, 2_100);

    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, scan);
}
