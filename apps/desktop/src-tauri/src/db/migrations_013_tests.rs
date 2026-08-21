use crate::db::Database;
use rusqlite::Connection;

fn pre_013_conn() -> Connection {
    let conn = Connection::open_in_memory().expect("open db");
    super::ensure_version_table(&conn).expect("version table");
    super::apply_pending(&conn, &super::MIGRATIONS[..11], 0).expect("migrate through 011");
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable foreign keys");
    conn.execute_batch(
        r#"
        INSERT INTO projects (id, name, path, secret_namespace)
        VALUES (1, 'Cutover fixture', '/tmp/cutover', 'cutover-fixture');
        INSERT INTO environments (id, project_id, url, label, environment)
        VALUES (1, 1, 'https://example.com', 'Production', 'production');
        INSERT INTO sites (id, project_id, url)
        VALUES (1, 1, 'https://example.com');

        INSERT INTO scans (
            id, site_id, timestamp, mode, scan_type, overall_score,
            issues_total, issues_critical, issues_high, issues_medium,
            issues_low, issues_passed, duration_ms, issue_snapshot_version
        ) VALUES
            (1, 1, '2026-07-20T00:00:00Z', 'live', 'health', 91,
             1, 0, 0, 1, 0, 2, 1000, 1),
            (2, 1, '2026-07-21T00:00:00Z', 'live', 'health', 72,
             1, 0, 1, 0, 0, 1, 1200, 1);
        INSERT INTO scan_issues (
            scan_id, ordinal, check_id, category, title, description,
            check_status, severity, raw_data, confidence
        ) VALUES
            (1, 0, 'seo.description', 'seo', 'Description', 'short',
             'warn', 'medium', '{"length":40}', 'high'),
            (2, 0, 'security.headers.hsts', 'security', 'HSTS', 'missing',
             'fail', 'high', '{"header":"hsts"}', 'confirmed');
        "#,
    )
    .expect("seed v11 history");
    super::apply_pending(&conn, &super::MIGRATIONS[..12], 11).expect("migrate through 012");

    conn.execute_batch(
        r#"
        INSERT INTO issue_links (
            id, project_id, check_id, scan_id, provider, external_id,
            external_url, status, created_at
        ) VALUES (
            1, 1, 'security.headers.hsts', 2, 'github', 'SITE-1',
            'https://github.com/example/repo/issues/1', 'open',
            '2026-07-21T00:05:00Z'
        );

        INSERT INTO regressions (
            id, project_id, env_url, scan_type, prev_scan_id, scan_id,
            prev_score, score, commit_from, commit_to, commit_count,
            commits_json, fixed_check_ids_json, created_at
        ) VALUES (
            1, 1, 'https://example.com', 'web', 1, 2,
            91, 72, 'aaa', 'bbb', 1, '[]', '[]', 1784592300000
        );
        INSERT INTO regression_check_ids(regression_id, check_id)
        VALUES (1, 'security.headers.hsts');

        INSERT INTO alerts (
            id, project_id, env_url, source, alert_id, severity, title,
            description, detail_json, occurred_at, first_seen_at, last_seen_at
        ) VALUES (
            1, 1, 'https://example.com', 'regression',
            'deploy-regression:web:2', 'high', 'Regression', 'score dropped',
            '{"alert_type":"deploy_regression","regression_id":1,"scan_id":2}',
            1784592300000, 1784592300000, 1784592300000
        );

        INSERT INTO events (
            id, project_id, event_type, severity, occurred_at_ms, title,
            summary, source, source_id
        ) VALUES
            (1, 1, 'scan', 'info', 1784505600000, 'Web scan', '', 'internal', 'scan_1'),
            (2, 1, 'scan', 'warning', 1784592000000, 'Web scan', '', 'internal', 'scan_2');
        INSERT INTO site_event_check_ids(event_id, check_id)
        VALUES (2, 'security.headers.hsts');
        "#,
    )
    .expect("seed cutover references");
    conn
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
    .expect("inspect table")
}

#[test]
fn migration_013_maps_references_rebuilds_events_and_removes_legacy_tables() {
    let conn = pre_013_conn();
    super::apply_pending(&conn, &super::MIGRATIONS[12..13], 12).expect("cut over");

    assert_eq!(super::current_version(&conn).unwrap(), 13);
    for table in [
        "scans",
        "scan_sessions",
        "code_scans",
        "scan_issues",
        "session_issues",
        "code_scan_issues",
    ] {
        assert!(!table_exists(&conn, table), "legacy table {table} survived");
    }

    let linked_run: (i64, String, i64) = conn
        .query_row(
            "SELECT link.run_id, run.legacy_source, run.legacy_id
             FROM issue_links link JOIN scan_runs run ON run.id = link.run_id
             WHERE link.id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("mapped issue link");
    assert_eq!(linked_run.1, "web_scan");
    assert_eq!(linked_run.2, 2);

    let regression_runs: (i64, i64) = conn
        .query_row(
            "SELECT previous.legacy_id, current.legacy_id
             FROM regressions regression
             JOIN scan_runs previous ON previous.id = regression.prev_run_id
             JOIN scan_runs current ON current.id = regression.run_id
             WHERE regression.id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("mapped regression");
    assert_eq!(regression_runs, (1, 2));

    let alert_detail: (String, i64, Option<i64>) = conn
        .query_row(
            "SELECT alert_id,
                    json_extract(detail_json, '$.run_id'),
                    json_extract(detail_json, '$.scan_id')
             FROM alerts WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("rewritten alert");
    assert_eq!(
        alert_detail.0,
        format!("deploy-regression:web:{}", linked_run.0)
    );
    assert_eq!(alert_detail.1, linked_run.0);
    assert_eq!(alert_detail.2, None);

    let event_shape: (i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*),
                    SUM(source_id GLOB 'scan_execution_[0-9]*'),
                    SUM(source_id GLOB 'scan_[0-9]*')
             FROM events WHERE source = 'internal'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("canonical events");
    assert_eq!(event_shape, (2, 2, 0));

    let junction_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM site_event_check_ids
             WHERE check_id = 'security.hsts'",
            [],
            |row| row.get(0),
        )
        .expect("event junction");
    assert_eq!(junction_count, 1);

    let fk_failures = conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |_| Ok(()))
        .unwrap()
        .count();
    assert_eq!(fk_failures, 0);
}

#[test]
fn migration_013_validation_failure_rolls_back_the_entire_cutover() {
    let conn = pre_013_conn();
    conn.execute(
        "UPDATE scan_findings SET title = 'corrupted canonical copy' WHERE id = 1",
        [],
    )
    .expect("corrupt canonical copy");

    let error = super::apply_pending(&conn, &super::MIGRATIONS[12..13], 12)
        .expect_err("validation must abort");
    assert!(
        error.contains("Migration 13 failed"),
        "unexpected error: {error}"
    );
    assert_eq!(super::current_version(&conn).unwrap(), 12);
    assert!(table_exists(&conn, "scans"));
    assert!(table_exists(&conn, "scan_issues"));
    assert!(
        conn.prepare("SELECT run_id FROM issue_links").is_err(),
        "replacement issue_links schema leaked through rollback"
    );

    let legacy_link: i64 = conn
        .query_row("SELECT scan_id FROM issue_links WHERE id = 1", [], |row| {
            row.get(0)
        })
        .expect("legacy issue link remains");
    assert_eq!(legacy_link, 2);
    let fk_failures = conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |_| Ok(()))
        .unwrap()
        .count();
    assert_eq!(fk_failures, 0);
}

#[test]
fn database_open_creates_a_recoverable_v12_backup_before_cutover() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sitecmd.db");
    {
        let conn = Connection::open(&path).expect("open v12 fixture");
        super::ensure_version_table(&conn).expect("version table");
        super::apply_pending(&conn, &super::MIGRATIONS[..12], 0).expect("create v12 fixture");
    }

    let db = Database::open(path.clone()).expect("open and cut over");
    let live_version = db
        .execute(|conn| super::current_version(conn).expect("live version"))
        .expect("db worker");
    assert_eq!(live_version, super::latest_version());

    let backup_path = path.with_extension("db.pre-unified-scan-v13.bak");
    assert!(backup_path.exists(), "pre-cutover backup was not created");
    let backup =
        Connection::open_with_flags(&backup_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open backup");
    assert_eq!(super::current_version(&backup).unwrap(), 12);
    assert!(table_exists(&backup, "scans"));
    assert!(!table_exists(&backup, "issue_links_legacy"));

    let backup_fk_failures = backup
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |_| Ok(()))
        .unwrap()
        .count();
    assert_eq!(backup_fk_failures, 0);
}
