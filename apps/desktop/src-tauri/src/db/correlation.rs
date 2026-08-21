//! Correlation observations, baselines, enrichment caches, and pattern persistence.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, OptionalExtension};

use super::Database;
use super::DbError;

/// Summed `(co_resolved, co_active)` counts keyed by
/// `(cause_check_id, effect_check_id)`.
pub type CausalLinkObservationCounts = HashMap<(String, String), (u32, u32)>;

/// One (cause, effect) co-occurrence observation written after a scan window.
#[derive(Debug, Clone)]
pub struct CausalLinkObservationInput {
    pub cause_check_id: String,
    pub effect_check_id: String,
    pub observed_at_ms: i64,
    pub co_active: u32,
    pub co_resolved: u32,
    pub resolution_event_id: Option<i64>,
}

impl Database {
    /// Insert a batch of causal-link observations in one transaction.
    #[tracing::instrument(skip(self, rows), fields(project_id, row_count = rows.len()))]
    pub fn insert_causal_link_observations(
        &self,
        project_id: i64,
        rows: Vec<CausalLinkObservationInput>,
    ) -> Result<(), DbError> {
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO causal_link_observations
                       (project_id, cause_check_id, effect_check_id, observed_at, co_active, co_resolved, resolution_event_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )?;
                for row in &rows {
                    stmt.execute(params![
                        project_id,
                        row.cause_check_id,
                        row.effect_check_id,
                        row.observed_at_ms,
                        row.co_active,
                        row.co_resolved,
                        row.resolution_event_id
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })?
    }

    /// Summed `(co_resolved, co_active)` observation counts per
    /// (cause_check_id, effect_check_id) pair for a project.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_causal_link_observation_counts(
        &self,
        project_id: i64,
    ) -> Result<CausalLinkObservationCounts, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT cause_check_id, effect_check_id,
                        COALESCE(SUM(co_resolved), 0) AS resolved,
                        COALESCE(SUM(co_active),   0) AS active
                 FROM causal_link_observations
                 WHERE project_id = ?1
                 GROUP BY cause_check_id, effect_check_id",
            )?;
            let rows = stmt.query_map(params![project_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u32>(2)?,
                    r.get::<_, u32>(3)?,
                ))
            })?;
            let mut out = HashMap::new();
            for row in rows {
                let (cause, effect, resolved, active) = row?;
                out.insert((cause, effect), (resolved, active));
            }
            Ok(out)
        })?
    }

    /// `(ts_ms, value)` history points for a signal at or after `since_ms`,
    /// oldest first.
    #[tracing::instrument(skip(self, signal_key), fields(project_id, since_ms))]
    pub fn get_signal_history(
        &self,
        project_id: i64,
        signal_key: &str,
        since_ms: i64,
    ) -> Result<Vec<(i64, f64)>, DbError> {
        let signal_key = signal_key.to_string();
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT ts_ms, value FROM signal_history
                 WHERE project_id = ?1 AND signal_key = ?2 AND ts_ms >= ?3
                 ORDER BY ts_ms ASC",
            )?;
            let rows = stmt.query_map(params![project_id, signal_key, since_ms], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?
    }

    /// Most recent `(ts_ms, value)` history point for a signal, if any.
    #[tracing::instrument(skip(self, signal_key), fields(project_id))]
    pub fn get_latest_signal_point(
        &self,
        project_id: i64,
        signal_key: &str,
    ) -> Result<Option<(i64, f64)>, DbError> {
        let signal_key = signal_key.to_string();
        self.execute(move |conn| {
            conn.query_row(
                "SELECT ts_ms, value FROM signal_history
                 WHERE project_id = ?1 AND signal_key = ?2
                 ORDER BY ts_ms DESC LIMIT 1",
                params![project_id, signal_key],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?)),
            )
            .optional()
            .map_err(DbError::from)
        })?
    }

    /// Write (or replace) the computed baseline for a signal.
    #[tracing::instrument(skip(self, signal_key), fields(project_id, window_days))]
    pub fn upsert_signal_baseline(
        &self,
        project_id: i64,
        signal_key: &str,
        window_days: i64,
        mean: f64,
        stddev: f64,
        sample_count: i64,
        updated_at_ms: i64,
    ) -> Result<(), DbError> {
        let signal_key = signal_key.to_string();
        self.execute(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO signal_baselines
                   (project_id, signal_key, window_days, mean, stddev, sample_count, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    project_id,
                    signal_key,
                    window_days,
                    mean,
                    stddev,
                    sample_count,
                    updated_at_ms
                ],
            )?;
            Ok(())
        })?
    }

    /// `(mean, stddev, sample_count)` baseline for a signal, if one exists.
    #[tracing::instrument(skip(self, signal_key), fields(project_id, window_days))]
    pub fn get_signal_baseline(
        &self,
        project_id: i64,
        signal_key: &str,
        window_days: i64,
    ) -> Result<Option<(f64, f64, i64)>, DbError> {
        let signal_key = signal_key.to_string();
        self.execute(move |conn| {
            conn.query_row(
                "SELECT mean, stddev, sample_count FROM signal_baselines
                 WHERE project_id = ?1 AND signal_key = ?2 AND window_days = ?3",
                params![project_id, signal_key, window_days],
                |r| {
                    Ok((
                        r.get::<_, f64>(0)?,
                        r.get::<_, f64>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(DbError::from)
        })?
    }

    /// Enrichment cache rows refreshed at or after `cutoff_ms`, as
    /// `(integration, signal_key, payload_json)` tuples.
    #[tracing::instrument(skip(self), fields(project_id, cutoff_ms))]
    pub fn get_fresh_enrichment_payloads(
        &self,
        project_id: i64,
        cutoff_ms: i64,
    ) -> Result<Vec<(String, String, String)>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT integration, signal_key, payload_json
                 FROM integration_enrichment_cache
                 WHERE project_id = ?1 AND refreshed_at >= ?2",
            )?;
            let rows = stmt.query_map(params![project_id, cutoff_ms], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?
    }

    /// Write (or replace) one integration enrichment cache payload.
    #[tracing::instrument(skip(self, integration, signal_key, payload_json), fields(project_id))]
    pub fn upsert_enrichment_cache_payload(
        &self,
        project_id: i64,
        integration: &str,
        signal_key: &str,
        payload_json: &str,
        refreshed_at_ms: i64,
    ) -> Result<(), DbError> {
        let integration = integration.to_string();
        let signal_key = signal_key.to_string();
        let payload_json = payload_json.to_string();
        self.execute(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO integration_enrichment_cache
                   (project_id, integration, signal_key, payload_json, refreshed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    project_id,
                    integration,
                    signal_key,
                    payload_json,
                    refreshed_at_ms
                ],
            )?;
            Ok(())
        })?
    }

    /// Insert `(work_item_id, payload_json)` historical enrichment rows for
    /// one integration in a single transaction.
    #[tracing::instrument(skip(self, integration, rows), fields(row_count = rows.len()))]
    pub fn insert_historical_enrichments(
        &self,
        integration: &str,
        rows: Vec<(i64, String)>,
        created_at_ms: i64,
    ) -> Result<(), DbError> {
        let integration = integration.to_string();
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT OR REPLACE INTO historical_enrichments
                       (work_item_id, integration, payload, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                )?;
                for (work_item_id, payload) in &rows {
                    stmt.execute(params![work_item_id, integration, payload, created_at_ms])?;
                }
            }
            tx.commit()?;
            Ok(())
        })?
    }

    /// `(work_item_id, check_id)` for items first seen at or after `cutoff_ms`.
    #[tracing::instrument(skip(self), fields(project_id, cutoff_ms))]
    pub fn get_recent_work_item_check_ids(
        &self,
        project_id: i64,
        cutoff_ms: i64,
    ) -> Result<Vec<(i64, String)>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, check_id FROM work_items
                 WHERE project_id = ?1 AND first_seen_at >= ?2",
            )?;
            let rows = stmt.query_map(params![project_id, cutoff_ms], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?
    }

    /// Distinct check_ids with an unresolved work item for the project.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_active_check_ids(&self, project_id: i64) -> Result<HashSet<String>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT check_id FROM work_items \
                 WHERE project_id = ?1 AND resolved_at IS NULL",
            )?;
            let rows = stmt.query_map(params![project_id], |r| r.get::<_, String>(0))?;
            Ok(rows.collect::<Result<HashSet<_>, _>>()?)
        })?
    }

    /// Earliest non-production `first_seen_at` per check_id inside the window,
    /// excluding the current environment URL.
    #[tracing::instrument(skip(self, current_env_url), fields(project_id, cutoff_ms))]
    pub fn get_nonprod_first_seen_by_check(
        &self,
        project_id: i64,
        current_env_url: &str,
        cutoff_ms: i64,
    ) -> Result<Vec<(String, i64)>, DbError> {
        let current_env_url = current_env_url.to_string();
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT wi.check_id, MIN(wi.first_seen_at)
                 FROM work_items wi
                 INNER JOIN environments e
                   ON e.url = wi.env_url AND e.project_id = wi.project_id
                 WHERE wi.project_id = ?1
                   AND wi.env_url != ?2
                   AND e.environment != 'production'
                   AND wi.first_seen_at >= ?3
                 GROUP BY wi.check_id",
            )?;
            let rows = stmt.query_map(params![project_id, current_env_url, cutoff_ms], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?
    }

    /// `(check_id, distinct project count, latest first_seen_at)` across all
    /// projects except the current one, inside the window.
    #[tracing::instrument(skip(self), fields(current_project_id, cutoff_ms))]
    pub fn get_cross_project_check_counts(
        &self,
        current_project_id: i64,
        cutoff_ms: i64,
    ) -> Result<Vec<(String, i64, i64)>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT check_id, COUNT(DISTINCT project_id), MAX(first_seen_at)
                 FROM work_items
                 WHERE project_id != ?1
                   AND first_seen_at >= ?2
                 GROUP BY check_id",
            )?;
            let rows = stmt.query_map(params![current_project_id, cutoff_ms], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?
    }

    /// Rebuild the cross-project pattern index from work items first seen at
    /// or after `cutoff_ms`. Delete + repopulate commit atomically.
    #[tracing::instrument(skip(self), fields(cutoff_ms))]
    pub fn rebuild_cross_project_pattern_index(
        &self,
        cutoff_ms: i64,
        updated_at_ms: i64,
    ) -> Result<(), DbError> {
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM cross_project_pattern_index", [])?;
            tx.execute(
                "INSERT INTO cross_project_pattern_index
                     (check_id, project_count, latest_seen_ms, updated_at)
                 SELECT check_id, COUNT(DISTINCT project_id), MAX(first_seen_at), ?2
                 FROM work_items
                 WHERE first_seen_at >= ?1
                 GROUP BY check_id",
                params![cutoff_ms, updated_at_ms],
            )?;
            tx.commit()?;
            Ok(())
        })?
    }
}
