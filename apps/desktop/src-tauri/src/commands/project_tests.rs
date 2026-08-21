use super::{
    build_update_work_items, load_relevant_code_scan, normalize_environment_for_url,
    take_monitored_integrations,
};
use crate::checks::Severity;
use crate::db::test_helpers::temp_db;
use crate::db::work_items::WorkItemMetadata;
use crate::licensing::{
    config::{Tier, FINAL_GRACE_PERIOD_SECS, OFFLINE_GRACE_PERIOD_SECS},
    store::{self, LicenseState},
};

fn stale_core_license_state() -> LicenseState {
    LicenseState {
        license_key: "test-key".into(),
        instance_id: "inst-123".into(),
        variant_id: 42,
        tier: Tier::Core,
        status: "active".into(),
        // Past the FULL window (offline grace plus the promised
        // final-warning day), which is where entitlement now cuts to Free.
        last_validated_at: (chrono::Utc::now()
            - chrono::Duration::seconds(
                (OFFLINE_GRACE_PERIOD_SECS + FINAL_GRACE_PERIOD_SECS) as i64 + 60,
            ))
        .to_rfc3339(),
        activated_at: chrono::Utc::now().to_rfc3339(),
        expires_at: None,
    }
}

#[test]
fn normalize_environment_for_url_uses_canonical_environment_names() {
    assert_eq!(
        normalize_environment_for_url("http://127.0.0.1:4321", "production"),
        "local"
    );
    assert_eq!(
        normalize_environment_for_url("https://dev.example.com", "production"),
        "development"
    );
    assert_eq!(
        normalize_environment_for_url("https://preview-my-app.vercel.app", "production"),
        "staging"
    );
    assert_eq!(
        normalize_environment_for_url("https://qa.example.com", "preview"),
        "staging"
    );
    assert_eq!(
        normalize_environment_for_url("https://example.com", "prod"),
        "production"
    );
    assert_eq!(
        normalize_environment_for_url("https://example.com", "production"),
        "production"
    );
    assert_eq!(
        normalize_environment_for_url("http://127.0.0.1:4321", "staging"),
        "local"
    );
}

#[test]
fn project_creation_has_no_limit_seam_whatever_the_license_state() {
    let db = temp_db();
    db.upsert_project("Existing", "/tmp/existing-project", None)
        .expect("project");

    let stale_license = stale_core_license_state();
    db.execute(move |conn| store::save(conn, &stale_license).expect("save license"))
        .expect("db worker");

    for n in 0..12 {
        db.upsert_project(&format!("Site {n}"), &format!("/tmp/site-{n}"), None)
            .expect("every additional project creates cleanly");
    }
}

#[test]
fn monitored_integrations_include_all_supported_types() {
    let enabled = vec![
        "plausible".to_string(),
        "uptimerobot".to_string(),
        "cloudflare".to_string(),
        "github".to_string(),
    ];

    let expected_monitored = vec![
        "plausible".to_string(),
        "uptimerobot".to_string(),
        "cloudflare".to_string(),
    ];

    assert_eq!(take_monitored_integrations(&enabled), expected_monitored);
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

fn sample_code_scan_report_with_context() -> crate::core::code_scan::CodeScanReport {
    crate::core::code_scan::CodeScanReport {
        skipped_scopes: Default::default(),
        checked_at: "2026-04-11T12:00:00Z".to_string(),
        framework: Some("Next.js".to_string()),
        issue_count: 2,
        critical_count: 1,
        high_count: 1,
        medium_count: 0,
        low_count: 0,
        issues: vec![
            crate::core::code_scan::CodeIssue {
                check_id: String::new(),
                id: "critical-db-owner-scope".to_string(),
                category: "database".to_string(),
                severity: crate::checks::Severity::Critical,
                title: "Missing owner scope".to_string(),
                description: "Query is not scoped to the current owner.".to_string(),
                relative_path: "app/api/projects/route.ts".to_string(),
                absolute_path: "/tmp/code-test/app/api/projects/route.ts".to_string(),
                line: Some(18),
                source_excerpt: Some("return db.project.findMany({});".to_string()),
                evidence: Some("Owner filter is missing.".to_string()),
                why_now: Some("A related auth change widened access.".to_string()),
                likely_fix: Some("Scope the query to the current owner id.".to_string()),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                verify_hint: Some("Re-run the code scan after the auth fix.".to_string()),
            },
            crate::core::code_scan::CodeIssue {
                check_id: String::new(),
                id: "high-client-auth".to_string(),
                category: "security".to_string(),
                severity: crate::checks::Severity::High,
                title: "Client-enforced auth check".to_string(),
                description: "The API route relies on client-side auth state.".to_string(),
                relative_path: "app/api/auth/route.ts".to_string(),
                absolute_path: "/tmp/code-test/app/api/auth/route.ts".to_string(),
                line: Some(9),
                source_excerpt: Some("if (!session) return null;".to_string()),
                evidence: Some("Missing server-side guard.".to_string()),
                why_now: Some("A refactor removed the server check.".to_string()),
                likely_fix: Some("Restore server-side authorization.".to_string()),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                verify_hint: Some("Exercise the route without a session.".to_string()),
            },
        ],
    }
}

// Save a code scan and immediately persist its issues as work_items.
// Mirrors the runtime path in commands/scan.rs so that queries that read
// from work_items (domain summaries, issue views, etc.) work in tests.
fn save_code_scan_with_work_items(
    db: &crate::db::Database,
    project_id: i64,
    env_url: &str,
    report: &crate::core::code_scan::CodeScanReport,
    duration_ms: u64,
) -> i64 {
    use crate::db::work_items::WorkItemInput;
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
fn load_relevant_code_scan_can_skip_detail() {
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

    let (summary, previous_summary, detail) =
        load_relevant_code_scan(&db, project_id, Some("https://example.com"), false)
            .expect("load without detail");

    assert!(summary.is_some());
    assert!(previous_summary.is_none());
    assert!(detail.is_none());
}

#[test]
fn load_relevant_code_scan_can_include_detail() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    let scan_id = save_code_scan_with_work_items(
        &db,
        project_id,
        "https://example.com",
        &sample_code_scan_report(),
        120,
    );

    let (summary, previous_summary, detail) =
        load_relevant_code_scan(&db, project_id, Some("https://example.com"), true)
            .expect("load with detail");

    assert_eq!(summary.expect("summary").id, scan_id);
    assert!(previous_summary.is_none());
    assert_eq!(detail.expect("detail").issues.len(), 1);
}

#[test]
fn load_relevant_code_scan_returns_previous_summary_for_matching_environment() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    let first_id = db
        .save_code_scan(
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

    let latest_id = db
        .save_code_scan(
            project_id,
            Some("https://example.com".to_string()),
            "/tmp/code-test".to_string(),
            &sample_code_scan_report_at("2026-04-03T12:00:00Z"),
            120,
        )
        .expect("save example.com scan 2");

    let (summary, previous_summary, detail) =
        load_relevant_code_scan(&db, project_id, Some("https://example.com"), false)
            .expect("load without detail");

    assert_eq!(summary.expect("summary").id, latest_id);
    assert_eq!(previous_summary.expect("previous summary").id, first_id);
    assert!(detail.is_none());
}

#[test]
fn code_scan_issue_views_stay_lightweight_while_full_detail_keeps_context() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    let scan_id = save_code_scan_with_work_items(
        &db,
        project_id,
        "https://example.com",
        &sample_code_scan_report_with_context(),
        120,
    );

    let lightweight = db
        .get_code_scan_issue_views(scan_id)
        .expect("load lightweight issue views");
    let full = db
        .get_code_scan_detail(scan_id)
        .expect("load full detail")
        .expect("detail present");

    assert_eq!(lightweight.len(), 2);
    assert!(lightweight[0].source_excerpt.is_none());
    assert!(lightweight[0].evidence.is_none());
    assert!(lightweight[0].why_now.is_none());
    assert!(lightweight[0].likely_fix.is_none());
    assert!(lightweight[0].verify_hint.is_none());

    assert_eq!(
        full.issues[0].source_excerpt.as_deref(),
        Some("return db.project.findMany({});")
    );
    assert_eq!(
        full.issues[0].evidence.as_deref(),
        Some("Owner filter is missing.")
    );
    assert_eq!(
        full.issues[0].verify_hint.as_deref(),
        Some("Re-run the code scan after the auth fix.")
    );
}

#[test]
fn code_scan_count_keys_match_view_based_grouped_counts() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    let scan_id = save_code_scan_with_work_items(
        &db,
        project_id,
        "https://example.com",
        &sample_code_scan_report_with_context(),
        120,
    );

    let views = db
        .get_code_scan_issue_views(scan_id)
        .expect("load issue views");
    let keys = db
        .get_code_scan_issue_count_keys(scan_id)
        .expect("load count keys");

    assert_eq!(views.len(), keys.len());
    let mut view_fields: Vec<String> = views
        .iter()
        .map(|v| {
            format!(
                "{}|{}|{}|{}",
                v.check_id,
                v.domain.as_str(),
                v.severity.as_str(),
                v.title
            )
        })
        .collect();
    let mut key_fields: Vec<String> = keys
        .iter()
        .map(|k| {
            format!(
                "{}|{}|{}|{}",
                k.check_id,
                k.domain.as_str(),
                k.severity.as_str(),
                k.title
            )
        })
        .collect();
    view_fields.sort();
    key_fields.sort();
    assert_eq!(view_fields, key_fields);

    assert_eq!(
        crate::commands::project_signal_state::grouped_active_code_counts(&views),
        crate::commands::project_signal_state::grouped_code_counts_from_keys(&keys),
    );
}

#[test]
fn top_code_scan_issue_view_prefers_highest_severity() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");

    let scan_id = save_code_scan_with_work_items(
        &db,
        project_id,
        "https://example.com",
        &sample_code_scan_report_with_context(),
        120,
    );

    let top_issue = db
        .get_top_code_scan_issue_view(scan_id)
        .expect("load top issue")
        .expect("top issue present");

    assert_eq!(top_issue.id, "critical-db-owner-scope");
    assert!(top_issue.source_excerpt.is_none());
    assert!(top_issue.verify_hint.is_none());
}

#[test]
fn build_update_work_items_use_exact_package_targets() {
    let report = crate::updates::types::UpdateReport {
        packages: vec![],
        updates: vec![crate::updates::types::PackageUpdate {
            name: "react".to_string(),
            current_version: "18.2.0".to_string(),
            latest_version: "19.0.0".to_string(),
            ecosystem: crate::updates::types::Ecosystem::Npm,
            update_type: crate::updates::types::UpdateType::Major,
            is_security: true,
            advisory_severity: Some("critical".to_string()),
            advisory_url: None,
            source: "package.json".to_string(),
            is_dev: false,
            ..Default::default()
        }],
        ecosystems_detected: vec![crate::updates::types::Ecosystem::Npm],
        scan_duration_ms: 12,
    };

    let items = build_update_work_items(7, Some("https://example.com"), Some(&report));
    let item = items.first().expect("update work item");

    assert_eq!(item.stable_key, "update:7:npm:react");
    assert_eq!(item.target.page, "updates");
    assert_eq!(item.target.item_id.as_deref(), Some("npm:react"));
    assert_eq!(item.target.reason.as_deref(), Some("security-update"));
}

#[test]
fn build_update_work_items_only_surfaces_critical_security_updates() {
    let report = crate::updates::types::UpdateReport {
        packages: vec![],
        updates: vec![
            crate::updates::types::PackageUpdate {
                name: "next".to_string(),
                current_version: "14.2.0".to_string(),
                latest_version: "14.2.9".to_string(),
                ecosystem: crate::updates::types::Ecosystem::Npm,
                update_type: crate::updates::types::UpdateType::Patch,
                is_security: true,
                advisory_severity: Some("critical".to_string()),
                advisory_url: None,
                source: "package.json".to_string(),
                is_dev: false,
                ..Default::default()
            },
            crate::updates::types::PackageUpdate {
                name: "vite".to_string(),
                current_version: "5.4.0".to_string(),
                latest_version: "5.4.4".to_string(),
                ecosystem: crate::updates::types::Ecosystem::Npm,
                update_type: crate::updates::types::UpdateType::Patch,
                is_security: true,
                advisory_severity: Some("high".to_string()),
                advisory_url: None,
                source: "package.json".to_string(),
                is_dev: true,
                ..Default::default()
            },
            crate::updates::types::PackageUpdate {
                name: "react".to_string(),
                current_version: "18.2.0".to_string(),
                latest_version: "19.0.0".to_string(),
                ecosystem: crate::updates::types::Ecosystem::Npm,
                update_type: crate::updates::types::UpdateType::Major,
                is_security: false,
                advisory_severity: None,
                advisory_url: None,
                source: "package.json".to_string(),
                is_dev: false,
                ..Default::default()
            },
        ],
        ecosystems_detected: vec![crate::updates::types::Ecosystem::Npm],
        scan_duration_ms: 12,
    };

    let items = build_update_work_items(7, Some("https://example.com"), Some(&report));

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].package_name.as_deref(), Some("next"));
    assert_eq!(items[0].severity, Some(Severity::Critical));
}
