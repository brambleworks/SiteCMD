//! Pages (sitemap) CRUD.

use super::DbError;
use rusqlite::{named_params, params};

use super::from_row;
use super::types::PageRecord;
use super::Database;

impl Database {
    /// Save discovered pages for a site (upsert - updates last_seen_at on conflict)
    #[tracing::instrument(skip(self, urls), fields(site_id, source = %source))]
    pub fn save_pages(
        &self,
        site_id: i64,
        urls: &[String],
        source: &str,
    ) -> Result<usize, DbError> {
        let urls = urls.to_vec();
        let source = source.to_string();
        self.execute(move |conn| {
            let mut count = 0;
            for url_str in &urls {
                let path = url::Url::parse(url_str)
                    .map(|u| u.path().to_string())
                    .unwrap_or_else(|_| url_str.clone());
                conn.execute(
                    "INSERT INTO pages (site_id, url, path, source, last_seen_at)
                     VALUES (:site_id, :url, :path, :source, datetime('now'))
                     ON CONFLICT(site_id, url) DO UPDATE SET last_seen_at = datetime('now'), source = :source",
                    named_params! { ":site_id": site_id, ":url": url_str, ":path": path, ":source": source },
                )?;
                count += 1;
            }
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

    /// Clear all pages for a site
    #[tracing::instrument(skip(self), fields(site_id))]
    pub fn clear_pages(&self, site_id: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute("DELETE FROM pages WHERE site_id = ?1", params![site_id])?;
            Ok(())
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
