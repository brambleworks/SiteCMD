//! Event timeline CRUD and backfill.

use super::DbError;
use rusqlite::{named_params, params};

use super::from_row::FromRow;
use super::helpers::parse_required_enum;
use super::types::{EventSeverity, SiteEvent};
use super::Database;

impl Database {
    /// Insert an event, silently ignoring duplicates (same project + source + source_id).
    /// The event row and its `site_event_check_ids` junction rows commit atomically.
    #[tracing::instrument(skip(self, event))]
    pub fn insert_event(&self, event: &SiteEvent) -> Result<i64, DbError> {
        let event = event.clone();
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let inserted = tx.execute(
                "INSERT OR IGNORE INTO events (project_id, event_type, severity, occurred_at_ms, title, summary, detail, source, source_id, metadata)
                 VALUES (:project_id, :event_type, :severity, :occurred_at_ms, :title, :summary, :detail, :source, :source_id, :metadata)",
                named_params! {
                    ":project_id": event.project_id,
                    ":event_type": event.event_type.to_string(),
                    ":severity": event.severity.to_string(),
                    ":occurred_at_ms": event.occurred_at_ms,
                    ":title": event.title,
                    ":summary": event.summary,
                    ":detail": event.detail,
                    ":source": event.source.to_string(),
                    ":source_id": event.source_id,
                    ":metadata": event.metadata,
                },
            )?;
            // Never attach junction rows to stale `last_insert_rowid` state.
            if inserted == 0 {
                tx.commit()?;
                return Ok(0);
            }
            let event_id = tx.last_insert_rowid();
            if let Some(check_ids) = &event.affected_check_ids {
                let mut stmt = tx
                    .prepare(
                        "INSERT OR IGNORE INTO site_event_check_ids (event_id, check_id) VALUES (?1, ?2)",
                    )
                    ?;
                for id in check_ids {
                    stmt.execute(rusqlite::params![event_id, id])
                        ?;
                }
            }
            tx.commit()?;
            Ok(event_id)
        })?
    }

    /// Insert multiple events in a transaction
    #[tracing::instrument(skip(self, events))]
    pub fn insert_events(&self, events: &[SiteEvent]) -> Result<usize, DbError> {
        let events = events.to_vec();
        self.execute(move |conn| {
            let mut count = 0;
            let tx = conn.unchecked_transaction()?;
            let mut jxn_stmt = tx
                .prepare(
                    "INSERT OR IGNORE INTO site_event_check_ids (event_id, check_id) VALUES (?1, ?2)",
                )
                ?;
            for event in &events {
                let result = tx.execute(
                    "INSERT OR IGNORE INTO events (project_id, event_type, severity, occurred_at_ms, title, summary, detail, source, source_id, metadata)
                     VALUES (:project_id, :event_type, :severity, :occurred_at_ms, :title, :summary, :detail, :source, :source_id, :metadata)",
                    named_params! {
                        ":project_id": event.project_id,
                        ":event_type": event.event_type.to_string(),
                        ":severity": event.severity.to_string(),
                        ":occurred_at_ms": event.occurred_at_ms,
                        ":title": event.title,
                        ":summary": event.summary,
                        ":detail": event.detail,
                        ":source": event.source.to_string(),
                        ":source_id": event.source_id,
                        ":metadata": event.metadata,
                    },
                )?;
                if result > 0 {
                    count += 1;
                    let event_id = tx.last_insert_rowid();
                    if let Some(check_ids) = &event.affected_check_ids {
                        for id in check_ids {
                            jxn_stmt
                                .execute(rusqlite::params![event_id, id])
                                ?;
                        }
                    }
                }
            }
            drop(jxn_stmt);
            tx.commit()?;
            Ok(count)
        })?
    }

    /// Get events for a project, filtered by an epoch-ms range and optional event types
    #[tracing::instrument(skip(self, event_types), fields(project_id, start_ms, end_ms, since_ms = ?since_ms, since_event_id, limit))]
    pub fn get_events(
        &self,
        project_id: i64,
        start_ms: i64,
        end_ms: i64,
        event_types: Option<&[String]>,
        since_ms: Option<i64>,
        since_event_id: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<SiteEvent>, DbError> {
        let event_types = event_types.map(|s| s.to_vec());
        self.execute(move |conn| {
            let mut clauses = vec![
                "project_id = ?1".to_string(),
                "occurred_at_ms >= ?2".to_string(),
                "occurred_at_ms <= ?3".to_string(),
            ];
            let mut next_param_index = 4;

            if since_ms.is_some() {
                if since_event_id.is_some() {
                    clauses.push(format!(
                        "(occurred_at_ms > ?{0} OR (occurred_at_ms = ?{0} AND id > ?{1}))",
                        next_param_index,
                        next_param_index + 1
                    ));
                    next_param_index += 2;
                } else {
                    clauses.push(format!("occurred_at_ms > ?{}", next_param_index));
                    next_param_index += 1;
                }
            }

            let mut query;
            if let Some(ref types) = event_types {
                if types.is_empty() {
                    return Ok(Vec::new());
                }
                let placeholders: Vec<String> = types
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("?{}", i + next_param_index))
                    .collect();
                clauses.push(format!("event_type IN ({})", placeholders.join(", ")));
                query = format!(
                    "SELECT id, project_id, event_type, severity, occurred_at_ms, title, summary, detail, source, source_id, metadata
                     FROM events
                     WHERE {}
                     ORDER BY occurred_at_ms DESC, id DESC",
                    clauses.join(" AND ")
                );
            } else {
                query = format!(
                    "SELECT id, project_id, event_type, severity, occurred_at_ms, title, summary, detail, source, source_id, metadata
                         FROM events
                         WHERE {}
                         ORDER BY occurred_at_ms DESC, id DESC",
                    clauses.join(" AND ")
                );
            }

            if let Some(limit) = limit {
                query.push_str(&format!(" LIMIT {}", limit));
            }

            let mut stmt = conn.prepare(&query)?;

            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
                Box::new(project_id),
                Box::new(start_ms),
                Box::new(end_ms),
            ];
            if let Some(since) = since_ms {
                param_values.push(Box::new(since));
                if let Some(since_event_id) = since_event_id {
                    param_values.push(Box::new(since_event_id));
                }
            }
            if let Some(ref types) = event_types {
                for t in types {
                    param_values.push(Box::new(t.clone()));
                }
            }

            let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|b| b.as_ref()).collect();

            let rows = stmt.query_map(params_refs.as_slice(), SiteEvent::from_row)
                ?;

            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?
    }

    /// Fetch indexed recent events grouped by check ID.
    #[tracing::instrument(skip(self, check_ids), fields(project_id, check_id_count = check_ids.len(), since_ms))]
    pub fn get_events_for_check_ids(
        &self,
        project_id: i64,
        check_ids: &[String],
        since_ms: i64,
    ) -> Result<std::collections::HashMap<String, Vec<SiteEvent>>, DbError> {
        use std::collections::HashMap;
        if check_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let check_ids = check_ids.to_vec();
        self.execute(move |conn| {
            let placeholders = vec!["?"; check_ids.len()].join(", ");
            let sql = format!(
                "SELECT j.check_id, e.id, e.project_id, e.event_type, e.severity,
                        e.occurred_at_ms, e.title, e.summary, e.detail, e.source, e.source_id,
                        e.metadata
                 FROM events e
                 INNER JOIN site_event_check_ids j ON j.event_id = e.id
                 WHERE e.project_id = ? AND e.occurred_at_ms >= ?
                   AND j.check_id IN ({})
                 ORDER BY e.occurred_at_ms DESC",
                placeholders
            );

            let mut stmt = conn.prepare(&sql)?;

            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
                vec![Box::new(project_id), Box::new(since_ms)];
            for id in &check_ids {
                param_values.push(Box::new(id.clone()));
            }
            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                param_values.iter().map(|b| b.as_ref()).collect();

            let rows = stmt.query_map(params_refs.as_slice(), |row| {
                let check_id: String = row.get(0)?;
                let event = SiteEvent {
                    id: row.get(1)?,
                    project_id: row.get(2)?,
                    event_type: {
                        let s: String = row.get(3)?;
                        parse_required_enum(3, "events.event_type", &s)?
                    },
                    severity: {
                        let s: String = row.get(4)?;
                        parse_required_enum(4, "events.severity", &s)?
                    },
                    occurred_at_ms: row.get(5)?,
                    title: row.get(6)?,
                    summary: row.get(7)?,
                    detail: row.get(8)?,
                    source: {
                        let s: String = row.get(9)?;
                        parse_required_enum(9, "events.source", &s)?
                    },
                    source_id: row.get(10)?,
                    metadata: row.get(11)?,
                    // Not hydrated on reads; junction lookups use get_events_for_check_ids.
                    affected_check_ids: None,
                };
                Ok((check_id, event))
            })?;

            let mut out: HashMap<String, Vec<SiteEvent>> = HashMap::new();
            for r in rows {
                let r = r?;
                out.entry(r.0).or_default().push(r.1);
            }
            Ok(out)
        })?
    }

    /// Delete an event by id
    #[tracing::instrument(skip(self), fields(event_id))]
    pub fn delete_event(&self, event_id: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute("DELETE FROM events WHERE id = ?1", params![event_id])?;
            Ok(())
        })?
    }

    /// Delete all events for a project
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn clear_project_events(&self, project_id: i64) -> Result<u64, DbError> {
        self.execute(move |conn| {
            let count = conn.execute(
                "DELETE FROM events WHERE project_id = ?1",
                params![project_id],
            )? as u64;
            Ok(count)
        })?
    }

    /// Backfill events from existing scan history for a project
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn backfill_scan_events(&self, project_id: i64) -> Result<usize, DbError> {
        self.execute(move |conn| {
            let mut statement = conn.prepare(
                "SELECT execution.id, execution.requested_mode,
                        execution.web_focus, execution.status,
                        execution.environment_url, execution.started_at,
                        execution.completed_at, execution.web_status,
                        execution.code_status, score.overall,
                        score.critical_count, score.high_count,
                        COALESCE((
                            SELECT SUM(run.issues_total) FROM scan_runs run
                            WHERE run.execution_id = execution.id
                        ), 0) AS raw_issue_count
                 FROM scan_executions execution
                 LEFT JOIN score_snapshots score
                   ON score.id = execution.score_snapshot_id
                 WHERE execution.project_id = ?1
                   AND execution.status IN ('complete', 'partial', 'failed', 'cancelled')
                 ORDER BY execution.started_at ASC, execution.id ASC
                 LIMIT 500",
            )?;
            #[allow(clippy::type_complexity)]
            let executions: Vec<(
                i64,
                String,
                Option<String>,
                String,
                Option<String>,
                i64,
                Option<i64>,
                Option<String>,
                Option<String>,
                Option<f64>,
                Option<u32>,
                Option<u32>,
                u32,
            )> = statement
                .query_map(params![project_id], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                        row.get(12)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let mut total = 0;
            for (
                execution_id,
                requested_mode,
                web_focus,
                status,
                environment_url,
                started_at,
                completed_at,
                web_status,
                code_status,
                score,
                critical_count,
                high_count,
                raw_issue_count,
            ) in executions
            {
                let mode_label = match requested_mode.as_str() {
                    "full" => "Full scan",
                    "web" => "Web Scan",
                    "code" => "Code Scan",
                    _ => "Scan",
                };
                let score_label = score
                    .map(|value| format!(" · SiteCMD Score {}", value.round() as i64))
                    .unwrap_or_default();
                let severity = match score {
                    Some(value) => EventSeverity::from_scan_score(value.round() as u32),
                    None => EventSeverity::from_issue_counts(
                        critical_count.unwrap_or(0) as usize,
                        high_count.unwrap_or(0) as usize,
                    ),
                };
                let inserted = conn.execute(
                    "INSERT OR IGNORE INTO events (
                        project_id, event_type, severity, occurred_at_ms,
                        title, summary, detail, source, source_id
                     ) VALUES (?1, 'scan', ?2, ?3, ?4, ?5, ?6, 'internal', ?7)",
                    params![
                        project_id,
                        severity.to_string(),
                        completed_at.unwrap_or(started_at),
                        format!("{mode_label}: {status}{score_label}"),
                        format!("{raw_issue_count} collector findings across one {requested_mode} execution."),
                        serde_json::json!({
                            "execution_id": execution_id,
                            "requested_mode": requested_mode,
                            "web_focus": web_focus,
                            "status": status,
                            "web_status": web_status,
                            "code_status": code_status,
                            "sitecmd_score": score,
                            "url": environment_url,
                        })
                        .to_string(),
                        format!("scan_execution_{execution_id}"),
                    ],
                )?;
                total += inserted;
            }
            Ok(total)
        })?
    }
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
