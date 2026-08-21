//! Issue command routing and score-persistence tests.

#[cfg(test)]
mod verify_issue_tests {
    use crate::db::test_helpers::temp_db as test_db;

    // Return no match when the project has no work items.
    #[test]
    fn verify_issue_missing_check_id_returns_none() {
        let db = test_db();
        let groups = db
            .get_work_items_grouped(1, None, 0)
            .expect("grouped query");
        let found = groups.iter().find(|g| g.check_id == "nonexistent_check");
        assert!(found.is_none(), "expected None for unknown check_id");
    }

    // Coalesce successful batch verification scans while allowing retries.
    #[test]
    fn code_scan_pending_coalesces_batch_but_retries_after_failure() {
        use crate::commands::issues::code_scan_pending;
        use std::collections::HashSet;

        let mut dedup: HashSet<(i64, String)> = HashSet::new();

        assert!(code_scan_pending(&dedup, 7, "https://example.com"));
        // Failed scans do not record the dedup key.
        assert!(code_scan_pending(&dedup, 7, "https://example.com"));

        dedup.insert((7, "https://example.com".to_string()));
        assert!(!code_scan_pending(&dedup, 7, "https://example.com"));

        assert!(code_scan_pending(&dedup, 7, "https://other.example.com"));
        assert!(code_scan_pending(&dedup, 8, "https://example.com"));
    }

    #[test]
    fn verification_status_never_calls_a_queued_or_still_failing_issue_verified() {
        use crate::commands::issues::{verification_status, IssueVerificationStatus};

        assert_eq!(
            verification_status(true, false),
            IssueVerificationStatus::Queued
        );
        assert_eq!(
            verification_status(false, true),
            IssueVerificationStatus::StillPresent
        );
        assert_eq!(
            verification_status(false, false),
            IssueVerificationStatus::Verified
        );
    }

    #[test]
    fn verify_lookup_preserves_each_web_page_and_producer_target() {
        use crate::checks::{CheckStatus, ScanCategory, Severity};
        use crate::db::work_items::{WorkItemInput, WorkItemMetadata};

        let db = test_db();
        let project_id = db
            .upsert_project("Verify targets", "/tmp/verify-targets", None)
            .expect("project");
        let env_url = "https://example.com";
        let input = |signal: &str, page: &str, producer: &str| WorkItemInput {
            project_id,
            env_url: env_url.to_string(),
            source: "web_scan".to_string(),
            signal_id: signal.to_string(),
            check_id: "security.csp".to_string(),
            category: "security".to_string(),
            severity: Severity::High,
            title: "CSP issue".to_string(),
            description: "CSP issue".to_string(),
            detail_json: None,
            scan_ref: None,
            page_url: Some(page.to_string()),
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            observed_at: 1_000,
            metadata: WorkItemMetadata {
                check_status: Some(CheckStatus::Fail),
                producer_check_id: Some(producer.to_string()),
                producer_category: Some(ScanCategory::Security),
                ..Default::default()
            },
        };
        db.upsert_work_items_diff(
            "web_scan",
            project_id,
            env_url,
            vec![
                input(
                    "web_scan:security.headers.csp:home",
                    "https://example.com/",
                    "security.headers.csp",
                ),
                input(
                    "web_scan:security.headers.csp:about",
                    "https://example.com/about",
                    "security.headers.csp",
                ),
            ],
            1_000,
        )
        .expect("seed targets");

        let info = crate::commands::issues::lookup_issue_verify_info(
            &db,
            project_id,
            env_url,
            "security.csp",
        )
        .expect("lookup")
        .expect("group");
        assert_eq!(info.web_targets.len(), 2);
        assert_eq!(info.web_targets[0].url, "https://example.com/");
        assert_eq!(
            info.web_targets[0].producer_check_ids,
            vec!["security.headers.csp"]
        );
        assert_eq!(info.web_targets[1].url, "https://example.com/about");
    }

    #[test]
    fn verify_lookup_keeps_code_rule_active_while_any_location_remains() {
        use crate::checks::Severity;
        use crate::db::work_items::{WorkItemInput, WorkItemMetadata};

        let db = test_db();
        let project_id = db
            .upsert_project("Verify code group", "/tmp/verify-code-group", None)
            .expect("project");
        let env_url = "https://example.com";
        let input = |signal: &str, check_id: &str| WorkItemInput {
            project_id,
            env_url: env_url.to_string(),
            source: "code_scan".to_string(),
            signal_id: signal.to_string(),
            check_id: check_id.to_string(),
            category: "database".to_string(),
            severity: Severity::High,
            title: "Query inside a loop".to_string(),
            description: "N+1 query".to_string(),
            detail_json: None,
            scan_ref: None,
            page_url: None,
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            observed_at: 1_000,
            metadata: WorkItemMetadata::default(),
        };
        let representative = "code_scan.n-plus-one-query";
        let sibling = "code_scan.n-plus-one-query";
        db.upsert_work_items_diff(
            "code_scan",
            project_id,
            env_url,
            vec![
                input("code_scan:n-plus-one:data", representative),
                input("code_scan:n-plus-one:users", sibling),
            ],
            1_000,
        )
        .expect("seed locations");

        // A fresh Code Scan clears the representative location but still finds
        // the same rule elsewhere. Verification must remain still-present.
        db.upsert_work_items_diff(
            "code_scan",
            project_id,
            env_url,
            vec![input("code_scan:n-plus-one:users", sibling)],
            2_000,
        )
        .expect("refresh locations");

        let info = crate::commands::issues::lookup_issue_verify_info(
            &db,
            project_id,
            env_url,
            representative,
        )
        .expect("lookup")
        .expect("rule remains active through sibling");
        assert_eq!(info.sources, vec!["code_scan"]);
    }
}

#[cfg(test)]
mod score_persistence_tests {
    use crate::commands::issues::compute_and_record_current_score;
    use crate::db::test_helpers::temp_db;

    const ENV: &str = "https://example.com";

    fn seed_web_scan(db: &crate::db::Database, site_id: i64) {
        db.save_scan(
            site_id,
            &crate::core::scanner::ScanResult {
                page_signals: None,
                site_facts: None,
                url: ENV.to_string(),
                mode: "full".to_string(),
                scan_type: crate::core::scanner::ScanType::Health,
                overall_score: 91,
                categories: Vec::new(),
                issues: Vec::new(),
                detected_stack: None,
                duration_ms: 10,
                timestamp: "2026-07-20T00:00:00Z".to_string(),
            },
        )
        .expect("seed canonical web scan");
    }

    #[test]
    fn never_scanned_project_gets_no_synthetic_score_baseline() {
        let db = temp_db();
        let project_id = db
            .upsert_project("fresh", "/tmp/fresh", None)
            .expect("project");
        db.add_environment(project_id, ENV, "Prod", "production", "manual")
            .expect("env");

        let snapshot =
            compute_and_record_current_score(&db, project_id, Some(ENV), 1_000).expect("compute");
        assert_eq!(snapshot.overall, 100.0);
        let history = db
            .get_score_snapshot_history(project_id, Some(ENV), 90)
            .expect("history");
        assert!(
            history.is_empty(),
            "no-signal compute must not persist a baseline, got {history:?}"
        );
    }

    #[test]
    fn scanned_project_with_no_issues_persists_the_clean_baseline() {
        // Positive control: once a real web scan exists, a clean 100 is a
        // genuine signal and must land as the trend baseline.
        let db = temp_db();
        let project_id = db
            .upsert_project("scanned", "/tmp/scanned", None)
            .expect("project");
        db.add_environment(project_id, ENV, "Prod", "production", "manual")
            .expect("env");
        let site_id = db.get_or_create_site(ENV).expect("site");
        seed_web_scan(&db, site_id);

        let snapshot =
            compute_and_record_current_score(&db, project_id, Some(ENV), 1_000).expect("compute");
        assert_eq!(snapshot.overall, 100.0);
        let history = db
            .get_score_snapshot_history(project_id, Some(ENV), 90)
            .expect("history");
        assert_eq!(history.len(), 1, "scanned clean project persists 100");
        assert_eq!(history[0].overall, 100.0);
    }

    #[test]
    fn code_scan_alone_counts_as_score_signal() {
        // A code-only project (no web scan yet) still has a real signal, same
        // as the overview's code_scan_summary arm of the guard.
        let db = temp_db();
        let project_id = db
            .upsert_project("code-only", "/tmp/code-only", None)
            .expect("project");
        db.add_environment(project_id, ENV, "Prod", "production", "manual")
            .expect("env");
        db.save_code_scan(
            project_id,
            Some(ENV.to_string()),
            "/tmp/code-only".to_string(),
            &crate::core::code_scan::CodeScanReport {
                skipped_scopes: Default::default(),
                checked_at: "2026-07-20T00:00:00Z".to_string(),
                framework: None,
                issue_count: 0,
                critical_count: 0,
                high_count: 0,
                medium_count: 0,
                low_count: 0,
                issues: Vec::new(),
            },
            10,
        )
        .expect("seed canonical code scan");

        compute_and_record_current_score(&db, project_id, Some(ENV), 1_000).expect("compute");
        let history = db
            .get_score_snapshot_history(project_id, Some(ENV), 90)
            .expect("history");
        assert_eq!(history.len(), 1, "code scan signal persists the baseline");
    }

    #[test]
    fn signal_probe_failure_degrades_to_skip_persisting_not_a_failed_read() {
        let db = temp_db();
        let project_id = db
            .upsert_project("faulty", "/tmp/faulty", None)
            .expect("project");
        db.add_environment(project_id, ENV, "Prod", "production", "manual")
            .expect("env");
        db.execute(|conn| {
            conn.execute_batch("DROP TABLE scan_runs;")
                .map_err(|e| e.to_string())
        })
        .expect("worker")
        .expect("drop tables");

        let snapshot = compute_and_record_current_score(&db, project_id, Some(ENV), 1_000)
            .expect("score read must survive a failing signal probe");
        assert_eq!(snapshot.overall, 100.0);
        let history = db
            .get_score_snapshot_history(project_id, Some(ENV), 90)
            .expect("history");
        assert!(history.is_empty(), "failed probe must skip persistence");
    }

    #[test]
    fn active_issues_persist_even_without_scan_history() {
        // Non-empty groups ARE the signal: integration-sourced work items can
        // exist before any scan, and their score is real.
        let db = temp_db();
        let project_id = db
            .upsert_project("issues", "/tmp/issues", None)
            .expect("project");
        db.add_environment(project_id, ENV, "Prod", "production", "manual")
            .expect("env");
        db.execute(move |conn| {
            conn.execute(
                "INSERT INTO work_items (project_id, env_url, source, signal_id, check_id,
                                         category, severity, title, description,
                                         first_seen_at, last_seen_at)
                 VALUES (?1, ?2, 'updates', 'sig-1', 'updates.dep', 'updates', 'high',
                         't', 'd', 1000, 1000)",
                rusqlite::params![project_id, ENV],
            )
            .map_err(|e| e.to_string())?;
            Ok::<_, String>(())
        })
        .expect("worker")
        .expect("seed work item");

        let snapshot =
            compute_and_record_current_score(&db, project_id, Some(ENV), 1_000).expect("compute");
        assert_eq!(snapshot.high_count, 1);
        let history = db
            .get_score_snapshot_history(project_id, Some(ENV), 90)
            .expect("history");
        assert_eq!(history.len(), 1, "active issues persist the real score");
        assert_eq!(history[0].high_count, 1);
    }
}
