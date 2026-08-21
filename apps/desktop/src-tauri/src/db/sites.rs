//! Per-project site rows that key scan history. URL joins must also apply the
//! shared project-scope predicate.

use super::DbError;
use rusqlite::{params, OptionalExtension};

use super::helpers::{normalize_url, site_project_scope_predicate};
use super::Database;

impl Database {
    /// Get or create a site using project identity to disambiguate shared URLs.
    #[tracing::instrument(skip(self, url), fields(project_id))]
    pub fn get_or_create_site_for_project(
        &self,
        project_id: i64,
        url: &str,
    ) -> Result<i64, DbError> {
        let url = url.to_string();
        self.execute(move |conn| {
            let (normalized, url_slash) = normalize_url(&url);
            let owns_environment = conn
                .query_row(
                    "SELECT 1 FROM environments
                     WHERE project_id = ?1 AND (url = ?2 OR url = ?3)
                     LIMIT 1",
                    params![project_id, normalized, url_slash],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !owns_environment {
                return Err(DbError::Other(
                    "Site URL is not an environment of the selected project".to_string(),
                ));
            }

            let existing = conn
                .query_row(
                    "SELECT id FROM sites
                     WHERE project_id = ?1 AND (url = ?2 OR url = ?3)",
                    params![project_id, normalized, url_slash],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(id) = existing {
                return Ok(id);
            }

            conn.execute(
                "INSERT INTO sites (project_id, url) VALUES (?1, ?2)",
                params![project_id, normalized],
            )?;
            Ok(conn.last_insert_rowid())
        })?
    }

    /// Get or create a site row for `url`, scoped to the project whose
    /// environment matches the URL (NULL scope when no project matches).
    #[tracing::instrument(skip(self, url))]
    pub fn get_or_create_site(&self, url: &str) -> Result<i64, DbError> {
        let url = url.to_string();
        self.execute(move |conn| {
            let (normalized, url_slash) = normalize_url(&url);

            let existing: Option<i64> = conn
                .query_row(
                    &format!(
                        "SELECT id FROM sites si WHERE (si.url = ?1 OR si.url = ?2) AND {}",
                        site_project_scope_predicate(1, 2)
                    ),
                    params![normalized, url_slash],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(id) = existing {
                return Ok(id);
            }

            conn.execute(
                "INSERT INTO sites (project_id, url)
                 VALUES ((SELECT project_id FROM environments
                          WHERE url = ?1 OR url = ?2
                          ORDER BY id ASC LIMIT 1), ?1)",
                params![normalized, url_slash],
            )?;

            Ok(conn.last_insert_rowid())
        })?
    }
}
