use super::DbError;
use rusqlite::{params, OptionalExtension};

use super::helpers::normalize_env_url;
use super::types::{ProjectMonitoringSignals, ProjectSignalSnapshotRecord};
use super::Database;

impl Database {
    #[tracing::instrument(skip(self, environment_url), fields(project_id))]
    pub fn get_project_signal_snapshot_record(
        &self,
        project_id: i64,
        environment_url: Option<&str>,
    ) -> Result<Option<ProjectSignalSnapshotRecord>, DbError> {
        let environment_key = normalize_env_url(environment_url);
        self.execute(move |conn| {
            conn.query_row(
                "SELECT project_id, environment_url, monitoring_json, monitoring_refreshed_at, updates_json, updates_refreshed_at
                 FROM project_signal_snapshots
                 WHERE project_id = ?1 AND environment_url = ?2",
                params![project_id, environment_key],
                |row| {
                    let environment_url: String = row.get(1)?;
                    Ok(ProjectSignalSnapshotRecord {
                        project_id: row.get(0)?,
                        environment_url: if environment_url.is_empty() {
                            None
                        } else {
                            Some(environment_url)
                        },
                        monitoring_json: row.get(2)?,
                        monitoring_refreshed_at: row.get(3)?,
                        updates_json: row.get(4)?,
                        updates_refreshed_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(DbError::from)
        })?
    }

    #[tracing::instrument(skip(self, monitoring, environment_url), fields(project_id, refreshed_at = %refreshed_at))]
    pub fn save_project_monitoring_snapshot(
        &self,
        project_id: i64,
        environment_url: Option<&str>,
        monitoring: &ProjectMonitoringSignals,
        refreshed_at: &str,
    ) -> Result<(), DbError> {
        let environment_key = normalize_env_url(environment_url);
        let monitoring_json = serde_json::to_string(monitoring)?;
        let refreshed_at = refreshed_at.to_string();
        self.execute(move |conn| {
            conn.execute(
                "INSERT INTO project_signal_snapshots (
                    project_id, environment_url, monitoring_json, monitoring_refreshed_at
                 ) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(project_id, environment_url) DO UPDATE SET
                    monitoring_json = excluded.monitoring_json,
                    monitoring_refreshed_at = excluded.monitoring_refreshed_at,
                    updated_at = datetime('now')",
                params![project_id, environment_key, monitoring_json, refreshed_at],
            )?;
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self, report, environment_url), fields(project_id, refreshed_at = %refreshed_at))]
    pub fn save_project_updates_snapshot(
        &self,
        project_id: i64,
        environment_url: Option<&str>,
        report: &crate::updates::types::UpdateReport,
        refreshed_at: &str,
    ) -> Result<(), DbError> {
        let environment_key = normalize_env_url(environment_url);
        let updates_json = serde_json::to_string(report)?;
        let refreshed_at = refreshed_at.to_string();
        self.execute(move |conn| {
            conn.execute(
                "INSERT INTO project_signal_snapshots (
                    project_id, environment_url, updates_json, updates_refreshed_at
                 ) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(project_id, environment_url) DO UPDATE SET
                    updates_json = excluded.updates_json,
                    updates_refreshed_at = excluded.updates_refreshed_at,
                    updated_at = datetime('now')",
                params![project_id, environment_key, updates_json, refreshed_at],
            )?;
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self, environment_url), fields(project_id))]
    pub fn invalidate_project_signal_snapshots(
        &self,
        project_id: i64,
        environment_url: Option<&str>,
    ) -> Result<(), DbError> {
        let environment_key = normalize_env_url(environment_url);
        self.execute(move |conn| {
            if environment_key.is_empty() {
                conn.execute(
                    "DELETE FROM project_signal_snapshots WHERE project_id = ?1",
                    params![project_id],
                )
                ?;
            } else {
                conn.execute(
                    "DELETE FROM project_signal_snapshots WHERE project_id = ?1 AND environment_url = ?2",
                    params![project_id, environment_key],
                )
                ?;
            }
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn is_first_scan_banner_dismissed(&self, project_id: i64) -> Result<bool, DbError> {
        self.execute(move |conn| {
            conn.query_row(
                "SELECT first_scan_banner_dismissed_at
                 FROM project_ui_state
                 WHERE project_id = ?1",
                params![project_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|value| value.flatten().is_some())
            .map_err(DbError::from)
        })?
    }

    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn dismiss_first_scan_banner(&self, project_id: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute(
                "INSERT INTO project_ui_state (
                    project_id,
                    first_scan_banner_dismissed_at
                 ) VALUES (?1, datetime('now'))
                 ON CONFLICT(project_id) DO UPDATE SET
                    first_scan_banner_dismissed_at = datetime('now'),
                    updated_at = datetime('now')",
                params![project_id],
            )?;
            Ok(())
        })?
    }
}
