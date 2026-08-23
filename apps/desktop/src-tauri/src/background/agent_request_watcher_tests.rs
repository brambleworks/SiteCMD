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
