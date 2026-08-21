use super::build_project_maintenance_items;
use crate::core::scanner::ScanResult;
use crate::db::test_helpers::temp_db;
use crate::db::{
    types::{EventSeverity, EventSource, EventType, ProjectMonitoringSignals, SiteEvent},
    Database, WorkItemKind, WorkItemStatus,
};

fn project_site(db: &Database, project_id: i64) -> i64 {
    db.add_environment(
        project_id,
        "https://example.com",
        "Production",
        "production",
        "manual",
    )
    .expect("environment");
    db.get_or_create_site_for_project(project_id, "https://example.com")
        .expect("project site")
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
        duration_ms: 1000,
        timestamp: ts.to_string(),
    };
    db.save_scan(site_id, &result).expect("save_scan");
}

fn hours_ago(hours: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::hours(hours)).to_rfc3339()
}

fn hours_ago_ms(hours: i64) -> i64 {
    (chrono::Utc::now() - chrono::Duration::hours(hours)).timestamp_millis()
}

fn latest_scan_detail(db: &Database, url: &str) -> Option<ScanResult> {
    let history = db.get_scan_history(url, 1).expect("scan history");
    let scan_id = history.first().expect("latest scan").id;
    Some(
        db.get_scan_detail(scan_id)
            .expect("load scan detail")
            .expect("scan detail"),
    )
}

#[test]
fn build_project_maintenance_items_adds_scan_after_deploy_when_release_is_newer_than_scan() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");
    let site_id = project_site(&db, project_id);
    let scan_ts = hours_ago(3);
    let deploy_ts_ms = hours_ago_ms(1);
    save_scan_with_type(&db, site_id, "health", 88, &scan_ts);
    db.insert_events(&[SiteEvent {
        id: 0,
        project_id,
        event_type: EventType::Deploy,
        severity: EventSeverity::Info,
        occurred_at_ms: deploy_ts_ms,
        title: "Ship release 42".to_string(),
        summary: "abc123 - Kyle".to_string(),
        detail: None,
        source: EventSource::Git,
        source_id: Some("abc123".to_string()),
        metadata: None,
        affected_check_ids: None,
    }])
    .expect("insert deploy");

    let latest_scan = latest_scan_detail(&db, "https://example.com");
    let items = build_project_maintenance_items(
        &db,
        project_id,
        Some("https://example.com"),
        latest_scan.as_ref(),
        None,
        None,
        &ProjectMonitoringSignals::default(),
    )
    .expect("build maintenance items");

    let deploy_item = items
        .iter()
        .find(|item| item.stable_key == "maintenance:https://example.com:scan-after-deploy")
        .expect("deploy maintenance item");

    assert_eq!(deploy_item.title, "Re-run Web Scan after deploy");
    assert_eq!(
        deploy_item.target.reason.as_deref(),
        Some("scan-after-deploy")
    );
    assert_eq!(deploy_item.target.page, "issues");
    assert_eq!(deploy_item.target.scan_kind.as_deref(), Some("site"));
}

#[test]
fn build_project_maintenance_items_skips_scan_after_deploy_when_scan_is_current() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");
    let site_id = project_site(&db, project_id);
    let deploy_ts_ms = hours_ago_ms(3);
    let scan_ts = hours_ago(1);
    save_scan_with_type(&db, site_id, "health", 88, &scan_ts);
    db.insert_events(&[SiteEvent {
        id: 0,
        project_id,
        event_type: EventType::Deploy,
        severity: EventSeverity::Info,
        occurred_at_ms: deploy_ts_ms,
        title: "Ship release 42".to_string(),
        summary: "abc123 - Kyle".to_string(),
        detail: None,
        source: EventSource::Git,
        source_id: Some("abc123".to_string()),
        metadata: None,
        affected_check_ids: None,
    }])
    .expect("insert deploy");

    let latest_scan = latest_scan_detail(&db, "https://example.com");
    let items = build_project_maintenance_items(
        &db,
        project_id,
        Some("https://example.com"),
        latest_scan.as_ref(),
        None,
        None,
        &ProjectMonitoringSignals::default(),
    )
    .expect("build maintenance items");

    assert!(items
        .iter()
        .all(|item| item.stable_key != "maintenance:https://example.com:scan-after-deploy"));
}

#[test]
fn build_project_maintenance_items_adds_deploy_regression_for_latest_scan_drop() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");
    let site_id = project_site(&db, project_id);
    let first_scan_ts = hours_ago(4);
    let deploy_ts_ms = hours_ago_ms(3);
    let latest_scan_ts = hours_ago(1);
    save_scan_with_type(&db, site_id, "health", 95, &first_scan_ts);
    db.insert_events(&[SiteEvent {
        id: 0,
        project_id,
        event_type: EventType::Deploy,
        severity: EventSeverity::Info,
        occurred_at_ms: deploy_ts_ms,
        title: "Ship release 43".to_string(),
        summary: "def456 - Kyle".to_string(),
        detail: None,
        source: EventSource::Git,
        source_id: Some("def456".to_string()),
        metadata: None,
        affected_check_ids: None,
    }])
    .expect("insert deploy");
    save_scan_with_type(&db, site_id, "health", 81, &latest_scan_ts);

    let latest_scan = latest_scan_detail(&db, "https://example.com");
    let items = build_project_maintenance_items(
        &db,
        project_id,
        Some("https://example.com"),
        latest_scan.as_ref(),
        None,
        None,
        &ProjectMonitoringSignals::default(),
    )
    .expect("build maintenance items");

    let deploy_item = items
        .iter()
        .find(|item| item.stable_key == "maintenance:https://example.com:deploy-regression")
        .expect("deploy regression maintenance item");

    assert!(matches!(deploy_item.status, WorkItemStatus::Regressed));
    assert_eq!(deploy_item.target.page, "deploys");
    assert_eq!(
        deploy_item.target.reason.as_deref(),
        Some("deploy-regression")
    );
}

#[test]
fn build_project_maintenance_items_adds_search_recheck_after_watched_file_change() {
    let db = temp_db();
    let dir = tempfile::tempdir().expect("project tempdir");
    let project_path = dir.path().join("repo");
    std::fs::create_dir_all(project_path.join("public")).expect("create public dir");
    std::fs::write(
        project_path.join("public/robots.txt"),
        "User-agent: *\nAllow: /\n",
    )
    .expect("write robots");

    let project_id = db
        .upsert_project(
            "Signal Test",
            project_path.to_str().expect("project path"),
            Some("nextjs"),
        )
        .expect("upsert");
    let site_id = project_site(&db, project_id);
    save_scan_with_type(&db, site_id, "health", 88, "2026-04-01T10:00:00Z");

    let latest_scan = latest_scan_detail(&db, "https://example.com");
    let items = build_project_maintenance_items(
        &db,
        project_id,
        Some("https://example.com"),
        latest_scan.as_ref(),
        None,
        None,
        &ProjectMonitoringSignals::default(),
    )
    .expect("build maintenance items");

    let watch_item = items
        .iter()
        .find(|item| item.target.reason.as_deref() == Some("changed-search-file"))
        .expect("search watch maintenance item");

    assert_eq!(watch_item.target.page, "search-console");
    assert_eq!(watch_item.target.focus.as_deref(), Some("seo.robots"));
    assert_eq!(watch_item.kind, WorkItemKind::Web);
    assert!(watch_item
        .target
        .file_path
        .as_deref()
        .expect("file path")
        .ends_with("/public/robots.txt"));
}

#[test]
fn build_project_maintenance_items_skips_watched_search_file_when_scan_is_newer() {
    let db = temp_db();
    let dir = tempfile::tempdir().expect("project tempdir");
    let project_path = dir.path().join("repo");
    std::fs::create_dir_all(project_path.join("public")).expect("create public dir");
    std::fs::write(
        project_path.join("public/robots.txt"),
        "User-agent: *\nAllow: /\n",
    )
    .expect("write robots");

    let project_id = db
        .upsert_project(
            "Signal Test",
            project_path.to_str().expect("project path"),
            Some("nextjs"),
        )
        .expect("upsert");
    let site_id = project_site(&db, project_id);
    save_scan_with_type(&db, site_id, "health", 88, "2100-04-01T10:00:00Z");

    let latest_scan = latest_scan_detail(&db, "https://example.com");
    let items = build_project_maintenance_items(
        &db,
        project_id,
        Some("https://example.com"),
        latest_scan.as_ref(),
        None,
        None,
        &ProjectMonitoringSignals::default(),
    )
    .expect("build maintenance items");

    assert!(items
        .iter()
        .all(|item| item.target.reason.as_deref() != Some("changed-search-file")));
}

#[test]
fn build_project_maintenance_items_adds_updates_refresh_after_dependency_change() {
    let db = temp_db();
    let dir = tempfile::tempdir().expect("project tempdir");
    let project_path = dir.path().join("repo");
    std::fs::create_dir_all(&project_path).expect("create project dir");
    std::fs::write(
        project_path.join("package-lock.json"),
        "{ \"name\": \"demo\" }\n",
    )
    .expect("write lockfile");

    let project_id = db
        .upsert_project(
            "Signal Test",
            project_path.to_str().expect("project path"),
            Some("nextjs"),
        )
        .expect("upsert");

    let items = build_project_maintenance_items(
        &db,
        project_id,
        Some("https://example.com"),
        None,
        None,
        Some("2026-04-01T10:00:00Z"),
        &ProjectMonitoringSignals::default(),
    )
    .expect("build maintenance items");

    let watch_item = items
        .iter()
        .find(|item| item.target.reason.as_deref() == Some("changed-dependencies"))
        .expect("updates watch maintenance item");

    assert_eq!(watch_item.target.page, "updates");
    assert_eq!(watch_item.kind, WorkItemKind::Update);
    assert!(watch_item
        .target
        .file_path
        .as_deref()
        .expect("file path")
        .ends_with("/package-lock.json"));
}

#[test]
fn build_project_maintenance_items_propagates_event_history_failures() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");
    db.execute(|conn| conn.execute("DROP TABLE events", []))
        .expect("database worker")
        .expect("drop events table");

    let error = build_project_maintenance_items(
        &db,
        project_id,
        Some("https://example.com"),
        None,
        None,
        None,
        &ProjectMonitoringSignals::default(),
    )
    .expect_err("event read failure must not become an empty maintenance list");

    assert!(error.contains("deploy events"));
}

#[test]
fn build_project_maintenance_items_rejects_malformed_scan_timestamps() {
    let db = temp_db();
    let project_id = db
        .upsert_project("Signal Test", "/tmp/code-test", Some("nextjs"))
        .expect("upsert");
    let site_id = project_site(&db, project_id);
    save_scan_with_type(&db, site_id, "health", 88, "2026-04-01T10:00:00Z");
    let mut latest_scan = latest_scan_detail(&db, "https://example.com").expect("latest scan");
    latest_scan.timestamp = "not-a-timestamp".to_string();

    let error = build_project_maintenance_items(
        &db,
        project_id,
        Some("https://example.com"),
        Some(&latest_scan),
        None,
        None,
        &ProjectMonitoringSignals::default(),
    )
    .expect_err("malformed issue provenance time must fail closed");

    assert!(error.contains("latest Web Scan has an invalid timestamp"));
}
