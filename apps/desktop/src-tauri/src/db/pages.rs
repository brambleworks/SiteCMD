//! Pages (sitemap) CRUD.

use super::DbError;
use rusqlite::{named_params, params};

use super::from_row;
use super::types::PageRecord;
use super::Database;

impl Database {
    /// Save discovered pages for a site (upsert - updates last_seen_at on conflict).
    ///
    /// One transaction and one prepared statement for the whole import: a
    /// sitemap can carry thousands of URLs, and a per-row commit both costs a
    /// disk sync each time and leaves a half-imported sitemap behind when a
    /// row fails.
    #[tracing::instrument(skip(self, urls), fields(site_id, source = %source))]
    pub fn save_pages(
        &self,
        site_id: i64,
        urls: &[String],
        source: &str,
    ) -> Result<usize, DbError> {
        let urls = urls.to_vec();
        let source = source.to_string();
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            let count = upsert_pages(&tx, site_id, &urls, &source)?;
            tx.commit()?;
            Ok(count)
        })?
    }

    /// Replace a site's pages with a freshly discovered set.
    ///
    /// The clear and the import commit together. A refused row leaves the
    /// sitemap the site already had rather than wiping it, which a separate
    /// clear followed by a failed import would do.
    #[tracing::instrument(skip(self, urls), fields(site_id, source = %source))]
    pub fn replace_pages(
        &self,
        site_id: i64,
        urls: &[String],
        source: &str,
    ) -> Result<usize, DbError> {
        let urls = urls.to_vec();
        let source = source.to_string();
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM pages WHERE site_id = ?1", params![site_id])?;
            let count = upsert_pages(&tx, site_id, &urls, &source)?;
            tx.commit()?;
            Ok(count)
        })?
    }

    /// Get all pages for a site
    #[tracing::instrument(skip(self), fields(site_id))]
    pub fn get_pages(&self, site_id: i64) -> Result<Vec<PageRecord>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, site_id, url, path, title, last_seen_at, source FROM pages WHERE site_id = ?1 ORDER BY path"
            )?;
            from_row::query_vec::<PageRecord>(&mut stmt, &[&site_id])
        })?
    }

    /// Set the sitemap URL for a site
    #[tracing::instrument(skip(self, sitemap_url), fields(site_id))]
    pub fn set_sitemap_url(&self, site_id: i64, sitemap_url: Option<&str>) -> Result<(), DbError> {
        let sitemap_url = sitemap_url.map(|s| s.to_string());
        self.execute(move |conn| {
            conn.execute(
                "UPDATE sites SET sitemap_url = ?1 WHERE id = ?2",
                params![sitemap_url, site_id],
            )?;
            Ok(())
        })?
    }

    /// Get the sitemap URL for a site
    #[tracing::instrument(skip(self), fields(site_id))]
    pub fn get_sitemap_url(&self, site_id: i64) -> Result<Option<String>, DbError> {
        self.execute(move |conn| {
            let result: Option<String> = conn.query_row(
                "SELECT sitemap_url FROM sites WHERE id = ?1",
                params![site_id],
                |row| row.get(0),
            )?;
            Ok(result)
        })?
    }
}

/// Upsert one import's rows through a single prepared statement.
fn upsert_pages(
    tx: &rusqlite::Transaction<'_>,
    site_id: i64,
    urls: &[String],
    source: &str,
) -> Result<usize, DbError> {
    let mut count = 0;
    let mut upsert = tx.prepare(
        "INSERT INTO pages (site_id, url, path, source, last_seen_at)
         VALUES (:site_id, :url, :path, :source, datetime('now'))
         ON CONFLICT(site_id, url) DO UPDATE SET last_seen_at = datetime('now'), source = :source",
    )?;
    for url_str in urls {
        let path = url::Url::parse(url_str)
            .map(|parsed| parsed.path().to_string())
            .unwrap_or_else(|_| url_str.clone());
        upsert.execute(
            named_params! { ":site_id": site_id, ":url": url_str, ":path": path, ":source": source },
        )?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
#[path = "pages_tests.rs"]
mod tests;
