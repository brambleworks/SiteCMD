//! Retention for append-only and resolved records outside scan-history pruning.
//!
//! User-managed reports, fix attempts, regressions, and bounded signal baselines
//! are intentionally retained.

use super::Database;
use super::DbError;
use rusqlite::params;

/// Rows removed by one retention sweep, for logging.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RetentionStats {
    pub dismissed_alerts: usize,
    pub old_events: usize,
    pub resolved_signal_items: usize,
    pub abandoned_scan_executions: usize,
    pub old_signal_history: usize,
    pub old_causal_observations: usize,
    pub old_score_snapshots: usize,
}

impl RetentionStats {
    pub fn total(&self) -> usize {
        self.dismissed_alerts
            + self.old_events
            + self.resolved_signal_items
            + self.abandoned_scan_executions
            + self.old_signal_history
            + self.old_causal_observations
            + self.old_score_snapshots
    }
}

impl Database {
    /// Delete aged rows per the retention windows in `constants.rs`. Runs in
    /// one transaction on the worker thread; called at startup and daily.
    #[tracing::instrument(skip(self), fields(now_ms))]
    pub fn run_retention_sweep(&self, now_ms: i64) -> Result<RetentionStats, DbError> {
        let alert_cutoff_ms =
            now_ms - crate::constants::DISMISSED_ALERT_RETENTION_DAYS * 24 * 60 * 60 * 1000;
        let resolved_cutoff_ms =
            now_ms - crate::constants::RESOLVED_SIGNAL_RETENTION_DAYS * 24 * 60 * 60 * 1000;
        let event_cutoff_ms = now_ms - crate::constants::EVENT_RETENTION_DAYS * 24 * 60 * 60 * 1000;
        let execution_cutoff_ms = now_ms
            - crate::constants::ABANDONED_SCAN_EXECUTION_RETENTION_DAYS * 24 * 60 * 60 * 1000;
        let signal_history_cutoff_ms =
            now_ms - crate::constants::SIGNAL_HISTORY_RETENTION_DAYS * 24 * 60 * 60 * 1000;
        let observation_cutoff_ms =
            now_ms - crate::constants::CAUSAL_OBSERVATION_RETENTION_DAYS * 24 * 60 * 60 * 1000;
        let score_snapshot_cutoff_ms =
            now_ms - crate::constants::SCORE_SNAPSHOT_RETENTION_DAYS * 24 * 60 * 60 * 1000;

        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            let dismissed_alerts = tx.execute(
                "DELETE FROM alerts
                     WHERE dismissed_at IS NOT NULL AND dismissed_at < ?1",
                params![alert_cutoff_ms],
            )?;
            let old_events = tx.execute(
                "DELETE FROM events WHERE occurred_at_ms < ?1",
                params![event_cutoff_ms],
            )?;
            // Only signal-sourced rows (scan_ref IS NULL: psi/gsc/updates/
            // uptimerobot/cloudflare/site adapters). Scan-anchored rows are
            // already pruned with their scans, and active rows always stay.
            let resolved_signal_items = tx.execute(
                "DELETE FROM work_items
                     WHERE scan_ref IS NULL
                       AND resolved_at IS NOT NULL
                       AND resolved_at < ?1",
                params![resolved_cutoff_ms],
            )?;
            // Terminal history is governed by execution-count retention. This
            // sweep only releases abandoned plans/runs left by a crashed app.
            let abandoned_scan_executions = tx.execute(
                "DELETE FROM scan_executions
                     WHERE started_at < ?1
                       AND status IN ('planned', 'running')",
                params![execution_cutoff_ms],
            )?;
            let old_signal_history = tx.execute(
                "DELETE FROM signal_history WHERE ts_ms < ?1",
                params![signal_history_cutoff_ms],
            )?;
            let old_causal_observations = tx.execute(
                "DELETE FROM causal_link_observations WHERE observed_at < ?1",
                params![observation_cutoff_ms],
            )?;
            // Keep each series' latest write-on-change row as its current state.
            let old_score_snapshots = tx.execute(
                "DELETE FROM score_snapshots
                     WHERE computed_at < ?1
                       AND id NOT IN (
                           SELECT MAX(id) FROM score_snapshots
                           GROUP BY project_id, environment_url
                       )",
                params![score_snapshot_cutoff_ms],
            )?;
            tx.commit()?;
            Ok(RetentionStats {
                dismissed_alerts,
                old_events,
                resolved_signal_items,
                abandoned_scan_executions,
                old_signal_history,
                old_causal_observations,
                old_score_snapshots,
            })
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::temp_db;

    fn seed(db: &Database, project_id: i64, now_ms: i64) {
        let day_ms = 24 * 60 * 60 * 1000;
        db.execute(move |conn| {
            // Alerts: one freshly dismissed, one anciently dismissed, one active.
            conn.execute_batch(&format!(
                "INSERT INTO alerts (project_id, env_url, source, alert_id, severity, title, description, occurred_at, first_seen_at, last_seen_at, dismissed_at)
                 VALUES ({p}, '', 'web_scan', 'fresh', 'high', 't', 'd', {now}, {now}, {now}, {now}),
                        ({p}, '', 'web_scan', 'old', 'high', 't', 'd', {old}, {old}, {old}, {old}),
                        ({p}, '', 'web_scan', 'active', 'high', 't', 'd', {old}, {old}, {old}, NULL);
                 INSERT INTO events (project_id, event_type, severity, occurred_at_ms, title, summary, source)
                 VALUES ({p}, 'scan', 'info', {old}, 'ancient', '', 'internal'),
                        ({p}, 'scan', 'info', {future}, 'future', '', 'internal');
                 INSERT INTO work_items (project_id, env_url, source, signal_id, check_id, category, severity, title, description, first_seen_at, last_seen_at, resolved_at, scan_ref)
                 VALUES ({p}, 'https://example.com', 'uptimerobot', 'u:1', 'uptime.down', 'monitoring', 'high', 't', 'd', {old}, {old}, {old}, NULL),
                        ({p}, 'https://example.com', 'uptimerobot', 'u:2', 'uptime.down', 'monitoring', 'high', 't', 'd', {now}, {now}, {now}, NULL),
                        ({p}, 'https://example.com', 'uptimerobot', 'u:3', 'uptime.down', 'monitoring', 'high', 't', 'd', {old}, {old}, NULL, NULL);
                 INSERT INTO signal_history (project_id, signal_key, ts_ms, value)
                 VALUES ({p}, 'traffic', {old}, 1.0),
                        ({p}, 'traffic', {now}, 2.0);
                 INSERT INTO causal_link_observations (project_id, cause_check_id, effect_check_id, observed_at, co_active, co_resolved)
                 VALUES ({p}, 'a', 'b', {ancient}, 1, 1),
                        ({p}, 'a', 'b', {now}, 1, 1);
                 INSERT INTO score_snapshots (project_id, environment_url, overall, critical_count, high_count, medium_count, low_count, exploitable_capped, computed_at)
                 VALUES ({p}, 'https://example.com', 90.0, 0, 1, 0, 0, 0, {ancient}),
                        ({p}, 'https://example.com', 85.0, 0, 2, 0, 0, 0, {now}),
                        ({p}, 'https://stable.example.com', 88.0, 0, 1, 1, 0, 0, {ancient});",
                p = project_id,
                now = now_ms,
                old = now_ms - 400 * day_ms,
                ancient = now_ms - 500 * day_ms,
                future = now_ms + 400 * day_ms,
            ))
            .map_err(|e| e.to_string())
        })
        .expect("db op")
        .expect("seed");
    }

    #[test]
    fn retention_sweep_deletes_only_aged_rows() {
        let db = temp_db();
        let project_id = db
            .upsert_project("retention", "/tmp/retention", None)
            .expect("project");
        let now_ms = chrono::Utc::now().timestamp_millis();
        seed(&db, project_id, now_ms);

        // Executions: one aged abandoned plan, one aged completed row, and one
        // fresh plan. Only the abandoned execution is swept.
        db.execute(move |conn| {
            let old = now_ms - 400 * 24 * 60 * 60 * 1000;
            conn.execute_batch(&format!(
                "INSERT INTO scan_executions (
                    project_id, environment_url, environment_scope_key,
                    requested_mode, trigger, admission_class, status,
                    idempotency_key, request_fingerprint, started_at,
                    completed_at, web_status
                 ) VALUES
                    ({project_id}, 'https://example.com', 'https://example.com',
                     'web', 'manual', 'general_scan', 'planned', 'old-plan',
                     'v1:old-plan', {old}, NULL, 'planned'),
                    ({project_id}, 'https://example.com', 'https://example.com',
                     'web', 'manual', 'general_scan', 'complete', 'old-complete',
                     'v1:old-complete', {old}, {old}, 'complete'),
                    ({project_id}, 'https://example.com', 'https://example.com',
                     'web', 'manual', 'general_scan', 'planned', 'fresh-plan',
                     'v1:fresh-plan', {now_ms}, NULL, 'planned');"
            ))
            .map_err(|e| e.to_string())
        })
        .expect("db op")
        .expect("seed executions");

        let stats = db.run_retention_sweep(now_ms).expect("sweep");
        assert_eq!(
            stats,
            RetentionStats {
                dismissed_alerts: 1,
                old_events: 1,
                resolved_signal_items: 1,
                abandoned_scan_executions: 1,
                old_signal_history: 1,
                old_causal_observations: 1,
                old_score_snapshots: 1,
            },
            "exactly one aged row per swept store; terminal/fresh rows stay"
        );

        let counts = db
            .execute(|conn| {
                let count = |table: &str| -> Result<i64, String> {
                    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                        .map_err(|e| e.to_string())
                };
                Ok::<_, String>((
                    count("alerts")?,
                    count("events")?,
                    count("work_items")?,
                    count("scan_executions")?,
                    count("signal_history")?,
                    count("causal_link_observations")?,
                    count("score_snapshots")?,
                ))
            })
            .expect("db op")
            .expect("counts");
        assert_eq!(counts, (2, 1, 2, 2, 1, 1, 2));

        // Idempotent: nothing left to remove.
        let again = db.run_retention_sweep(now_ms).expect("second sweep");
        assert_eq!(again.total(), 0);
    }
}
