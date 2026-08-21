//! Write-on-change history for the live SiteCMD score.
//!
//! Rows are keyed by project and environment and pruned by daily retention.

use rusqlite::{named_params, OptionalExtension};

use super::helpers::{normalize_env_url, normalize_url};
use super::types::ScoreSnapshotPoint;
use super::Database;
use super::DbError;
use crate::core::types_work_items::ScoreSnapshot;

impl Database {
    /// Whether a project and environment has a stored web or code scan. This
    /// prevents an unscanned project from persisting a synthetic score of 100;
    /// `None` checks every environment in the project.
    #[tracing::instrument(skip(self, environment_url), fields(project_id))]
    pub fn has_scan_score_signal(
        &self,
        project_id: i64,
        environment_url: Option<&str>,
    ) -> Result<bool, DbError> {
        let (url, url_slash, raw, any_env) = match environment_url {
            Some(env_url) => {
                let (normalized, with_slash) = normalize_url(env_url);
                (normalized, with_slash, env_url.to_string(), false)
            }
            None => (String::new(), String::new(), String::new(), true),
        };
        self.execute(move |conn| {
            let has_signal: bool = conn.query_row(
                "SELECT EXISTS (
                     SELECT 1 FROM scan_runs run
                     WHERE run.project_id = :project_id
                       AND run.status = 'complete'
                       AND (
                           :any_env
                           OR run.source = 'code_scan'
                           OR run.environment_scope_key IN (:url, :url_slash, :raw)
                       )
                 )",
                named_params! {
                    ":project_id": project_id,
                    ":any_env": any_env,
                    ":url": url,
                    ":url_slash": url_slash,
                    ":raw": raw,
                },
                |row| row.get(0),
            )?;
            Ok(has_signal)
        })?
    }

    /// Return whether a score has active groups or stored scan evidence.
    /// This prevents synthetic 100s on unscanned projects, and probe failures
    /// skip persistence without failing the score read.
    #[tracing::instrument(skip(self, environment_url), fields(project_id))]
    pub fn has_persistable_score_signal(
        &self,
        project_id: i64,
        environment_url: Option<&str>,
        has_active_groups: bool,
    ) -> bool {
        has_active_groups
            || self
                .has_scan_score_signal(project_id, environment_url)
                .unwrap_or_else(|e| {
                    tracing::warn!("score signal probe failed; skipping persistence: {}", e);
                    false
                })
    }

    /// Persist the freshly computed live score when it differs from the
    /// latest stored row for (project, environment). Returns whether a row
    /// was written.
    #[tracing::instrument(skip(self, snapshot, environment_url), fields(project_id))]
    pub fn record_score_snapshot_if_changed(
        &self,
        project_id: i64,
        environment_url: Option<&str>,
        snapshot: &ScoreSnapshot,
    ) -> Result<bool, DbError> {
        let environment_key = normalize_env_url(environment_url);
        let point = ScoreSnapshotPoint {
            overall: snapshot.overall,
            critical_count: snapshot.critical_count as u32,
            high_count: snapshot.high_count as u32,
            medium_count: snapshot.medium_count as u32,
            low_count: snapshot.low_count as u32,
            exploitable_capped: snapshot.exploitable_capped,
            computed_at: snapshot.computed_at,
        };
        self.execute(move |conn| {
            let latest: Option<ScoreSnapshotPoint> = conn
                .query_row(
                    "SELECT overall, critical_count, high_count, medium_count, low_count,
                            exploitable_capped, computed_at
                     FROM score_snapshots
                     WHERE project_id = :project_id AND environment_url = :env
                     ORDER BY id DESC LIMIT 1",
                    named_params! { ":project_id": project_id, ":env": environment_key },
                    score_snapshot_point_from_row,
                )
                .optional()?;
            if let Some(latest) = latest {
                let unchanged = latest.overall == point.overall
                    && latest.critical_count == point.critical_count
                    && latest.high_count == point.high_count
                    && latest.medium_count == point.medium_count
                    && latest.low_count == point.low_count
                    && latest.exploitable_capped == point.exploitable_capped;
                if unchanged {
                    return Ok(false);
                }
            }
            conn.execute(
                "INSERT INTO score_snapshots (
                    project_id, environment_url, overall, critical_count, high_count,
                    medium_count, low_count, exploitable_capped, computed_at
                 ) VALUES (
                    :project_id, :env, :overall, :critical, :high,
                    :medium, :low, :capped, :computed_at
                 )",
                named_params! {
                    ":project_id": project_id,
                    ":env": environment_key,
                    ":overall": point.overall,
                    ":critical": point.critical_count,
                    ":high": point.high_count,
                    ":medium": point.medium_count,
                    ":low": point.low_count,
                    ":capped": point.exploitable_capped,
                    ":computed_at": point.computed_at,
                },
            )?;
            Ok(true)
        })?
    }

    /// Recent live-score history for a project environment, newest first,
    /// bounded by `limit` (callers pass `SCORE_SNAPSHOT_HISTORY_LIMIT`).
    #[tracing::instrument(skip(self, environment_url), fields(project_id))]
    pub fn get_score_snapshot_history(
        &self,
        project_id: i64,
        environment_url: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ScoreSnapshotPoint>, DbError> {
        let environment_key = normalize_env_url(environment_url);
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT overall, critical_count, high_count, medium_count, low_count,
                        exploitable_capped, computed_at
                 FROM score_snapshots
                 WHERE project_id = :project_id AND environment_url = :env
                 ORDER BY id DESC LIMIT :limit",
            )?;
            let rows = stmt.query_map(
                named_params! { ":project_id": project_id, ":env": environment_key, ":limit": limit },
                score_snapshot_point_from_row,
            )?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?
    }
}

fn score_snapshot_point_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScoreSnapshotPoint> {
    Ok(ScoreSnapshotPoint {
        overall: row.get(0)?,
        critical_count: row.get(1)?,
        high_count: row.get(2)?,
        medium_count: row.get(3)?,
        low_count: row.get(4)?,
        exploitable_capped: row.get(5)?,
        computed_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::temp_db;
    use std::collections::HashMap;

    fn snapshot(overall: f64, high: usize, computed_at: i64) -> ScoreSnapshot {
        ScoreSnapshot {
            overall,
            per_category: HashMap::new(),
            critical_count: 0,
            high_count: high,
            medium_count: 0,
            low_count: 0,
            exploitable_capped: false,
            breakdown: Default::default(),
            computed_at,
        }
    }

    #[test]
    fn first_write_lands_identical_recompute_skips_change_writes() {
        let db = temp_db();
        let project_id = db
            .upsert_project("scores", "/tmp/scores", None)
            .expect("project");
        let env = Some("https://example.com");

        assert!(db
            .record_score_snapshot_if_changed(project_id, env, &snapshot(91.0, 1, 1_000))
            .expect("first write"));
        assert!(!db
            .record_score_snapshot_if_changed(project_id, env, &snapshot(91.0, 1, 2_000))
            .expect("unchanged skip"));
        assert!(db
            .record_score_snapshot_if_changed(project_id, env, &snapshot(85.0, 1, 3_000))
            .expect("changed overall"));
        assert!(db
            .record_score_snapshot_if_changed(project_id, env, &snapshot(85.0, 2, 4_000))
            .expect("changed counts"));

        let history = db
            .get_score_snapshot_history(project_id, env, 90)
            .expect("history");
        assert_eq!(history.len(), 3);
        // Newest first.
        assert_eq!(history[0].computed_at, 4_000);
        assert_eq!(history[0].high_count, 2);
        assert_eq!(history[2].computed_at, 1_000);
        assert_eq!(history[2].overall, 91.0);
    }

    #[test]
    fn has_persistable_score_signal_is_the_one_shared_no_signal_predicate() {
        // Every score surface shares this persistability predicate.
        let db = temp_db();
        let project_id = db
            .upsert_project("signal", "/tmp/signal", None)
            .expect("project");
        let env = Some("https://example.com");

        // Active groups ARE the signal: true even with zero stored scans, and
        // it short-circuits before touching the DB.
        assert!(db.has_persistable_score_signal(project_id, env, true));

        // No groups and no stored scan: no signal (a synthetic 100 must not
        // persist a fake baseline).
        assert!(!db.has_persistable_score_signal(project_id, env, false));

        // A stored code scan is a signal even with no groups and no web scan.
        db.save_code_scan(
            project_id,
            None,
            "/tmp/signal".to_string(),
            &crate::core::code_scan::CodeScanReport {
                skipped_scopes: Default::default(),
                checked_at: "2026-07-20T00:00:00Z".to_string(),
                framework: None,
                issue_count: 0,
                critical_count: 0,
                high_count: 0,
                medium_count: 0,
                low_count: 0,
                issues: Vec::new(),
            },
            10,
        )
        .expect("seed canonical code scan");
        assert!(db.has_persistable_score_signal(project_id, env, false));

        // Probe failure degrades to "no signal" (best-effort), never panics.
        db.execute(|conn| {
            conn.execute_batch("DROP TABLE scan_runs;")
                .map_err(|e| e.to_string())
        })
        .expect("worker")
        .expect("drop tables");
        assert!(!db.has_persistable_score_signal(project_id, env, false));
        assert!(db.has_persistable_score_signal(project_id, env, true));
    }

    #[test]
    fn cap_flag_flip_alone_writes_a_row() {
        let db = temp_db();
        let project_id = db.upsert_project("cap", "/tmp/cap", None).expect("project");
        let env = Some("https://example.com");
        let mut capped = snapshot(49.0, 0, 1_000);
        capped.critical_count = 1;
        capped.exploitable_capped = true;
        assert!(db
            .record_score_snapshot_if_changed(project_id, env, &capped)
            .expect("capped write"));
        let mut uncapped = capped.clone();
        uncapped.exploitable_capped = false;
        uncapped.computed_at = 2_000;
        assert!(db
            .record_score_snapshot_if_changed(project_id, env, &uncapped)
            .expect("cap flip writes"));
    }

    #[test]
    fn history_is_scoped_per_environment_and_bounded() {
        let db = temp_db();
        let project_id = db
            .upsert_project("bounds", "/tmp/bounds", None)
            .expect("project");
        let prod = Some("https://example.com");
        let staging = Some("https://staging.example.com");
        for i in 0..5 {
            db.record_score_snapshot_if_changed(
                project_id,
                prod,
                &snapshot(90.0 - f64::from(i), 0, i64::from(i)),
            )
            .expect("prod write");
        }
        db.record_score_snapshot_if_changed(project_id, staging, &snapshot(50.0, 3, 9_000))
            .expect("staging write");

        // Bounded read returns only the newest `limit` rows.
        let bounded = db
            .get_score_snapshot_history(project_id, prod, 2)
            .expect("bounded");
        assert_eq!(bounded.len(), 2);
        assert_eq!(bounded[0].overall, 86.0);

        // Environments do not bleed into each other; a different env's write
        // also does not suppress prod's change detection (negative control).
        let staging_rows = db
            .get_score_snapshot_history(project_id, staging, 90)
            .expect("staging rows");
        assert_eq!(staging_rows.len(), 1);
        assert_eq!(staging_rows[0].overall, 50.0);
        let other_project = db
            .upsert_project("other", "/tmp/other", None)
            .expect("other project");
        assert!(db
            .get_score_snapshot_history(other_project, prod, 90)
            .expect("other project rows")
            .is_empty());
    }

    #[test]
    fn deleting_a_project_cascades_its_score_history() {
        let db = temp_db();
        let project_id = db
            .upsert_project("gone", "/tmp/gone", None)
            .expect("project");
        db.record_score_snapshot_if_changed(
            project_id,
            Some("https://example.com"),
            &snapshot(80.0, 1, 1_000),
        )
        .expect("write");
        db.delete_project(project_id).expect("delete project");
        let orphans: i64 = db
            .execute(|conn| {
                conn.query_row("SELECT COUNT(*) FROM score_snapshots", [], |row| row.get(0))
                    .map_err(|e| e.to_string())
            })
            .expect("db op")
            .expect("count");
        assert_eq!(orphans, 0, "FK cascade must clean up score history");
    }
}
