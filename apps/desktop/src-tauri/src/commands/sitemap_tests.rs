//! Sitemap-refresh persistence tests.
//!
//! The fetch itself is network work; these cover everything the command does
//! with what it fetched.

use super::*;
use crate::core::sitemap::{SitemapResult, SitemapStatus};
use crate::db::test_helpers::{temp_db_arc, TestDbArc};

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

/// Refuse one URL at the SQLite level, standing in for a constraint or disk
/// error partway through a real import.
fn refuse_url(db: &Database, url: &str) {
    let sql = format!(
        "CREATE TRIGGER refuse_one_page BEFORE INSERT ON pages
         WHEN NEW.url = '{url}'
         BEGIN SELECT RAISE(ABORT, 'sitemap row refused'); END"
    );
    db.execute_mut(move |conn| conn.execute_batch(&sql).expect("create trigger"))
        .expect("refuse url");
}

fn found(urls: Vec<String>) -> SitemapResult {
    SitemapResult {
        status: SitemapStatus::Found,
        urls,
        source_url: Some(format!("{SITE}/sitemap.xml")),
        partial_because: None,
    }
}

fn site_with_stored_sitemap() -> (TestDbArc, i64) {
    let db = temp_db_arc();
    let site_id = db.get_or_create_site(SITE).expect("site");
    db.save_pages(site_id, &[page(0)], "manual")
        .expect("stored sitemap");
    (db, site_id)
}

#[tokio::test]
async fn a_refused_row_reports_the_failure_instead_of_emptying_the_sitemap() {
    let (db, site_id) = site_with_stored_sitemap();
    refuse_url(&db, &page(2));

    let error = persist_refreshed_sitemap(
        db.db.clone(),
        site_id,
        &found(vec![page(1), page(2)]),
        false,
    )
    .await
    .expect_err("a refresh that could not be stored must not report success");

    assert!(!error.is_empty(), "the failure reaches the caller");
    assert_eq!(
        stored_urls(&db, site_id),
        vec![page(0)],
        "a refused refresh leaves the previous sitemap rather than an empty page list"
    );
    assert_eq!(
        db.get_sitemap_url(site_id).expect("sitemap url"),
        None,
        "and the source URL is not recorded for a sitemap that was never stored"
    );
}

#[tokio::test]
async fn a_stored_refresh_replaces_the_pages_and_records_its_source() {
    let (db, site_id) = site_with_stored_sitemap();

    persist_refreshed_sitemap(db.db.clone(), site_id, &found(vec![page(1), page(2)]), true)
        .await
        .expect("refresh persists");

    assert_eq!(stored_urls(&db, site_id), vec![page(1), page(2)]);
    assert_eq!(
        db.get_sitemap_url(site_id).expect("sitemap url"),
        Some(format!("{SITE}/sitemap.xml"))
    );
}

#[tokio::test]
async fn a_sitemap_that_was_not_found_leaves_the_stored_pages_alone() {
    let (db, site_id) = site_with_stored_sitemap();

    persist_refreshed_sitemap(
        db.db.clone(),
        site_id,
        &SitemapResult {
            status: SitemapStatus::NotFound,
            urls: Vec::new(),
            source_url: None,
            partial_because: None,
        },
        false,
    )
    .await
    .expect("nothing to persist");

    assert_eq!(stored_urls(&db, site_id), vec![page(0)]);
}
