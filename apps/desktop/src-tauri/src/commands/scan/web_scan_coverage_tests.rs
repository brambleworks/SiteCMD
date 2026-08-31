//! Coverage tests for findings keyed to a redirect's effective URL.

use super::*;
use crate::checks::CheckStatus;

const ENVIRONMENT_URL: &str = "https://redirect-resolve.example";
const AUTHORED_URL: &str = "https://redirect-resolve.example/pricing";
const EFFECTIVE_URL: &str = "https://redirect-resolve.example/pricing/";
const SIBLING_URL: &str = "https://redirect-resolve.example/about";

fn scan_result(url: &str, status: CheckStatus) -> scanner::ScanResult {
    scanner::ScanResult {
        page_signals: None,
        site_facts: None,
        url: url.to_string(),
        mode: "live".to_string(),
        scan_type: ScanType::Health,
        overall_score: 90,
        categories: vec![],
        issues: vec![crate::checks::CheckResult {
            check_id: "seo.title".into(),
            category: crate::checks::ScanCategory::Seo,
            title: "Title".into(),
            description: "Title is missing".into(),
            status,
            severity: crate::checks::Severity::Medium,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::Confirmed,
            confidence_reason: None,
            why_it_matters: None,
        }],
        detected_stack: None,
        duration_ms: 10,
        timestamp: "2026-01-01T00:00:00Z".to_string(),
    }
}

// Persist one Web run that was asked for `authored` and answered from
// `result.url`, through the production persistence path.
fn persist(
    db: &Arc<Database>,
    project_id: i64,
    key: &str,
    authored: &str,
    result: &scanner::ScanResult,
) -> i64 {
    let key = key.to_string();
    let execution_id = db
        .execute(move |conn| {
            conn.execute(
                "INSERT INTO scan_executions (
                    project_id, environment_url, environment_scope_key, requested_mode,
                    web_focus, trigger, admission_class, status,
                    idempotency_key, request_fingerprint, started_at, web_status
                 ) VALUES (
                    ?1, ?2, ?2, 'web', 'health', 'manual', 'general_scan', 'running',
                    ?3, ?4, 1, 'running'
                 )",
                rusqlite::params![project_id, ENVIRONMENT_URL, key, format!("v1:{key}")],
            )
            .expect("insert execution");
            conn.last_insert_rowid()
        })
        .expect("execution dispatch");
    let (outcome, _) = persist_scan_blocking(
        db,
        ENVIRONMENT_URL,
        authored,
        ScanType::Health,
        Some(project_id),
        execution_id,
        false,
        false,
        false,
        None,
        None,
        result,
    );
    outcome.scan_id.expect("scan persistence")
}

// The pages still carrying an open finding, in page order.
fn open_pages(db: &Database) -> Vec<String> {
    db.execute(|conn| {
        let mut stmt = conn
            .prepare("SELECT page_url FROM work_items WHERE resolved_at IS NULL ORDER BY page_url")
            .expect("prepare");
        stmt.query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect")
    })
    .expect("open pages")
}

#[test]
fn a_fix_on_a_redirecting_page_resolves_on_a_clean_rescan() {
    let db = crate::db::test_helpers::temp_db_arc();
    let project_id = db
        .upsert_project("Redirect", "/tmp/redirect-resolve", None)
        .expect("project");
    db.add_environment(
        project_id,
        ENVIRONMENT_URL,
        "Production",
        "production",
        "manual",
    )
    .expect("environment");

    // /pricing redirects to /pricing/, so the failing check's row is keyed to
    // the URL that answered rather than the one the scan asked for.
    persist(
        &db.db,
        project_id,
        "redirect-seed",
        AUTHORED_URL,
        &scan_result(EFFECTIVE_URL, CheckStatus::Fail),
    );
    // The negative control: a second page fails the same check, and nothing
    // this run proves about /pricing may speak for it.
    persist(
        &db.db,
        project_id,
        "sibling-seed",
        SIBLING_URL,
        &scan_result(SIBLING_URL, CheckStatus::Fail),
    );
    assert_eq!(open_pages(&db.db), vec![SIBLING_URL, EFFECTIVE_URL]);

    // The user fixes the title. The rescan asks for /pricing again and again
    // lands on /pricing/.
    persist(
        &db.db,
        project_id,
        "redirect-clean",
        AUTHORED_URL,
        &scan_result(EFFECTIVE_URL, CheckStatus::Pass),
    );

    assert_eq!(
        open_pages(&db.db),
        vec![SIBLING_URL],
        "the redirected page's finding resolves; the page nobody rescanned stays open"
    );
}

#[test]
fn persistence_keeps_environment_scope_separate_from_the_target_page() {
    let db = crate::db::test_helpers::temp_db_arc();
    let project_id = db
        .upsert_project("Redirect", "/tmp/redirect-scope", None)
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
        .expect("environment site");
    let result = scan_result(EFFECTIVE_URL, CheckStatus::Skipped);
    let run_id = persist(&db.db, project_id, "redirect-scope", AUTHORED_URL, &result);

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
                "SELECT project_id, site_id, environment_url, coverage_json, diagnostics_json,
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
    let coverage: crate::core::normalized_scan::ScanCoverageManifest =
        serde_json::from_str(&stored.3).expect("coverage");
    let diagnostics: crate::core::normalized_scan::NormalizedRunDiagnostics =
        serde_json::from_str(&stored.4).expect("diagnostics");

    assert_eq!(stored.0, Some(project_id));
    assert_eq!(stored.1, Some(expected_site_id));
    assert_eq!(stored.2.as_deref(), Some(ENVIRONMENT_URL));
    assert_eq!(coverage.page_urls, vec![AUTHORED_URL, EFFECTIVE_URL]);
    assert!(!coverage.covers(Some(AUTHORED_URL), "seo.title"));
    assert!(!coverage.covers(Some(EFFECTIVE_URL), "seo.title"));
    assert_eq!(diagnostics.page_url.as_deref(), Some(AUTHORED_URL));
    assert_eq!(stored.5.as_deref(), Some(EFFECTIVE_URL));
}
