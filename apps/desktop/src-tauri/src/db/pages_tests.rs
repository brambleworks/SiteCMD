//! Sitemap import tests: the import is one atomic unit, not a row at a time.

use crate::db::test_helpers::temp_db;
use crate::db::Database;

const SITE: &str = "https://example.com";

fn page(index: usize) -> String {
    format!("{SITE}/page-{index}")
}

fn stored_urls(db: &Database, site_id: i64) -> Vec<String> {
    db.get_pages(site_id)
        .expect("pages")
        .into_iter()
        .map(|page| page.url)
        .collect()
}

/// Refuse one URL at the SQLite level, the way a constraint or a disk error
/// would refuse a row partway through a real import.
fn refuse_url(db: &Database, url: &str) {
    let sql = format!(
        "CREATE TRIGGER refuse_one_page BEFORE INSERT ON pages
         WHEN NEW.url = '{url}'
         BEGIN SELECT RAISE(ABORT, 'sitemap row refused'); END"
    );
    db.execute_mut(move |conn| conn.execute_batch(&sql).expect("create trigger"))
        .expect("refuse url");
}

#[test]
fn a_failing_row_leaves_nothing_from_that_import_behind() {
    let db = temp_db();
    let site_id = db.get_or_create_site(SITE).expect("site");
    db.save_pages(site_id, &[page(0)], "auto")
        .expect("earlier import");
    refuse_url(&db, &page(2));

    let error = db
        .save_pages(site_id, &[page(1), page(2), page(3)], "auto")
        .expect_err("a refused row must fail the import");

    assert!(
        error.to_string().contains("sitemap row refused"),
        "unexpected error: {error}"
    );
    assert_eq!(
        stored_urls(&db, site_id),
        vec![page(0)],
        "the rows written before the failure roll back with it"
    );
}

#[test]
fn a_large_import_persists_every_row_and_reimports_without_duplicating() {
    let db = temp_db();
    let site_id = db.get_or_create_site(SITE).expect("site");
    let urls: Vec<String> = (0..3_000).map(page).collect();

    assert_eq!(
        db.save_pages(site_id, &urls, "auto").expect("import"),
        3_000
    );
    assert_eq!(stored_urls(&db, site_id).len(), 3_000);

    assert_eq!(
        db.save_pages(site_id, &urls, "manual").expect("re-import"),
        3_000
    );
    let stored = db.get_pages(site_id).expect("pages");
    assert_eq!(
        stored.len(),
        3_000,
        "the upsert updates the row it already has"
    );
    assert!(stored.iter().all(|page| page.source == "manual"));
}

#[test]
fn a_refused_replacement_leaves_the_previous_sitemap_in_place() {
    let db = temp_db();
    let site_id = db.get_or_create_site(SITE).expect("site");
    db.save_pages(site_id, &[page(0)], "manual")
        .expect("stored sitemap");
    refuse_url(&db, &page(2));

    let error = db
        .replace_pages(site_id, &[page(1), page(2)], "auto")
        .expect_err("a refused row must fail the replacement");

    assert!(
        error.to_string().contains("sitemap row refused"),
        "unexpected error: {error}"
    );
    assert_eq!(
        stored_urls(&db, site_id),
        vec![page(0)],
        "the clear rolls back with the import, so the site keeps the sitemap it had"
    );
}

#[test]
fn a_replacement_drops_the_pages_the_new_sitemap_no_longer_lists() {
    let db = temp_db();
    let site_id = db.get_or_create_site(SITE).expect("site");
    db.save_pages(site_id, &[page(0), page(1)], "auto")
        .expect("first import");

    assert_eq!(
        db.replace_pages(site_id, &[page(1), page(2)], "auto")
            .expect("replacement"),
        2
    );
    assert_eq!(
        stored_urls(&db, site_id),
        vec![page(1), page(2)],
        "a replacement is the new sitemap, not the union of both"
    );
}
