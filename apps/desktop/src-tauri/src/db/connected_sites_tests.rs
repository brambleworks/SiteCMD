//! Connected-site binding tests.

use crate::db::test_helpers::{temp_db, TestDb};

const NOW_MS: i64 = 1_800_000_000_000;
const SITE: &str = "https://example.com";

fn seeded() -> (TestDb, i64) {
    let db = temp_db();
    let project_id = db
        .upsert_project("Connected", "/tmp/connected", Some("nextjs"))
        .expect("upsert project");
    (db, project_id)
}

fn connected(db: &TestDb, project_id: i64, scope_key: &str) {
    db.connect_site(project_id, scope_key, "site_9f2c81d0a4b3", NOW_MS)
        .expect("connect");
    db.mark_site_bootstrapped(project_id, scope_key, NOW_MS + 1)
        .expect("bootstrap");
}

#[test]
fn an_environment_with_no_binding_is_not_connected() {
    let (db, project_id) = seeded();
    assert_eq!(
        db.get_connected_site(project_id, SITE).expect("read"),
        None,
        "an environment nobody connected owes a service nothing"
    );
}

#[test]
fn a_binding_only_accepts_mutations_once_bootstrap_has_committed() {
    let (db, project_id) = seeded();
    db.connect_site(project_id, SITE, "site_9f2c81d0a4b3", NOW_MS)
        .expect("connect");

    let before = db
        .get_connected_site(project_id, SITE)
        .expect("read")
        .expect("bound");
    assert!(
        !before.accepts_mutations(),
        "before bootstrap the group set has not been sent, so there is nothing \
         for a mutation to guard against"
    );

    db.mark_site_bootstrapped(project_id, SITE, NOW_MS + 5)
        .expect("bootstrap");
    let after = db
        .get_connected_site(project_id, SITE)
        .expect("read")
        .expect("bound");
    assert!(after.accepts_mutations());
    assert_eq!(after.bootstrapped_at, Some(NOW_MS + 5));
}

#[test]
fn the_bootstrap_marker_is_set_once_and_then_stands() {
    let (db, project_id) = seeded();
    connected(&db, project_id, SITE);
    db.mark_site_bootstrapped(project_id, SITE, NOW_MS + 900)
        .expect("second bootstrap");

    let site = db
        .get_connected_site(project_id, SITE)
        .expect("read")
        .expect("bound");
    assert_eq!(
        site.bootstrapped_at,
        Some(NOW_MS + 1),
        "the service refuses a second bootstrap, so moving the marker would \
         misdate the point after which decisions started traveling as mutations"
    );
}

#[test]
fn bootstrapping_an_environment_that_is_not_connected_is_an_error() {
    let (db, project_id) = seeded();
    let error = db
        .mark_site_bootstrapped(project_id, SITE, NOW_MS)
        .expect_err("nothing to bootstrap");
    assert!(error.to_string().contains("not connected"));
}

#[test]
fn a_second_connection_to_a_different_site_is_refused_rather_than_rebound() {
    let (db, project_id) = seeded();
    db.connect_site(project_id, SITE, "site_first", NOW_MS)
        .expect("connect");

    let error = db
        .connect_site(project_id, SITE, "site_second", NOW_MS + 1)
        .expect_err("rebinding must be refused");
    assert!(
        error.to_string().contains("site_first"),
        "revisions and pending decisions are numbered by one site; silently \
         rebinding would guard a mutation with a revision from another stream"
    );

    // Reconnecting to the same site is the retry case and stays fine.
    db.connect_site(project_id, SITE, "site_first", NOW_MS + 2)
        .expect("idempotent reconnect");
}

#[test]
fn a_group_the_service_has_not_reported_reads_as_the_genesis_revision() {
    let (db, project_id) = seeded();
    connected(&db, project_id, SITE);
    let revision = db
        .record_pulled_group_revision(project_id, SITE, "security.csp", 0, NOW_MS)
        .expect("record genesis");
    assert_eq!(revision, 0);
}

#[test]
fn a_group_revision_never_moves_backwards() {
    let (db, project_id) = seeded();
    connected(&db, project_id, SITE);

    db.record_pulled_group_revision(project_id, SITE, "security.csp", 12, NOW_MS)
        .expect("pull");
    let after_stale_read = db
        .record_pulled_group_revision(project_id, SITE, "security.csp", 7, NOW_MS + 1)
        .expect("stale pull");
    assert_eq!(
        after_stale_read, 12,
        "a reordered or replayed read must not lower the basis the next \
         decision will be guarded by"
    );
}

#[test]
fn a_complete_pull_commits_group_revisions_and_watermark_atomically() {
    let (db, project_id) = seeded();
    connected(&db, project_id, SITE);
    db.execute(move |conn| -> Result<(), crate::db::DbError> {
        conn.execute_batch(
            "CREATE TRIGGER reject_second_connected_group
             BEFORE INSERT ON connected_group_revisions
             WHEN NEW.check_id = 'seo.title'
             BEGIN SELECT RAISE(ABORT, 'simulated page failure'); END;",
        )?;
        Ok(())
    })
    .expect("install failure trigger")
    .expect("create trigger");

    let error = db
        .record_connected_pull(
            project_id,
            SITE,
            42,
            vec![("security.csp".into(), 7), ("seo.title".into(), 9)],
            NOW_MS,
        )
        .expect_err("second group refuses the pull");
    assert!(error.to_string().contains("simulated page failure"));
    assert_eq!(
        row_counts(&db),
        (0, 0),
        "a failed pull must expose neither its earlier groups nor its watermark"
    );
}

#[test]
fn revisions_are_kept_per_group_and_per_site() {
    let (db, project_id) = seeded();
    connected(&db, project_id, SITE);
    let staging = "https://staging.example.com";
    connected(&db, project_id, staging);

    db.record_pulled_group_revision(project_id, SITE, "security.csp", 4, NOW_MS)
        .expect("pull prod");
    assert_eq!(
        db.record_pulled_group_revision(project_id, staging, "security.csp", 0, NOW_MS)
            .expect("pull staging"),
        0,
        "one group moving on one site says nothing about the same check elsewhere"
    );
    assert_eq!(
        db.record_pulled_group_revision(project_id, SITE, "seo.title", 0, NOW_MS)
            .expect("pull sibling"),
        0,
        "the guard is per group; a sibling's revision is not this group's basis"
    );
}

#[test]
fn a_code_only_projects_scope_key_is_not_run_through_url_normalization() {
    let (db, project_id) = seeded();
    let scope_key = format!("project:{project_id}");
    connected(&db, project_id, &scope_key);

    let site = db
        .get_connected_site(project_id, &scope_key)
        .expect("read")
        .expect("bound");
    assert_eq!(site.site_id, "site_9f2c81d0a4b3");
    assert_eq!(
        db.record_pulled_group_revision(
            project_id,
            &scope_key,
            "code_scan.n-plus-one-query",
            3,
            NOW_MS
        )
        .expect("pull"),
        3,
        "a code-only project keys its rows the way the lifecycle overlay does"
    );
}

#[test]
fn disconnecting_forgets_the_pulled_revisions_and_the_event_watermark() {
    let (db, project_id) = seeded();
    connected(&db, project_id, SITE);
    db.record_pulled_group_revision(project_id, SITE, "security.csp", 9, NOW_MS)
        .expect("pull");
    db.record_pulled_event_sequence(project_id, SITE, 42, NOW_MS)
        .expect("pull events");

    db.disconnect_site(project_id, SITE).expect("disconnect");

    assert_eq!(db.get_connected_site(project_id, SITE).expect("read"), None);
    assert_eq!(
        row_counts(&db),
        (0, 0),
        "a watermark left behind would make the next scan of a newly connected \
         site declare a basis taken from a stream it never pulled"
    );
}

#[test]
fn the_bindings_read_names_the_project_behind_every_connected_site_id() {
    let (db, project_id) = seeded();
    connected(&db, project_id, SITE);
    let other = db
        .upsert_project("Second", "/tmp/second", None)
        .expect("upsert project");
    db.connect_site(other, "https://second.example.com", "site_b", NOW_MS)
        .expect("connect");

    let mut bindings = db.connected_site_bindings().expect("read bindings");
    bindings.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].site_id, "site_9f2c81d0a4b3");
    assert_eq!(bindings[0].project_name, "Connected");
    assert_eq!(bindings[0].project_id, project_id);
    assert_eq!(bindings[1].site_id, "site_b");
    assert_eq!(bindings[1].env_url, "https://second.example.com");
}

#[test]
fn a_disconnected_environment_stops_appearing_in_the_bindings_read() {
    // A stale binding would let an alert for a site this machine no longer
    // holds resolve to a project that cannot show it.
    let (db, project_id) = seeded();
    connected(&db, project_id, SITE);
    db.disconnect_site(project_id, SITE).expect("disconnect");
    assert!(db.connected_site_bindings().expect("read").is_empty());
}

// (pulled group revisions, event watermarks) still stored.
fn row_counts(db: &TestDb) -> (i64, i64) {
    db.execute(move |conn| -> Result<(i64, i64), crate::db::DbError> {
        let revisions: i64 = conn.query_row(
            "SELECT COUNT(*) FROM connected_group_revisions",
            [],
            |row| row.get(0),
        )?;
        let watermarks: i64 = conn.query_row(
            "SELECT COUNT(*) FROM connected_site_watermarks",
            [],
            |row| row.get(0),
        )?;
        Ok((revisions, watermarks))
    })
    .expect("count rows")
    .expect("read counts")
}
