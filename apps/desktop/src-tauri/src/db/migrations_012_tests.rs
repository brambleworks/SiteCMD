use rusqlite::Connection;

fn pre_012_conn() -> Connection {
    let conn = Connection::open_in_memory().expect("open db");
    super::ensure_version_table(&conn).expect("version table");
    super::apply_pending(&conn, &super::MIGRATIONS[..11], 0).expect("migrate through 011");
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable foreign keys");
    conn
}

fn seed_history_fixture(conn: &Connection) {
    conn.execute_batch(
        r#"
        INSERT INTO projects (id, name, path, secret_namespace)
        VALUES (1, 'Canonical history', '/tmp/canonical-history', 'canonical-history');
        INSERT INTO environments (id, project_id, url, label, environment)
        VALUES (1, 1, 'https://example.com', 'Production', 'production');
        INSERT INTO sites (id, project_id, url)
        VALUES (1, 1, 'https://example.com');

        -- A post-Slice-1 Full execution must be reused exactly. The migration
        -- must not split it or infer any additional Full pairing.
        INSERT INTO scan_executions (
            id, project_id, environment_id, environment_url,
            environment_scope_key, requested_mode, web_focus, trigger,
            admission_class, status, idempotency_key, request_fingerprint,
            started_at, completed_at, quota_date, quota_state,
            counts_toward_quota, web_status, code_status
        ) VALUES (
            1, 1, 1, 'https://example.com', 'https://example.com', 'full',
            'health', 'manual', 'general_scan', 'complete', 'fixture-full',
            'v1:fixture-full', 1784592000000, 1784592005000, '2026-07-21',
            'consumed', 1, 'complete', 'complete'
        );

        INSERT INTO scans (
            id, site_id, timestamp, mode, scan_type, overall_score,
            security_score, performance_score, seo_score,
            accessibility_score, compliance_score, config_score, polish_score,
            issues_total, issues_critical, issues_high, issues_medium,
            issues_low, issues_passed, duration_ms, detected_stack,
            issue_snapshot_version, execution_id
        ) VALUES (
            1, 1, '2026-07-21T00:00:00Z', 'live', 'health', 82,
            70, 80, 90, 95, 88, 77, 85,
            4, 1, 1, 1, 1, 1, 5000, '{"framework":"nextjs"}', 1, 1
        );

        INSERT INTO scan_issues (
            scan_id, ordinal, check_id, category, title, description,
            check_status, severity, fix_prompt, manual_fix, raw_data,
            confidence, confidence_reason, why_it_matters
        ) VALUES
            (1, 0, 'security.headers.csp', 'security', 'CSP', 'missing',
             'fail', 'critical', 'add CSP', 'manual CSP', '{"header":"csp"}',
             'confirmed', 'response header', 'prevents injection'),
            (1, 1, 'seo.title', 'seo', 'Title', 'present',
             'pass', 'low', NULL, NULL, '{"length":42}',
             'high', NULL, NULL),
            (1, 2, 'performance.lcp', 'performance', 'LCP', 'uncertain',
             'warn', 'high', 'optimize', NULL, '{"milliseconds":2800}',
             'needs_review', 'single sample', 'user experience'),
            (1, 3, 'accessibility.axe.color-contrast', 'accessibility',
             'Contrast', 'not measured', 'skipped', 'medium', NULL, NULL,
             '{"reason":"browser unavailable"}', 'high', NULL, NULL);

        INSERT INTO code_scans (
            id, project_id, environment_url, project_path, checked_at,
            overall_score, framework, issue_count, critical_count, high_count,
            medium_count, low_count, duration_ms, issue_snapshot_version,
            execution_id
        ) VALUES (
            1, 1, 'https://example.com', '/tmp/canonical-history',
            '2026-07-21T00:00:01Z', 76, 'nextjs', 1, 0, 1, 0, 0, 4000, 1, 1
        );
        INSERT INTO code_scan_issues (
            scan_id, ordinal, canonical_check_id, domain, severity, title,
            issue_json
        ) VALUES (
            1, 0, 'code_scan.hardcoded-secret', 'security', 'high', 'Secret',
            '{"id":"hardcoded-secret","checkId":"code_scan.hardcoded-secret","category":"security","severity":"high","title":"Secret","description":"secret found","relativePath":"src/a.ts","absolutePath":"/tmp/canonical-history/src/a.ts","line":17,"sourceExcerpt":"token = ...","evidence":"literal token","whyNow":"credential exposure","likelyFix":"move to env","confidence":"confirmed","confidenceReason":"literal assignment","verifyHint":"rescan"}'
        );

        -- An older multi-page session remains one Web execution with a parent
        -- and page children. It is never paired with the old Code scan below.
        INSERT INTO scan_sessions (
            id, site_id, total_pages, completed_pages, status, started_at,
            completed_at, overall_score, duration_ms, axe_enabled,
            issue_snapshot_version
        ) VALUES (
            2, 1, 2, 2, 'complete', '2026-07-20T00:00:00Z',
            '2026-07-20T00:00:06Z', 88, 6000, 1, 1
        );
        INSERT INTO scans (
            id, site_id, timestamp, mode, scan_type, overall_score,
            issues_total, issues_critical, issues_high, issues_medium,
            issues_low, issues_passed, duration_ms, session_id, page_url,
            issue_snapshot_version
        ) VALUES
            (2, 1, '2026-07-20T00:00:01Z', 'live', 'health', 90,
             1, 0, 0, 1, 0, 2, 2000, 2, 'https://example.com/', 1),
            (3, 1, '2026-07-20T00:00:03Z', 'live', 'health', 86,
             1, 0, 1, 0, 0, 1, 2000, 2, 'https://example.com/pricing', 1);
        INSERT INTO scan_issues (
            scan_id, ordinal, check_id, category, title, description,
            check_status, severity, raw_data, confidence
        ) VALUES
            (2, 0, 'seo.description', 'seo', 'Description', 'short',
             'warn', 'medium', '{"page":"/"}', 'high'),
            (3, 0, 'security.headers.hsts', 'security', 'HSTS', 'missing',
             'fail', 'high', '{"page":"/pricing"}', 'confirmed');
        INSERT INTO session_issues (
            session_id, ordinal, check_id, category, title, description,
            check_status, severity, raw_data, confidence
        ) VALUES (
            2, 0, 'seo.duplicate-title', 'seo', 'Duplicate title',
            'two pages share a title', 'fail', 'medium',
            '{"pages":["/","/pricing"]}', 'confirmed'
        );

        INSERT INTO code_scans (
            id, project_id, environment_url, project_path, checked_at,
            overall_score, issue_count, duration_ms, issue_snapshot_version
        ) VALUES (
            2, 1, 'https://example.com', '/tmp/canonical-history',
            '2026-07-19T00:00:00Z', 100, 0, 1000, 1
        );
        "#,
    )
    .expect("seed canonical history fixture");
}

#[test]
fn migration_012_backfills_lossless_runs_without_fabricating_full() {
    let conn = pre_012_conn();
    seed_history_fixture(&conn);

    super::run_all(&conn).expect("apply canonical history migration");

    let execution_counts: (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*),
                    SUM(requested_mode = 'full'),
                    SUM(requested_mode = 'web'),
                    SUM(requested_mode = 'code')
             FROM scan_executions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("execution counts");
    assert_eq!(execution_counts, (3, 1, 1, 1));

    let run_counts: (i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*),
                    SUM(run_kind = 'single'),
                    SUM(run_kind = 'multi_parent'),
                    SUM(run_kind = 'page'),
                    SUM(run_kind = 'code')
             FROM scan_runs",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("run counts");
    assert_eq!(run_counts, (6, 1, 1, 2, 2));

    let full_children: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM scan_runs WHERE execution_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("full children");
    assert_eq!(full_children, 2);

    let parent_children: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM scan_runs child
             JOIN scan_runs parent ON parent.id = child.parent_run_id
             WHERE parent.run_kind = 'multi_parent' AND child.run_kind = 'page'",
            [],
            |row| row.get(0),
        )
        .expect("parent children");
    assert_eq!(parent_children, 2);

    let verdicts: Vec<String> = conn
        .prepare(
            "SELECT verdict FROM scan_findings
             WHERE run_id = (SELECT id FROM scan_runs
                              WHERE legacy_source = 'web_scan' AND legacy_id = 1)
             ORDER BY ordinal",
        )
        .expect("prepare verdicts")
        .query_map([], |row| row.get(0))
        .expect("query verdicts")
        .collect::<Result<_, _>>()
        .expect("collect verdicts");
    assert_eq!(verdicts, ["fail", "pass", "warn", "skipped"]);

    let web_evidence: String = conn
        .query_row(
            "SELECT raw_data FROM scan_findings
             WHERE producer_check_id = 'security.headers.csp'",
            [],
            |row| row.get(0),
        )
        .expect("web evidence");
    assert_eq!(web_evidence, r#"{"header":"csp"}"#);

    let code: (String, String, String, String, String, i64) = conn
        .query_row(
            "SELECT canonical_check_id, producer_check_id, domain, confidence,
                    relative_path, line
             FROM scan_findings
             WHERE source = 'code_scan' AND relative_path IS NOT NULL",
            [],
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
        .expect("code evidence");
    assert_eq!(
        code,
        (
            "code_scan.hardcoded-secret".into(),
            "hardcoded-secret".into(),
            "security".into(),
            "confirmed".into(),
            "src/a.ts".into(),
            17,
        )
    );

    let finding_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM scan_findings", [], |row| row.get(0))
        .expect("finding count");
    assert_eq!(finding_count, 8);

    let foreign_key_failures: i64 = conn
        .prepare("PRAGMA foreign_key_check")
        .expect("prepare foreign key check")
        .query_map([], |_| Ok(()))
        .expect("foreign key check")
        .count() as i64;
    assert_eq!(foreign_key_failures, 0);
}

#[test]
fn migration_012_marks_history_without_snapshots_as_limited() {
    let conn = pre_012_conn();
    conn.execute_batch(
        "INSERT INTO projects (id, name, secret_namespace) VALUES (1, 'p', 'p');
         INSERT INTO sites (id, project_id, url) VALUES (1, 1, 'https://example.com');
         INSERT INTO scans
            (id, site_id, timestamp, mode, overall_score, duration_ms,
             issue_snapshot_version)
         VALUES (1, 1, '2026-07-01T00:00:00Z', 'live', 90, 10, 0);",
    )
    .expect("seed limited legacy row");

    super::run_all(&conn).expect("apply canonical history migration");
    let state: String = conn
        .query_row("SELECT detail_state FROM scan_runs", [], |row| row.get(0))
        .expect("detail state");
    let findings: i64 = conn
        .query_row("SELECT COUNT(*) FROM scan_findings", [], |row| row.get(0))
        .expect("finding count");
    assert_eq!(state, "limited_legacy");
    assert_eq!(
        findings, 0,
        "mutable work_items must never reconstruct history"
    );
}
