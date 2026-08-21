//! Scan-scope persistence tests.

use crate::db::test_helpers::temp_db;

#[test]
fn a_site_with_no_stored_scope_reads_as_empty_at_revision_zero() {
    // Empty is what every existing install starts at, and callers read it
    // as "the entry page only" rather than as "scan nothing".
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    assert!(db
        .get_scan_scope_routes(site_id)
        .expect("routes")
        .is_empty());
    assert_eq!(db.get_scan_scope_revision(site_id).expect("revision"), 0);
}

#[test]
fn saving_a_scope_stores_the_authored_order_and_advances_the_revision() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");

    let first = db
        .replace_scan_scope(site_id, &["/".into(), "/pricing".into(), "/docs".into()])
        .expect("save");
    assert_eq!(first, 1);
    assert_eq!(
        db.get_scan_scope_routes(site_id).expect("routes"),
        vec!["/", "/pricing", "/docs"]
    );

    let second = db
        .replace_scan_scope(site_id, &["/".into(), "/about".into()])
        .expect("save");
    assert_eq!(second, 2);
}

#[test]
fn writing_the_same_scope_again_does_not_move_the_revision() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    let first = db
        .replace_scan_scope(site_id, &["/".into(), "/pricing".into()])
        .expect("save");
    let again = db
        .replace_scan_scope(site_id, &["/".into(), "/pricing".into()])
        .expect("save");
    assert_eq!(first, again);

    // Reordering IS a change: the stored order is the authored one.
    let reordered = db
        .replace_scan_scope(site_id, &["/pricing".into(), "/".into()])
        .expect("save");
    assert_eq!(reordered, first + 1);
}

#[test]
fn a_replacement_leaves_no_deselected_route_behind() {
    // The scope is the complete answer to what a site is watched on, so a
    // merge would keep scanning pages the person had just unticked.
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    db.replace_scan_scope(site_id, &["/".into(), "/pricing".into(), "/docs".into()])
        .expect("save");
    db.replace_scan_scope(site_id, &["/".into()]).expect("save");
    assert_eq!(
        db.get_scan_scope_routes(site_id).expect("routes"),
        vec!["/"]
    );
}

#[test]
fn two_sites_keep_their_own_scopes_and_revisions() {
    let db = temp_db();
    let first = db
        .get_or_create_site("https://first.example")
        .expect("site");
    let second = db
        .get_or_create_site("https://second.example")
        .expect("site");
    db.replace_scan_scope(first, &["/".into(), "/a".into()])
        .expect("save");

    assert_eq!(db.get_scan_scope_routes(first).expect("routes").len(), 2);
    assert!(db.get_scan_scope_routes(second).expect("routes").is_empty());
    assert_eq!(db.get_scan_scope_revision(second).expect("revision"), 0);
}

#[test]
fn a_route_listed_twice_cannot_be_stored_twice() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").expect("site");
    let error = db
        .replace_scan_scope(site_id, &["/".into(), "/".into()])
        .expect_err("duplicate route rejected");
    assert!(
        format!("{error}").to_lowercase().contains("unique"),
        "{error}"
    );
    assert!(db
        .get_scan_scope_routes(site_id)
        .expect("routes")
        .is_empty());
    assert_eq!(db.get_scan_scope_revision(site_id).expect("revision"), 0);
}

#[test]
fn a_local_site_resolves_the_connected_target_for_the_same_project_environment() {
    const URL: &str = "https://example.com";
    let db = temp_db();
    let project_id = db
        .upsert_project("Example", "/tmp/example", None)
        .expect("project");
    db.add_environment(project_id, URL, "Production", "production", "manual")
        .expect("environment");
    let site_id = db.get_or_create_site(URL).expect("site");
    db.connect_site(project_id, URL, "site_remote", 1)
        .expect("connected binding");

    let target = db
        .connected_scan_scope_target(site_id)
        .expect("target read")
        .expect("connected target");

    assert_eq!(target.project_id, project_id);
    assert_eq!(target.environment_scope_key, URL);
    assert_eq!(target.remote_site_id, "site_remote");
}

#[test]
fn connected_scope_delivery_stays_pending_until_its_local_revision_is_acknowledged() {
    const URL: &str = "https://example.com";
    let db = temp_db();
    let project_id = db
        .upsert_project("Example", "/tmp/scope-outbox", None)
        .expect("project");
    db.add_environment(project_id, URL, "Production", "production", "manual")
        .expect("environment");
    let site_id = db
        .get_or_create_site_for_project(project_id, URL)
        .expect("site");
    db.connect_site(project_id, URL, "site_remote", 1)
        .expect("connected binding");

    let revision = db
        .replace_scan_scope(site_id, &["/".into(), "/pricing".into()])
        .expect("scope");
    assert_eq!(
        db.pending_connected_scan_scope_site_ids().expect("pending"),
        vec![site_id]
    );

    db.mark_connected_scan_scope_synced(project_id, URL, "site_remote", 1, revision)
        .expect("acknowledge");
    assert!(db
        .pending_connected_scan_scope_site_ids()
        .expect("settled")
        .is_empty());

    db.replace_scan_scope(site_id, &["/".into(), "/docs".into()])
        .expect("new scope");
    assert_eq!(
        db.pending_connected_scan_scope_site_ids()
            .expect("pending again"),
        vec![site_id]
    );
}

#[test]
fn a_stale_scope_acknowledgement_cannot_settle_a_replacement_binding() {
    const URL: &str = "https://scope-race.example";
    let db = temp_db();
    let project_id = db
        .upsert_project("Example", "/tmp/scope-race", None)
        .expect("project");
    db.add_environment(project_id, URL, "Production", "production", "manual")
        .expect("environment");
    let site_id = db
        .get_or_create_site_for_project(project_id, URL)
        .expect("site");
    db.connect_site(project_id, URL, "site_remote_a", 1)
        .expect("first binding");
    let revision = db
        .replace_scan_scope(site_id, &["/".into(), "/pricing".into()])
        .expect("scope");
    let target = db
        .connected_scan_scope_target(site_id)
        .expect("target")
        .expect("connected target");

    db.disconnect_site(project_id, URL).expect("disconnect");
    // Reconnecting the same remote site is still a new local binding. Site ID
    // alone cannot distinguish the response that belonged to the deleted row.
    db.connect_site(project_id, URL, "site_remote_a", 2)
        .expect("replacement binding");

    let error = db
        .mark_connected_scan_scope_synced(
            project_id,
            URL,
            &target.remote_site_id,
            target.binding_connected_at,
            revision,
        )
        .expect_err("the stale acknowledgement must not settle the replacement");
    assert!(error.to_string().contains("binding changed"), "{error}");
    assert_eq!(
        db.pending_connected_scan_scope_site_ids().expect("pending"),
        vec![site_id]
    );
}
