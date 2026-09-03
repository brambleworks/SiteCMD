//! Read projections for one scan execution and its findings.

use rusqlite::Connection;

use super::helpers::parse_required_enum;
use super::{Database, DbError};

/// An execution's joined score snapshot: overall, then the critical, high,
/// medium, and low row counts. All optional because an execution that has not
/// finalized yet has no snapshot to join.
type ExecutionScoreRow = (
    Option<f64>,
    Option<u32>,
    Option<u32>,
    Option<u32>,
    Option<u32>,
);

fn json_column<T: serde::de::DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    index: usize,
    field: &str,
) -> rusqlite::Result<T> {
    let value: String = row.get(index)?;
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid persisted {field}: {error}"),
            )),
        )
    })
}

fn load_normalized_findings(
    conn: &Connection,
    run_id: i64,
) -> Result<Vec<crate::core::normalized_scan::NormalizedFinding>, DbError> {
    let mut statement = conn.prepare(
        "SELECT occurrence_id, source, canonical_check_id, producer_check_id,
                producer_category, category, domain, verdict, severity,
                confidence, confidence_reason, title, description, fix_prompt,
                producer_fix_prompt, manual_fix, why_it_matters,
                verification_hint, raw_data, detail_json, location_kind,
                page_url, relative_path, line
         FROM scan_findings
         WHERE run_id = ?1
         ORDER BY ordinal",
    )?;
    let findings = statement
        .query_map([run_id], |row| {
            Ok(crate::core::normalized_scan::NormalizedFinding {
                occurrence_id: row.get(0)?,
                source: parse_required_enum(1, "scan_findings.source", &row.get::<_, String>(1)?)?,
                canonical_check_id: row.get(2)?,
                producer_check_id: row.get(3)?,
                producer_category: row.get(4)?,
                category: row.get(5)?,
                domain: row.get(6)?,
                verdict: parse_required_enum(
                    7,
                    "scan_findings.verdict",
                    &row.get::<_, String>(7)?,
                )?,
                severity: parse_required_enum(
                    8,
                    "scan_findings.severity",
                    &row.get::<_, String>(8)?,
                )?,
                confidence: parse_required_enum(
                    9,
                    "scan_findings.confidence",
                    &row.get::<_, String>(9)?,
                )?,
                confidence_reason: row.get(10)?,
                title: row.get(11)?,
                description: row.get(12)?,
                fix_prompt: row.get(13)?,
                producer_fix_prompt: row.get(14)?,
                manual_fix: row.get(15)?,
                why_it_matters: row.get(16)?,
                verification_hint: row.get(17)?,
                raw_data: row.get(18)?,
                detail_json: row.get(19)?,
                location_kind: parse_required_enum(
                    20,
                    "scan_findings.location_kind",
                    &row.get::<_, String>(20)?,
                )?,
                page_url: row.get(21)?,
                relative_path: row.get(22)?,
                line: row.get(23)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(findings)
}

impl Database {
    /// The whole execution: every run, each with all of its findings.
    pub fn get_scan_execution_detail(
        &self,
        execution_id: i64,
    ) -> Result<Option<crate::core::scan_execution::ScanExecutionDetail>, DbError> {
        self.load_scan_execution_detail(execution_id, None)
    }

    /// One run of an execution. The summary still describes the whole
    /// execution; only the returned run details, and the findings read out of
    /// SQLite at all, are narrowed to `run_id`.
    pub fn get_scan_execution_detail_for_run(
        &self,
        execution_id: i64,
        run_id: i64,
    ) -> Result<Option<crate::core::scan_execution::ScanExecutionDetail>, DbError> {
        self.load_scan_execution_detail(execution_id, Some(run_id))
    }

    #[tracing::instrument(skip(self), fields(execution_id, run_filter))]
    fn load_scan_execution_detail(
        &self,
        execution_id: i64,
        run_filter: Option<i64>,
    ) -> Result<Option<crate::core::scan_execution::ScanExecutionDetail>, DbError> {
        let Some(execution) = self.get_scan_execution(execution_id)? else {
            return Ok(None);
        };
        let (score, critical_count, high_count, medium_count, low_count): ExecutionScoreRow = self
            .execute(move |conn| {
                conn.query_row(
                    "SELECT score.overall, score.critical_count, score.high_count,
                        score.medium_count, score.low_count
                 FROM scan_executions execution
                 LEFT JOIN score_snapshots score
                   ON score.id = execution.score_snapshot_id
                 WHERE execution.id = ?1",
                    [execution_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .map_err(DbError::from)
            })??;
        let (runs, run_summaries) = self.execute(move |conn| {
            let mut statement = conn.prepare(
                "SELECT id, parent_run_id, source, run_kind, status,
                        timestamp_text, raw_score, duration_ms, issues_total,
                        issues_critical, issues_high, issues_medium, issues_low,
                        diagnostics_json, started_at, completed_at,
                        coverage_json, status_detail, detail_state
                 FROM scan_runs
                 WHERE execution_id = ?1
                 ORDER BY CASE run_kind
                     WHEN 'multi_parent' THEN 0
                     WHEN 'single' THEN 1
                     WHEN 'page' THEN 2
                     ELSE 3 END,
                     started_at, id",
            )?;
            // Each row yields the run's summary plus the extra columns only the
            // detail carries. The stored issue counts are written from the run's
            // findings in the same transaction that inserts them, so a summary
            // never depends on the caller having read those findings back.
            let headers = statement
                .query_map([execution_id], |row| {
                    Ok((
                        crate::core::scan_execution::ScanRunSummary {
                            id: row.get(0)?,
                            parent_run_id: row.get(1)?,
                            source: parse_required_enum(
                                2,
                                "scan_runs.source",
                                &row.get::<_, String>(2)?,
                            )?,
                            run_kind: parse_required_enum(
                                3,
                                "scan_runs.run_kind",
                                &row.get::<_, String>(3)?,
                            )?,
                            status: parse_required_enum(
                                4,
                                "scan_runs.status",
                                &row.get::<_, String>(4)?,
                            )?,
                            timestamp: row.get(5)?,
                            raw_score: row.get(6)?,
                            duration_ms: super::from_row::row_u64(row, 7)?,
                            issues_total: row.get(8)?,
                            issues_critical: row.get(9)?,
                            issues_high: row.get(10)?,
                            issues_medium: row.get(11)?,
                            issues_low: row.get(12)?,
                            diagnostics: json_column(row, 13, "scan_runs.diagnostics_json")?,
                        },
                        (
                            row.get::<_, i64>(14)?,
                            row.get::<_, Option<i64>>(15)?,
                            json_column(row, 16, "scan_runs.coverage_json")?,
                            row.get::<_, Option<String>>(17)?,
                            row.get::<_, String>(18)?,
                        ),
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut runs = Vec::with_capacity(headers.len());
            let mut run_summaries = Vec::with_capacity(headers.len());
            for (summary, (started_at, completed_at, coverage, status_detail, detail_state)) in
                headers
            {
                runs.push(crate::core::scan_execution::ScanRunDetail {
                    id: summary.id,
                    parent_run_id: summary.parent_run_id,
                    source: summary.source,
                    run_kind: summary.run_kind,
                    status: summary.status,
                    timestamp: summary.timestamp.clone(),
                    started_at,
                    completed_at,
                    raw_score: summary.raw_score,
                    duration_ms: summary.duration_ms,
                    coverage,
                    diagnostics: summary.diagnostics.clone(),
                    status_detail,
                    detail_state,
                    findings: if run_filter.is_none_or(|requested| requested == summary.id) {
                        load_normalized_findings(conn, summary.id)?
                    } else {
                        Vec::new()
                    },
                });
                run_summaries.push(summary);
            }
            Ok::<_, DbError>((runs, run_summaries))
        })??;
        let latest_run = |kind| {
            runs.iter()
                .rev()
                .find(|run| run.run_kind == kind)
                .map(|run| run.id)
        };
        let summary = crate::core::scan_execution::ScanExecutionSummary {
            id: execution.id,
            project_id: execution.project_id,
            environment_id: execution.environment_id,
            environment_url: execution.environment_url,
            requested_mode: execution.requested_mode,
            web_focus: execution.web_focus,
            trigger: execution.trigger,
            status: execution.status,
            started_at: execution.started_at,
            completed_at: execution.completed_at,
            score,
            critical_count,
            high_count,
            medium_count,
            low_count,
            web_status: execution.web_status,
            web_detail: execution.web_detail,
            code_status: execution.code_status,
            code_detail: execution.code_detail,
            web_scan_id: latest_run(crate::core::normalized_scan::ScanRunKind::Single),
            web_session_id: latest_run(crate::core::normalized_scan::ScanRunKind::MultiParent),
            web_page_count: u32::try_from(
                runs.iter()
                    .filter(|run| run.run_kind == crate::core::normalized_scan::ScanRunKind::Page)
                    .count(),
            )
            .unwrap_or(u32::MAX),
            code_scan_id: latest_run(crate::core::normalized_scan::ScanRunKind::Code),
            runs: run_summaries,
        };
        Ok(Some(crate::core::scan_execution::ScanExecutionDetail {
            summary,
            runs: match run_filter {
                Some(requested) => runs.into_iter().filter(|run| run.id == requested).collect(),
                None => runs,
            },
        }))
    }

    pub fn get_scan_execution_event_stats(
        &self,
        execution_id: i64,
    ) -> Result<(Option<f64>, u32, u32, u32), DbError> {
        self.execute(move |conn| {
            conn.query_row(
                "SELECT score.overall,
                        COALESCE(score.critical_count, SUM(run.issues_critical), 0),
                        COALESCE(score.high_count, SUM(run.issues_high), 0),
                        COALESCE(SUM(
                            CASE WHEN run.run_kind = 'multi_parent'
                                 THEN 0 ELSE run.issues_total END
                        ), 0)
                 FROM scan_executions execution
                 LEFT JOIN score_snapshots score
                   ON score.id = execution.score_snapshot_id
                 LEFT JOIN scan_runs run ON run.execution_id = execution.id
                 WHERE execution.id = ?1
                 GROUP BY execution.id, score.id",
                [execution_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(DbError::from)
        })?
    }

    pub fn get_scan_execution_affected_check_ids(
        &self,
        execution_id: i64,
    ) -> Result<Vec<String>, DbError> {
        self.execute(move |conn| {
            let mut statement = conn.prepare(
                "SELECT DISTINCT finding.canonical_check_id
                 FROM scan_findings finding
                 JOIN scan_runs run ON run.id = finding.run_id
                 WHERE run.execution_id = ?1
                   AND finding.verdict IN ('fail', 'warn')
                 ORDER BY finding.canonical_check_id",
            )?;
            let check_ids = statement
                .query_map([execution_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(DbError::from)?;
            Ok(check_ids)
        })?
    }
}

#[cfg(test)]
#[path = "scan_execution_detail_tests.rs"]
mod tests;
