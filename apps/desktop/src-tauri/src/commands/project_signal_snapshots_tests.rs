use super::{
    build_lightweight_project_signal_snapshot, choose_dashboard_integration_host,
    dashboard_integration_cache_scope, is_dashboard_reference_integration,
    load_dashboard_code_scan_trend, load_dashboard_scan_state, load_nav_badge_failed_issues,
};
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::core::scanner::ScanResult;
use crate::db::test_helpers::temp_db;
use crate::db::work_items::WorkItemMetadata;
use crate::db::{work_items::WorkItemInput, Database};

const TEST_ENV_URL: &str = "https://example.com";

fn project_site(db: &Database, project_id: i64) -> i64 {
    db.add_environment(
        project_id,
        TEST_ENV_URL,
        "Production",
        "production",
        "manual",
    )
    .expect("environment");
    db.get_or_create_site_for_project(project_id, TEST_ENV_URL)
        .expect("project site")
}

fn new_project_site(db: &Database, suffix: &str) -> (i64, i64) {
    let project_id = db
        .upsert_project(
            &format!("Signal Test {suffix}"),
            &format!("/tmp/signal-test-{suffix}"),
            None,
        )
        .expect("project");
    (project_id, project_site(db, project_id))
}

fn save_scan_with_type(db: &Database, site_id: i64, scan_type: &str, score: u32, ts: &str) {
    let result = ScanResult {
        page_signals: None,
        site_facts: None,
        url: "https://example.com".to_string(),
        mode: "full".to_string(),
        scan_type: scan_type.parse().expect("valid scan type"),
        overall_score: score,
        categories: vec![],
        issues: vec![],
        detected_stack: None,
        duration_ms: 1_000,
        timestamp: ts.to_string(),
    };
    db.save_scan(site_id, &result).expect("save_scan");
}

fn save_scan_with_snapshot_issue(
    db: &Database,
    site_id: i64,
    scan_type: &str,
    issue: CheckResult,
    ts: &str,
) -> i64 {
    let result = ScanResult {
        page_signals: None,
        site_facts: None,
        url: "https://example.com".to_string(),
        mode: "full".to_string(),
        scan_type: scan_type.parse().expect("valid scan type"),
        overall_score: 80,
        categories: vec![],
        issues: vec![issue],
        detected_stack: None,
        duration_ms: 1_000,
        timestamp: ts.to_string(),
    };
    db.save_scan(site_id, &result).expect("save scan")
}

fn snapshot_issue(check_id: &str, status: CheckStatus, severity: Severity) -> CheckResult {
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Security,
        title: format!("Issue from {check_id}"),
        description: format!("Description from {check_id}"),
        status,
        severity,
        fix_prompt: Some(format!("Fix {check_id}")),
        manual_fix: Some(format!("Manually fix {check_id}")),
        raw_data: Some(serde_json::json!({ "producer": check_id })),
        confidence: IssueConfidence::High,
        confidence_reason: Some(format!("Observed by {check_id}")),
        why_it_matters: Some(format!("Impact from {check_id}")),
    }
}

fn sample_code_scan_report() -> crate::core::code_scan::CodeScanReport {
    sample_code_scan_report_at("2026-04-10T12:00:00Z")
}

fn sample_code_scan_report_at(timestamp: &str) -> crate::core::code_scan::CodeScanReport {
    crate::core::code_scan::CodeScanReport {
        skipped_scopes: Default::default(),
        checked_at: timestamp.to_string(),
        framework: Some("Next.js".to_string()),
        issue_count: 1,
        critical_count: 1,
        high_count: 0,
        medium_count: 0,
        low_count: 0,
        issues: vec![crate::core::code_scan::CodeIssue {
            check_id: String::new(),
            id: "db-owner-scope".to_string(),
            category: "database".to_string(),
            severity: crate::checks::Severity::Critical,
            title: "Missing owner scope".to_string(),
            description: "Query is not scoped to the current owner.".to_string(),
            relative_path: "app/api/projects/route.ts".to_string(),
            absolute_path: "/tmp/code-test/app/api/projects/route.ts".to_string(),
            line: Some(18),
            source_excerpt: None,
            evidence: None,
            why_now: None,
            likely_fix: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        }],
    }
}

fn sample_update_report(package_name: &str) -> crate::updates::types::UpdateReport {
    crate::updates::types::UpdateReport {
        packages: Vec::new(),
        updates: vec![crate::updates::types::PackageUpdate {
            name: package_name.to_string(),
            current_version: "4.0.0".to_string(),
            latest_version: "4.1.0".to_string(),
            ecosystem: crate::updates::types::Ecosystem::Npm,
            update_type: crate::updates::types::UpdateType::Minor,
            is_security: true,
            advisory_severity: Some("high".to_string()),
            advisory_url: None,
            source: "package-lock.json".to_string(),
            is_dev: false,
            ..Default::default()
        }],
        ecosystems_detected: vec![crate::updates::types::Ecosystem::Npm],
        scan_duration_ms: 42,
    }
}

#[test]
fn dashboard_integration_host_uses_requested_public_environment() {
    let project_urls = vec![
        "http://localhost:4321".to_string(),
        "https://production.example.com".to_string(),
    ];

    assert_eq!(
        choose_dashboard_integration_host(Some("https://staging.example.com"), &project_urls),
        Some("staging.example.com".to_string())
    );
}

#[test]
fn dashboard_integration_host_falls_back_from_local_to_public_environment() {
    let project_urls = vec![
        "http://localhost:4321".to_string(),
        "http://127.0.0.1:4321".to_string(),
        "Https://SiteCMD.com".to_string(),
    ];

    assert_eq!(
        choose_dashboard_integration_host(Some("http://localhost:4321"), &project_urls),
        Some("sitecmd.com".to_string())
    );
}

#[test]
fn dashboard_reference_integrations_include_search_providers() {
    assert!(is_dashboard_reference_integration("googlesearchconsole"));
    assert!(is_dashboard_reference_integration("bingwebmaster"));
    assert!(is_dashboard_reference_integration("plausible"));
    assert!(!is_dashboard_reference_integration("github"));
}

#[test]
fn dashboard_reference_integrations_use_search_cache_windows() {
    assert_eq!(
        dashboard_integration_cache_scope("googlesearchconsole", Some("example.com")),
        "dashboard:28d"
    );
    assert_eq!(
        dashboard_integration_cache_scope("bingwebmaster", Some("example.com")),
        "dashboard:30d"
    );
}

fn save_code_scan_with_work_items(
    db: &Database,
    project_id: i64,
    env_url: &str,
    report: &crate::core::code_scan::CodeScanReport,
    duration_ms: u64,
) -> i64 {
    let scan_id = db
        .save_code_scan(
            project_id,
            Some(env_url.to_string()),
            "/tmp/code-test".to_string(),
            report,
            duration_ms,
        )
        .expect("save_code_scan");
    let now_ms = 1_000_000i64;
    let inputs: Vec<WorkItemInput> = report
        .issues
        .iter()
        .map(|ci| {
            let line = ci.line.map(|l| l.to_string()).unwrap_or_default();
            WorkItemInput {
                project_id,
                env_url: env_url.to_string(),
                source: "code_scan".to_string(),
                signal_id: format!("code_scan:{}:{}:{}", ci.id, ci.relative_path, line),
                check_id: crate::core::code_scan::canonical_code_check_id(&ci.id),
                category: "code_quality".to_string(),
                severity: ci.severity,
                title: ci.title.clone(),
                description: ci.description.clone(),
                detail_json: serde_json::to_string(ci).ok(),
                scan_ref: Some(scan_id),
                page_url: None,
                fix_prompt: None,
                manual_fix: None,
                why_it_matters: None,
                observed_at: now_ms,
                metadata: WorkItemMetadata::default(),
            }
        })
        .collect();
    db.upsert_work_items_diff("code_scan", project_id, env_url, inputs, now_ms)
        .expect("upsert_work_items_diff");
    scan_id
}

#[test]
fn dashboard_state_picks_previous_scan_of_same_type() {
    let db = temp_db();
    let (project_id, site_id) = new_project_site(&db, "same-type");

    save_scan_with_type(&db, site_id, "health", 80, "2026-03-01T00:00:00Z");
    save_scan_with_type(&db, site_id, "security", 65, "2026-03-02T00:00:00Z");
    save_scan_with_type(&db, site_id, "health", 90, "2026-03-03T00:00:00Z");

    let state =
        load_dashboard_scan_state(&db, project_id, TEST_ENV_URL).expect("dashboard scan state");

    let latest = state.latest_detail.as_ref().expect("latest detail");
    assert_eq!(latest.scan_type.as_str(), "health");
    assert_eq!(latest.overall_score, 90);

    let previous = state.previous_detail.as_ref().expect("previous detail");
    assert_eq!(previous.scan_type.as_str(), "health");
    assert_eq!(previous.overall_score, 80);
}

#[test]
fn dashboard_state_falls_back_to_any_type_when_no_same_type_prior() {
    let db = temp_db();
    let (project_id, site_id) = new_project_site(&db, "fallback-type");

    save_scan_with_type(&db, site_id, "security", 65, "2026-03-02T00:00:00Z");
    save_scan_with_type(&db, site_id, "health", 90, "2026-03-03T00:00:00Z");

    let state =
        load_dashboard_scan_state(&db, project_id, TEST_ENV_URL).expect("dashboard scan state");

    let latest = state.latest_detail.as_ref().expect("latest detail");
    assert_eq!(latest.scan_type.as_str(), "health");

    let previous = state.previous_detail.as_ref().expect("previous detail");
    assert_eq!(previous.scan_type.as_str(), "security");
    assert_eq!(previous.overall_score, 65);
}

#[test]
fn dashboard_state_has_no_previous_on_first_scan() {
    let db = temp_db();
    let (project_id, site_id) = new_project_site(&db, "first-scan");

    save_scan_with_type(&db, site_id, "health", 90, "2026-03-03T00:00:00Z");

    let state =
        load_dashboard_scan_state(&db, project_id, TEST_ENV_URL).expect("dashboard scan state");

    assert!(state.latest_detail.is_some());
    assert!(state.previous_detail.is_none());
}

#[test]
fn dashboard_and_badge_aggregate_aliases_under_canonical_id() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/signal-test", None)
        .expect("project");
    let site_id = project_site(&db, project_id);

    let medium = snapshot_issue("security.headers.csp", CheckStatus::Fail, Severity::Medium);
    let medium_scan = save_scan_with_snapshot_issue(
        &db,
        site_id,
        "health",
        medium.clone(),
        "2026-03-01T00:00:00Z",
    );
    let high = snapshot_issue("security.csp", CheckStatus::Warn, Severity::High);
    let high_scan = save_scan_with_snapshot_issue(
        &db,
        site_id,
        "security",
        high.clone(),
        "2026-03-02T00:00:00Z",
    );
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://example.com",
        vec![
            crate::commands::scan::work_items::check_result_to_work_item_input(
                &medium,
                project_id,
                "https://example.com",
                medium_scan,
                1_000,
                None,
            ),
            crate::commands::scan::work_items::check_result_to_work_item_input(
                &high,
                project_id,
                "https://example.com",
                high_scan,
                2_000,
                None,
            ),
        ],
        2_000,
    )
    .expect("active Web issues");

    let state = load_dashboard_scan_state(&db, project_id, "https://example.com")
        .expect("dashboard scan state");
    assert_eq!(state.aggregated_check_counts.total, 1);
    assert_eq!(state.aggregated_check_counts.failed, 1);
    assert_eq!(state.aggregated_failed_issues.len(), 1);
    assert_eq!(state.aggregated_failed_issues[0].check_id, "security.csp");
    assert_eq!(state.aggregated_failed_issues[0].severity, Severity::High);
    assert_eq!(state.aggregated_failed_issues[0].status, CheckStatus::Warn);

    let badge = load_nav_badge_failed_issues(&db, project_id, "https://example.com")
        .expect("nav badge issues");
    assert_eq!(badge.len(), 1);
    assert_eq!(badge[0].check_id, "security.csp");
    assert_eq!(badge[0].severity, Severity::High);
}

#[test]
fn dashboard_and_badge_include_active_web_findings_from_every_page() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/signal-test", None)
        .expect("project");
    let issue = snapshot_issue("seo.meta_description", CheckStatus::Warn, Severity::Medium);
    let inputs = ["https://example.com/a", "https://example.com/b"]
        .into_iter()
        .enumerate()
        .map(|(index, page_url)| {
            crate::commands::scan::work_items::check_result_to_page_work_item_input(
                &issue,
                project_id,
                "https://example.com",
                page_url,
                index as i64 + 1,
                1_000,
                None,
            )
        })
        .collect();
    db.upsert_work_items_observe_only("web_scan", project_id, "https://example.com", inputs, 1_000)
        .expect("active page issues");

    let state = load_dashboard_scan_state(&db, project_id, "https://example.com")
        .expect("dashboard scan state");
    assert_eq!(state.aggregated_check_counts.failed, 1);
    assert_eq!(state.aggregated_failed_issues.len(), 1);

    let badge = load_nav_badge_failed_issues(&db, project_id, "https://example.com")
        .expect("nav badge issues");
    assert_eq!(badge.len(), 1);
    assert!(badge
        .iter()
        .all(|entry| entry.check_id == "seo.meta_description"));

    let pages = db
        .get_pages_with_issues(project_id, "https://example.com", 1_000)
        .expect("affected pages");
    assert_eq!(pages.len(), 2);
    assert!(pages.iter().all(|page| page.issue_count == 1));
}

#[test]
fn dashboard_and_badge_fail_loudly_on_malformed_issue_snapshot() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/signal-test", None)
        .expect("project");
    let site_id = project_site(&db, project_id);
    let scan_id = save_scan_with_snapshot_issue(
        &db,
        site_id,
        "health",
        snapshot_issue("security.csp", CheckStatus::Fail, Severity::High),
        "2026-03-01T00:00:00Z",
    );
    db.execute(move |conn| {
        conn.execute(
            "UPDATE scan_findings SET raw_data = '{ malformed' WHERE run_id = ?1",
            rusqlite::params![scan_id],
        )
        .map(|_| ())
    })
    .expect("database worker")
    .expect("corrupt snapshot fixture");

    assert!(load_dashboard_scan_state(&db, project_id, "https://example.com").is_err());
    let badge = load_nav_badge_failed_issues(&db, project_id, "https://example.com")
        .expect("nav badge reads active issues, not immutable history");
    assert_eq!(badge.len(), 1);
    assert_eq!(badge[0].check_id, "security.csp");
    let lightweight =
        build_lightweight_project_signal_snapshot(&db, project_id, Some("https://example.com"))
            .expect("lightweight snapshot reads canonical work, not immutable history");
    assert_eq!(lightweight.work_summary.issue_count, 1);
}

// Save matching immutable scan evidence and mutable lifecycle state.
fn save_scan_with_guidance_issue(
    db: &Database,
    project_id: i64,
    check_id: &str,
    score: u32,
    ts: &str,
    observed_at: i64,
) -> i64 {
    let site_id = project_site(db, project_id);
    let result = ScanResult {
        page_signals: None,
        site_facts: None,
        url: "https://example.com".to_string(),
        mode: "full".to_string(),
        scan_type: crate::core::scanner::ScanType::Health,
        overall_score: score,
        categories: vec![],
        issues: vec![CheckResult {
            check_id: check_id.to_string(),
            category: ScanCategory::Security,
            title: format!("Check {}", check_id),
            description: "test".to_string(),
            status: CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: Some("Add the missing header.".to_string()),
            manual_fix: Some("Set the header in your server config.".to_string()),
            raw_data: Some(serde_json::json!({ "header": null })),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("Without it, downgrade attacks succeed.".to_string()),
        }],
        detected_stack: None,
        duration_ms: 1_000,
        timestamp: ts.to_string(),
    };
    let scan_id = db.save_scan(site_id, &result).expect("save_scan");
    let item = WorkItemInput {
        project_id,
        env_url: "https://example.com".to_string(),
        source: "web_scan".to_string(),
        signal_id: format!("web_scan:{}:https://example.com", check_id),
        check_id: check_id.to_string(),
        category: "security".to_string(),
        severity: Severity::High,
        title: format!("Check {}", check_id),
        description: "test".to_string(),
        detail_json: Some("{\"header\":null}".to_string()),
        scan_ref: Some(scan_id),
        page_url: Some("https://example.com".to_string()),
        fix_prompt: Some("Add the missing header.".to_string()),
        manual_fix: Some("Set the header in your server config.".to_string()),
        why_it_matters: Some("Without it, downgrade attacks succeed.".to_string()),
        observed_at,
        metadata: WorkItemMetadata::default(),
    };
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://example.com",
        vec![item],
        observed_at,
    )
    .expect("upsert_work_items_diff");
    scan_id
}

// Free installs receive complete dashboard guidance.
#[test]
fn dashboard_scan_state_serves_free_tier_complete_guidance() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    save_scan_with_guidance_issue(
        &db,
        project_id,
        "security.csp",
        80,
        "2026-03-01T00:00:00Z",
        1_000,
    );
    save_scan_with_guidance_issue(
        &db,
        project_id,
        "security.hsts",
        90,
        "2026-03-02T00:00:00Z",
        2_000,
    );

    let state = load_dashboard_scan_state(&db, project_id, "https://example.com")
        .expect("dashboard scan state");

    let latest = state.latest_detail.as_ref().expect("latest detail");
    assert_eq!(latest.issues.len(), 1);
    assert!(latest.issues[0].fix_prompt.is_some());
    assert!(latest.issues[0].manual_fix.is_some());
    assert!(latest.issues[0].raw_data.is_some());
    assert_eq!(
        latest.issues[0].why_it_matters.as_deref(),
        Some("Without it, downgrade attacks succeed.")
    );

    let previous = state.previous_detail.as_ref().expect("previous detail");
    assert_eq!(previous.issues.len(), 1);
    assert!(previous.issues[0].fix_prompt.is_some());

    assert!(!state.aggregated_failed_issues.is_empty());
    for issue in &state.aggregated_failed_issues {
        assert!(issue.fix_prompt.is_some());
        assert!(issue.manual_fix.is_some());
    }
}

// The complete dashboard payload keeps remediation guidance. Only the
// purpose-built nav badge projection below strips fields it never consumes.
#[test]
fn dashboard_scan_state_preserves_complete_guidance() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    save_scan_with_guidance_issue(
        &db,
        project_id,
        "security.hsts",
        90,
        "2026-03-02T00:00:00Z",
        1_000,
    );

    let state = load_dashboard_scan_state(&db, project_id, "https://example.com")
        .expect("dashboard scan state");

    let latest = state.latest_detail.as_ref().expect("latest detail");
    assert_eq!(
        latest.issues[0].fix_prompt.as_deref(),
        Some("Add the missing header.")
    );
    assert_eq!(
        latest.issues[0].manual_fix.as_deref(),
        Some("Set the header in your server config.")
    );
    assert!(latest.issues[0].raw_data.is_some());

    assert_eq!(state.aggregated_failed_issues.len(), 1);
    assert!(state.aggregated_failed_issues[0].fix_prompt.is_some());
    assert!(state.aggregated_failed_issues[0].manual_fix.is_some());
    assert!(state.aggregated_failed_issues[0].raw_data.is_some());
}

// The nav badge consumes counts and identity only, so its projection omits
// remediation and raw evidence without making that a subscription boundary.
#[test]
fn nav_badge_failed_issues_omit_unused_remediation_fields() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    save_scan_with_guidance_issue(
        &db,
        project_id,
        "security.hsts",
        90,
        "2026-03-02T00:00:00Z",
        1_000,
    );

    let issues = load_nav_badge_failed_issues(&db, project_id, "https://example.com")
        .expect("nav badge issues");

    assert_eq!(issues.len(), 1);
    assert!(issues[0].fix_prompt.is_none());
    assert!(issues[0].manual_fix.is_none());
    assert!(issues[0].raw_data.is_none());
    assert_eq!(
        issues[0].why_it_matters.as_deref(),
        Some("Without it, downgrade attacks succeed.")
    );
}

#[test]
fn nav_badge_failed_issues_keep_the_fields_needed_for_counts() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    save_scan_with_guidance_issue(
        &db,
        project_id,
        "security.hsts",
        90,
        "2026-03-02T00:00:00Z",
        1_000,
    );

    let issues = load_nav_badge_failed_issues(&db, project_id, "https://example.com")
        .expect("nav badge issues");

    assert_eq!(issues.len(), 1);
    assert!(issues[0].fix_prompt.is_none());
    assert!(issues[0].manual_fix.is_none());
    assert!(issues[0].raw_data.is_none());
    assert_eq!(issues[0].check_id, "security.hsts");
    assert!(!issues[0].title.is_empty());
}

#[test]
fn lightweight_nav_badge_snapshot_uses_cached_data_and_canonical_work_summary() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    save_code_scan_with_work_items(
        &db,
        project_id,
        "https://example.com",
        &sample_code_scan_report(),
        120,
    );
    save_scan_with_guidance_issue(
        &db,
        project_id,
        "security.hsts",
        90,
        "2026-04-10T12:05:00Z",
        1_000_100,
    );
    db.upsert_work_items_diff(
        "updates",
        project_id,
        "https://example.com",
        vec![WorkItemInput {
            project_id,
            env_url: "https://example.com".to_string(),
            source: "updates".to_string(),
            signal_id: "updates:outdated-major:astro".to_string(),
            check_id: crate::core::correlation::resolve_check_id("updates", "outdated-major"),
            category: "dependencies".to_string(),
            severity: Severity::Low,
            title: "Astro has a major update".to_string(),
            description: "A newer major version is available.".to_string(),
            detail_json: None,
            scan_ref: None,
            page_url: None,
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            observed_at: 1_000_200,
            metadata: WorkItemMetadata::default(),
        }],
        1_000_200,
    )
    .expect("save update issue group");

    let report = sample_update_report("astro");
    db.save_project_updates_snapshot(
        project_id,
        Some("https://example.com"),
        &report,
        "2026-04-10T12:30:00Z",
    )
    .expect("save cached updates snapshot");

    let snapshot =
        build_lightweight_project_signal_snapshot(&db, project_id, Some("https://example.com"))
            .expect("lightweight snapshot");

    assert!(snapshot.code_scan_detail.is_none());
    assert_eq!(
        snapshot
            .code_scan_summary
            .as_ref()
            .expect("code scan summary")
            .grouped_issue_count,
        1
    );
    assert_eq!(
        snapshot
            .updates
            .as_ref()
            .expect("cached updates")
            .updates
            .len(),
        1
    );
    assert_eq!(snapshot.work_summary.issue_count, 3);
    assert_eq!(snapshot.work_summary.issue_web_count, 2);
    assert_eq!(snapshot.work_summary.issue_code_count, 1);
    assert_eq!(snapshot.work_summary.issue_critical_count, 1);
    assert_eq!(snapshot.work_summary.issue_high_count, 1);
    assert_eq!(snapshot.work_summary.issue_low_count, 1);
}

#[test]
fn lightweight_nav_badge_snapshot_reports_configured_integrations_on_cold_cache() {
    use crate::integrations::{IntegrationConfig, IntegrationType};

    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    for integration_type in [
        IntegrationType::GoogleSearchConsole,
        IntegrationType::BingWebmaster,
    ] {
        db.save_integration(
            project_id,
            &IntegrationConfig {
                integration_type,
                api_key: Some("key".to_string()),
                site_id: Some("https://example.com/".to_string()),
                extra: None,
                enabled: true,
            },
        )
        .expect("save_integration");
    }

    // No snapshot has been persisted, so the monitoring cache is cold - the same
    // state the nav-badge refresh hits right after a connect invalidates it.
    let snapshot =
        build_lightweight_project_signal_snapshot(&db, project_id, Some("https://example.com"))
            .expect("lightweight snapshot");

    assert!(snapshot
        .monitoring
        .enabled_integrations
        .contains(&"googlesearchconsole".to_string()));
    assert!(snapshot
        .monitoring
        .enabled_integrations
        .contains(&"bingwebmaster".to_string()));
}

#[test]
fn lightweight_nav_badge_snapshot_falls_back_to_project_wide_updates() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");
    let report = sample_update_report("react");

    db.save_project_updates_snapshot(project_id, None, &report, "2026-04-10T12:30:00Z")
        .expect("save project-wide updates snapshot");

    let snapshot =
        build_lightweight_project_signal_snapshot(&db, project_id, Some("https://example.com"))
            .expect("lightweight snapshot");

    let updates = snapshot.updates.as_ref().expect("cached updates");
    assert_eq!(updates.updates.len(), 1);
    assert_eq!(updates.updates[0].name, "react");
    assert_eq!(
        snapshot.updates_refreshed_at.as_deref(),
        Some("2026-04-10T12:30:00Z")
    );
}

#[test]
fn lightweight_nav_badge_snapshot_prefers_newer_project_wide_updates() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");
    let stale_empty_report = crate::updates::types::UpdateReport {
        packages: Vec::new(),
        updates: Vec::new(),
        ecosystems_detected: Vec::new(),
        scan_duration_ms: 12,
    };
    let newer_project_report = sample_update_report("react");

    db.save_project_updates_snapshot(
        project_id,
        Some("https://example.com"),
        &stale_empty_report,
        "2026-04-10T12:00:00Z",
    )
    .expect("save stale environment updates snapshot");
    db.save_project_updates_snapshot(
        project_id,
        None,
        &newer_project_report,
        "2026-04-10T12:30:00Z",
    )
    .expect("save newer project-wide updates snapshot");

    let snapshot =
        build_lightweight_project_signal_snapshot(&db, project_id, Some("https://example.com"))
            .expect("lightweight snapshot");

    let updates = snapshot.updates.as_ref().expect("cached updates");
    assert_eq!(updates.updates.len(), 1);
    assert_eq!(updates.updates[0].name, "react");
    assert_eq!(
        snapshot.updates_refreshed_at.as_deref(),
        Some("2026-04-10T12:30:00Z")
    );
}

#[test]
fn dashboard_code_scan_trend_uses_relevant_environment_history_in_ascending_order() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    db.save_code_scan(
        project_id,
        Some("https://example.com".to_string()),
        "/tmp/code-test".to_string(),
        &sample_code_scan_report_at("2026-04-01T12:00:00Z"),
        120,
    )
    .expect("save example.com scan 1");

    db.save_code_scan(
        project_id,
        Some("https://other.example.com".to_string()),
        "/tmp/code-test".to_string(),
        &sample_code_scan_report_at("2026-04-02T12:00:00Z"),
        120,
    )
    .expect("save other scan");

    db.save_code_scan(
        project_id,
        Some("https://example.com".to_string()),
        "/tmp/code-test".to_string(),
        &sample_code_scan_report_at("2026-04-03T12:00:00Z"),
        120,
    )
    .expect("save example.com scan 2");

    let trend = load_dashboard_code_scan_trend(&db, project_id, Some("https://example.com"))
        .expect("code scan trend");

    assert_eq!(trend.len(), 2);
    assert_eq!(trend[0].timestamp, "2026-04-01T12:00:00Z");
    assert_eq!(trend[0].issue_count, 1);
    assert_eq!(trend[0].critical_count, 1);
    assert_eq!(trend[0].high_count, 0);
    assert_eq!(trend[1].timestamp, "2026-04-03T12:00:00Z");
    assert_eq!(trend[1].issue_count, 1);
}
