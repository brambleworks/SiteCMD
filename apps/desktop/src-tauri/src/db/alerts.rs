use super::DbError;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::helpers::normalize_url;
use super::Database;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct AlertRow {
    pub id: i64,
    pub project_id: i64,
    pub env_url: Option<String>,
    pub source: String,
    pub alert_id: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub detail_json: Option<String>,
    pub occurred_at: i64,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub viewed_at: Option<i64>,
    pub dismissed_at: Option<i64>,
}

/// Unread alert totals for the app-shell badges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export_to = "ipc-bindings.ts")]
pub struct UnreadAlertCounts {
    pub total: i64,
    pub critical: i64,
}

#[derive(Debug, Clone)]
pub struct AlertInput {
    pub project_id: i64,
    pub env_url: Option<String>,
    pub source: String,
    pub alert_id: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub detail_json: Option<String>,
    pub occurred_at: i64,
    pub observed_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum AlertFilter {
    Unread,
    All,
    Viewed,
    Dismissed,
}

impl Database {
    #[tracing::instrument(skip(self, input))]
    pub fn upsert_alert(&self, input: AlertInput) -> Result<i64, DbError> {
        self.execute(move |conn| {
            let env_url = input
                .env_url
                .as_deref()
                .map(|url| normalize_url(url).0)
                .unwrap_or_default();
            conn.query_row(
                "INSERT INTO alerts
                    (project_id, env_url, source, alert_id, severity, title, description,
                     detail_json, occurred_at, first_seen_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
                 ON CONFLICT(project_id, env_url, source, alert_id) DO UPDATE SET
                    last_seen_at = excluded.last_seen_at,
                    severity = excluded.severity,
                    title = excluded.title,
                    description = excluded.description,
                    detail_json = excluded.detail_json
                 RETURNING id",
                params![
                    input.project_id,
                    env_url,
                    input.source,
                    input.alert_id,
                    input.severity,
                    input.title,
                    input.description,
                    input.detail_json,
                    input.occurred_at,
                    input.observed_at,
                ],
                |row| row.get::<_, i64>(0),
            )
            .map_err(DbError::from)
        })?
    }

    #[tracing::instrument(skip(self, filter), fields(project_id, since_ms))]
    pub fn get_alerts(
        &self,
        project_id: i64,
        filter: AlertFilter,
        since_ms: Option<i64>,
    ) -> Result<Vec<AlertRow>, DbError> {
        self.execute(move |conn| {
            let filter_clause = match filter {
                AlertFilter::Unread => "AND viewed_at IS NULL AND dismissed_at IS NULL",
                AlertFilter::Viewed => {
                    "AND viewed_at IS NOT NULL AND dismissed_at IS NULL"
                }
                AlertFilter::Dismissed => "AND dismissed_at IS NOT NULL",
                AlertFilter::All => "AND dismissed_at IS NULL",
            };
            let since_clause = if since_ms.is_some() {
                "AND occurred_at >= ?2"
            } else {
                ""
            };
            let max_rows = crate::constants::MAX_ALERT_ROWS;
            let sql = format!(
                "SELECT id, project_id, env_url, source, alert_id, severity, title, description,
                        detail_json, occurred_at, first_seen_at, last_seen_at, viewed_at, dismissed_at
                 FROM alerts
                 WHERE project_id = ?1 {filter_clause} {since_clause}
                 ORDER BY occurred_at DESC
                 LIMIT {max_rows}"
            );
            let mut stmt = conn.prepare(&sql)?;
            let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<AlertRow> {
                Ok(AlertRow {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    env_url: row.get::<_, Option<String>>(2)?.filter(|value| !value.is_empty()),
                    source: row.get(3)?,
                    alert_id: row.get(4)?,
                    severity: row.get(5)?,
                    title: row.get(6)?,
                    description: row.get(7)?,
                    detail_json: row.get(8)?,
                    occurred_at: row.get(9)?,
                    first_seen_at: row.get(10)?,
                    last_seen_at: row.get(11)?,
                    viewed_at: row.get(12)?,
                    dismissed_at: row.get(13)?,
                })
            };
            let rows: Vec<AlertRow> = if let Some(since) = since_ms {
                stmt.query_map(params![project_id, since], map_row)
                    ?
                    .collect::<Result<Vec<_>, _>>()
                    ?
            } else {
                stmt.query_map(params![project_id], map_row)
                    ?
                    .collect::<Result<Vec<_>, _>>()
                    ?
            };
            Ok(rows)
        })?
    }

    #[tracing::instrument(skip(self), fields(alert_id, at_ms))]
    pub fn mark_alert_viewed(&self, alert_id: i64, at_ms: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute(
                "UPDATE alerts SET viewed_at = ?1 WHERE id = ?2",
                params![at_ms, alert_id],
            )?;
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self), fields(alert_id))]
    pub fn mark_alert_unread(&self, alert_id: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute(
                "UPDATE alerts SET viewed_at = NULL WHERE id = ?1",
                params![alert_id],
            )?;
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self), fields(alert_id, at_ms))]
    pub fn dismiss_alert(&self, alert_id: i64, at_ms: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute(
                "UPDATE alerts SET dismissed_at = ?1 WHERE id = ?2",
                params![at_ms, alert_id],
            )?;
            Ok(())
        })?
    }

    /// Count unread and critical alerts for shell badges without loading rows.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn count_unread_alerts(&self, project_id: i64) -> Result<UnreadAlertCounts, DbError> {
        self.execute(move |conn| {
            conn.query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN severity = 'critical' THEN 1 ELSE 0 END), 0)
                 FROM alerts
                 WHERE project_id = ?1 AND viewed_at IS NULL AND dismissed_at IS NULL",
                params![project_id],
                |row| {
                    Ok(UnreadAlertCounts {
                        total: row.get::<_, i64>(0)?,
                        critical: row.get::<_, i64>(1)?,
                    })
                },
            )
            .map_err(DbError::from)
        })?
    }

    #[tracing::instrument(skip(self, alert_ids), fields(at_ms))]
    pub fn mark_alerts_viewed_bulk(&self, alert_ids: Vec<i64>, at_ms: i64) -> Result<(), DbError> {
        if alert_ids.is_empty() {
            return Ok(());
        }
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            for id in alert_ids {
                tx.execute(
                    "UPDATE alerts SET viewed_at = ?1 WHERE id = ?2 AND viewed_at IS NULL",
                    params![at_ms, id],
                )?;
            }
            tx.commit().map_err(DbError::from)
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::{temp_db, TestDb};

    fn test_db() -> TestDb {
        let db = temp_db();
        db.upsert_project("test", "https://example.com", None)
            .expect("insert test project");
        db
    }

    fn input(alert_id: &str, occurred_at: i64) -> AlertInput {
        input_for_env(alert_id, occurred_at, "https://example.com")
    }

    fn input_for_env(alert_id: &str, occurred_at: i64, env_url: &str) -> AlertInput {
        AlertInput {
            project_id: 1,
            env_url: Some(env_url.into()),
            source: "uptimerobot".into(),
            alert_id: alert_id.into(),
            severity: "critical".into(),
            title: "Site down".into(),
            description: "Monitor flagged 500 status".into(),
            detail_json: None,
            occurred_at,
            observed_at: occurred_at,
        }
    }

    #[test]
    fn upsert_and_filter_unread() {
        let db = test_db();
        db.upsert_alert(input("alert-a", 1_000)).unwrap();
        db.upsert_alert(input("alert-b", 2_000)).unwrap();

        let rows = db.get_alerts(1, AlertFilter::Unread, None).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn viewed_excludes_from_unread() {
        let db = test_db();
        let id = db.upsert_alert(input("alert-a", 1_000)).unwrap();
        db.mark_alert_viewed(id, 500).unwrap();

        let unread = db.get_alerts(1, AlertFilter::Unread, None).unwrap();
        assert_eq!(unread.len(), 0);

        let viewed = db.get_alerts(1, AlertFilter::Viewed, None).unwrap();
        assert_eq!(viewed.len(), 1);
    }

    #[test]
    fn mark_unread_restores_viewed_alert_to_unread() {
        let db = test_db();
        let id = db.upsert_alert(input("alert-a", 1_000)).unwrap();
        db.mark_alert_viewed(id, 500).unwrap();
        db.mark_alert_unread(id).unwrap();

        let unread = db.get_alerts(1, AlertFilter::Unread, None).unwrap();
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].viewed_at, None);

        let viewed = db.get_alerts(1, AlertFilter::Viewed, None).unwrap();
        assert_eq!(viewed.len(), 0);
    }

    #[test]
    fn dismiss_hides_from_all_except_dismissed_filter() {
        let db = test_db();
        let id = db.upsert_alert(input("alert-a", 1_000)).unwrap();
        db.dismiss_alert(id, 500).unwrap();

        let unread = db.get_alerts(1, AlertFilter::Unread, None).unwrap();
        assert_eq!(unread.len(), 0);

        let all = db.get_alerts(1, AlertFilter::All, None).unwrap();
        assert_eq!(all.len(), 0);

        let dismissed = db.get_alerts(1, AlertFilter::Dismissed, None).unwrap();
        assert_eq!(dismissed.len(), 1);
    }

    #[test]
    fn unread_count_excludes_viewed_and_dismissed() {
        let db = test_db();
        let id_a = db.upsert_alert(input("alert-a", 1_000)).unwrap();
        let id_b = db.upsert_alert(input("alert-b", 2_000)).unwrap();
        let id_c = db.upsert_alert(input("alert-c", 3_000)).unwrap();

        db.mark_alert_viewed(id_b, 500).unwrap();
        db.dismiss_alert(id_c, 500).unwrap();

        let _ = id_a; // alert-a stays unread
        let counts = db.count_unread_alerts(1).unwrap();
        assert_eq!(counts.total, 1);
        assert_eq!(
            counts.critical, 1,
            "the fixture severity is critical, so the badge counts must agree"
        );
    }

    #[test]
    fn same_alert_id_is_distinct_per_environment() {
        let db = test_db();
        let prod_id = db
            .upsert_alert(input_for_env("outage", 1_000, "https://example.com"))
            .unwrap();
        let staging_id = db
            .upsert_alert(input_for_env(
                "outage",
                2_000,
                "https://staging.example.com",
            ))
            .unwrap();

        assert_ne!(prod_id, staging_id);
        let rows = db.get_alerts(1, AlertFilter::Unread, None).unwrap();
        assert_eq!(rows.len(), 2);
    }
}
