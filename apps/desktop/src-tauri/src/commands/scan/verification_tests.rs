//! Bounded-verification persistence and the coverage it claims.

use super::*;

#[test]
fn bounded_verification_persists_against_the_environment_site_for_a_child_page() {
    const ENVIRONMENT_URL: &str = "https://verify-scope.example";
    const PAGE_URL: &str = "https://verify-scope.example/docs";
    const EFFECTIVE_URL: &str = "https://verify-scope.example/docs/";
    let db = crate::db::test_helpers::temp_db_arc();
    let project_id = db
        .upsert_project("Verify scope", "/tmp/verify-scope", None)
        .expect("project");
    db.add_environment(
        project_id,
        ENVIRONMENT_URL,
        "Production",
        "production",
        "manual",
    )
    .expect("environment");
    let expected_site_id = db
        .get_or_create_site_for_project(project_id, ENVIRONMENT_URL)
        .expect("site");
    let execution_id = db
        .execute(move |conn| {
            conn.execute(
                "INSERT INTO scan_executions (
                    project_id, environment_url, environment_scope_key, requested_mode,
                    web_focus, trigger, admission_class, status, idempotency_key,
                    request_fingerprint, started_at, web_status
                 ) VALUES (?1, ?2, ?2, 'web', 'health', 'verification',
                           'bounded_verification', 'running', 'verify-child',
                           'v1:verify-child', 1, 'running')",
                rusqlite::params![project_id, ENVIRONMENT_URL],
            )
            .expect("execution");
            conn.last_insert_rowid()
        })
        .expect("database worker");
    let scan_result = ScanResult {
        url: EFFECTIVE_URL.into(),
        mode: "verification".into(),
        scan_type: ScanType::Health,
        overall_score: 0,
        categories: Vec::new(),
        issues: vec![
            crate::checks::CheckResult {
                check_id: "seo.title".into(),
                category: crate::checks::ScanCategory::Seo,
                title: "Title".into(),
                description: "Title is present".into(),
                status: crate::checks::CheckStatus::Pass,
                severity: crate::checks::Severity::Medium,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::Confirmed,
                confidence_reason: None,
                why_it_matters: None,
            },
            crate::checks::CheckResult {
                check_id: "seo.meta_description".into(),
                category: crate::checks::ScanCategory::Seo,
                title: "Description".into(),
                description: "Description could not be verified".into(),
                status: crate::checks::CheckStatus::Skipped,
                severity: crate::checks::Severity::Medium,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::Confirmed,
                confidence_reason: None,
                why_it_matters: None,
            },
        ],
        detected_stack: None,
        duration_ms: 10,
        timestamp: "2026-08-13T12:00:00Z".into(),
        page_signals: None,
        site_facts: None,
    };
    let routes = covered_routes(PAGE_URL, EFFECTIVE_URL);
    let outcomes = scan_result
        .issues
        .iter()
        .flat_map(|check| {
            routes.iter().map(move |route| CheckOutcome {
                route: Some(*route),
                check_id: &check.check_id,
                status: check.status,
            })
        })
        .collect::<Vec<_>>();
    let coverage = ScanCoverageManifest::derive(
        ScanCoverageKind::CheckSet,
        routes.iter().map(|route| (*route).to_string()).collect(),
        &outcomes,
        ClaimBasis::PerRoute,
    );

    let run_id = persist_bounded_web_verification(
        &db.db,
        Some(project_id),
        ENVIRONMENT_URL,
        PAGE_URL,
        &scan_result,
        coverage,
        execution_id,
        1,
        11,
    )
    .expect("verification persistence");
    let stored: (
        Option<i64>,
        Option<i64>,
        Option<String>,
        String,
        String,
        Option<String>,
    ) = db
        .execute(move |conn| {
            conn.query_row(
                "SELECT project_id, site_id, environment_url, diagnostics_json, coverage_json,
                        (SELECT page_url FROM scan_findings
                          WHERE run_id = scan_runs.id AND canonical_check_id = 'seo.title')
                   FROM scan_runs WHERE id = ?1",
                [run_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("stored run")
        })
        .expect("database worker");
    let diagnostics: crate::core::normalized_scan::NormalizedRunDiagnostics =
        serde_json::from_str(&stored.3).expect("diagnostics");
    let coverage: ScanCoverageManifest = serde_json::from_str(&stored.4).expect("coverage");

    assert_eq!(stored.0, Some(project_id));
    assert_eq!(stored.1, Some(expected_site_id));
    assert_eq!(stored.2.as_deref(), Some(ENVIRONMENT_URL));
    assert_eq!(diagnostics.page_url.as_deref(), Some(PAGE_URL));
    assert!(coverage.covers(Some(PAGE_URL), "seo.title"));
    assert!(!coverage.covers(Some(PAGE_URL), "seo.meta_description"));
    assert!(coverage.covers(Some(EFFECTIVE_URL), "seo.title"));
    assert!(!coverage.covers(Some(EFFECTIVE_URL), "seo.meta_description"));
    assert_eq!(stored.5.as_deref(), Some(EFFECTIVE_URL));
}
