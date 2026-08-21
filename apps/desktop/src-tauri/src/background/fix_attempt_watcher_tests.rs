//! Tests for `fix_attempt_watcher`.

use super::*;
use crate::core::types_work_items::{IssueStatus, VerifiedBy};
use crate::db::test_helpers::{insert_test_work_item, insert_test_work_item_at, temp_db};
use crate::db::FixAttemptTarget;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| s.to_string()).collect()
}

fn attempt_row(check_id: &str, env_url: &str) -> FixAttemptRow {
    let check_id = check_id.to_string();
    let producer_rule = crate::core::code_scan::code_rule_id(&check_id).map(str::to_string);
    FixAttemptRow {
        id: 1,
        project_id: 1,
        env_url: env_url.to_string(),
        check_id,
        producer_rule,
        target_kind: "group".to_string(),
        target_relative_path: None,
        target_line: None,
        agent_tool: "claude-code".to_string(),
        status: "verifying".to_string(),
        brief_md: String::new(),
        agent_summary: None,
        failure_detail: None,
        verify_started_at: None,
        brief_fetched_at: None,
        created_at: 0,
        updated_at: 0,
    }
}

fn occurrence_attempt_row(check_id: &str, env_url: &str, path: &str) -> FixAttemptRow {
    let mut row = attempt_row(check_id, env_url);
    row.target_kind = "occurrence".to_string();
    row.target_relative_path = Some(path.to_string());
    row
}

// Failure detail must distinguish undeployed remote fixes from local recheck failures.
#[test]
fn still_failing_detail_names_the_recheck_that_failed() {
    let remote_web = still_failing_detail(&attempt_row(
        "compliance.accessibility_statement",
        "https://sitecmd.com",
    ));
    assert!(
        remote_web.contains("not live until you deploy"),
        "{remote_web}"
    );
    assert!(remote_web.contains("https://sitecmd.com"), "{remote_web}");

    let local_web = still_failing_detail(&attempt_row("security.csp", "http://localhost:4321"));
    assert_eq!(local_web, STILL_FAILING_DETAIL);

    let code_check = still_failing_detail(&occurrence_attempt_row(
        "code_scan.hardcoded-secret",
        "https://sitecmd.com",
        "src/env.ts",
    ));
    assert_eq!(
        code_check,
        "SiteCMD re-ran the code scan after the agent finished and src/env.ts \
         still fails the hardcoded-secret check."
    );
}

#[test]
fn verify_requested_attempt_with_resolved_issue_reaches_verified_state() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Fix Loop", "/tmp/fix-loop", Some("astro"))
        .expect("upsert");
    let id = db
        .create_fix_attempt(
            project_id,
            "https://example.com",
            "security.csp",
            "claude-code",
            1_000,
        )
        .expect("create attempt");

    // Agent callback, exactly as the MCP server writes it.
    db.set_fix_attempt_status(
        id,
        "verify_requested",
        Some("Added the header"),
        None,
        2_000,
    )
    .expect("agent requests verification");

    // The watcher's pickup query (tick step 2) must see the attempt.
    let picked_up = db
        .list_fix_attempts_in_status(&["verify_requested"])
        .expect("pickup query");
    assert!(
        picked_up.iter().any(|row| row.id == id),
        "the watcher's pickup query must return the agent-flipped attempt"
    );

    // start_verification moves the attempt to verifying.
    db.set_fix_attempt_status(id, "verifying", None, None, 2_500)
        .expect("start verifying");

    // settle_attempt: no work_items were seeded, so the issue is resolved
    // and the policy verifies it.
    let still_active = db
        .is_fix_attempt_target_active(
            project_id,
            "https://example.com",
            "security.csp",
            "group",
            None,
            None,
        )
        .expect("active query");
    assert!(!still_active, "no unresolved work item means resolved");
    assert_eq!(
        decide_outcome(still_active, true, false, 2_500, 3_000),
        Some(Outcome::Verified)
    );

    // evaluate_attempt wires is_issue_active + get_active_issue_sources
    // into decide_outcome; with the issue resolved it must verify.
    let verifying = db.get_fix_attempt(id).expect("get").expect("row exists");
    assert_eq!(verifying.status, "verifying");
    assert_eq!(
        evaluate_attempt(&db, &verifying, 3_000),
        Ok(Some(Outcome::Verified)),
        "the wired evaluation must verify a resolved issue"
    );

    // The Verified arm's writes, as settle_attempt applies them.
    db.set_fix_attempt_status(id, "verified", None, None, 3_000)
        .expect("mark verified");
    db.set_issue_group_state(
        project_id,
        "https://example.com",
        "security.csp",
        IssueLifecycle::Verified {
            by: VerifiedBy::LocalScan,
        },
        3_000,
    )
    .expect("flip issue lifecycle");

    let row = db.get_fix_attempt(id).expect("get").expect("row exists");
    assert_eq!(row.status, "verified");
    assert_eq!(
        row.agent_summary.as_deref(),
        Some("Added the header"),
        "the summary written at verify_requested must survive settling"
    );
    let state = db
        .get_issue_state(project_id, Some("https://example.com"), "security.csp")
        .expect("get issue state")
        .expect("lifecycle row exists");
    assert_eq!(
        state,
        (
            IssueStatus::Verified,
            None,
            None,
            Some(VerifiedBy::LocalScan)
        ),
        "a fix the watcher settled was proven by a scan, not claimed by the user"
    );
}

// Inverse walk: a still-active work item for the check means the agent's
// fix did not work, so the settle path lands on `verify_failed` with the
// failure detail recorded.
#[test]
fn verify_requested_attempt_with_still_active_issue_fails() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Fix Loop", "/tmp/fix-loop", Some("astro"))
        .expect("upsert");
    insert_test_work_item(&db, project_id, "http://localhost:4321", "security.csp")
        .expect("seed unresolved work item");
    let id = db
        .create_fix_attempt(
            project_id,
            "http://localhost:4321",
            "security.csp",
            "claude-code",
            1_000,
        )
        .expect("create attempt");

    db.set_fix_attempt_status(id, "verify_requested", Some("Tried a fix"), None, 2_000)
        .expect("agent requests verification");
    db.set_fix_attempt_status(id, "verifying", None, None, 2_500)
        .expect("start verifying");

    let still_active = db
        .is_fix_attempt_target_active(
            project_id,
            "http://localhost:4321",
            "security.csp",
            "group",
            None,
            None,
        )
        .expect("active query");
    assert!(still_active, "the seeded work item keeps the issue active");
    assert_eq!(
        decide_outcome(still_active, true, false, 2_500, 3_000),
        Some(Outcome::Failed)
    );

    // evaluate_attempt end to end: the seeded work item's web_scan source
    // settles inline, so an active issue fails with no grace window.
    let verifying = db.get_fix_attempt(id).expect("get").expect("row exists");
    assert_eq!(verifying.status, "verifying");
    assert_eq!(
        evaluate_attempt(&db, &verifying, 3_000),
        Ok(Some(Outcome::Failed)),
        "the wired evaluation must fail a still-active issue"
    );

    db.set_fix_attempt_status(id, "verify_failed", None, Some(STILL_FAILING_DETAIL), 3_000)
        .expect("mark failed");

    let row = db.get_fix_attempt(id).expect("get").expect("row exists");
    assert_eq!(row.status, "verify_failed");
    assert_eq!(row.failure_detail.as_deref(), Some(STILL_FAILING_DETAIL));
}

// A sibling file failing the same rule must not fail a file-scoped attempt.
#[test]
fn file_scoped_code_attempt_verifies_despite_sibling_file_failing_same_rule() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Fix Loop", "/tmp/fix-loop", Some("astro"))
        .expect("upsert");
    insert_test_work_item_at(
        &db,
        project_id,
        "https://example.com",
        "code_scan.external-call-retry",
        Some("src/pages/api/latest-release.ts"),
        Some(18),
    )
    .expect("seed unresolved sibling work item");
    let id = db
        .create_fix_attempt_with_target(
            project_id,
            "https://example.com",
            "code_scan.external-call-retry",
            "claude-code",
            FixAttemptTarget::occurrence("src/pages/api/contact.ts".to_string(), Some(24)),
            1_000,
        )
        .expect("create attempt");
    db.set_fix_attempt_status(id, "verifying", None, None, 2_000)
        .expect("start verifying");

    let verifying = db.get_fix_attempt(id).expect("get").expect("row exists");
    assert_eq!(
        evaluate_attempt(&db, &verifying, 3_000),
        Ok(Some(Outcome::Verified)),
        "a sibling file failing the same rule must not veto this file's verified fix"
    );
}

#[test]
fn inactive_issue_means_verified() {
    // Once no matching work item is active, the attempt is verified no
    // matter which sources contributed or how long verification ran.
    assert_eq!(
        decide_outcome(false, true, false, 0, 0),
        Some(Outcome::Verified)
    );
    assert_eq!(
        decide_outcome(false, false, false, 0, INTEGRATION_GRACE_MS * 2),
        Some(Outcome::Verified)
    );
}

#[test]
fn active_issue_with_immediate_sources_fails_now() {
    // web/code scans are awaited inline, so a still-active issue means
    // the fix did not work; no grace window applies.
    assert_eq!(
        decide_outcome(true, true, false, 0, 0),
        Some(Outcome::Failed)
    );
}

#[test]
fn active_issue_with_queued_sources_waits_until_grace_expires() {
    let started = 1_000;
    assert_eq!(
        decide_outcome(true, false, false, started, started),
        None,
        "queued polls have not landed yet; keep waiting"
    );
    assert_eq!(
        decide_outcome(true, false, false, started, started + INTEGRATION_GRACE_MS),
        None,
        "exactly at the grace boundary still waits"
    );
    assert_eq!(
        decide_outcome(
            true,
            false,
            false,
            started,
            started + INTEGRATION_GRACE_MS + 1
        ),
        Some(Outcome::Failed),
        "past the grace window an active issue is a failure"
    );
}

// Remote Web fixes wait for deployment before failing verification.
#[test]
fn remote_web_attempts_wait_out_the_deploy_window() {
    let started = 10_000;
    assert_eq!(
        decide_outcome(true, true, true, started, started),
        None,
        "still failing right after the agent finished: awaiting deploy"
    );
    assert_eq!(
        decide_outcome(
            true,
            true,
            true,
            started,
            started + REMOTE_WEB_DEPLOY_WAIT_MS
        ),
        None,
        "exactly at the deploy-window boundary still waits"
    );
    assert_eq!(
        decide_outcome(
            true,
            true,
            true,
            started,
            started + REMOTE_WEB_DEPLOY_WAIT_MS + 1
        ),
        Some(Outcome::Failed),
        "past the deploy window the attempt fails for real"
    );
    assert_eq!(
        decide_outcome(false, true, true, started, started + 1),
        Some(Outcome::Verified),
        "a deploy plus recheck flips the issue inactive: verified"
    );
}

#[test]
fn remote_web_attempt_predicate_keys_on_check_type_and_env() {
    assert!(is_remote_web_attempt(
        "compliance.accessibility_statement",
        None,
        "https://sitecmd.com"
    ));
    assert!(!is_remote_web_attempt(
        "security.csp",
        None,
        "http://localhost:4321"
    ));
    assert!(!is_remote_web_attempt(
        "code_scan.hardcoded-secret",
        Some("hardcoded-secret"),
        "https://sitecmd.com"
    ));
    assert!(!is_remote_web_attempt(
        "security.hsts",
        Some("hsts_missing"),
        "https://sitecmd.com"
    ));
    assert!(!is_remote_web_attempt("security.csp", None, "not a url"));
}

#[test]
fn mapped_code_attempt_uses_code_retry_detail_on_remote_environment() {
    let mut attempt = occurrence_attempt_row("security.hsts", "https://sitecmd.com", "src/http.ts");
    attempt.producer_rule = Some("hsts_missing".to_string());

    assert!(!is_remote_web_attempt(
        &attempt.check_id,
        attempt.producer_rule.as_deref(),
        &attempt.env_url
    ));
    assert_eq!(
        still_failing_detail(&attempt),
        "SiteCMD re-ran the code scan after the agent finished and src/http.ts \
         still fails the hsts_missing check."
    );
}

#[test]
fn retry_detail_reads_structured_target_not_canonical_id_shape() {
    let attempt = occurrence_attempt_row(
        "code_scan.sensitive-auth",
        "https://example.com",
        "src/auth.ts",
    );
    assert_eq!(attempt.check_id, "code_scan.sensitive-auth");
    assert_eq!(attempt.target_relative_path.as_deref(), Some("src/auth.ts"));
    assert_eq!(
        still_failing_detail(&attempt),
        "SiteCMD re-ran the code scan after the agent finished and src/auth.ts \
         still fails the sensitive-auth check."
    );
}

#[test]
fn issue_title_from_brief_strips_the_heading_prefix() {
    let brief = "# SiteCMD Fix Brief: Missing Content Security Policy\n\nAttempt: 7";
    assert_eq!(
        issue_title_from_brief(brief),
        Some("Missing Content Security Policy".to_string())
    );
}

#[test]
fn issue_title_from_brief_keeps_hash_sequences_inside_the_title() {
    // Only the leading prefix is stripped; a "# " inside the title is part
    // of the title itself.
    let brief = "# SiteCMD Fix Brief: Heading uses # instead of ## levels\n\nbody";
    assert_eq!(
        issue_title_from_brief(brief),
        Some("Heading uses # instead of ## levels".to_string())
    );
}

#[test]
fn issue_title_from_brief_rejects_unusable_briefs() {
    assert_eq!(issue_title_from_brief(""), None, "empty brief");
    assert_eq!(
        issue_title_from_brief("# Some other heading\n\nbody"),
        None,
        "foreign first line"
    );
    assert_eq!(
        issue_title_from_brief("# SiteCMD Fix Brief:   \n\nbody"),
        None,
        "prefix with a blank title"
    );
}

#[test]
fn immediate_sources_are_web_and_code_scans_only() {
    assert!(sources_settle_immediately(&strings(&[
        "web_scan",
        "code_scan"
    ])));
    assert!(!sources_settle_immediately(&strings(&["web_scan", "psi"])));
    assert!(
        sources_settle_immediately(&[]),
        "no sources means nothing to wait for"
    );
}

#[test]
fn get_active_issue_sources_is_targeted_and_distinct() {
    let db = temp_db();
    let project_id = db
        .upsert_project("sources", "/tmp/sources", None)
        .expect("project");
    insert_test_work_item(&db, project_id, "https://example.com", "security.csp")
        .expect("seed web item");
    insert_test_work_item(
        &db,
        project_id,
        "https://example.com",
        "code_scan.hardcoded-secret",
    )
    .expect("seed code item");

    assert_eq!(
        db.get_active_issue_sources(project_id, "https://example.com", "security.csp")
            .unwrap(),
        vec!["web_scan".to_string()],
        "only the matching check's sources"
    );
    assert_eq!(
        db.get_active_issue_sources(project_id, "https://example.com", "seo.robots_txt")
            .unwrap(),
        Vec::<String>::new(),
        "unknown check yields no sources (settles immediately)"
    );

    db.execute(|conn| {
        conn.execute(
            "UPDATE work_items SET resolved_at = 2000 WHERE check_id = 'security.csp'",
            [],
        )
        .map_err(|e| e.to_string())
    })
    .expect("db op")
    .expect("resolve");
    assert_eq!(
        db.get_active_issue_sources(project_id, "https://example.com", "security.csp")
            .unwrap(),
        Vec::<String>::new(),
        "resolved rows drop out of the source list"
    );
}

#[test]
fn environment_is_production_reads_one_environment_row() {
    let db = temp_db();
    let project_id = db
        .upsert_project("demo", "/tmp/demo", None)
        .expect("project");
    db.add_environment(
        project_id,
        "https://example.com",
        "Prod",
        "production",
        "manual",
    )
    .expect("prod env");
    db.add_environment(
        project_id,
        "http://localhost:3000",
        "Local",
        "development",
        "manual",
    )
    .expect("dev env");

    assert!(db
        .environment_is_production(project_id, "https://example.com")
        .unwrap());
    assert!(!db
        .environment_is_production(project_id, "http://localhost:3000")
        .unwrap());
    assert!(!db
        .environment_is_production(project_id, "https://unknown.example")
        .unwrap());
}
