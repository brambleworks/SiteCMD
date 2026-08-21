//! Engine-release persistence tests.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::core::normalized_scan::{normalize_web_scan, ScanRunKind};
use crate::core::scan_execution::{
    NewScanExecution, ScanAdmissionClass, ScanComponentStatus, ScanExecutionMode, ScanTrigger,
};
use crate::core::scanner::{ScanResult, ScanType};
use crate::db::test_helpers::{temp_db, TestDb};

fn execution(db: &TestDb, project_id: i64, url: &str, key: &str) -> i64 {
    db.admit_scan_execution(
        NewScanExecution {
            project_id: Some(project_id),
            environment_id: None,
            environment_url: Some(url.into()),
            environment_scope_key: url.into(),
            requested_mode: ScanExecutionMode::Web,
            web_focus: Some(ScanType::Health),
            trigger: ScanTrigger::Manual,
            admission_class: ScanAdmissionClass::GeneralScan,
            idempotency_key: key.into(),
            request_fingerprint: format!("v1:{key}"),
            now_ms: 100,
            web_status: Some(ScanComponentStatus::Planned),
            web_detail: None,
            code_status: None,
            code_detail: None,
        },
        900,
    )
    .expect("execution")
    .execution
    .id
}

fn result(url: &str, check_id: &str) -> ScanResult {
    ScanResult {
        url: url.into(),
        mode: "live".into(),
        scan_type: ScanType::Health,
        overall_score: 80,
        categories: Vec::new(),
        issues: vec![CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Security,
            title: check_id.into(),
            description: "detail".into(),
            status: CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: Some("producer fix".into()),
            manual_fix: None,
            raw_data: None,
            confidence: IssueConfidence::Confirmed,
            confidence_reason: Some("observed".into()),
            why_it_matters: Some("impact".into()),
        }],
        detected_stack: None,
        duration_ms: 10,
        timestamp: "2026-07-21T00:00:00Z".into(),
        page_signals: None,
        site_facts: None,
    }
}

// Persist one web run and hand back its id.
fn stamped_run(db: &TestDb, key: &str) -> i64 {
    let project_id = db
        .upsert_project("p", &format!("/tmp/engine-release-{key}"), None)
        .expect("project");
    let url = "https://example.com";
    let site_id = db.get_or_create_site(url).expect("site");
    let execution_id = execution(db, project_id, url, key);
    let batch = normalize_web_scan(
        &result(url, "security.headers.csp"),
        execution_id,
        None,
        Some(project_id),
        site_id,
        ScanRunKind::Single,
        100,
    )
    .expect("normalize");
    db.persist_normalized_scan_run(batch).expect("persist")
}

#[test]
fn a_persisted_run_carries_the_build_that_produced_it() {
    let db = temp_db();
    let run_id = stamped_run(&db, "stamp-one");

    let basis = db
        .run_release_basis(run_id)
        .expect("basis read")
        .expect("a run persisted by this build is stamped");

    assert_eq!(basis.stamp.engine_release, env!("CARGO_PKG_VERSION"));
    assert_eq!(
        basis.stamp.manifest_digest,
        sitecmd_engine::manifest::capability_manifest().manifest_digest
    );
    assert_eq!(
        basis.stamp.canonicalizer,
        sitecmd_engine::release::CANONICALIZER_VERSION
    );
    assert_eq!(
        basis.stamp.crawl_profile,
        sitecmd_engine::release::CRAWL_PROFILE
    );
}

#[test]
fn the_stamp_records_the_runtime_facts_a_verdict_can_depend_on() {
    let db = temp_db();
    let run_id = stamped_run(&db, "stamp-profile");

    let basis = db
        .run_release_basis(run_id)
        .expect("read")
        .expect("stamped");

    assert_eq!(
        basis.stamp.execution.transport.as_deref(),
        Some("reqwest_rustls")
    );
    assert!(basis
        .stamp
        .execution
        .layers_run
        .contains(&"transport".to_string()));
}

#[test]
fn the_stamp_records_the_browser_build_that_produced_browser_evidence() {
    let db = temp_db();
    let project_id = db
        .upsert_project("p", "/tmp/engine-release-browser", None)
        .expect("project");
    let url = "https://example.com";
    let site_id = db.get_or_create_site(url).expect("site");
    let execution_id = execution(&db, project_id, url, "stamp-browser");
    let mut batch = normalize_web_scan(
        &result(url, "performance.cwv.lcp"),
        execution_id,
        None,
        Some(project_id),
        site_id,
        ScanRunKind::Single,
        100,
    )
    .expect("normalize");
    batch.diagnostics.browser_ran = Some(true);
    batch.diagnostics.browser_build = Some("621.1.15".into());

    let run_id = db.persist_normalized_scan_run(batch).expect("persist");
    let basis = db
        .run_release_basis(run_id)
        .expect("read")
        .expect("stamped");

    assert_eq!(
        basis.stamp.execution.browser_build.as_deref(),
        Some("621.1.15")
    );
    assert!(basis
        .stamp
        .execution
        .layers_run
        .contains(&"browser".to_string()));
}

#[test]
fn the_run_records_the_scope_revision_it_was_scoped_by() {
    let db = temp_db();
    let url = "https://example.com";
    let site_id = db.get_or_create_site(url).expect("site");
    db.replace_scan_scope(site_id, &["/".into(), "/pricing".into()])
        .expect("scope write");
    let run_id = stamped_run(&db, "stamp-scope");

    let stored: Option<i64> = db
        .execute(move |conn| {
            conn.query_row(
                "SELECT scope_revision FROM scan_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .expect("scope revision column")
        })
        .expect("query");

    assert_eq!(
        stored,
        Some(1),
        "a run must record which scope it covered, or two runs of different scopes compare as if they covered the same routes"
    );
}

#[test]
fn the_inventory_is_recorded_with_the_first_run_that_uses_it() {
    let db = temp_db();
    let run_id = stamped_run(&db, "inventory-one");

    let basis = db
        .run_release_basis(run_id)
        .expect("read")
        .expect("stamped");

    assert_eq!(
        basis.inventory.len(),
        crate::core::engine_release::CURRENT_INVENTORY.len()
    );
    assert!(basis.inventory.lookup("security.headers.csp").is_some());
}

#[test]
fn the_recorded_inventory_survives_as_rows_not_as_a_current_build_lookup() {
    let db = temp_db();
    stamped_run(&db, "inventory-rows");

    let recorded: i64 = db
        .execute(|conn| {
            conn.query_row("SELECT COUNT(*) FROM engine_release_checks", [], |row| {
                row.get(0)
            })
            .expect("count")
        })
        .expect("query");

    assert_eq!(
        recorded as usize,
        crate::core::engine_release::CURRENT_INVENTORY.len()
    );
}

#[test]
fn a_second_run_reuses_the_recorded_inventory_instead_of_duplicating_it() {
    let db = temp_db();
    stamped_run(&db, "inventory-first");
    stamped_run(&db, "inventory-second");

    let (releases, checks): (i64, i64) = db
        .execute(|conn| {
            let releases = conn
                .query_row("SELECT COUNT(*) FROM engine_releases", [], |row| row.get(0))
                .expect("release count");
            let checks = conn
                .query_row("SELECT COUNT(*) FROM engine_release_checks", [], |row| {
                    row.get(0)
                })
                .expect("check count");
            (releases, checks)
        })
        .expect("query");

    assert_eq!(releases, 1);
    assert_eq!(
        checks as usize,
        crate::core::engine_release::CURRENT_INVENTORY.len()
    );
}

#[test]
fn code_checks_are_recorded_without_a_contract() {
    let db = temp_db();
    let run_id = stamped_run(&db, "inventory-code");
    let code_check_id = crate::core::code_scan::canonical_code_check_id(
        crate::core::code_scan::registry::CODE_CHECKS[0].slug,
    );

    let basis = db
        .run_release_basis(run_id)
        .expect("read")
        .expect("stamped");
    let entry = basis
        .inventory
        .lookup(&code_check_id)
        .expect("code checks are inventoried too");

    assert!(entry.contract.is_none());
}

#[test]
fn a_family_prefix_survives_the_round_trip_and_still_resolves_dynamic_ids() {
    let db = temp_db();
    let run_id = stamped_run(&db, "inventory-family");

    let basis = db
        .run_release_basis(run_id)
        .expect("read")
        .expect("stamped");

    assert!(
        basis
            .inventory
            .lookup("accessibility.axe.color-contrast")
            .is_some(),
        "a dynamic id must resolve through its stored family row"
    );
}

#[test]
fn an_unstamped_run_reads_back_as_no_basis_at_all() {
    let db = temp_db();
    let run_id = stamped_run(&db, "unstamped");
    db.execute(move |conn| {
        conn.execute(
            "UPDATE scan_runs SET engine_release = NULL, manifest_digest = NULL WHERE id = ?1",
            [run_id],
        )
        .expect("clear stamp");
    })
    .expect("update");

    assert!(
        db.run_release_basis(run_id).expect("read").is_none(),
        "a run with no stamp must not borrow the current build's identity"
    );
}

#[test]
fn a_stamp_whose_inventory_is_missing_reads_back_as_no_basis() {
    let db = temp_db();
    let run_id = stamped_run(&db, "orphan-stamp");
    db.execute(|conn| {
        conn.execute("DELETE FROM engine_release_checks", [])
            .expect("clear inventory");
    })
    .expect("delete");

    assert!(db.run_release_basis(run_id).expect("read").is_none());
}

#[test]
fn an_unknown_run_id_has_no_basis() {
    let db = temp_db();
    assert!(db.run_release_basis(9_999).expect("read").is_none());
}
