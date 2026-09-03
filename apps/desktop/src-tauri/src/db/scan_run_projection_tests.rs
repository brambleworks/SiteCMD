//! Candidate-loading tests for the scan-run projection.
//!
//! `covers` decides which open rows a run resolves. These tests pin the
//! narrower question: which rows the projection reads out of SQLite at all.

use super::*;
use crate::core::normalized_scan::ScanCoverageKind;
use crate::db::test_helpers::temp_db;
use crate::db::Database;

const ENV: &str = "https://example.com";

fn page_url(page: usize) -> String {
    format!("{ENV}/page-{page}")
}

fn check_id(index: usize) -> String {
    format!("security.check_{index}")
}

/// Seed `pages` x `checks` open web-scan rows plus one routeless row.
fn seed_open_items(db: &Database, project_id: i64, pages: usize, checks: usize) {
    seed_rows(
        db,
        project_id,
        "web_scan",
        (0..pages)
            .flat_map(|page| (0..checks).map(move |index| (check_id(index), Some(page_url(page)))))
            .collect(),
    );
}

fn seed_rows(db: &Database, project_id: i64, source: &str, rows: Vec<(String, Option<String>)>) {
    let source = source.to_string();
    db.execute_mut(move |conn| {
        let tx = conn.transaction().expect("transaction");
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO work_items
                        (project_id, env_url, source, signal_id, check_id, category,
                         severity, title, description, first_seen_at, last_seen_at,
                         resolved_at, page_url)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'security', 'high', ?5, ?5,
                             100, 100, NULL, ?6)",
                )
                .expect("prepare");
            for (check_id, page_url) in &rows {
                let signal_id = format!(
                    "{source}:{check_id}:{}",
                    page_url.as_deref().unwrap_or_default()
                );
                insert
                    .execute(params![
                        project_id, ENV, source, signal_id, check_id, page_url,
                    ])
                    .expect("insert work item");
            }
        }
        tx.commit().expect("commit");
    })
    .expect("seed");
}

/// Run the production candidate query the way `resolve_covered_absences`
/// does, through the same stored-key normalization.
fn candidates(
    db: &Database,
    project_id: i64,
    source: &'static str,
    coverage: &ScanCoverageManifest,
) -> Vec<OpenCandidate> {
    let coverage = as_stored_keys(coverage);
    db.execute_mut(move |conn| {
        let tx = conn.transaction().expect("transaction");
        let scope = ResolveScope {
            run_id: 1,
            observed_at: 1_000,
            project_id,
            source,
            environment_url: ENV,
        };
        load_open_candidates(&tx, &scope, &coverage).expect("load candidates")
    })
    .expect("candidate query")
}

fn seeded_project(name: &str) -> (crate::db::test_helpers::TestDb, i64) {
    let db = temp_db();
    let project_id = db
        .upsert_project("p", &format!("/tmp/{name}"), None)
        .expect("project");
    (db, project_id)
}

#[test]
fn a_finished_page_reads_only_the_open_rows_on_its_own_route() {
    let (db, project_id) = seeded_project("projection-route-bound");
    seed_open_items(&db, project_id, 100, 20);

    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::Page,
        vec![page_url(7)],
        (0..20).map(check_id).collect(),
    );
    let loaded = candidates(&db, project_id, "web_scan", &coverage);

    assert_eq!(
        loaded.len(),
        20,
        "one page's coverage must not materialize the whole site's 2000 open rows"
    );
    assert!(
        loaded
            .iter()
            .all(|candidate| candidate.page_url.as_deref() == Some(page_url(7).as_str())),
        "every candidate belongs to the covered route"
    );
    assert!(
        loaded
            .iter()
            .all(|candidate| coverage.covers(candidate.page_url.as_deref(), &candidate.check_id)),
        "the bound must not be wider than what the claim covers here"
    );
}

#[test]
fn a_routeless_row_survives_the_route_bound() {
    let (db, project_id) = seeded_project("projection-routeless");
    seed_rows(
        &db,
        project_id,
        "web_scan",
        vec![
            ("seo.canonical_loop".into(), None),
            ("security.csp".into(), Some(page_url(1))),
            ("security.csp".into(), Some(page_url(2))),
        ],
    );

    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::PageSet,
        vec![page_url(1)],
        vec!["seo.canonical_loop".into(), "security.csp".into()],
    );
    let loaded = candidates(&db, project_id, "web_scan", &coverage);

    let mut routes: Vec<Option<String>> = loaded
        .iter()
        .map(|candidate| candidate.page_url.clone())
        .collect();
    routes.sort();
    assert_eq!(
        routes,
        vec![None, Some(page_url(1))],
        "a site-level row has no route to bound, so the claim still has to answer for it"
    );
}

#[test]
fn a_mixed_case_host_survives_the_route_bound() {
    let (db, project_id) = seeded_project("projection-mixed-case");
    seed_rows(
        &db,
        project_id,
        "web_scan",
        vec![("security.csp".into(), Some("https://EXAMPLE.com/a".into()))],
    );

    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::Page,
        vec!["https://example.com/a".into()],
        vec!["security.csp".into()],
    );
    let loaded = candidates(&db, project_id, "web_scan", &coverage);

    assert_eq!(
        loaded.len(),
        1,
        "the bound compares routes without ASCII case"
    );
    assert!(
        as_stored_keys(&coverage).covers(
            Some(normalize_occurrence_url("https://EXAMPLE.com/a").as_str()),
            "security.csp"
        ),
        "and the row it kept is one the claim covers"
    );
}

#[test]
fn a_trailing_slash_route_is_left_outside_the_bound() {
    let (db, project_id) = seeded_project("projection-trailing-slash");
    seed_rows(
        &db,
        project_id,
        "web_scan",
        vec![(
            "security.csp".into(),
            Some("https://example.com/checkout/".into()),
        )],
    );

    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::Page,
        vec!["https://example.com/checkout".into()],
        vec!["security.csp".into()],
    );

    assert!(
        candidates(&db, project_id, "web_scan", &coverage).is_empty(),
        "a clean /checkout observation cannot speak for /checkout/, so its row is never read"
    );
}

#[test]
fn a_claim_with_no_route_bound_reads_every_open_row() {
    let (db, project_id) = seeded_project("projection-no-route-bound");
    seed_rows(
        &db,
        project_id,
        "code_scan",
        vec![
            ("code_scan.security".into(), None),
            ("code_scan.quality".into(), Some(page_url(1))),
        ],
    );

    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::Project,
        Vec::new(),
        vec!["code_scan.security".into(), "code_scan.quality".into()],
    );

    assert_eq!(
        candidates(&db, project_id, "code_scan", &coverage).len(),
        2,
        "a project claim observes no routes, so nothing may be narrowed away"
    );
}

#[test]
fn a_route_scoped_claim_with_no_routes_still_queries() {
    let (db, project_id) = seeded_project("projection-empty-route-bound");
    seed_rows(
        &db,
        project_id,
        "web_scan",
        vec![
            ("seo.canonical_loop".into(), None),
            ("security.csp".into(), Some(page_url(1))),
        ],
    );

    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::CheckSet,
        Vec::new(),
        vec!["seo.canonical_loop".into(), "security.csp".into()],
    );
    let loaded = candidates(&db, project_id, "web_scan", &coverage);

    assert_eq!(
        loaded.len(),
        1,
        "an empty route bound keeps only the routeless rows, and covers still refuses them"
    );
    assert!(!coverage.covers(None, "seo.canonical_loop"));
}

#[test]
fn a_route_bound_the_size_of_a_whole_sitemap_still_binds() {
    let (db, project_id) = seeded_project("projection-large-route-bound");
    seed_rows(
        &db,
        project_id,
        "web_scan",
        vec![("security.csp".into(), Some(page_url(4_999)))],
    );

    let coverage = ScanCoverageManifest::declared(
        ScanCoverageKind::PageSet,
        (0..5_000).map(page_url).collect(),
        vec!["security.csp".into()],
    );

    assert_eq!(
        candidates(&db, project_id, "web_scan", &coverage).len(),
        1,
        "a sitemap's worth of routes binds without exceeding SQLite's parameter limit"
    );
}
