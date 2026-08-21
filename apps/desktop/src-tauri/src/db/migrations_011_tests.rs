use rusqlite::Connection;

// A migrated `fix_attempts` row as the identity assertions read it:
// check_id, target_kind, target_relative_path, target_line, status.
type MigratedFixAttempt = (String, String, Option<String>, Option<i64>, String);

fn pre_011_conn() -> Connection {
    let conn = Connection::open_in_memory().expect("open db");
    super::ensure_version_table(&conn).expect("version table");
    super::apply_pending(&conn, &super::MIGRATIONS[..10], 0).expect("migrate through 010");
    conn
}

fn seed_identity_fixture(conn: &Connection) {
    conn.execute_batch(
        r#"
        INSERT INTO projects (name, path, secret_namespace)
        VALUES ('Identity', '/tmp/identity', 'identity');
        INSERT INTO sites (project_id, url) VALUES (1, 'https://example.com');
        INSERT INTO scans
            (site_id, timestamp, mode, scan_type, overall_score, duration_ms)
        VALUES (1, '2026-07-20T00:00:00Z', 'live', 'health', 80, 1);
        INSERT INTO code_scans
            (project_id, environment_url, project_path, checked_at,
             overall_score, issue_count, duration_ms, issue_snapshot_version)
        VALUES (1, 'https://example.com', '/tmp/identity',
                '2026-07-20T00:00:00Z', 80, 2, 1, 1);
        INSERT INTO events
            (project_id, event_type, occurred_at_ms, title, source, source_id)
        VALUES (1, 'scan_completed', 1000, 'scan', 'internal', 'identity-event');

        INSERT INTO work_items
            (project_id, env_url, source, signal_id, check_id, category,
             severity, title, description, detail_json, first_seen_at,
             last_seen_at, relative_path, line)
        VALUES
            (1, 'https://example.com', 'code_scan', 'code:a',
             'code_scan.hardcoded-secret:src/a.ts', 'code_quality', 'high',
             'Secret', 'a',
             '{"id":"hardcoded-secret:src/a.ts","checkId":"code_scan.hardcoded-secret:src/a.ts","relativePath":"src/a.ts","line":10}',
             100, 200, 'src/a.ts', 10),
            (1, 'https://example.com', 'code_scan', 'code:b',
             'code_scan.hardcoded-secret:src/b.ts', 'code_quality', 'high',
             'Secret renamed', 'b',
             '{"id":"hardcoded-secret:src/b.ts","checkId":"code_scan.hardcoded-secret:src/b.ts","relativePath":"src/b.ts","line":20}',
             110, 220, 'src/b.ts', 20);

        INSERT INTO code_scan_issues
            (scan_id, ordinal, canonical_check_id, domain, severity, title, issue_json)
        VALUES
            (1, 0, 'code_scan.hardcoded-secret:src/a.ts', 'security', 'high', 'Secret',
             '{"id":"hardcoded-secret:src/a.ts","checkId":"code_scan.hardcoded-secret:src/a.ts","relativePath":"src/a.ts","line":10}'),
            (1, 1, 'code_scan.hardcoded-secret:src/b.ts', 'security', 'high', 'Secret renamed',
             '{"id":"hardcoded-secret:src/b.ts","checkId":"code_scan.hardcoded-secret:src/b.ts","relativePath":"src/b.ts","line":20}');

        INSERT INTO project_issue_states
            (project_id, env_url, check_id, status, last_status_changed_at)
        VALUES
            (1, 'https://example.com', 'code_scan.hardcoded-secret:src/a.ts', 'ignored', 100),
            (1, 'https://example.com', 'code_scan.hardcoded-secret:src/b.ts', 'blocked', 200);

        INSERT INTO fix_attempts
            (project_id, env_url, check_id, agent_tool, status, created_at, updated_at)
        VALUES
            (1, 'https://example.com', 'code_scan.hardcoded-secret:src/a.ts', 'codex', 'briefed', 100, 100),
            (1, 'https://example.com', 'code_scan.hardcoded-secret:src/b.ts', 'codex', 'verifying', 200, 200);

        INSERT INTO issue_links
            (project_id, check_id, scan_id, provider, external_id,
             external_url, created_at)
        VALUES
            (1, 'code_scan.hardcoded-secret:src/a.ts', 1, 'github', '1', 'https://example.com/1', 'now'),
            (1, 'code_scan.hardcoded-secret:src/b.ts', 1, 'github', '2', 'https://example.com/2', 'now');

        INSERT INTO site_event_check_ids(event_id, check_id) VALUES
            (1, 'code_scan.hardcoded-secret:src/a.ts'),
            (1, 'code_scan.hardcoded-secret:src/b.ts');
        INSERT INTO dismissed_integration_hints
            (project_id, check_id, integration_type, dismissed_at)
        VALUES
            (1, 'code_scan.hardcoded-secret:src/a.ts', 'github', 100),
            (1, 'code_scan.hardcoded-secret:src/b.ts', 'github', 200);
        INSERT INTO causal_link_observations
            (project_id, cause_check_id, effect_check_id, observed_at,
             co_active, co_resolved)
        VALUES
            (1, 'code_scan.hardcoded-secret:src/a.ts', 'security.csp', 100, 1, 0),
            (1, 'code_scan.hardcoded-secret:src/b.ts', 'security.csp', 100, 1, 0);
        INSERT INTO cross_project_pattern_index
            (check_id, project_count, latest_seen_ms, updated_at)
        VALUES
            ('code_scan.hardcoded-secret:src/a.ts', 1, 200, 200),
            ('code_scan.hardcoded-secret:src/b.ts', 1, 220, 220);

        INSERT INTO regressions
            (project_id, env_url, scan_type, prev_scan_id, scan_id,
             prev_score, score, commit_from, commit_to, commits_json,
             fixed_check_ids_json, created_at)
        VALUES
            (1, 'https://example.com', 'code', 1, 2, 90, 80, 'a', 'b', '[]',
             '["code_scan.hardcoded-secret:src/a.ts","code_scan.hardcoded-secret:src/b.ts","security.csp"]',
             300);
        INSERT INTO regression_check_ids(regression_id, check_id) VALUES
            (1, 'code_scan.hardcoded-secret:src/a.ts'),
            (1, 'code_scan.hardcoded-secret:src/b.ts');
        "#,
    )
    .expect("seed fixture");
}

#[test]
fn migration_011_collapses_groups_and_preserves_occurrence_targets() {
    let conn = pre_011_conn();
    seed_identity_fixture(&conn);
    super::apply_pending(&conn, &super::MIGRATIONS[..11], 10).expect("apply 011");

    let occurrences: Vec<(String, String, Option<String>, Option<i64>)> = conn
        .prepare(
            "SELECT check_id, producer_check_id, relative_path, line
             FROM work_items ORDER BY signal_id",
        )
        .expect("prepare occurrences")
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query occurrences")
        .collect::<Result<_, _>>()
        .expect("collect occurrences");
    assert_eq!(occurrences.len(), 2);
    assert!(occurrences
        .iter()
        .all(|row| row.0 == "code_scan.hardcoded-secret" && row.1 == "hardcoded-secret"));
    assert_eq!(occurrences[0].2.as_deref(), Some("src/a.ts"));
    assert_eq!(occurrences[1].3, Some(20));

    let state: (String, String) = conn
        .query_row(
            "SELECT check_id, status FROM project_issue_states",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("state");
    assert_eq!(
        state,
        ("code_scan.hardcoded-secret".into(), "blocked".into())
    );

    let attempts: Vec<MigratedFixAttempt> = conn
        .prepare(
            "SELECT check_id, target_kind, target_relative_path, target_line, status
             FROM fix_attempts ORDER BY id",
        )
        .expect("prepare attempts")
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .expect("query attempts")
        .collect::<Result<_, _>>()
        .expect("collect attempts");
    assert_eq!(attempts.len(), 2, "different sibling targets both survive");
    assert!(attempts.iter().all(|row| {
        row.0 == "code_scan.hardcoded-secret" && row.1 == "occurrence" && row.4 != "canceled"
    }));
    assert_eq!(attempts[0].2.as_deref(), Some("src/a.ts"));
    assert_eq!(attempts[1].3, Some(20));

    let snapshot_mismatches: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM code_scan_issues
             WHERE canonical_check_id <> json_extract(issue_json, '$.checkId')",
            [],
            |row| row.get(0),
        )
        .expect("snapshot parity");
    assert_eq!(snapshot_mismatches, 0);

    for (table, expected) in [
        ("site_event_check_ids", 1_i64),
        ("dismissed_integration_hints", 1),
        ("regression_check_ids", 1),
        ("causal_link_observations", 1),
        ("cross_project_pattern_index", 1),
    ] {
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count migrated references");
        assert_eq!(count, expected, "{table} collision policy");
    }

    let fixed: String = conn
        .query_row("SELECT fixed_check_ids_json FROM regressions", [], |row| {
            row.get(0)
        })
        .expect("fixed ids");
    let fixed: Vec<String> = serde_json::from_str(&fixed).expect("fixed ids json");
    assert_eq!(fixed.len(), 2);
    assert!(fixed.contains(&"code_scan.hardcoded-secret".to_string()));
    assert!(fixed.contains(&"security.csp".to_string()));
}

#[test]
fn migration_011_rolls_back_when_serialized_code_identity_is_unreadable() {
    let conn = pre_011_conn();
    conn.execute_batch(
        "INSERT INTO projects (name, secret_namespace) VALUES ('p', 'rollback');
         INSERT INTO work_items
            (project_id, env_url, source, signal_id, check_id, category,
             severity, title, description, detail_json, first_seen_at, last_seen_at)
         VALUES (1, 'https://example.com', 'code_scan', 'code:a',
                 'code_scan.hardcoded-secret:src/a.ts', 'code_quality', 'high',
                 'Secret', 'bad payload', '{not-json', 1, 1);",
    )
    .expect("seed corrupt payload");

    let error = super::apply_pending(&conn, &super::MIGRATIONS[..11], 10)
        .expect_err("validation must abort migration");
    assert!(error.contains("Migration 11 failed"), "{error}");
    let version: u32 = conn
        .query_row("SELECT MAX(version) FROM _schema_version", [], |row| {
            row.get(0)
        })
        .expect("version");
    assert_eq!(version, 10);
    let check_id: String = conn
        .query_row("SELECT check_id FROM work_items", [], |row| row.get(0))
        .expect("rolled-back row");
    assert_eq!(check_id, "code_scan.hardcoded-secret:src/a.ts");
}
