//! Persist scan routes; canonical semantics live in `sitecmd_engine::scope`.

use super::connected_sites::connected_site;
use super::helpers::lifecycle_env_url;
use super::{Database, DbError};
use rusqlite::{params, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedScanScopeTarget {
    pub project_id: i64,
    pub environment_scope_key: String,
    pub remote_site_id: String,
    pub binding_connected_at: i64,
    pub local_scope_revision: i64,
    pub synced_scope_revision: i64,
}

impl Database {
    /// Resolve a local scan-scope row to the connected site bound to the same
    /// project environment. Ad-hoc and local-only sites return `None`.
    #[tracing::instrument(skip(self), fields(site_id))]
    pub fn connected_scan_scope_target(
        &self,
        site_id: i64,
    ) -> Result<Option<ConnectedScanScopeTarget>, DbError> {
        self.execute(move |conn| {
            let local = conn
                .query_row(
                    "SELECT project_id, url, scope_revision FROM sites WHERE id = ?1",
                    params![site_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<i64>>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()?;
            let Some((Some(project_id), url, local_scope_revision)) = local else {
                return Ok(None);
            };
            let environment_scope_key = lifecycle_env_url(&url);
            let Some(remote) = connected_site(conn, project_id, &environment_scope_key)? else {
                return Ok(None);
            };
            let synced_scope_revision = conn.query_row(
                "SELECT scope_synced_revision FROM connected_sites
                  WHERE project_id = ?1 AND env_url = ?2",
                params![project_id, environment_scope_key],
                |row| row.get(0),
            )?;
            Ok(Some(ConnectedScanScopeTarget {
                project_id,
                environment_scope_key,
                remote_site_id: remote.site_id,
                binding_connected_at: remote.connected_at,
                local_scope_revision,
                synced_scope_revision,
            }))
        })?
    }

    /// Sites whose connected binding has not acknowledged the local scope revision.
    #[tracing::instrument(skip(self))]
    pub fn pending_connected_scan_scope_site_ids(&self) -> Result<Vec<i64>, DbError> {
        self.execute(|conn| {
            let mut statement = conn.prepare(
                "SELECT s.id
                   FROM connected_sites cs
                   JOIN sites s ON s.project_id = cs.project_id
                    AND (s.url = cs.env_url OR s.url = cs.env_url || '/')
                  WHERE s.scope_revision > cs.scope_synced_revision
                  ORDER BY s.id",
            )?;
            let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?
    }

    /// Acknowledge one captured local revision after the remote resource was
    /// observed or written with the same canonical scope. A newer local edit
    /// racing the request remains pending because it has a higher revision.
    #[tracing::instrument(
        skip(self, environment_scope_key, remote_site_id),
        fields(project_id, binding_connected_at, revision)
    )]
    pub fn mark_connected_scan_scope_synced(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        remote_site_id: &str,
        binding_connected_at: i64,
        revision: i64,
    ) -> Result<(), DbError> {
        let env_url = lifecycle_env_url(environment_scope_key);
        let remote_site_id = remote_site_id.to_string();
        self.execute(move |conn| {
            let changed = conn.execute(
                "UPDATE connected_sites
                    SET scope_synced_revision = MAX(scope_synced_revision, :revision)
                  WHERE project_id = :project_id
                    AND env_url = :env_url
                    AND site_id = :site_id
                    AND connected_at = :connected_at",
                rusqlite::named_params! {
                    ":project_id": project_id,
                    ":env_url": env_url,
                    ":site_id": remote_site_id,
                    ":connected_at": binding_connected_at,
                    ":revision": revision,
                },
            )?;
            if changed == 0 {
                let current: Option<(String, i64)> = conn
                    .query_row(
                        "SELECT site_id, connected_at FROM connected_sites
                          WHERE project_id = :project_id AND env_url = :env_url",
                        rusqlite::named_params! {
                            ":project_id": project_id,
                            ":env_url": env_url,
                        },
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                if current.is_some() {
                    return Err(DbError::Other(
                        "cannot acknowledge scope because the connected binding changed".into(),
                    ));
                }
                return Err(DbError::Other(
                    "cannot acknowledge scope for an environment that is not connected".into(),
                ));
            }
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn connected_scan_scope_pending(
        &self,
        project_id: i64,
        environment_scope_key: &str,
    ) -> Result<bool, DbError> {
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| {
            conn.query_row(
                "SELECT EXISTS (
                         SELECT 1 FROM connected_sites cs
                         JOIN sites s ON s.project_id = cs.project_id
                          AND (s.url = cs.env_url OR s.url = cs.env_url || '/')
                        WHERE cs.project_id = ?1 AND cs.env_url = ?2
                          AND s.scope_revision > cs.scope_synced_revision
                     )",
                params![project_id, env_url],
                |row| row.get(0),
            )
            .map_err(DbError::from)
        })?
    }

    /// The routes stored for a site, in the order they were authored.
    /// Empty means no scope has been set yet, which callers read as "the
    /// entry page only" rather than as "nothing".
    #[tracing::instrument(skip(self), fields(site_id))]
    pub fn get_scan_scope_routes(&self, site_id: i64) -> Result<Vec<String>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT route FROM site_scan_scope WHERE site_id = ?1 ORDER BY position",
            )?;
            let rows = stmt.query_map(params![site_id], |row| row.get::<_, String>(0))?;
            let mut routes = Vec::new();
            for row in rows {
                routes.push(row?);
            }
            Ok(routes)
        })?
    }

    /// The revision guard. Advances on every write, so a client that
    /// authored against an older scope can be told its basis is stale
    /// instead of overwriting someone else's edit.
    #[tracing::instrument(skip(self), fields(site_id))]
    pub fn get_scan_scope_revision(&self, site_id: i64) -> Result<i64, DbError> {
        self.execute(move |conn| {
            let revision = conn
                .query_row(
                    "SELECT scope_revision FROM sites WHERE id = ?1",
                    params![site_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0);
            Ok(revision)
        })?
    }

    /// Atomically replaces a site's complete scope and returns its new
    /// revision. Removed routes must not survive as they would under a merge.
    #[tracing::instrument(skip(self, routes), fields(site_id, route_count = routes.len()))]
    pub fn replace_scan_scope(&self, site_id: i64, routes: &[String]) -> Result<i64, DbError> {
        let routes = routes.to_vec();
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            // Do not advance the revision for an unchanged scope.
            let current: Vec<String> = {
                let mut stmt = tx.prepare(
                    "SELECT route FROM site_scan_scope WHERE site_id = ?1 ORDER BY position",
                )?;
                let rows = stmt.query_map(params![site_id], |row| row.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            if current == routes {
                let revision = tx.query_row(
                    "SELECT scope_revision FROM sites WHERE id = ?1",
                    params![site_id],
                    |row| row.get::<_, i64>(0),
                )?;
                return Ok(revision);
            }
            tx.execute(
                "DELETE FROM site_scan_scope WHERE site_id = ?1",
                params![site_id],
            )?;
            for (position, route) in routes.iter().enumerate() {
                tx.execute(
                    "INSERT INTO site_scan_scope (site_id, route, position) VALUES (?1, ?2, ?3)",
                    params![site_id, route, position as i64],
                )?;
            }
            tx.execute(
                "UPDATE sites
                    SET scope_revision = scope_revision + 1,
                        scope_updated_at = datetime('now')
                  WHERE id = ?1",
                params![site_id],
            )?;
            let revision = tx.query_row(
                "SELECT scope_revision FROM sites WHERE id = ?1",
                params![site_id],
                |row| row.get::<_, i64>(0),
            )?;
            tx.commit()?;
            Ok(revision)
        })?
    }
}

/// Resolve the stored scan scope, defaulting to the entry URL.
pub fn scan_scope_urls(db: &Database, site_url: &str) -> Vec<String> {
    scan_scope_urls_for_scope(db, None, site_url)
}

/// The URLs an explicitly selected project environment should cover.
pub fn scan_scope_urls_for_project(db: &Database, project_id: i64, site_url: &str) -> Vec<String> {
    scan_scope_urls_for_scope(db, Some(project_id), site_url)
}

fn scan_scope_urls_for_scope(
    db: &Database,
    project_id: Option<i64>,
    site_url: &str,
) -> Vec<String> {
    let fallback = || vec![site_url.to_string()];
    let Ok(entry) = url::Url::parse(site_url) else {
        return fallback();
    };
    let site_id = match project_id {
        Some(project_id) => db.get_or_create_site_for_project(project_id, site_url),
        None => db.get_or_create_site(site_url),
    };
    let Ok(site_id) = site_id else {
        return fallback();
    };
    let routes = db.get_scan_scope_routes(site_id).unwrap_or_default();
    if routes.is_empty() {
        return fallback();
    }
    let canonical: Vec<sitecmd_engine::route::CanonicalRoute> = routes
        .into_iter()
        .map(|route| sitecmd_engine::route::CanonicalRoute::new(route, false))
        .collect();
    let urls = sitecmd_engine::scope::scope_urls(&entry, &canonical);
    if urls.is_empty() {
        fallback()
    } else {
        urls
    }
}

#[cfg(test)]
#[path = "scan_scope_tests.rs"]
mod tests;
