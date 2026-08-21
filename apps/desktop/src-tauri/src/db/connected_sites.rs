//! Connected site bindings and per-group pulled revisions.
//!
//! Bindings never store credentials; installation tokens remain in the OS keychain.

use rusqlite::{params, Connection, OptionalExtension};

use super::connected_producer::raise_site_event_watermark;
use super::helpers::lifecycle_env_url;
use super::{Database, DbError};

/// A project environment's binding to a connected site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedSite {
    pub site_id: String,
    pub connected_at: i64,
    /// When the bootstrap submission committed, or `None` while it is still
    /// owed. See [`ConnectedSite::accepts_mutations`].
    pub bootstrapped_at: Option<i64>,
    /// The epoch of the fingerprint key in the keychain's current slot.
    pub fingerprint_key_version: i64,
    /// The version this desktop claimed and has not completed, matching a
    /// candidate key in the keychain's pending slot. `None` when no rotation
    /// is in flight from this machine.
    pub pending_key_version: Option<i64>,
}

/// Local project environment bound to an opaque connected-site id. Sites bound
/// only on another machine do not resolve here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedSiteBinding {
    pub site_id: String,
    pub project_id: i64,
    pub project_name: String,
    pub env_url: String,
}

impl ConnectedSite {
    /// Whether lifecycle changes should emit mutations instead of relying on bootstrap.
    pub fn accepts_mutations(&self) -> bool {
        self.bootstrapped_at.is_some()
    }
}

pub(super) fn connected_site(
    conn: &Connection,
    project_id: i64,
    env_url: &str,
) -> Result<Option<ConnectedSite>, DbError> {
    Ok(conn
        .query_row(
            "SELECT site_id, connected_at, bootstrapped_at,
                    fingerprint_key_version, pending_key_version
               FROM connected_sites
              WHERE project_id = ?1 AND env_url = ?2",
            params![project_id, env_url],
            |row| {
                Ok(ConnectedSite {
                    site_id: row.get(0)?,
                    connected_at: row.get(1)?,
                    bootstrapped_at: row.get(2)?,
                    fingerprint_key_version: row.get(3)?,
                    pending_key_version: row.get(4)?,
                })
            },
        )
        .optional()?)
}

/// Return the pulled group revision, defaulting unknown groups to genesis 0.
pub(super) fn group_revision(
    conn: &Connection,
    project_id: i64,
    env_url: &str,
    check_id: &str,
) -> Result<i64, DbError> {
    Ok(conn
        .query_row(
            "SELECT state_revision FROM connected_group_revisions
             WHERE project_id = ?1 AND env_url = ?2 AND check_id = ?3",
            params![project_id, env_url, check_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0))
}

/// Record a group revision this installation has learned, never lowering the
/// stored one. A reordered or replayed read that lowered it would make the
/// next decision guard against state older than what the user was shown.
pub(super) fn raise_group_revision(
    conn: &Connection,
    project_id: i64,
    env_url: &str,
    check_id: &str,
    state_revision: i64,
    pulled_at: i64,
) -> Result<i64, DbError> {
    conn.execute(
        "INSERT INTO connected_group_revisions
            (project_id, env_url, check_id, state_revision, pulled_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(project_id, env_url, check_id) DO UPDATE SET
             state_revision = excluded.state_revision,
             pulled_at = excluded.pulled_at
         WHERE excluded.state_revision > connected_group_revisions.state_revision",
        params![project_id, env_url, check_id, state_revision, pulled_at],
    )?;
    group_revision(conn, project_id, env_url, check_id)
}

impl Database {
    /// Bind an environment to a site. Require disconnect before rebinding so
    /// site-scoped revisions and pending decisions cannot cross streams.
    #[tracing::instrument(skip(self, environment_scope_key, site_id), fields(project_id))]
    pub fn connect_site(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        site_id: &str,
        now_ms: i64,
    ) -> Result<(), DbError> {
        if site_id.is_empty() {
            return Err("a connected site needs a site id".into());
        }
        let env_url = lifecycle_env_url(environment_scope_key);
        if env_url.is_empty() {
            return Err("an environment is required to connect a site".into());
        }
        let site_id = site_id.to_string();
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO connected_sites (project_id, env_url, site_id, connected_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(project_id, env_url) DO NOTHING",
                params![project_id, env_url, site_id, now_ms],
            )?;
            let bound = connected_site(&tx, project_id, &env_url)?
                .ok_or_else(|| DbError::Other("connection row vanished".to_string()))?;
            if bound.site_id != site_id {
                return Err(DbError::Other(format!(
                    "this environment is already connected to {}; disconnect it first",
                    bound.site_id
                )));
            }
            tx.commit()?;
            Ok(())
        })?
    }

    /// Record the first committed bootstrap for an environment.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn mark_site_bootstrapped(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        now_ms: i64,
    ) -> Result<(), DbError> {
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| {
            let changed = conn.execute(
                "UPDATE connected_sites
                    SET bootstrapped_at = COALESCE(bootstrapped_at, ?3)
                  WHERE project_id = ?1 AND env_url = ?2",
                params![project_id, env_url, now_ms],
            )?;
            if changed == 0 {
                return Err(DbError::Other(
                    "cannot bootstrap an environment that is not connected".to_string(),
                ));
            }
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn get_connected_site(
        &self,
        project_id: i64,
        environment_scope_key: &str,
    ) -> Result<Option<ConnectedSite>, DbError> {
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| connected_site(conn, project_id, &env_url))?
    }

    /// Return every local connected-site binding for batch alert resolution.
    #[tracing::instrument(skip(self))]
    pub fn connected_site_bindings(&self) -> Result<Vec<ConnectedSiteBinding>, DbError> {
        self.execute(|conn| {
            let mut statement = conn.prepare(
                "SELECT cs.site_id, cs.project_id, p.name, cs.env_url
                   FROM connected_sites cs
                   JOIN projects p ON p.id = cs.project_id",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(ConnectedSiteBinding {
                    site_id: row.get(0)?,
                    project_id: row.get(1)?,
                    project_name: row.get(2)?,
                    env_url: row.get(3)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?
    }

    /// Record a rotation claim this desktop made: the candidate's version,
    /// beside the current one. Refused while another claim is already pending
    /// locally - the keychain's pending slot holds one candidate.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn claim_pending_key_rotation(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        version: i64,
    ) -> Result<(), DbError> {
        if version < 2 {
            return Err("a rotation claims version 2 or later".into());
        }
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| {
            let changed = conn.execute(
                "UPDATE connected_sites SET pending_key_version = ?3
                  WHERE project_id = ?1 AND env_url = ?2 AND pending_key_version IS NULL",
                params![project_id, env_url, version],
            )?;
            if changed == 0 {
                return Err(DbError::Other(
                    "this environment already holds a pending key rotation or is not connected"
                        .to_string(),
                ));
            }
            Ok(())
        })?
    }

    /// Clear a pending claim without completing it: the abort path, and the
    /// convergent cleanup when the service reports the claim expired.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn clear_pending_key_rotation(
        &self,
        project_id: i64,
        environment_scope_key: &str,
    ) -> Result<(), DbError> {
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| {
            conn.execute(
                "UPDATE connected_sites SET pending_key_version = NULL
                  WHERE project_id = ?1 AND env_url = ?2",
                params![project_id, env_url],
            )?;
            Ok(())
        })?
    }

    /// The completion the service just committed: the pending version becomes
    /// current. Gated on the version matching the recorded claim, so a stale
    /// caller cannot promote a version this desktop never claimed.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn complete_key_rotation(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        version: i64,
    ) -> Result<(), DbError> {
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| {
            let changed = conn.execute(
                "UPDATE connected_sites
                    SET fingerprint_key_version = ?3, pending_key_version = NULL
                  WHERE project_id = ?1 AND env_url = ?2 AND pending_key_version = ?3",
                params![project_id, env_url, version],
            )?;
            if changed == 0 {
                return Err(DbError::Other(
                    "no matching pending key rotation to complete".to_string(),
                ));
            }
            Ok(())
        })?
    }

    /// Adopt the key version an imported connection reports; the import path
    /// stores the matching key bytes in the keychain beside it.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn set_fingerprint_key_version(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        version: i64,
    ) -> Result<(), DbError> {
        if version < 1 {
            return Err("a fingerprint key version starts at 1".into());
        }
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| {
            let changed = conn.execute(
                "UPDATE connected_sites SET fingerprint_key_version = ?3
                  WHERE project_id = ?1 AND env_url = ?2",
                params![project_id, env_url, version],
            )?;
            if changed == 0 {
                return Err(DbError::Other(
                    "cannot set a key version on an environment that is not connected".to_string(),
                ));
            }
            Ok(())
        })?
    }

    /// Removes a site binding with its revisions, undelivered decisions, and
    /// event watermark so a later binding cannot inherit stale stream state.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn disconnect_site(
        &self,
        project_id: i64,
        environment_scope_key: &str,
    ) -> Result<(), DbError> {
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            // Revisions and outbox rows cascade from the binding.
            tx.execute(
                "DELETE FROM connected_sites WHERE project_id = ?1 AND env_url = ?2",
                params![project_id, env_url],
            )?;
            tx.execute(
                "DELETE FROM connected_site_watermarks WHERE project_id = ?1 AND env_url = ?2",
                params![project_id, env_url],
            )?;
            tx.commit()?;
            Ok(())
        })?
    }

    /// Record a group revision read from the service, returning the revision
    /// in force afterwards.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id, check_id = %check_id))]
    pub fn record_pulled_group_revision(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        check_id: &str,
        state_revision: i64,
        pulled_at: i64,
    ) -> Result<i64, DbError> {
        if state_revision < 0 {
            return Err("a group revision cannot be negative".into());
        }
        let env_url = lifecycle_env_url(environment_scope_key);
        let check_id = check_id.to_string();
        self.execute(move |conn| {
            raise_group_revision(
                conn,
                project_id,
                &env_url,
                &check_id,
                state_revision,
                pulled_at,
            )
        })?
    }

    /// Atomically persist a pull's event watermark and group revisions.
    #[tracing::instrument(
        skip(self, environment_scope_key, group_revisions),
        fields(project_id, group_count = group_revisions.len())
    )]
    pub fn record_connected_pull(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        event_sequence: i64,
        group_revisions: Vec<(String, i64)>,
        pulled_at: i64,
    ) -> Result<(), DbError> {
        if event_sequence < 0 {
            return Err("a pulled event sequence cannot be negative".into());
        }
        if group_revisions
            .iter()
            .any(|(check_id, revision)| check_id.trim().is_empty() || *revision < 0)
        {
            return Err("a pulled group needs a check id and non-negative revision".into());
        }
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            for (check_id, revision) in group_revisions {
                raise_group_revision(&tx, project_id, &env_url, &check_id, revision, pulled_at)?;
            }
            raise_site_event_watermark(&tx, project_id, &env_url, event_sequence, pulled_at)?;
            tx.commit()?;
            Ok(())
        })?
    }
}

#[cfg(test)]
#[path = "connected_sites_tests.rs"]
mod tests;
