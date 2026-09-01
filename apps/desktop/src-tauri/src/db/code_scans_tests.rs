use super::code_issue_view_from_row;
use crate::core::code_scan::{CodeIssue, CodeScanDomain, CodeScanReport};
use crate::db::test_helpers::{temp_db, TestDb};

const STORED_CHECK_ID: &str = "code_scan.security.persisted-check";

fn rich_code_issue() -> CodeIssue {
    CodeIssue {
        check_id: String::new(),
        id: "security.persisted-check".to_string(),
        category: "security".to_string(),
        severity: crate::checks::Severity::High,
        title: "t".to_string(),
        description: "d".to_string(),
        relative_path: "a.ts".to_string(),
        absolute_path: "/tmp/a.ts".to_string(),
        line: Some(3),
        source_excerpt: Some("excerpt".to_string()),
        evidence: Some("evidence".to_string()),
        why_now: Some("why".to_string()),
        likely_fix: Some("fix".to_string()),
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        verify_hint: Some("verify".to_string()),
    }
}

fn one_issue_report() -> CodeScanReport {
    CodeScanReport {
        skipped_scopes: Default::default(),
        checked_at: "2026-07-14T12:00:00Z".into(),
        framework: Some("test".into()),
        issue_count: 1,
        critical_count: 0,
        high_count: 1,
        medium_count: 0,
        low_count: 0,
        issues: vec![rich_code_issue()],
    }
}

#[test]
fn code_issue_view_from_row_keeps_rich_fields_when_not_stripping() {
    let json = serde_json::to_string(&rich_code_issue()).unwrap();
    let view = code_issue_view_from_row(
        Some(json),
        Some("database".to_string()),
        STORED_CHECK_ID.to_string(),
        "high".to_string(),
        "t".to_string(),
        false,
    )
    .unwrap();
    // Domain comes from the column when present.
    assert_eq!(view.domain, CodeScanDomain::Database);
    assert_eq!(view.check_id, STORED_CHECK_ID);
    assert_eq!(view.source_excerpt.as_deref(), Some("excerpt"));
    assert_eq!(view.evidence.as_deref(), Some("evidence"));
    assert_eq!(view.why_now.as_deref(), Some("why"));
    assert_eq!(view.likely_fix.as_deref(), Some("fix"));
    assert_eq!(view.verify_hint.as_deref(), Some("verify"));
}

#[test]
fn code_issue_view_from_row_strips_rich_fields_when_flagged() {
    let json = serde_json::to_string(&rich_code_issue()).unwrap();
    let view = code_issue_view_from_row(
        Some(json),
        Some("security".to_string()),
        STORED_CHECK_ID.to_string(),
        "high".to_string(),
        "t".to_string(),
        true,
    )
    .unwrap();
    assert!(view.source_excerpt.is_none());
    assert!(view.evidence.is_none());
    assert!(view.why_now.is_none());
    assert!(view.likely_fix.is_none());
    assert!(view.verify_hint.is_none());
}

#[test]
fn code_issue_view_from_row_propagates_serde_error() {
    // Each caller decides fatal-vs-skip, so the shared mapper must surface
    // the serde error rather than swallow a malformed blob.
    assert!(code_issue_view_from_row(
        Some("{ not json".to_string()),
        None,
        STORED_CHECK_ID.to_string(),
        "high".to_string(),
        "t".to_string(),
        false,
    )
    .is_err());
}

#[test]
fn code_issue_view_from_row_rejects_unknown_persisted_domain() {
    let json = serde_json::to_string(&rich_code_issue()).unwrap();
    assert!(
        code_issue_view_from_row(
            Some(json),
            Some("quality".to_string()),
            STORED_CHECK_ID.to_string(),
            "high".to_string(),
            "t".to_string(),
            false,
        )
        .is_err(),
        "an unknown stored domain must not silently fall back to the current descriptor"
    );
}

#[test]
fn code_issue_view_from_row_rejects_empty_persisted_check_id() {
    let json = serde_json::to_string(&rich_code_issue()).unwrap();
    assert!(code_issue_view_from_row(
        Some(json),
        Some("security".to_string()),
        String::new(),
        "high".to_string(),
        "t".to_string(),
        false,
    )
    .is_err());
}

#[test]
fn code_issue_view_from_row_rejects_duplicate_column_mismatches() {
    let mut mismatched_id = rich_code_issue();
    mismatched_id.check_id = "code_scan.security.other".into();
    let json = serde_json::to_string(&mismatched_id).unwrap();
    assert!(code_issue_view_from_row(
        Some(json),
        Some("security".into()),
        STORED_CHECK_ID.into(),
        "high".into(),
        "t".into(),
        false,
    )
    .is_err());

    let json = serde_json::to_string(&rich_code_issue()).unwrap();
    assert!(code_issue_view_from_row(
        Some(json.clone()),
        Some("security".into()),
        STORED_CHECK_ID.into(),
        "low".into(),
        "t".into(),
        false,
    )
    .is_err());
    assert!(code_issue_view_from_row(
        Some(json),
        Some("security".into()),
        STORED_CHECK_ID.into(),
        "high".into(),
        "different title".into(),
        false,
    )
    .is_err());
}

#[test]
fn save_code_scan_rejects_inconsistent_report_counts() {
    let db = temp_db();
    let project_id = db
        .upsert_project("test", "/tmp/code-scan-counts", None)
        .unwrap();
    let mut report = one_issue_report();
    report.issue_count = 2;

    assert!(db
        .save_code_scan(
            project_id,
            None,
            "/tmp/code-scan-counts".into(),
            &report,
            10,
        )
        .is_err());
}

#[test]
fn save_code_scan_snapshots_canonical_id_inside_payload() {
    let db = temp_db();
    let project_id = db
        .upsert_project("test", "/tmp/code-scan-identity", None)
        .unwrap();
    let scan_id = db
        .save_code_scan(
            project_id,
            None,
            "/tmp/code-scan-identity".into(),
            &one_issue_report(),
            10,
        )
        .unwrap();
    let (column_id, payload_id) = db
        .execute(move |conn| {
            conn.query_row(
                "SELECT canonical_check_id, detail_json FROM scan_findings WHERE run_id = ?1",
                rusqlite::params![scan_id],
                |row| {
                    let column_id: String = row.get(0)?;
                    let payload: CodeIssue = serde_json::from_str(&row.get::<_, String>(1)?)
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok((column_id, payload.check_id))
                },
            )
        })
        .unwrap()
        .unwrap();
    assert!(!column_id.is_empty());
    assert_eq!(payload_id, column_id);
}

#[test]
fn skipped_code_evidence_is_not_returned_as_an_active_history_issue() {
    let db = temp_db();
    let project_id = db
        .upsert_project("test", "/tmp/code-scan-skipped", None)
        .unwrap();
    let scan_id = db
        .save_code_scan(
            project_id,
            Some("https://example.com".to_string()),
            "/tmp/code-scan-skipped".into(),
            &one_issue_report(),
            100,
        )
        .unwrap();
    db.execute(move |conn| {
        conn.execute(
            "UPDATE scan_findings SET verdict = 'skipped' WHERE run_id = ?1",
            [scan_id],
        )
        .map_err(|error| error.to_string())?;
        conn.execute(
            "UPDATE scan_runs
             SET issues_total = 0, issues_high = 0
             WHERE id = ?1",
            [scan_id],
        )
        .map_err(|error| error.to_string())?;
        Ok::<(), String>(())
    })
    .unwrap()
    .unwrap();

    let detail = db
        .get_code_scan_detail(scan_id)
        .unwrap()
        .expect("scan detail");
    assert_eq!(detail.issue_count, 0);
    assert!(detail.issues.is_empty());
    assert!(detail.domain_summaries.is_empty());
    assert!(db.get_top_code_scan_issue_view(scan_id).unwrap().is_none());
}

fn seed_code_scan(db: &TestDb, project_id: i64) -> i64 {
    db.save_code_scan(
        project_id,
        None,
        "/tmp/p".to_string(),
        &CodeScanReport {
            skipped_scopes: Default::default(),
            checked_at: "2026-06-12T00:00:00Z".to_string(),
            framework: None,
            issue_count: 0,
            critical_count: 0,
            high_count: 0,
            medium_count: 0,
            low_count: 0,
            issues: Vec::new(),
        },
        100,
    )
    .unwrap()
}

fn seed_code_work_item(db: &TestDb, project_id: i64, scan_ref: i64, signal: &str, resolved: bool) {
    let signal = signal.to_string();
    let resolved_at: Option<i64> = resolved.then_some(2_000);
    db.execute(move |conn| {
        conn.execute(
            "INSERT INTO work_items
                    (project_id, env_url, source, signal_id, check_id, category, severity,
                     title, description, scan_ref, first_seen_at, last_seen_at, resolved_at)
                 VALUES (?1, '', 'code_scan', ?2, 'code.security.x', 'security', 'high',
                         't', 'd', ?3, 1000, 1000, ?4)",
            rusqlite::params![project_id, signal, scan_ref, resolved_at],
        )
        .map_err(|e| e.to_string())
    })
    .unwrap()
    .unwrap();
}

#[test]
fn execution_retention_keeps_recent_code_runs_and_preserves_active_issues() {
    let db = temp_db();
    let project_id = db.upsert_project("p", "/tmp/p", None).unwrap();

    // Four scans, oldest (s1) to newest (s4).
    let s1 = seed_code_scan(&db, project_id);
    let s2 = seed_code_scan(&db, project_id);
    let s3 = seed_code_scan(&db, project_id);
    let s4 = seed_code_scan(&db, project_id);

    seed_code_work_item(&db, project_id, s1, "code_scan:old1", true);
    seed_code_work_item(&db, project_id, s2, "code_scan:old2", true);
    seed_code_work_item(&db, project_id, s4, "code_scan:active", false);

    let pruned = db
        .prune_scan_executions_for_scope(
            Some(project_id),
            &format!("project:{project_id}"),
            2,
            crate::db::ScanRetentionWindow::All,
        )
        .unwrap();
    assert_eq!(pruned, 2, "the two oldest scans should be deleted");

    let remaining: Vec<i64> = db
        .execute(move |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id FROM scan_runs
                     WHERE project_id = ?1 AND source = 'code_scan'
                     ORDER BY id",
                )
                .unwrap();
            let ids = stmt
                .query_map([project_id], |row| row.get::<_, i64>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            ids
        })
        .unwrap();
    assert_eq!(remaining, vec![s3, s4], "only the two newest scans remain");

    // Resolved history on pruned scans is gone; the active issue carried on
    // the newest scan must survive a prune.
    let active = db.get_active_work_items(project_id, None).unwrap();
    assert_eq!(active.len(), 1, "the active issue must not be pruned");
    assert_eq!(active[0].signal_id, "code_scan:active");
}
