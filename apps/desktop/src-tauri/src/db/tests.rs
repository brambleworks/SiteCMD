//! Smoke tests for the database layer.

#[cfg(test)]
#[allow(clippy::module_inception)] // file is named tests.rs; inner mod wraps 1600+ lines, not worth reindenting
mod tests {
    use crate::checks::Severity;
    use crate::core::scanner::ScheduledScanType;
    use crate::db::test_helpers::temp_db;
    use crate::db::work_items::WorkItemMetadata;
    use crate::db::{Database, IssueLifecycle};
    use rusqlite::Connection;
    use std::path::Path;

    fn temp_db_at(path: std::path::PathBuf) -> Database {
        Database::open(path).expect("open")
    }

    #[test]
    fn execute_times_out_instead_of_blocking_forever_on_a_stalled_op() {
        use std::time::{Duration, Instant};
        let db = temp_db();
        let started = Instant::now();
        let result = db.execute_with_timeout(
            |_conn| {
                std::thread::sleep(Duration::from_secs(2));
                7
            },
            Duration::from_millis(100),
        );
        assert!(
            result.is_err(),
            "a stalled op must time out, got {:?}",
            result
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "caller must return at the ~100ms timeout, not wait out the op"
        );
    }

    #[test]
    fn execute_with_timeout_returns_the_value_when_the_op_is_fast() {
        use std::time::Duration;
        let db = temp_db();
        let value = db
            .execute_with_timeout(|_conn| 42, Duration::from_secs(5))
            .expect("a fast op must return its value within the timeout");
        assert_eq!(value, 42);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_yields_to_the_runtime_while_the_worker_is_busy() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let db = temp_db();
        let ticked = Arc::new(AtomicBool::new(false));
        let ticker = tokio::spawn({
            let ticked = ticked.clone();
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                ticked.store(true, Ordering::SeqCst);
            }
        });

        let value = db
            .run(|_conn| {
                std::thread::sleep(std::time::Duration::from_millis(200));
                7
            })
            .await
            .expect("worker answers");

        assert_eq!(value, 7);
        assert!(
            ticked.load(Ordering::SeqCst),
            "a 20ms timer must fire during a 200ms database operation on a single-threaded runtime; the sync execute would have blocked it"
        );
        ticker.await.expect("ticker task");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_mut_delivers_the_worker_s_value() {
        let db = temp_db();
        let value = db
            .run_mut(|_conn| 9)
            .await
            .expect("worker answers on the mutable path");
        assert_eq!(value, 9);
    }

    #[test]
    fn open_sets_wal_and_normal_synchronous_pragmas() {
        let db = temp_db();
        let journal_mode: String = db
            .execute(|conn| {
                conn.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                    .expect("read journal_mode")
            })
            .expect("db worker");
        let synchronous: i64 = db
            .execute(|conn| {
                conn.query_row("PRAGMA synchronous", [], |row| row.get::<_, i64>(0))
                    .expect("read synchronous")
            })
            .expect("db worker");
        assert_eq!(
            journal_mode.to_lowercase(),
            "wal",
            "journal_mode must be WAL"
        );
        assert_eq!(
            synchronous, 1,
            "synchronous must be NORMAL (1), not FULL (2) - WAL makes NORMAL crash-safe"
        );
        let busy_timeout: i64 = db
            .execute(|conn| {
                conn.query_row("PRAGMA busy_timeout", [], |row| row.get::<_, i64>(0))
                    .expect("read busy_timeout")
            })
            .expect("db worker");
        assert_eq!(busy_timeout, 5000, "busy_timeout must be 5000ms");
    }

    fn seed_incompatible_db(path: &Path) {
        let conn = Connection::open(path).expect("open legacy db");
        conn.execute_batch(
            "CREATE TABLE projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                path TEXT NOT NULL DEFAULT '' UNIQUE,
                framework TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE environments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                url TEXT NOT NULL,
                label TEXT NOT NULL,
                environment TEXT NOT NULL DEFAULT 'production',
                source TEXT,
                last_scanned_at TEXT,
                UNIQUE(project_id, url)
            );
            CREATE TABLE sites (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                label TEXT,
                project_path TEXT,
                framework TEXT,
                sitemap_url TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_scanned_at TEXT,
                UNIQUE(url)
            );
            CREATE TABLE scans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                site_id INTEGER NOT NULL REFERENCES sites(id),
                timestamp TEXT NOT NULL,
                mode TEXT NOT NULL,
                scan_type TEXT NOT NULL DEFAULT 'health',
                overall_score INTEGER NOT NULL,
                security_score INTEGER,
                performance_score INTEGER,
                seo_score INTEGER,
                accessibility_score INTEGER,
                compliance_score INTEGER,
                config_score INTEGER,
                polish_score INTEGER,
                issues_total INTEGER NOT NULL DEFAULT 0,
                issues_critical INTEGER NOT NULL DEFAULT 0,
                issues_high INTEGER NOT NULL DEFAULT 0,
                issues_medium INTEGER NOT NULL DEFAULT 0,
                issues_low INTEGER NOT NULL DEFAULT 0,
                issues_passed INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL,
                detected_stack TEXT,
                session_id INTEGER,
                page_url TEXT
            );
            CREATE TABLE integration_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL,
                integration_type TEXT NOT NULL,
                api_key TEXT,
                site_id TEXT,
                extra TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                UNIQUE(project_id, integration_type)
            );
            CREATE TABLE issue_links (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL,
                check_id TEXT NOT NULL,
                scan_id INTEGER NOT NULL,
                provider TEXT NOT NULL,
                external_id TEXT NOT NULL,
                external_url TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'open',
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                FOREIGN KEY (project_id) REFERENCES projects(id),
                FOREIGN KEY (scan_id) REFERENCES scans(id)
            );
            CREATE TABLE project_work_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                stable_key TEXT NOT NULL UNIQUE,
                project_id INTEGER NOT NULL,
                environment_url TEXT NOT NULL DEFAULT '',
                kind TEXT NOT NULL,
                status TEXT NOT NULL,
                severity TEXT,
                title TEXT NOT NULL,
                summary TEXT NOT NULL,
                category TEXT,
                domain TEXT,
                package_name TEXT,
                target_json TEXT NOT NULL,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                last_verified_at TEXT,
                last_status_changed_at TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
            );
            CREATE TABLE _schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            INSERT INTO projects (name, path, framework, created_at)
            VALUES ('Legacy Import', '__url__https://legacy.example', 'nextjs', '2026-01-01T00:00:00Z');",
        )
        .expect("seed legacy schema");
        conn.execute(
            "INSERT INTO _schema_version (version) VALUES (?1)",
            [crate::db::migrations::latest_version() + 1],
        )
        .expect("mark schema as incompatible");
    }

    #[test]
    fn open_and_init_tables() {
        let db = temp_db();
        assert!(!db.path().is_empty());
    }

    #[test]
    fn migration_018_adds_page_url_column() {
        let db = temp_db();
        db.execute(|conn| {
            conn.query_row("SELECT page_url FROM work_items LIMIT 1", [], |_row| Ok(()))
                .or_else(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Ok(()),
                    other => Err(format!("unexpected error: {other}")),
                })
        })
        .unwrap()
        .unwrap();
    }

    #[test]
    fn migration_018_creates_dismissed_hints_table() {
        let db = temp_db();
        db.execute(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM dismissed_integration_hints",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .unwrap()
        .unwrap();
    }

    #[test]
    fn insert_and_get_project() {
        let db = temp_db();
        let id = db
            .upsert_project("Test Site", "/tmp/test", Some("react"))
            .expect("upsert");
        assert!(id > 0);

        let projects = db.get_projects().expect("get");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Test Site");
        assert_eq!(projects[0].framework.as_deref(), Some("react"));
        assert!(!projects[0].secret_namespace.is_empty());
    }

    #[test]
    fn get_projects_dedupes_loopback_environment_aliases() {
        let db = temp_db();
        let id = db
            .upsert_project("Local Site", "/tmp/local-site", Some("astro"))
            .expect("upsert");
        db.add_environment(
            id,
            "http://127.0.0.1:4321/",
            "Local Site (production)",
            "production",
            "detected",
        )
        .expect("add detected env");
        db.add_environment(
            id,
            "http://localhost:4321/",
            "Local Site (local)",
            "local",
            "manual",
        )
        .expect("add manual env");

        let projects = db.get_projects().expect("get");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].environments.len(), 1);
        // add_environment stores the normalized form: trailing slash stripped.
        assert_eq!(projects[0].environments[0].url, "http://localhost:4321");
        assert_eq!(projects[0].environments[0].environment, "local");
    }

    #[test]
    fn add_environment_stores_normalized_url_and_resolves_project() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Mixed Case", "/tmp/mixed-case", None)
            .expect("upsert");
        db.add_environment(
            project_id,
            "https://MySite.com/",
            "Production",
            "production",
            "manual",
        )
        .expect("add env");

        let projects = db.get_projects().expect("get");
        assert_eq!(projects[0].environments.len(), 1);
        assert_eq!(projects[0].environments[0].url, "https://mysite.com");

        // The whole point of write-boundary normalization: a scan of the
        // canonical URL resolves back to the project.
        assert_eq!(
            db.find_project_for_url("https://mysite.com"),
            Some(project_id)
        );
        // The lookup normalizes its input too, so the verbatim spelling also
        // resolves.
        assert_eq!(
            db.find_project_for_url("https://MySite.com/"),
            Some(project_id)
        );
    }

    #[test]
    fn project_env_listings_order_production_first() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Ordered", "/tmp/ordered", None)
            .expect("upsert");
        // Local env registered first: without the ORDER BY it would win.
        db.add_environment(
            project_id,
            "http://localhost:3000",
            "Local",
            "local",
            "manual",
        )
        .expect("add local env");
        db.add_environment(
            project_id,
            "https://ordered.example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("add production env");

        let all = db.list_all_project_envs().expect("list all");
        assert_eq!(
            all,
            vec![
                (project_id, "https://ordered.example.com".to_string()),
                (project_id, "http://localhost:3000".to_string()),
            ]
        );

        let one = db.list_project_envs(project_id).expect("list one");
        assert_eq!(
            one,
            vec![
                "https://ordered.example.com".to_string(),
                "http://localhost:3000".to_string(),
            ]
        );
    }

    #[test]
    fn add_environment_upserts_existing_row_across_url_spellings() {
        // Re-registering the same URL with different host case or a trailing
        // slash must update the existing row, not insert a duplicate.
        let db = temp_db();
        let project_id = db
            .upsert_project("Same Site", "/tmp/same-site", None)
            .expect("upsert");
        let first = db
            .add_environment(
                project_id,
                "https://example.com",
                "Production",
                "production",
                "manual",
            )
            .expect("add env");
        let second = db
            .add_environment(
                project_id,
                "https://Example.com/",
                "Prod",
                "production",
                "detected",
            )
            .expect("re-add env");
        assert_eq!(first, second);

        let projects = db.get_projects().expect("get");
        assert_eq!(projects[0].environments.len(), 1);
        assert_eq!(projects[0].environments[0].label, "Prod");
    }

    #[test]
    fn find_project_for_url_shared_url_resolves_to_oldest_registration() {
        let db = temp_db();
        let first = db
            .upsert_project("First", "/tmp/first", None)
            .expect("upsert first");
        let second = db
            .upsert_project("Second", "/tmp/second", None)
            .expect("upsert second");
        db.add_environment(
            first,
            "https://shared.example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("env on first");
        db.add_environment(
            second,
            "https://shared.example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("env on second");

        // Deterministic pick: the oldest environment row (lowest id) wins,
        // not whichever row SQLite visits first.
        assert_eq!(
            db.find_project_for_url("https://shared.example.com"),
            Some(first)
        );
    }

    #[test]
    fn strict_project_url_lookup_distinguishes_storage_failure_from_no_project() {
        let db = temp_db();
        assert_eq!(
            db.find_project_for_url_result("https://unlinked.example.com")
                .expect("healthy lookup"),
            None
        );
        db.execute(|conn| conn.execute("DROP TABLE environments", []).map(|_| ()))
            .expect("database worker")
            .expect("drop environments");

        let error = db
            .find_project_for_url_result("https://unlinked.example.com")
            .expect_err("storage failure must not look like an unlinked scan");
        assert!(error.to_string().contains("environments"));
    }

    #[test]
    fn restore_from_backup_replaces_live_database_contents() {
        let target = temp_db();
        target
            .upsert_project("Before Restore", "/tmp/before", Some("react"))
            .expect("seed target project");

        let source_dir = tempfile::tempdir().expect("tempdir");
        let source_path = source_dir.path().join("backup.db");
        let source = temp_db_at(source_path.clone());
        source
            .upsert_project("After Restore", "/tmp/after", Some("nextjs"))
            .expect("seed source project");
        drop(source);

        target
            .restore_from_backup(source_path)
            .expect("restore from backup");

        let projects = target.get_projects().expect("get projects after restore");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "After Restore");
        assert_eq!(projects[0].framework.as_deref(), Some("nextjs"));
    }

    #[test]
    fn open_moves_pre_squash_db_aside_and_starts_fresh() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("legacy-incompatible.db");
        seed_incompatible_db(&path);

        let db = Database::open(path.clone()).expect("open must recover by starting fresh");
        let projects = db.get_projects().expect("projects on fresh db");
        assert!(
            projects.is_empty(),
            "fresh database must not carry legacy rows"
        );

        let schema_version: u32 = db
            .execute(|conn| {
                conn.query_row(
                    "SELECT COALESCE(MAX(version), 0) FROM _schema_version",
                    [],
                    |row| row.get(0),
                )
                .expect("schema version")
            })
            .expect("db worker");
        assert_eq!(schema_version, crate::db::migrations::latest_version());

        let backup = path.with_extension("db.pre-squash.bak");
        assert!(
            backup.exists(),
            "incompatible database must be preserved as {}",
            backup.display()
        );
    }

    #[test]
    fn restore_from_backup_rejects_pre_squash_backup_without_touching_live_data() {
        let target = temp_db();
        target
            .upsert_project("Before Restore", "/tmp/before", Some("react"))
            .expect("seed target project");

        let source_dir = tempfile::tempdir().expect("tempdir");
        let source_path = source_dir.path().join("legacy-incompatible.db");
        seed_incompatible_db(&source_path);

        let err = target
            .restore_from_backup(source_path)
            .expect_err("pre-squash backup must be rejected");
        assert!(
            err.contains("incompatible-schema:"),
            "error must explain the incompatibility, got: {err}"
        );

        // The rejection happened before conn.restore, so live data survives.
        let projects = target.get_projects().expect("projects after rejection");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Before Restore");
    }

    #[test]
    fn restore_from_backup_rejects_backup_with_unreadable_schema_version() {
        let target = temp_db();
        target
            .upsert_project("Before Restore", "/tmp/before", Some("react"))
            .expect("seed target project");

        let source_dir = tempfile::tempdir().expect("tempdir");
        let source_path = source_dir.path().join("no-schema-version.db");
        {
            let conn = rusqlite::Connection::open(&source_path).expect("open source");
            conn.execute_batch("CREATE TABLE unrelated (id INTEGER);")
                .expect("create dummy table");
        }

        let err = target
            .restore_from_backup(source_path)
            .expect_err("backup without a readable schema version must be rejected");
        assert!(
            err.contains("incompatible-schema:"),
            "error must explain the unverifiable schema, got: {err}"
        );

        // The rejection happened before conn.restore, so live data survives.
        let projects = target.get_projects().expect("projects after rejection");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Before Restore");
    }

    #[test]
    fn insert_and_get_scan() {
        let db = temp_db();
        let site_id = db.get_or_create_site("https://example.com").expect("site");

        let result = crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: "https://example.com".to_string(),
            mode: "full".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 85,
            categories: vec![],
            issues: vec![],
            detected_stack: None,
            duration_ms: 1234,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };

        let scan_id = db.save_scan(site_id, &result).expect("save_scan");
        assert!(scan_id > 0);

        let history = db
            .get_scan_history("https://example.com", 10)
            .expect("history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].overall_score, 85);
    }

    #[test]
    fn scan_history_preserves_medium_and_low_issue_counts() {
        let db = temp_db();
        let site_id = db.get_or_create_site("https://example.com").expect("site");

        let result = crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: "https://example.com".to_string(),
            mode: "full".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 72,
            categories: vec![],
            issues: vec![
                crate::checks::CheckResult {
                    check_id: "seo-title".to_string(),
                    category: crate::checks::ScanCategory::Seo,
                    title: "Missing title".to_string(),
                    description: "Title tag is missing.".to_string(),
                    status: crate::checks::CheckStatus::Fail,
                    severity: crate::checks::Severity::Medium,
                    fix_prompt: None,
                    manual_fix: None,
                    raw_data: None,
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: None,
                },
                crate::checks::CheckResult {
                    check_id: "perf-cache".to_string(),
                    category: crate::checks::ScanCategory::Performance,
                    title: "Weak cache headers".to_string(),
                    description: "Responses are not cached.".to_string(),
                    status: crate::checks::CheckStatus::Fail,
                    severity: crate::checks::Severity::Low,
                    fix_prompt: None,
                    manual_fix: None,
                    raw_data: None,
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: None,
                },
            ],
            detected_stack: None,
            duration_ms: 980,
            timestamp: "2026-04-10T00:00:00Z".to_string(),
        };

        db.save_scan(site_id, &result).expect("save_scan");

        let history = db
            .get_scan_history("https://example.com", 10)
            .expect("history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].issues_medium, 1);
        assert_eq!(history[0].issues_low, 1);
    }

    #[test]
    fn score_trend_returns_latest_points_in_ascending_order() {
        let db = temp_db();
        let site_id = db.get_or_create_site("https://example.com").expect("site");

        for (score, timestamp) in [
            (70, "2026-01-01T00:00:00Z"),
            (80, "2026-01-02T00:00:00Z"),
            (90, "2026-01-03T00:00:00Z"),
        ] {
            let result = crate::core::scanner::ScanResult {
                page_signals: None,
                site_facts: None,
                url: "https://example.com".to_string(),
                mode: "full".to_string(),
                scan_type: crate::core::scanner::ScanType::Health,
                overall_score: score,
                categories: vec![],
                issues: vec![],
                detected_stack: None,
                duration_ms: 1000,
                timestamp: timestamp.to_string(),
            };

            db.save_scan(site_id, &result).expect("save_scan");
        }

        let trend = db.get_score_trend("https://example.com", 2).expect("trend");

        assert_eq!(trend.len(), 2);
        assert_eq!(trend[0].overall, 80);
        assert_eq!(trend[1].overall, 90);
        assert_eq!(trend[0].timestamp, "2026-01-02T00:00:00Z");
        assert_eq!(trend[1].timestamp, "2026-01-03T00:00:00Z");
    }

    #[test]
    fn execution_retention_clamps_zero_to_keep_one_scan() {
        let db = temp_db();
        let site_id = db.get_or_create_site("https://example.com").expect("site");

        for day in 1..=3 {
            let result = crate::core::scanner::ScanResult {
                page_signals: None,
                site_facts: None,
                url: "https://example.com".to_string(),
                mode: "full".to_string(),
                scan_type: crate::core::scanner::ScanType::Health,
                overall_score: 80 + day,
                categories: vec![],
                issues: vec![],
                detected_stack: None,
                duration_ms: 1000,
                timestamp: format!("2026-01-0{}T00:00:00Z", day),
            };
            db.save_scan(site_id, &result).expect("save_scan");
        }

        let pruned = db
            .prune_scan_executions_for_scope(
                None,
                "https://example.com",
                0,
                crate::db::ScanRetentionWindow::All,
            )
            .expect("prune");
        let trend = db
            .get_score_trend("https://example.com", 10)
            .expect("trend");

        assert_eq!(pruned, 2);
        assert_eq!(trend.len(), 1);
        assert_eq!(trend[0].timestamp, "2026-01-03T00:00:00Z");
    }

    #[test]
    fn execution_retention_keeps_a_retained_multi_page_session_atomic() {
        let db = temp_db();
        let site_id = db.get_or_create_site("https://example.com").expect("site");
        let session_id = db.create_scan_session(site_id, 68, false).expect("session");

        for page in 0..68 {
            let result = crate::core::scanner::ScanResult {
                page_signals: None,
                site_facts: None,
                url: format!("https://example.com/page-{page}"),
                mode: "full".to_string(),
                scan_type: crate::core::scanner::ScanType::Health,
                overall_score: 80,
                categories: vec![],
                issues: vec![],
                detected_stack: None,
                duration_ms: 100,
                timestamp: "2026-01-03T00:00:00Z".to_string(),
            };
            db.save_scan_with_session(site_id, session_id, &result.url, &result)
                .expect("save session page");
        }

        let pruned = db
            .prune_scan_executions_for_scope(
                None,
                "https://example.com",
                50,
                crate::db::ScanRetentionWindow::All,
            )
            .expect("prune");
        let retained_pages: i64 = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM scan_runs WHERE parent_run_id = ?1",
                    rusqlite::params![session_id],
                    |row| row.get(0),
                )
            })
            .expect("database worker")
            .expect("count retained pages");

        assert_eq!(pruned, 0);
        assert_eq!(retained_pages, 68);
    }

    #[test]
    fn score_trend_preserves_scan_type_per_point() {
        // Dashboard trend filtering (same-type comparisons) relies on each point
        // carrying its own scan_type. Regression test for the 2-peer restructure.
        let db = temp_db();
        let site_id = db.get_or_create_site("https://example.com").expect("site");

        for (scan_type, timestamp, score) in [
            ("health", "2026-02-01T00:00:00Z", 85u32),
            ("security", "2026-02-02T00:00:00Z", 72),
            ("accessibility", "2026-02-03T00:00:00Z", 91),
            ("health", "2026-02-04T00:00:00Z", 88),
        ] {
            let result = crate::core::scanner::ScanResult {
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
                timestamp: timestamp.to_string(),
            };
            db.save_scan(site_id, &result).expect("save_scan");
        }

        let trend = db
            .get_score_trend("https://example.com", 10)
            .expect("trend");

        assert_eq!(trend.len(), 4);
        assert_eq!(trend[0].scan_type.as_str(), "health");
        assert_eq!(trend[1].scan_type.as_str(), "security");
        assert_eq!(trend[2].scan_type.as_str(), "accessibility");
        assert_eq!(trend[3].scan_type.as_str(), "health");
        assert_eq!(trend[0].overall, 85);
        assert_eq!(trend[3].overall, 88);
    }

    #[test]
    fn session_issues_include_site_wide_findings_without_pages() {
        let db = temp_db();
        let site_id = db.get_or_create_site("https://example.com").expect("site");

        let session_id = db.create_scan_session(site_id, 2, false).expect("session");
        let page_issue = crate::checks::CheckResult {
            check_id: "seo.meta_description".to_string(),
            category: crate::checks::ScanCategory::Seo,
            title: "Missing meta description".to_string(),
            description: "No meta description was found in the document head.".to_string(),
            status: crate::checks::CheckStatus::Warn,
            severity: Severity::Medium,
            fix_prompt: Some("Add a concise, page-specific meta description.".to_string()),
            manual_fix: Some("Write a page-specific description.".to_string()),
            raw_data: Some(serde_json::json!({ "selector": "head" })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some(
                "Search engines can generate a snippet from visible page text.".to_string(),
            ),
            why_it_matters: Some(
                "A useful description can improve how the page is presented in search results."
                    .to_string(),
            ),
        };
        let scan_result = crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: "https://example.com".to_string(),
            mode: "full".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 90,
            categories: vec![],
            issues: vec![page_issue],
            detected_stack: None,
            duration_ms: 100,
            timestamp: "2026-07-04T00:00:00Z".to_string(),
        };
        db.save_scan_with_session(site_id, session_id, "https://example.com/", &scan_result)
            .expect("entry scan");
        db.save_scan_with_session(
            site_id,
            session_id,
            "https://example.com/about",
            &scan_result,
        )
        .expect("second scan");

        let site_issue = crate::checks::CheckResult {
            check_id: "seo.duplicate_title_across_pages".to_string(),
            category: crate::checks::ScanCategory::Seo,
            title: "Duplicate titles across pages".to_string(),
            description: "2 scanned pages use the same non-empty title.".to_string(),
            status: crate::checks::CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: Some("Give each affected page a distinct title.".to_string()),
            manual_fix: Some("Edit each page's title element.".to_string()),
            raw_data: Some(serde_json::json!({ "duplicate_count": 2 })),
            confidence: crate::checks::IssueConfidence::Confirmed,
            confidence_reason: Some(
                "The duplicate was observed directly across two scanned documents.".to_string(),
            ),
            why_it_matters: Some(
                "Distinct titles help people and search systems tell pages apart.".to_string(),
            ),
        };
        db.save_session_issue_snapshot(session_id, &[site_issue])
            .expect("site-wide snapshot");

        let issues = db.get_session_issues(session_id).expect("session issues");

        let site_wide = issues
            .iter()
            .find(|i| i.check_id == "seo.duplicate_title_across_pages")
            .expect("site-wide finding present in session issues");
        // Empty pages renders as "All pages" in the session results view.
        assert!(site_wide.pages.is_empty());
        assert_eq!(site_wide.page_count, 0);
        assert_eq!(site_wide.severity, Severity::High);
        assert_eq!(site_wide.status, crate::checks::CheckStatus::Fail);
        assert_eq!(
            site_wide.confidence,
            crate::checks::IssueConfidence::Confirmed
        );
        assert_eq!(
            site_wide.confidence_reason.as_deref(),
            Some("The duplicate was observed directly across two scanned documents.")
        );
        assert_eq!(
            site_wide.fix_prompt.as_deref(),
            Some("Give each affected page a distinct title.")
        );
        assert_eq!(site_wide.instances.len(), 1);
        assert_eq!(site_wide.instances[0].page_url, None);
        assert_eq!(
            site_wide.instances[0].raw_data,
            Some(serde_json::json!({ "duplicate_count": 2 }))
        );

        let per_page = issues
            .iter()
            .find(|i| i.check_id == "seo.meta_description")
            .expect("per-page finding present");
        assert_eq!(per_page.page_count, 2);
        assert_eq!(per_page.status, crate::checks::CheckStatus::Warn);
        assert_eq!(
            per_page.manual_fix.as_deref(),
            Some("Write a page-specific description.")
        );
        assert_eq!(
            per_page.confidence_reason.as_deref(),
            Some("Search engines can generate a snippet from visible page text.")
        );
        assert_eq!(
            per_page.why_it_matters.as_deref(),
            Some("A useful description can improve how the page is presented in search results.")
        );
        assert_eq!(per_page.instances.len(), 2);
        assert!(per_page
            .instances
            .iter()
            .all(|instance| instance.raw_data == Some(serde_json::json!({ "selector": "head" }))));
    }

    #[test]
    fn clean_analyzed_session_does_not_fall_back_to_mutable_site_work_items() {
        let db = temp_db();
        let site_id = db.get_or_create_site("https://example.com").expect("site");
        let project_id = db
            .upsert_project("Site Test", "/tmp/site-test", None)
            .expect("project");
        let session_id = db.create_scan_session(site_id, 1, false).expect("session");
        let scan_result = crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: "https://example.com".to_string(),
            mode: "full".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 100,
            categories: vec![],
            issues: vec![],
            detected_stack: None,
            duration_ms: 100,
            timestamp: "2026-07-04T00:00:00Z".to_string(),
        };
        let scan_id = db
            .save_scan_with_session(site_id, session_id, "https://example.com/", &scan_result)
            .expect("scan");

        let stale_item = crate::db::work_items::WorkItemInput {
            project_id,
            env_url: "https://example.com".to_string(),
            source: "site_scan".to_string(),
            signal_id: "site_scan:seo.duplicate_title_across_pages:https://example.com".to_string(),
            check_id: "seo.duplicate_title_across_pages".to_string(),
            category: "seo".to_string(),
            severity: Severity::High,
            title: "Stale mutable finding".to_string(),
            description: "This belongs to lifecycle state, not the clean snapshot.".to_string(),
            detail_json: None,
            scan_ref: Some(scan_id),
            page_url: None,
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            observed_at: 1_000_000,
            metadata: WorkItemMetadata::default(),
        };
        db.upsert_work_items_diff(
            "site_scan",
            project_id,
            "https://example.com",
            vec![stale_item],
            1_000_000,
        )
        .expect("stale lifecycle item");

        db.save_session_issue_snapshot(session_id, &[])
            .expect("mark analyzed clean session");

        let issues = db.get_session_issues(session_id).expect("session issues");
        assert!(
            issues.is_empty(),
            "an explicit empty snapshot must not resurrect mutable work items: {issues:?}"
        );
    }

    #[test]
    fn insert_and_get_code_scan() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Code Test", "/tmp/code-test", Some("nextjs"))
            .expect("upsert");

        let issue = crate::core::code_scan::CodeIssue {
            check_id: String::new(),
            id: "ai-timeout".to_string(),
            category: "ai-safety".to_string(),
            severity: crate::checks::Severity::High,
            title: "Missing timeout".to_string(),
            description: "AI route has no timeout.".to_string(),
            relative_path: "app/api/chat/route.ts".to_string(),
            absolute_path: "/tmp/code-test/app/api/chat/route.ts".to_string(),
            line: Some(42),
            source_excerpt: Some("await client.responses.create({ input })".to_string()),
            evidence: Some("No timeout or abort signal was found on this request.".to_string()),
            why_now: Some("A stalled upstream request can exhaust concurrent capacity.".to_string()),
            likely_fix: Some("Pass an AbortSignal with a bounded timeout.".to_string()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: Some(
                "The route contains the request call, but framework middleware may impose a timeout."
                    .to_string(),
            ),
            verify_hint: Some("Run the route against a deliberately slow upstream.".to_string()),
        };
        let report = crate::core::code_scan::CodeScanReport {
            skipped_scopes: Default::default(),
            checked_at: "2026-04-09T12:00:00Z".to_string(),
            framework: Some("Next.js".to_string()),
            issue_count: 1,
            critical_count: 0,
            high_count: 1,
            medium_count: 0,
            low_count: 0,
            issues: vec![issue.clone()],
        };

        let scan_id = db
            .save_code_scan(
                project_id,
                Some("https://example.com".to_string()),
                "/tmp/code-test".to_string(),
                &report,
                250,
            )
            .expect("save_code_scan");

        let history = db
            .get_code_scan_history(project_id, 10)
            .expect("code history");
        assert_eq!(history.len(), 1);
        assert_eq!(
            history[0].top_domain,
            Some(crate::core::code_scan::CodeScanDomain::AiSafety)
        );
        assert_eq!(history[0].top_domain_count, 1);
        assert_eq!(history[0].domain_summaries.len(), 1);
        assert_eq!(
            history[0].domain_summaries[0].domain.to_string(),
            "ai-safety"
        );
        assert_eq!(history[0].domain_summaries[0].issue_count, 1);

        let detail = db
            .get_code_scan_detail(scan_id)
            .expect("code detail")
            .expect("code scan exists");
        assert_eq!(detail.issue_count, 1);
        assert_eq!(detail.issues[0].relative_path, "app/api/chat/route.ts");
        assert_eq!(detail.issues[0].domain.to_string(), "ai-safety");
        assert_eq!(
            detail.issues[0].evidence.as_deref(),
            Some("No timeout or abort signal was found on this request.")
        );
        assert_eq!(
            detail.issues[0].confidence_reason.as_deref(),
            Some(
                "The route contains the request call, but framework middleware may impose a timeout."
            )
        );
        assert_eq!(
            detail.issues[0].verify_hint.as_deref(),
            Some("Run the route against a deliberately slow upstream.")
        );
        assert_eq!(detail.domain_summaries.len(), 1);
        assert_eq!(detail.domain_summaries[0].domain.to_string(), "ai-safety");
        assert_eq!(detail.domain_summaries[0].issue_count, 1);

        let mut changed_issue = issue;
        changed_issue.title = "Changed finding from a later scan".to_string();
        changed_issue.severity = Severity::Low;
        let changed_report = crate::core::code_scan::CodeScanReport {
            skipped_scopes: Default::default(),
            checked_at: "2026-04-10T12:00:00Z".to_string(),
            framework: Some("Next.js".to_string()),
            issue_count: 1,
            critical_count: 0,
            high_count: 0,
            medium_count: 0,
            low_count: 1,
            issues: vec![changed_issue],
        };
        db.save_code_scan(
            project_id,
            Some("https://example.com".to_string()),
            "/tmp/code-test".to_string(),
            &changed_report,
            200,
        )
        .expect("later code scan");

        let original = db
            .get_code_scan_detail(scan_id)
            .expect("original detail after later scan")
            .expect("original scan still exists");
        assert_eq!(original.issues[0].title, "Missing timeout");
        assert_eq!(original.issues[0].severity, Severity::High);
    }

    #[test]
    fn insert_and_get_code_scan_overview() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Code Test", "/tmp/code-test", Some("nextjs"))
            .expect("upsert");

        let issue_db = crate::core::code_scan::CodeIssue {
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
        };
        let issue_ai = crate::core::code_scan::CodeIssue {
            check_id: String::new(),
            id: "ai-timeout".to_string(),
            category: "ai-safety".to_string(),
            severity: crate::checks::Severity::High,
            title: "Missing timeout".to_string(),
            description: "AI route has no timeout.".to_string(),
            relative_path: "app/api/chat/route.ts".to_string(),
            absolute_path: "/tmp/code-test/app/api/chat/route.ts".to_string(),
            line: Some(42),
            source_excerpt: None,
            evidence: None,
            why_now: None,
            likely_fix: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        };
        let report = crate::core::code_scan::CodeScanReport {
            skipped_scopes: Default::default(),
            checked_at: "2026-04-09T12:00:00Z".to_string(),
            framework: Some("Next.js".to_string()),
            issue_count: 2,
            critical_count: 1,
            high_count: 1,
            medium_count: 0,
            low_count: 0,
            issues: vec![issue_db.clone(), issue_ai.clone()],
        };

        let scan_id = db
            .save_code_scan(
                project_id,
                Some("https://example.com".to_string()),
                "/tmp/code-test".to_string(),
                &report,
                250,
            )
            .expect("save_code_scan");

        let overview = db
            .get_code_scan_overview(scan_id)
            .expect("code overview")
            .expect("code scan exists");
        assert_eq!(overview.issue_count, 2);
        assert!(overview.issues.is_empty());
        assert_eq!(overview.domain_summaries.len(), 2);
        assert_eq!(overview.domain_summaries[0].domain.to_string(), "database");
        assert_eq!(overview.domain_summaries[0].critical_count, 1);
    }

    #[test]
    fn save_and_invalidate_project_signal_snapshot() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Signal Test", "/tmp/signal-test", Some("nextjs"))
            .expect("upsert");

        let monitoring = crate::db::ProjectMonitoringSignals {
            enabled_integrations: vec!["plausible".to_string(), "googlesearchconsole".to_string()],
            integration_failure_count: 1,
            stale_integration_count: 0,
            search_regression: Some(crate::db::SearchRegressionSignal {
                source: "Google Search Console".to_string(),
                delta_pct: -31,
                focus: Some("seo.noindex".to_string()),
                item_id: Some("seo.noindex".to_string()),
            }),
        };
        let updates = crate::updates::types::UpdateReport {
            packages: Vec::new(),
            updates: Vec::new(),
            ecosystems_detected: vec![crate::updates::types::Ecosystem::Npm],
            scan_duration_ms: 42,
        };

        db.save_project_monitoring_snapshot(
            project_id,
            Some("https://example.com/"),
            &monitoring,
            "2026-04-10T12:00:00Z",
        )
        .expect("save monitoring");
        db.save_project_updates_snapshot(
            project_id,
            Some("https://example.com"),
            &updates,
            "2026-04-10T12:01:00Z",
        )
        .expect("save updates");

        let snapshot = db
            .get_project_signal_snapshot_record(project_id, Some("https://example.com/"))
            .expect("load snapshot")
            .expect("snapshot exists");
        assert_eq!(
            snapshot.environment_url.as_deref(),
            Some("https://example.com")
        );
        assert!(snapshot.monitoring_json.is_some());
        assert!(snapshot.updates_json.is_some());

        db.invalidate_project_signal_snapshots(project_id, Some("https://example.com"))
            .expect("invalidate");
        let cleared = db
            .get_project_signal_snapshot_record(project_id, Some("https://example.com"))
            .expect("reload snapshot");
        assert!(cleared.is_none());
    }

    #[test]
    fn dismiss_first_scan_banner_persists() {
        let db = temp_db();
        let project_id = db
            .upsert_project("UI State Test", "/tmp/ui-state-test", Some("nextjs"))
            .expect("upsert");

        assert!(!db
            .is_first_scan_banner_dismissed(project_id)
            .expect("load initial ui state"));

        db.dismiss_first_scan_banner(project_id)
            .expect("dismiss banner");

        assert!(db
            .is_first_scan_banner_dismissed(project_id)
            .expect("load dismissed ui state"));
    }

    #[test]
    fn insert_and_get_event() {
        use crate::db::{EventSeverity, EventSource, EventType, SiteEvent};

        let db = temp_db();
        let project_id = db
            .upsert_project("Event Test", "/tmp/evt", None)
            .expect("upsert");

        let event = SiteEvent {
            id: 0,
            project_id,
            event_type: EventType::Scan,
            severity: EventSeverity::Info,
            occurred_at_ms: 1735732800000, // 2025-01-01T12:00:00Z
            title: "Test scan".to_string(),
            summary: "Score 90".to_string(),
            detail: None,
            source: EventSource::Internal,
            source_id: Some("test_1".to_string()),
            metadata: None,
            affected_check_ids: None,
        };

        let event_id = db.insert_event(&event).expect("insert_event");
        assert!(event_id > 0);

        let events = db
            .get_events(project_id, 0, i64::MAX, None, None, None, None)
            .expect("get_events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Test scan");
    }

    #[test]
    fn get_events_since_cursor_includes_same_timestamp_newer_ids() {
        use crate::db::{EventSeverity, EventSource, EventType, SiteEvent};

        let db = temp_db();
        let project_id = db
            .upsert_project("Cursor Test", "/tmp/cursor", None)
            .expect("upsert");

        let occurred_at_ms: i64 = 1735732800000; // 2025-01-01T12:00:00Z
        let first_id = db
            .insert_event(&SiteEvent {
                id: 0,
                project_id,
                event_type: EventType::Scan,
                severity: EventSeverity::Info,
                occurred_at_ms,
                title: "First".to_string(),
                summary: "".to_string(),
                detail: None,
                source: EventSource::Internal,
                source_id: Some("cursor_1".to_string()),
                metadata: None,
                affected_check_ids: None,
            })
            .expect("insert first");

        let second_id = db
            .insert_event(&SiteEvent {
                id: 0,
                project_id,
                event_type: EventType::Verification,
                severity: EventSeverity::Warning,
                occurred_at_ms,
                title: "Second".to_string(),
                summary: "".to_string(),
                detail: None,
                source: EventSource::Internal,
                source_id: Some("cursor_2".to_string()),
                metadata: None,
                affected_check_ids: None,
            })
            .expect("insert second");

        let events = db
            .get_events(
                project_id,
                0,
                i64::MAX,
                None,
                Some(occurred_at_ms),
                Some(first_id),
                None,
            )
            .expect("get cursor events");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, second_id);
        assert_eq!(events[0].title, "Second");
    }

    #[test]
    fn duplicate_event_ignored() {
        use crate::db::{EventSeverity, EventSource, EventType, SiteEvent};

        let db = temp_db();
        let project_id = db
            .upsert_project("Dup Test", "/tmp/dup", None)
            .expect("upsert");

        let event = SiteEvent {
            id: 0,
            project_id,
            event_type: EventType::Deploy,
            severity: EventSeverity::Info,
            occurred_at_ms: 1749981600000, // 2025-06-15T10:00:00Z
            title: "Deploy v1".to_string(),
            summary: "".to_string(),
            detail: None,
            source: EventSource::Git,
            source_id: Some("abc123".to_string()),
            metadata: None,
            affected_check_ids: None,
        };

        db.insert_event(&event).expect("first");
        db.insert_event(&event).expect("second (ignored)");

        let events = db
            .get_events(project_id, 0, i64::MAX, None, None, None, None)
            .expect("get");
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn duplicate_event_with_check_ids_does_not_mislink_prior_event() {
        use crate::db::{EventSeverity, EventSource, EventType, SiteEvent};

        let db = temp_db();
        let project_id = db
            .upsert_project("Mislink Test", "/tmp/mislink", None)
            .expect("upsert");

        let make = |source_id: &str, title: &str, checks: Vec<String>| SiteEvent {
            id: 0,
            project_id,
            event_type: EventType::Scan,
            severity: EventSeverity::Info,
            occurred_at_ms: 1740823200000, // 2025-03-01T10:00:00Z
            title: title.to_string(),
            summary: String::new(),
            detail: None,
            source: EventSource::Internal,
            source_id: Some(source_id.to_string()),
            metadata: None,
            affected_check_ids: Some(checks),
        };

        // Event A owns check.a; event B lands next and now holds the
        // connection's last_insert_rowid.
        let a_id = db
            .insert_event(&make("evt_a", "A", vec!["check.a".to_string()]))
            .expect("insert A");
        let b_id = db
            .insert_event(&make("evt_b", "B", vec!["check.b".to_string()]))
            .expect("insert B");

        let dup_id = db
            .insert_event(&make("evt_a", "A dup", vec!["check.a".to_string()]))
            .expect("duplicate A ignored");
        assert_eq!(dup_id, 0, "ignored duplicate must report no new row");

        let conn = Connection::open(db.path()).expect("open");
        let checks_for = |event_id: i64| -> Vec<String> {
            let mut stmt = conn
                .prepare(
                    "SELECT check_id FROM site_event_check_ids WHERE event_id = ?1 ORDER BY check_id",
                )
                .expect("prepare");
            let rows = stmt
                .query_map([event_id], |r| r.get::<_, String>(0))
                .expect("query");
            rows.map(|r| r.expect("row")).collect()
        };

        assert_eq!(
            checks_for(b_id),
            vec!["check.b".to_string()],
            "re-emitting A must not attach A's check id to the unrelated event B"
        );
        assert_eq!(checks_for(a_id), vec!["check.a".to_string()]);
    }

    #[test]
    fn webhook_crud() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Webhook Test", "/tmp/wh", None)
            .expect("upsert");

        let id = db
            .save_webhook_config(
                project_id,
                "https://hooks.example.com/notify",
                "[\"scan_complete\"]",
                Some("secret123"),
                true,
            )
            .expect("save");
        assert!(id > 0);

        let configs = db.get_webhook_configs(project_id).expect("get");
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].url, "https://hooks.example.com/notify");

        db.delete_webhook_config(configs[0].id).expect("delete");
        let configs = db
            .get_webhook_configs(project_id)
            .expect("get after delete");
        assert_eq!(configs.len(), 0);
    }

    fn project_and_env(db: &Database, name: &str, url: &str) -> (i64, i64) {
        let project_id = db
            .upsert_project(
                name,
                &format!("/tmp/{}", name.replace(' ', "-")),
                Some("react"),
            )
            .expect("upsert project");
        let env_id = db
            .add_environment(project_id, url, "production", "production", "manual")
            .expect("add env");
        (project_id, env_id)
    }

    #[test]
    fn save_scan_schedule_inserts_when_absent() {
        let db = temp_db();
        let (project_id, env_id) = project_and_env(&db, "Schedule A", "https://a.example.com");

        db.save_scan_schedule(
            project_id,
            env_id,
            "daily",
            "09:30",
            None,
            ScheduledScanType::Health,
            Some("2026-04-20T09:30:00Z".to_string()),
        )
        .expect("save schedule");

        let loaded = db
            .get_scan_schedule(project_id, env_id, ScheduledScanType::Health)
            .expect("get schedule")
            .expect("schedule exists");
        assert_eq!(loaded.frequency, "daily");
        assert_eq!(loaded.time_of_day, "09:30");
        assert!(loaded.day_of_week.is_none());
        assert_eq!(loaded.scan_type, ScheduledScanType::Health);
        assert_eq!(loaded.next_run_at.as_deref(), Some("2026-04-20T09:30:00Z"));
        assert!(loaded.last_run_at.is_none());
    }

    #[test]
    fn save_scan_schedule_upserts_on_conflict() {
        let db = temp_db();
        let (project_id, env_id) = project_and_env(&db, "Upsert", "https://u.example.com");

        db.save_scan_schedule(
            project_id,
            env_id,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            None,
        )
        .expect("first save");
        db.save_scan_schedule(
            project_id,
            env_id,
            "weekly",
            "14:00",
            Some(3),
            ScheduledScanType::Health,
            Some("2026-04-22T14:00:00Z".to_string()),
        )
        .expect("second save (update)");

        let loaded = db
            .get_scan_schedule(project_id, env_id, ScheduledScanType::Health)
            .expect("get")
            .expect("schedule exists");
        assert_eq!(loaded.frequency, "weekly");
        assert_eq!(loaded.time_of_day, "14:00");
        assert_eq!(loaded.day_of_week, Some(3));
        assert_eq!(loaded.next_run_at.as_deref(), Some("2026-04-22T14:00:00Z"));
    }

    #[test]
    fn save_scan_schedule_keeps_separate_rows_per_scan_type() {
        // The unique key includes scan_type, so health + security should
        // coexist as independent schedules for the same env.
        let db = temp_db();
        let (project_id, env_id) = project_and_env(&db, "Multi", "https://m.example.com");

        db.save_scan_schedule(
            project_id,
            env_id,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            None,
        )
        .expect("health");
        db.save_scan_schedule(
            project_id,
            env_id,
            "weekly",
            "10:00",
            Some(1),
            ScheduledScanType::Security,
            None,
        )
        .expect("security");

        let h = db
            .get_scan_schedule(project_id, env_id, ScheduledScanType::Health)
            .expect("get health")
            .expect("exists");
        let s = db
            .get_scan_schedule(project_id, env_id, ScheduledScanType::Security)
            .expect("get security")
            .expect("exists");
        assert_eq!(h.frequency, "daily");
        assert_eq!(s.frequency, "weekly");
        assert_eq!(s.day_of_week, Some(1));
    }

    #[test]
    fn save_scan_schedule_rejects_invalid_frequency() {
        // CHECK constraint on frequency must reject anything not in
        // ('off', 'daily', 'weekly') - guards against typo'd input.
        let db = temp_db();
        let (project_id, env_id) = project_and_env(&db, "Invalid", "https://i.example.com");

        let result = db.save_scan_schedule(
            project_id,
            env_id,
            "hourly", // not allowed
            "09:00",
            None,
            ScheduledScanType::Health,
            None,
        );
        assert!(result.is_err(), "invalid frequency should be rejected");
    }

    #[test]
    fn get_scan_schedule_returns_none_when_absent() {
        let db = temp_db();
        let (project_id, env_id) = project_and_env(&db, "Empty", "https://e.example.com");

        let loaded = db
            .get_scan_schedule(project_id, env_id, ScheduledScanType::Health)
            .expect("query");
        assert!(loaded.is_none());
    }

    #[test]
    fn get_due_schedules_returns_only_overdue_active_rows() {
        let db = temp_db();
        let (p_due, e_due) = project_and_env(&db, "Due", "https://due.example.com");
        let (p_off, e_off) = project_and_env(&db, "Off", "https://off.example.com");
        let (p_future, e_future) = project_and_env(&db, "Future", "https://future.example.com");

        // (1) Past, active - should be returned.
        db.save_scan_schedule(
            p_due,
            e_due,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            Some("2020-01-01 09:00:00".to_string()),
        )
        .expect("due");

        // (2) frequency=off - must be excluded even when next_run_at is past.
        db.save_scan_schedule(
            p_off,
            e_off,
            "off",
            "09:00",
            None,
            ScheduledScanType::Health,
            Some("2020-01-01 09:00:00".to_string()),
        )
        .expect("off");

        // (3) Future next_run_at - must be excluded.
        db.save_scan_schedule(
            p_future,
            e_future,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            Some("2099-12-31 09:00:00".to_string()),
        )
        .expect("future");

        let due = db.get_due_schedules().expect("due query");
        let urls: Vec<&str> = due.iter().map(|(_, url)| url.as_str()).collect();
        assert_eq!(
            due.len(),
            1,
            "only the overdue active schedule should fire - got {:?}",
            urls
        );
        let (sched, env_url) = &due[0];
        assert_eq!(env_url, "https://due.example.com");
        assert_eq!(sched.scan_type, ScheduledScanType::Health);
        assert_eq!(sched.frequency, "daily");
    }

    #[test]
    fn get_due_schedules_skips_rows_with_null_next_run_at() {
        let db = temp_db();
        let (project_id, env_id) = project_and_env(&db, "NullNext", "https://nullnext.example.com");

        db.save_scan_schedule(
            project_id,
            env_id,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            None,
        )
        .expect("save");

        let due = db.get_due_schedules().expect("due query");
        assert!(
            due.is_empty(),
            "schedules with NULL next_run_at must not fire"
        );
    }

    #[test]
    fn get_due_schedules_same_day_boundary_in_the_production_format() {
        let db = temp_db();
        let (p_past, e_past) = project_and_env(&db, "BoundaryPast", "https://past.example.com");
        let (p_future, e_future) =
            project_and_env(&db, "BoundaryFuture", "https://future.example.com");

        let now = chrono::Local::now().naive_local();
        let wall = |dt: chrono::NaiveDateTime| dt.format("%Y-%m-%d %H:%M:%S").to_string();

        db.save_scan_schedule(
            p_past,
            e_past,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            Some(wall(now - chrono::Duration::minutes(5))),
        )
        .expect("past schedule");
        db.save_scan_schedule(
            p_future,
            e_future,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            Some(wall(now + chrono::Duration::minutes(5))),
        )
        .expect("future schedule");

        let due = db.get_due_schedules().expect("due query");
        let urls: Vec<&str> = due.iter().map(|(_, url)| url.as_str()).collect();
        assert_eq!(
            urls,
            vec!["https://past.example.com"],
            "five minutes past fires, five minutes ahead waits"
        );
    }

    // A multi-page collection remains one canonical execution however many
    // immutable page runs it owns. Quota is attached to that execution, not
    // inferred later from child-row counts.
    #[test]
    fn a_multi_page_run_is_one_execution_with_many_page_runs() {
        let db = temp_db();
        let site_id = db
            .get_or_create_site("https://pages.example.com")
            .expect("site");
        let page = |url: &str| crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: url.to_string(),
            mode: "full".into(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 80,
            categories: vec![],
            issues: vec![],
            detected_stack: None,
            duration_ms: 100,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        let session_id = db.create_scan_session(site_id, 4, false).expect("session");
        for path in ["/", "/about", "/pricing", "/contact"] {
            let url = format!("https://pages.example.com{path}");
            db.save_scan_with_session(site_id, session_id, &url, &page(&url))
                .expect("save page scan");
        }

        let (executions, parents, pages): (i64, i64, i64) = db
            .execute(|conn| {
                Ok::<_, crate::db::DbError>((
                    conn.query_row("SELECT COUNT(*) FROM scan_executions", [], |row| row.get(0))?,
                    conn.query_row(
                        "SELECT COUNT(*) FROM scan_runs WHERE run_kind = 'multi_parent'",
                        [],
                        |row| row.get(0),
                    )?,
                    conn.query_row(
                        "SELECT COUNT(*) FROM scan_runs WHERE run_kind = 'page'",
                        [],
                        |row| row.get(0),
                    )?,
                ))
            })
            .expect("database worker")
            .expect("canonical counts");
        assert_eq!((executions, parents, pages), (1, 1, 4));

        // A second run is a second scan, and a sessionless single-page scan
        // stands on its own rather than collapsing into the session above.
        db.save_scan(site_id, &page("https://pages.example.com/"))
            .expect("save single scan");
        let executions: i64 = db
            .execute(|conn| {
                conn.query_row("SELECT COUNT(*) FROM scan_executions", [], |row| row.get(0))
            })
            .expect("database worker")
            .expect("execution count");
        assert_eq!(executions, 2, "a separate action owns another execution");
    }

    #[test]
    fn scan_admission_has_no_usage_ledger() {
        let admit_at = |db: &Database, local: chrono::DateTime<chrono::Local>, key: &str| {
            db.admit_scan_execution(
                crate::core::scan_execution::NewScanExecution {
                    project_id: None,
                    environment_id: None,
                    environment_url: Some("https://count.example.com".to_string()),
                    environment_scope_key: "https://count.example.com".to_string(),
                    requested_mode: crate::core::scan_execution::ScanExecutionMode::Web,
                    web_focus: Some(crate::core::scanner::ScanType::Health),
                    trigger: crate::core::scan_execution::ScanTrigger::Manual,
                    admission_class: crate::core::scan_execution::ScanAdmissionClass::GeneralScan,
                    idempotency_key: key.to_string(),
                    request_fingerprint: format!("v1:fixture:{key}"),
                    now_ms: local.timestamp_millis(),
                    web_status: Some(crate::core::scan_execution::ScanComponentStatus::Planned),
                    web_detail: None,
                    code_status: None,
                    code_detail: None,
                },
                crate::constants::SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
            )
            .expect("admit fixture execution");
        };

        let db = temp_db();
        for (offset_days, key) in [(1, "yesterday"), (0, "now"), (0, "again")] {
            admit_at(
                &db,
                chrono::Local::now() - chrono::Duration::days(offset_days),
                key,
            );
        }
        // No quota columns survive in the schema for a meter to read.
        let quota_columns: i64 = db
            .execute(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('scan_executions')
                     WHERE name IN ('quota_date', 'quota_state', 'counts_toward_quota')",
                    [],
                    |row| row.get(0),
                )
            })
            .expect("database worker")
            .expect("pragma");
        assert_eq!(quota_columns, 0, "migration 023 dropped the quota ledger");
    }

    #[test]
    fn mark_schedule_run_updates_last_and_next() {
        let db = temp_db();
        let (project_id, env_id) = project_and_env(&db, "MarkRun", "https://markrun.example.com");

        db.save_scan_schedule(
            project_id,
            env_id,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            Some("2026-04-20 09:00:00".to_string()),
        )
        .expect("save");

        let before = db
            .get_scan_schedule(project_id, env_id, ScheduledScanType::Health)
            .expect("get")
            .expect("exists");
        let schedule_id = before.id.expect("schedule has id after save");
        assert!(before.last_run_at.is_none());

        db.mark_schedule_run(schedule_id, Some("2026-04-21 09:00:00".to_string()))
            .expect("mark run");

        let after = db
            .get_scan_schedule(project_id, env_id, ScheduledScanType::Health)
            .expect("get")
            .expect("exists");
        assert!(
            after.last_run_at.is_some(),
            "last_run_at must be set after mark_schedule_run"
        );
        assert_eq!(after.next_run_at.as_deref(), Some("2026-04-21 09:00:00"));
    }

    #[test]
    fn mark_schedule_run_can_clear_next_run_at() {
        // Passing None for next_run_at (e.g. user disabled the schedule
        // mid-cycle) should null the column.
        let db = temp_db();
        let (project_id, env_id) =
            project_and_env(&db, "ClearNext", "https://clearnext.example.com");

        db.save_scan_schedule(
            project_id,
            env_id,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            Some("2026-04-20T09:00:00".to_string()),
        )
        .expect("save");
        let saved = db
            .get_scan_schedule(project_id, env_id, ScheduledScanType::Health)
            .expect("get")
            .expect("exists");

        db.mark_schedule_run(saved.id.expect("id"), None)
            .expect("mark run with null next");

        let after = db
            .get_scan_schedule(project_id, env_id, ScheduledScanType::Health)
            .expect("get")
            .expect("exists");
        assert!(after.next_run_at.is_none(), "next_run_at should be NULL");
    }

    #[test]
    fn schedule_cascades_when_environment_deleted() {
        // ON DELETE CASCADE: deleting the project should remove its
        // schedules so the scheduler doesn't try to scan an orphaned env.
        let db = temp_db();
        let (project_id, env_id) = project_and_env(&db, "Cascade", "https://cascade.example.com");

        db.save_scan_schedule(
            project_id,
            env_id,
            "daily",
            "09:00",
            None,
            ScheduledScanType::Health,
            None,
        )
        .expect("save");

        db.delete_project(project_id).expect("delete project");

        let count: i64 = db
            .execute(|conn| {
                conn.query_row("SELECT COUNT(*) FROM scan_schedules", [], |r| r.get(0))
                    .expect("count")
            })
            .expect("db worker");
        assert_eq!(count, 0, "schedules must cascade-delete with the project");
    }

    fn project_with_scan(db: &Database, name: &str, url: &str) -> (i64, i64) {
        let project_id = db
            .upsert_project(name, &format!("/tmp/{}", name.replace(' ', "-")), None)
            .expect("upsert");
        db.add_environment(project_id, url, "production", "production", "manual")
            .expect("add env");
        let site_id = db.get_or_create_site(url).expect("site");
        let scan = crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: url.to_string(),
            mode: "full".into(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 80,
            categories: vec![],
            issues: vec![],
            detected_stack: None,
            duration_ms: 100,
            timestamp: "2026-04-19T00:00:00Z".into(),
        };
        let scan_id = db.save_scan(site_id, &scan).expect("save scan");
        (project_id, scan_id)
    }

    #[test]
    fn create_and_get_issue_link_round_trips() {
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Round", "https://il-round.example.com");

        let link_id = db
            .create_issue_link(
                project_id,
                "seo.title",
                scan_id,
                "github_issues",
                "1234",
                "https://github.com/owner/repo/issues/1234",
            )
            .expect("create");
        assert!(link_id > 0);

        let links = db.get_issue_links(project_id).expect("get");
        assert_eq!(links.len(), 1);
        let link = &links[0];
        assert_eq!(link.id, link_id);
        assert_eq!(link.check_id, "seo.title");
        assert_eq!(link.scan_id, scan_id);
        assert_eq!(link.provider, "github_issues");
        assert_eq!(link.external_id, "1234");
        assert_eq!(
            link.external_url,
            "https://github.com/owner/repo/issues/1234"
        );
        // Default status MUST be 'open' so resolution gating works.
        assert_eq!(link.status, "open");
        assert!(link.resolved_at.is_none());
        assert!(!link.created_at.is_empty());
    }

    #[test]
    fn get_issue_links_orders_newest_first() {
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Order", "https://il-order.example.com");

        // create_issue_link uses Utc::now for created_at - sleep briefly
        // between calls so we get distinct timestamps.
        let id1 = db
            .create_issue_link(project_id, "seo.title", scan_id, "github_issues", "1", "u1")
            .expect("create 1");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let id2 = db
            .create_issue_link(
                project_id,
                "seo.canonical",
                scan_id,
                "github_issues",
                "2",
                "u2",
            )
            .expect("create 2");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let id3 = db
            .create_issue_link(project_id, "perf.cache", scan_id, "jira", "PROJ-7", "u3")
            .expect("create 3");

        let links = db.get_issue_links(project_id).expect("get");
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].id, id3, "newest first");
        assert_eq!(links[1].id, id2);
        assert_eq!(links[2].id, id1);
    }

    #[test]
    fn get_issue_link_for_attempt_finds_the_exact_match_behind_newer_rows() {
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Attempt", "https://il-attempt.example.com");

        let exact = db
            .create_issue_link(
                project_id,
                "seo.title",
                scan_id,
                "github_issues",
                "42",
                "u-42",
            )
            .expect("create exact");
        std::thread::sleep(std::time::Duration::from_millis(20));
        db.create_issue_link(
            project_id,
            "seo.title",
            scan_id,
            "jira",
            "PROJ-9",
            "u-newer",
        )
        .expect("create newer");

        let found = db
            .get_issue_link_for_attempt(project_id, "seo.title", scan_id, "github_issues")
            .expect("query")
            .expect("the exact attempt is found behind the newer jira row");
        assert_eq!(found.id, exact);

        // An attempt never filed answers None on every axis of the identity.
        assert!(db
            .get_issue_link_for_attempt(project_id, "seo.title", scan_id + 999, "github_issues")
            .expect("query")
            .is_none());
        assert!(db
            .get_issue_link_for_attempt(project_id, "seo.title", scan_id, "gitlab")
            .expect("query")
            .is_none());
        assert!(db
            .get_issue_link_for_attempt(project_id, "seo.other", scan_id, "github_issues")
            .expect("query")
            .is_none());
    }

    #[test]
    fn a_second_link_for_the_same_attempt_identity_is_refused_by_the_store() {
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Unique", "https://il-unique.example.com");

        db.create_issue_link(project_id, "seo.title", scan_id, "github_issues", "1", "u1")
            .expect("first attempt row inserts");
        let duplicate =
            db.create_issue_link(project_id, "seo.title", scan_id, "github_issues", "2", "u2");
        assert!(
            duplicate.is_err(),
            "a second row for the same (project, check, scan, provider) must be refused"
        );

        // The identity is the whole quadruple: a different provider or check
        // is a different attempt and still inserts.
        db.create_issue_link(project_id, "seo.title", scan_id, "jira", "PROJ-1", "u3")
            .expect("different provider inserts");
        db.create_issue_link(project_id, "seo.other", scan_id, "github_issues", "3", "u4")
            .expect("different check inserts");
    }

    #[test]
    fn issue_links_are_scoped_to_project() {
        let db = temp_db();
        let (p_a, scan_a) = project_with_scan(&db, "IL A", "https://il-a.example.com");
        let (p_b, scan_b) = project_with_scan(&db, "IL B", "https://il-b.example.com");

        db.create_issue_link(p_a, "seo.title", scan_a, "github_issues", "1", "u1")
            .expect("a");
        db.create_issue_link(p_b, "seo.title", scan_b, "jira", "PROJ-1", "u-b")
            .expect("b");

        let a_links = db.get_issue_links(p_a).expect("get a");
        let b_links = db.get_issue_links(p_b).expect("get b");
        assert_eq!(a_links.len(), 1);
        assert_eq!(b_links.len(), 1);
        assert_eq!(a_links[0].provider, "github_issues");
        assert_eq!(b_links[0].provider, "jira");
    }

    #[test]
    fn get_open_issue_links_excludes_resolved() {
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Open", "https://il-open.example.com");

        let l1 = db
            .create_issue_link(project_id, "seo.title", scan_id, "github_issues", "1", "u1")
            .expect("l1");
        let _l2 = db
            .create_issue_link(
                project_id,
                "seo.canonical",
                scan_id,
                "github_issues",
                "2",
                "u2",
            )
            .expect("l2");
        db.resolve_issue_link(l1).expect("resolve l1");

        let all = db.get_issue_links(project_id).expect("all");
        let open = db.get_open_issue_links(project_id).expect("open");
        assert_eq!(all.len(), 2);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].check_id, "seo.canonical");
    }

    #[test]
    fn resolve_issue_link_sets_resolved_at_and_status() {
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Resolve", "https://il-resolve.example.com");

        let link_id = db
            .create_issue_link(project_id, "seo.title", scan_id, "github_issues", "1", "u1")
            .expect("create");

        db.resolve_issue_link(link_id).expect("resolve");

        let link = db
            .get_issue_link_for_check(project_id, "seo.title")
            .expect("get")
            .expect("exists");
        assert_eq!(link.status, "resolved");
        assert!(
            link.resolved_at.is_some(),
            "resolved_at must be set after resolve_issue_link",
        );
    }

    #[test]
    fn resolve_issue_link_is_idempotent() {
        // Resolving an already-resolved link should not error and should
        // refresh resolved_at (so the most recent action wins).
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Idem", "https://il-idem.example.com");

        let link_id = db
            .create_issue_link(project_id, "seo.title", scan_id, "github_issues", "1", "u1")
            .expect("create");
        db.resolve_issue_link(link_id).expect("first resolve");
        db.resolve_issue_link(link_id).expect("second resolve");

        let link = db
            .get_issue_link_for_check(project_id, "seo.title")
            .expect("get")
            .expect("exists");
        assert_eq!(link.status, "resolved");
    }

    #[test]
    fn find_resolvable_issue_links_returns_open_links_for_passing_checks_only() {
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Auto", "https://il-auto.example.com");
        let (other_project, other_scan) =
            project_with_scan(&db, "IL Other", "https://il-other.example.com");

        let passing_open = db
            .create_issue_link(project_id, "seo.title", scan_id, "github", "1", "u1")
            .expect("passing open");
        let already_resolved = db
            .create_issue_link(project_id, "seo.canonical", scan_id, "github", "2", "u2")
            .expect("already resolved");
        db.resolve_issue_link(already_resolved).expect("resolve");
        let _still_failing = db
            .create_issue_link(project_id, "security.csp", scan_id, "jira", "PROJ-3", "u3")
            .expect("still failing");
        let _other_projects = db
            .create_issue_link(other_project, "seo.title", other_scan, "github", "9", "u9")
            .expect("other project");

        let resolvable = db
            .find_resolvable_issue_links(
                project_id,
                vec!["seo.title".to_string(), "seo.canonical".to_string()],
            )
            .expect("find resolvable");

        assert_eq!(resolvable.len(), 1);
        let (link_id, check_id, provider, external_id) = &resolvable[0];
        assert_eq!(*link_id, passing_open);
        assert_eq!(check_id, "seo.title");
        assert_eq!(provider, "github");
        assert_eq!(external_id, "1");
    }

    #[test]
    fn find_resolvable_issue_links_with_no_passing_checks_is_empty() {
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Empty", "https://il-empty.example.com");
        db.create_issue_link(project_id, "seo.title", scan_id, "github", "1", "u1")
            .expect("open link");

        let resolvable = db
            .find_resolvable_issue_links(project_id, Vec::new())
            .expect("find resolvable");
        assert!(resolvable.is_empty());
    }

    #[test]
    fn get_issue_link_for_check_returns_most_recent() {
        // A check can be linked multiple times (e.g. reopened in a new
        // scan). The query returns the latest by created_at DESC.
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Recent", "https://il-recent.example.com");

        let _old = db
            .create_issue_link(
                project_id,
                "seo.title",
                scan_id,
                "github_issues",
                "1",
                "u-old",
            )
            .expect("old");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let newest = db
            .create_issue_link(project_id, "seo.title", scan_id, "jira", "PROJ-2", "u-new")
            .expect("new");

        let link = db
            .get_issue_link_for_check(project_id, "seo.title")
            .expect("get")
            .expect("exists");
        assert_eq!(link.id, newest);
        assert_eq!(link.external_id, "PROJ-2");
    }

    #[test]
    fn get_issue_link_for_check_returns_none_when_absent() {
        let db = temp_db();
        let project_id = db
            .upsert_project("IL Absent", "/tmp/il-absent", None)
            .expect("upsert");

        let result = db
            .get_issue_link_for_check(project_id, "never.linked")
            .expect("query");
        assert!(result.is_none());
    }

    #[test]
    fn get_issue_link_for_check_is_project_scoped() {
        // Same check_id in two projects must not bleed across.
        let db = temp_db();
        let (p_a, scan_a) = project_with_scan(&db, "IL Scope A", "https://il-scope-a.example.com");
        let (p_b, _scan_b) = project_with_scan(&db, "IL Scope B", "https://il-scope-b.example.com");

        db.create_issue_link(p_a, "seo.title", scan_a, "github_issues", "1", "u1")
            .expect("a");

        let in_b = db
            .get_issue_link_for_check(p_b, "seo.title")
            .expect("get b");
        assert!(in_b.is_none(), "issue links must not leak across projects");
    }

    #[test]
    fn delete_project_removes_issue_links() {
        let db = temp_db();
        let (project_id, scan_id) =
            project_with_scan(&db, "IL Delete", "https://il-delete.example.com");

        db.create_issue_link(project_id, "seo.title", scan_id, "github_issues", "1", "u1")
            .expect("create");

        db.delete_project(project_id)
            .expect("delete must succeed even when issue_links exist");

        let count: i64 = db
            .execute(|conn| {
                conn.query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
                    .expect("count")
            })
            .expect("db worker");
        assert_eq!(count, 0, "issue_links must be removed with the project");
    }

    #[test]
    fn project_delete_cascades() {
        let db = temp_db();
        let pid = db
            .upsert_project("Del Test", "/tmp/del", None)
            .expect("upsert");
        db.add_environment(
            pid,
            "https://del.example.com",
            "prod",
            "production",
            "manual",
        )
        .expect("add env");

        let site_id = db
            .get_or_create_site("https://del.example.com")
            .expect("site");
        let result = crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: "https://del.example.com".to_string(),
            mode: "full".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 70,
            categories: vec![],
            issues: vec![],
            detected_stack: None,
            duration_ms: 500,
            timestamp: "2025-03-01T00:00:00Z".to_string(),
        };
        db.save_scan(site_id, &result).expect("save");

        db.delete_project(pid).expect("delete");

        let projects = db.get_projects().expect("get");
        assert!(projects.is_empty());
    }

    #[test]
    fn execute_returns_result_with_correct_shape() {
        let db = temp_db();
        let res: Result<i64, String> = db
            .execute(|conn| {
                conn.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))
                    .map_err(|e| e.to_string())
            })
            .map_err(String::from)
            .and_then(|v| v);
        assert_eq!(res.unwrap(), 1);
    }
    #[test]
    fn delete_project_leaves_no_rows_behind() {
        let db = temp_db();
        let pid = db
            .upsert_project("Doomed", "/tmp/doomed", None)
            .expect("project");
        db.add_environment(
            pid,
            "https://doomed.example",
            "Prod",
            "production",
            "manual",
        )
        .expect("environment");
        let site_id = db
            .get_or_create_site("https://doomed.example")
            .expect("site");
        db.create_scan_session(site_id, 3, false)
            .expect("multi-page session");

        let result = crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: "https://doomed.example".to_string(),
            mode: "full".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 70,
            categories: vec![],
            issues: vec![],
            detected_stack: None,
            duration_ms: 100,
            timestamp: "2026-07-05T00:00:00Z".to_string(),
        };
        db.save_scan(site_id, &result).expect("scan");

        db.set_issue_group_state(
            pid,
            "https://doomed.example",
            "seo.title",
            IssueLifecycle::Ignored,
            1_000,
        )
        .expect("issue state");
        db.create_fix_attempt(pid, "https://doomed.example", "seo.title", "cursor", 1_000)
            .expect("fix attempt");
        db.execute(move |conn| {
            conn.execute(
                "INSERT INTO events (project_id, event_type, severity, occurred_at_ms, title)
                 VALUES (?1, 'deploy', 'info', 1000, 'seed')",
                rusqlite::params![pid],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO integration_configs (project_id, integration_type, api_key)
                 VALUES (?1, 'cloudflare', 'secret-key')",
                rusqlite::params![pid],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO signal_history (project_id, signal_key, ts_ms, value)
                 VALUES (?1, 'traffic', 1000, 1.0)",
                rusqlite::params![pid],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO alerts (project_id, env_url, source, alert_id, severity, title,
                                     description, occurred_at, first_seen_at, last_seen_at)
                 VALUES (?1, 'https://doomed.example', 'scan', 'a1', 'critical', 't', 'd', 1, 1, 1)",
                rusqlite::params![pid],
            )
            .map_err(|e| e.to_string())?;
            Ok::<_, String>(())
        })
        .expect("worker")
        .expect("seed rows");

        db.delete_project(pid)
            .expect("delete project must not hit FK errors");

        let leftovers: Vec<(String, i64)> = db
            .execute(|conn| {
                let tables = [
                    "environments",
                    "sites",
                    "scan_executions",
                    "scan_runs",
                    "scan_findings",
                    "events",
                    "work_items",
                    "alerts",
                    "project_issue_states",
                    "fix_attempts",
                    "integration_configs",
                    "signal_history",
                ];
                let mut counts = Vec::new();
                for table in tables {
                    let count: i64 = conn
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                        .expect("count");
                    counts.push((table.to_string(), count));
                }
                counts
            })
            .expect("worker");
        for (table, count) in leftovers {
            assert_eq!(count, 0, "table {table} must be empty after delete_project");
        }
    }

    #[test]
    fn delete_environment_removes_env_scoped_state_and_keeps_siblings() {
        let db = temp_db();
        let pid = db
            .upsert_project("Multi", "/tmp/multi", None)
            .expect("project");
        let env_a = db
            .add_environment(pid, "https://a.example", "A", "production", "manual")
            .expect("env a");
        db.add_environment(pid, "https://b.example", "B", "staging", "manual")
            .expect("env b");
        let site_a = db.get_or_create_site("https://a.example").expect("site a");
        db.create_scan_session(site_a, 2, false).expect("session a");

        for env in ["https://a.example", "https://b.example"] {
            db.set_issue_group_state(pid, env, "seo.title", IssueLifecycle::Ignored, 1_000)
                .expect("issue state");
            db.create_fix_attempt(pid, env, "seo.title", "cursor", 1_000)
                .expect("fix attempt");
            db.record_score_snapshot_if_changed(
                pid,
                Some(env),
                &crate::core::types_work_items::ScoreSnapshot {
                    overall: 91.0,
                    per_category: std::collections::HashMap::new(),
                    critical_count: 0,
                    high_count: 1,
                    medium_count: 0,
                    low_count: 0,
                    exploitable_capped: false,
                    breakdown: Default::default(),
                    computed_at: 1_000,
                },
            )
            .expect("score snapshot");
            let env_owned = env.to_string();
            db.execute(move |conn| {
                conn.execute(
                    "INSERT INTO work_items (project_id, env_url, source, signal_id, check_id,
                                             category, severity, title, description,
                                             first_seen_at, last_seen_at)
                     VALUES (?1, ?2, 'updates', 'sig-1', 'updates.dep', 'updates', 'high',
                             't', 'd', 1000, 1000)",
                    rusqlite::params![pid, env_owned],
                )
                .map_err(|e| e.to_string())?;
                conn.execute(
                    "INSERT INTO alerts (project_id, env_url, source, alert_id, severity, title,
                                         description, occurred_at, first_seen_at, last_seen_at)
                     VALUES (?1, ?2, 'scan', 'a1', 'critical', 't', 'd', 1, 1, 1)",
                    rusqlite::params![pid, env_owned],
                )
                .map_err(|e| e.to_string())?;
                Ok::<_, String>(())
            })
            .expect("worker")
            .expect("seed env rows");
        }

        db.delete_environment(env_a).expect("delete env a");

        let (a_counts, b_counts): (Vec<i64>, Vec<i64>) = db
            .execute(|conn| {
                let count = |table: &str, column: &str, url: &str| -> i64 {
                    conn.query_row(
                        &format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1"),
                        rusqlite::params![url],
                        |r| r.get(0),
                    )
                    .expect("count")
                };
                let per_env = |url: &str| {
                    vec![
                        count("work_items", "env_url", url),
                        count("project_issue_states", "env_url", url),
                        count("alerts", "env_url", url),
                        count("fix_attempts", "env_url", url),
                        count("sites", "url", url),
                        count("score_snapshots", "environment_url", url),
                    ]
                };
                (per_env("https://a.example"), per_env("https://b.example"))
            })
            .expect("worker");
        assert!(
            a_counts.iter().all(|&count| count == 0),
            "env A rows must be gone, got {a_counts:?}"
        );
        assert_eq!(b_counts, vec![1, 1, 1, 1, 0, 1], "env B state must survive");

        let sessions: i64 = db
            .execute(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM scan_runs WHERE run_kind = 'multi_parent'",
                    [],
                    |r| r.get(0),
                )
                .expect("count")
            })
            .expect("worker");
        assert_eq!(sessions, 0, "env A's scan session must cascade away");

        db.add_environment(pid, "https://a.example", "A", "production", "manual")
            .expect("re-add env a");
        assert!(
            db.record_score_snapshot_if_changed(
                pid,
                Some("https://a.example"),
                &crate::core::types_work_items::ScoreSnapshot {
                    overall: 91.0,
                    per_category: std::collections::HashMap::new(),
                    critical_count: 0,
                    high_count: 1,
                    medium_count: 0,
                    low_count: 0,
                    exploitable_capped: false,
                    breakdown: Default::default(),
                    computed_at: 2_000,
                },
            )
            .expect("baseline write"),
            "re-created env must get a fresh baseline row, not a suppressed write"
        );
    }

    #[test]
    fn shared_url_projects_do_not_destroy_each_other_on_delete() {
        let db = temp_db();
        let pid_a = db.upsert_project("Older", "/tmp/a", None).expect("a");
        db.add_environment(
            pid_a,
            "https://shared.example",
            "Prod",
            "production",
            "manual",
        )
        .expect("env a");
        let pid_b = db.upsert_project("Newer", "/tmp/b", None).expect("b");
        db.add_environment(
            pid_b,
            "https://shared.example",
            "Prod",
            "production",
            "manual",
        )
        .expect("env b");

        // Attribution rule: the site resolves to the oldest environment.
        let site_id = db
            .get_or_create_site("https://shared.example")
            .expect("site");
        let owner: Option<i64> = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT project_id FROM sites WHERE id = ?1",
                    rusqlite::params![site_id],
                    |r| r.get(0),
                )
                .expect("owner")
            })
            .expect("worker");
        assert_eq!(owner, Some(pid_a));

        db.delete_project(pid_b).expect("delete newer project");
        let site_survives: i64 = db
            .execute(|conn| {
                conn.query_row("SELECT COUNT(*) FROM sites", [], |r| r.get(0))
                    .expect("count")
            })
            .expect("worker");
        assert_eq!(site_survives, 1, "deleting B must not touch A's site");

        db.delete_project(pid_a).expect("delete older project");
        // With A gone, the URL now resolves to nothing (B was deleted too);
        // a fresh registration starts a clean history.
        let sites_left: i64 = db
            .execute(|conn| {
                conn.query_row("SELECT COUNT(*) FROM sites", [], |r| r.get(0))
                    .expect("count")
            })
            .expect("worker");
        assert_eq!(sites_left, 0);
    }

    #[test]
    fn explicit_project_site_resolution_keeps_shared_urls_separate() {
        let db = temp_db();
        let pid_a = db
            .upsert_project("First", "/tmp/shared-a", None)
            .expect("a");
        let pid_b = db
            .upsert_project("Second", "/tmp/shared-b", None)
            .expect("b");
        for project_id in [pid_a, pid_b] {
            db.add_environment(
                project_id,
                "https://shared.example",
                "Production",
                "production",
                "manual",
            )
            .expect("environment");
        }

        let site_a = db
            .get_or_create_site_for_project(pid_a, "https://shared.example/")
            .expect("site a");
        let site_b = db
            .get_or_create_site_for_project(pid_b, "https://shared.example")
            .expect("site b");

        assert_ne!(site_a, site_b);
        let owners = db
            .execute(move |conn| {
                let mut statement = conn
                    .prepare("SELECT project_id FROM sites WHERE id IN (?1, ?2) ORDER BY id")
                    .expect("statement");
                statement
                    .query_map(rusqlite::params![site_a, site_b], |row| {
                        row.get::<_, i64>(0)
                    })
                    .expect("query")
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .expect("owners")
            })
            .expect("worker");
        assert_eq!(owners, vec![pid_a, pid_b]);
    }

    #[test]
    fn project_scoped_history_and_trend_keep_shared_urls_separate() {
        let db = temp_db();
        let pid_a = db
            .upsert_project("First", "/tmp/history-shared-a", None)
            .expect("project a");
        let pid_b = db
            .upsert_project("Second", "/tmp/history-shared-b", None)
            .expect("project b");
        for project_id in [pid_a, pid_b] {
            db.add_environment(
                project_id,
                "https://history-shared.example",
                "Production",
                "production",
                "manual",
            )
            .expect("environment");
        }

        for (project_id, score, timestamp) in [
            (pid_a, 61, "2026-08-01T00:00:00Z"),
            (pid_b, 94, "2026-08-02T00:00:00Z"),
        ] {
            let site_id = db
                .get_or_create_site_for_project(project_id, "https://history-shared.example")
                .expect("project site");
            db.save_scan(
                site_id,
                &crate::core::scanner::ScanResult {
                    page_signals: None,
                    site_facts: None,
                    url: "https://history-shared.example".to_string(),
                    mode: "full".to_string(),
                    scan_type: crate::core::scanner::ScanType::Health,
                    overall_score: score,
                    categories: vec![],
                    issues: vec![],
                    detected_stack: None,
                    duration_ms: 1000,
                    timestamp: timestamp.to_string(),
                },
            )
            .expect("save scan");
        }

        let history_a = db
            .get_scan_history_for_project(pid_a, "https://history-shared.example", 10)
            .expect("history a");
        let history_b = db
            .get_scan_history_for_project(pid_b, "https://history-shared.example", 10)
            .expect("history b");
        assert_eq!(
            history_a
                .iter()
                .map(|entry| entry.overall_score)
                .collect::<Vec<_>>(),
            vec![61]
        );
        assert_eq!(
            history_b
                .iter()
                .map(|entry| entry.overall_score)
                .collect::<Vec<_>>(),
            vec![94]
        );

        let trend_a = db
            .get_score_trend_for_project(pid_a, "https://history-shared.example", 10)
            .expect("trend a");
        let trend_b = db
            .get_score_trend_for_project(pid_b, "https://history-shared.example", 10)
            .expect("trend b");
        assert_eq!(
            trend_a
                .iter()
                .map(|entry| entry.overall)
                .collect::<Vec<_>>(),
            vec![61]
        );
        assert_eq!(
            trend_b
                .iter()
                .map(|entry| entry.overall)
                .collect::<Vec<_>>(),
            vec![94]
        );
    }
}
