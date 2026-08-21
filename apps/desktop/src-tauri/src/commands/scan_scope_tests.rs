//! Scan-scope storage and coverage tests.

use crate::db::{scan_scope_urls, scan_scope_urls_for_project, test_helpers::temp_db};
use sitecmd_engine::scope::{build_scope, engine_check_families};

use super::scope_matches;

// The command's body without the Tauri `State` wrapper.
fn store(db: &crate::db::Database, site_id: i64, site_url: &str, routes: &[&str]) -> Vec<String> {
    let entry = url::Url::parse(site_url).expect("site url");
    let selected: Vec<String> = routes.iter().map(|route| route.to_string()).collect();
    let scope = build_scope(&entry, &selected, engine_check_families(), None).expect("scope");
    let stored: Vec<String> = scope.routes.into_iter().map(|route| route.route).collect();
    db.replace_scan_scope(site_id, &stored).expect("save");
    db.get_scan_scope_routes(site_id).expect("routes")
}

#[test]
fn a_selection_is_stored_as_canonical_routes_with_the_entry_first() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    let stored = store(
        &db,
        site_id,
        "https://example.com/",
        &["pricing", "/docs/../guides"],
    );
    assert_eq!(stored, vec!["/", "/pricing", "/guides"]);
}

#[test]
fn the_entry_page_survives_being_unticked() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    let stored = store(&db, site_id, "https://example.com/", &["/pricing"]);
    assert_eq!(stored.first().map(String::as_str), Some("/"));
}

#[test]
fn a_scan_covers_the_stored_scope() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    store(&db, site_id, "https://example.com/", &["/pricing"]);
    assert_eq!(
        scan_scope_urls(&db, "https://example.com"),
        vec!["https://example.com/", "https://example.com/pricing"]
    );
}

#[test]
fn project_scoped_scan_urls_keep_shared_url_scopes_separate() {
    let db = temp_db();
    let project_a = db
        .upsert_project("First", "/tmp/scope-shared-a", None)
        .expect("project a");
    let project_b = db
        .upsert_project("Second", "/tmp/scope-shared-b", None)
        .expect("project b");
    for project_id in [project_a, project_b] {
        db.add_environment(
            project_id,
            "https://shared-scope.example",
            "Production",
            "production",
            "manual",
        )
        .expect("environment");
    }

    let site_a = db
        .get_or_create_site_for_project(project_a, "https://shared-scope.example")
        .expect("site a");
    let site_b = db
        .get_or_create_site_for_project(project_b, "https://shared-scope.example")
        .expect("site b");
    store(&db, site_a, "https://shared-scope.example", &["/first"]);
    store(&db, site_b, "https://shared-scope.example", &["/second"]);

    assert_eq!(
        scan_scope_urls_for_project(&db, project_a, "https://shared-scope.example"),
        vec![
            "https://shared-scope.example/",
            "https://shared-scope.example/first"
        ]
    );
    assert_eq!(
        scan_scope_urls_for_project(&db, project_b, "https://shared-scope.example"),
        vec![
            "https://shared-scope.example/",
            "https://shared-scope.example/second"
        ]
    );
}

#[test]
fn a_site_with_no_scope_still_scans_its_entry_url() {
    // Every existing install is in this state, and it must keep behaving
    // exactly as it did before a scope existed.
    let db = temp_db();
    assert_eq!(
        scan_scope_urls(&db, "https://example.com"),
        vec!["https://example.com"]
    );
}

#[test]
fn an_unparseable_site_url_falls_back_rather_than_dropping_the_scan() {
    let db = temp_db();
    assert_eq!(scan_scope_urls(&db, "not a url"), vec!["not a url"]);
}

#[test]
fn connected_scope_equality_compares_the_whole_remote_scope() {
    let state: crate::connected_service::ConnectedSiteState =
        serde_json::from_value(serde_json::json!({
            "phase": "connected",
            "scope": {
                "scope_revision": 4,
                "routes": ["/", "/pricing"],
                "check_families": ["web"]
            }
        }))
        .expect("state");
    assert!(scope_matches(
        &state,
        &["/".into(), "/pricing".into()],
        &["web".into()]
    ));
    assert!(scope_matches(
        &state,
        &["/pricing".into(), "/".into()],
        &["web".into()]
    ));
    assert!(!scope_matches(
        &state,
        &["/".into(), "/docs".into()],
        &["web".into()]
    ));
}
