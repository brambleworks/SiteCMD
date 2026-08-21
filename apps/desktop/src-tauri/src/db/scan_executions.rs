//! Durable top-level scan intent, idempotent admission, and canonical child runs.

use rusqlite::{named_params, Connection, OptionalExtension, Row};

use crate::core::scan_execution::{
    NewScanExecution, ScanAdmissionError, ScanAdmissionOutcome, ScanComponent, ScanComponentStatus,
    ScanExecutionRecord, ScanExecutionStatus, ScanExecutionSummary, ScanRunSummary,
};

// Imported from `helpers` directly, like every other db module. The `db::`
// re-export is gated behind the `desktop` feature, and this module builds for
// the CLI target too.
use super::helpers::{normalize_env_url, parse_optional_enum_required, parse_required_enum};
use super::{Database, DbError};

const EXECUTION_COLUMNS: &str = "
    id,
    project_id,
    environment_id,
    environment_url,
    environment_scope_key,
    requested_mode,
    web_focus,
    trigger,
    admission_class,
    status,
    idempotency_key,
    request_fingerprint,
    started_at,
    completed_at,
    score_snapshot_id,
    failure_summary,
    web_status,
    web_detail,
    code_status,
    code_detail";

fn execution_from_row(row: &Row<'_>) -> rusqlite::Result<ScanExecutionRecord> {
    Ok(ScanExecutionRecord {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        environment_id: row.get("environment_id")?,
        environment_url: row.get("environment_url")?,
        environment_scope_key: row.get("environment_scope_key")?,
        requested_mode: parse_required_enum(
            5,
            "scan_executions.requested_mode",
            &row.get::<_, String>("requested_mode")?,
        )?,
        web_focus: parse_optional_enum_required(
            6,
            "scan_executions.web_focus",
            row.get::<_, Option<String>>("web_focus")?,
        )?,
        trigger: parse_required_enum(
            7,
            "scan_executions.trigger",
            &row.get::<_, String>("trigger")?,
        )?,
        admission_class: parse_required_enum(
            8,
            "scan_executions.admission_class",
            &row.get::<_, String>("admission_class")?,
        )?,
        status: parse_required_enum(
            9,
            "scan_executions.status",
            &row.get::<_, String>("status")?,
        )?,
        idempotency_key: row.get("idempotency_key")?,
        request_fingerprint: row.get("request_fingerprint")?,
        started_at: row.get("started_at")?,
        completed_at: row.get("completed_at")?,
        score_snapshot_id: row.get("score_snapshot_id")?,
        failure_summary: row.get("failure_summary")?,
        web_status: parse_optional_enum_required(
            19,
            "scan_executions.web_status",
            row.get::<_, Option<String>>("web_status")?,
        )?,
        web_detail: row.get("web_detail")?,
        code_status: parse_optional_enum_required(
            21,
            "scan_executions.code_status",
            row.get::<_, Option<String>>("code_status")?,
        )?,
        code_detail: row.get("code_detail")?,
    })
}

fn load_execution_run_summaries(
    conn: &Connection,
    execution_id: i64,
) -> Result<Vec<ScanRunSummary>, DbError> {
    let mut statement = conn.prepare(
        "SELECT id, parent_run_id, source, run_kind, status, timestamp_text,
                raw_score, duration_ms, issues_total, issues_critical,
                issues_high, issues_medium, issues_low, diagnostics_json
         FROM scan_runs
         WHERE execution_id = ?1
         ORDER BY CASE run_kind
             WHEN 'multi_parent' THEN 0 WHEN 'single' THEN 1
             WHEN 'page' THEN 2 ELSE 3 END,
             started_at, id",
    )?;
    let rows = statement.query_map([execution_id], |row| {
        let diagnostics_json: String = row.get(13)?;
        let diagnostics = serde_json::from_str(&diagnostics_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                13,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
        Ok(ScanRunSummary {
            id: row.get(0)?,
            parent_run_id: row.get(1)?,
            source: parse_required_enum(2, "scan_runs.source", &row.get::<_, String>(2)?)?,
            run_kind: parse_required_enum(3, "scan_runs.run_kind", &row.get::<_, String>(3)?)?,
            status: parse_required_enum(4, "scan_runs.status", &row.get::<_, String>(4)?)?,
            timestamp: row.get(5)?,
            raw_score: row.get(6)?,
            duration_ms: super::from_row::row_u64(row, 7)?,
            issues_total: row.get(8)?,
            issues_critical: row.get(9)?,
            issues_high: row.get(10)?,
            issues_medium: row.get(11)?,
            issues_low: row.get(12)?,
            diagnostics,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn validate_new_execution(request: &NewScanExecution) -> Result<(), ScanAdmissionError> {
    let key = request.idempotency_key.trim();
    if key.is_empty() || key.len() > 256 {
        return Err(ScanAdmissionError::InvalidRequest(
            "idempotency_key must contain 1 to 256 characters".into(),
        ));
    }
    if !request.request_fingerprint.starts_with("v1:") {
        return Err(ScanAdmissionError::InvalidRequest(
            "request_fingerprint must use the v1 digest format".into(),
        ));
    }
    if request.environment_scope_key.trim().is_empty() {
        return Err(ScanAdmissionError::InvalidRequest(
            "environment_scope_key is required".into(),
        ));
    }
    let has_planned_child = [request.web_status, request.code_status]
        .into_iter()
        .flatten()
        .any(|status| status == ScanComponentStatus::Planned);
    if !has_planned_child {
        return Err(ScanAdmissionError::InvalidRequest(
            "at least one collector must be planned".into(),
        ));
    }
    Ok(())
}

fn load_execution_by_key(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
) -> Result<Option<ScanExecutionRecord>, ScanAdmissionError> {
    tx.query_row(
        &format!(
            "SELECT {EXECUTION_COLUMNS}
             FROM scan_executions
             WHERE idempotency_key = :idempotency_key"
        ),
        named_params! { ":idempotency_key": key },
        execution_from_row,
    )
    .optional()
    .map_err(|error| ScanAdmissionError::Storage(error.to_string()))
}

impl Database {
    /// Atomically reuse or create an execution keyed by its idempotency key.
    #[tracing::instrument(skip(self, request), fields(mode = %request.requested_mode, trigger = request.trigger.as_str()))]
    pub fn admit_scan_execution(
        &self,
        request: NewScanExecution,
        terminal_retry_window_secs: i64,
    ) -> Result<ScanAdmissionOutcome, ScanAdmissionError> {
        validate_new_execution(&request)?;

        self.execute_mut(
            move |conn| -> Result<ScanAdmissionOutcome, ScanAdmissionError> {
                let tx = conn
                    .transaction()
                    .map_err(|error| ScanAdmissionError::Storage(error.to_string()))?;

                if let Some(existing) = load_execution_by_key(&tx, &request.idempotency_key)? {
                    if existing.request_fingerprint != request.request_fingerprint {
                        return Err(ScanAdmissionError::IdempotencyConflict);
                    }
                    let reusable = match existing.status {
                        ScanExecutionStatus::Planned | ScanExecutionStatus::Running => true,
                        terminal if terminal.is_terminal() => {
                            existing.completed_at.is_some_and(|completed_at| {
                                let age_ms = request.now_ms.saturating_sub(completed_at);
                                age_ms <= terminal_retry_window_secs.saturating_mul(1_000)
                            })
                        }
                        _ => false,
                    };
                    if !reusable {
                        return Err(ScanAdmissionError::IdempotencyStale);
                    }
                    tx.commit()
                        .map_err(|error| ScanAdmissionError::Storage(error.to_string()))?;
                    return Ok(ScanAdmissionOutcome {
                        execution: existing,
                        reused: true,
                    });
                }

                // Capture the event watermark at admission so updates arriving
                // mid-scan cannot raise the declared evidence basis.
                let based_on_event_sequence = super::connected_producer::site_event_watermark(
                    &tx,
                    request.project_id,
                    &request.environment_scope_key,
                )
                .map_err(|error| ScanAdmissionError::Storage(error.to_string()))?;

                tx.execute(
                    "INSERT INTO scan_executions (
                    project_id, environment_id, environment_url, environment_scope_key,
                    requested_mode, web_focus, trigger, admission_class, status,
                    idempotency_key, request_fingerprint, started_at, web_status,
                    web_detail, code_status, code_detail, based_on_event_sequence
                 ) VALUES (
                    :project_id, :environment_id, :environment_url, :environment_scope_key,
                    :requested_mode, :web_focus, :trigger, :admission_class, 'planned',
                    :idempotency_key, :request_fingerprint, :started_at, :web_status,
                    :web_detail, :code_status, :code_detail, :based_on_event_sequence
                 )",
                    named_params! {
                        ":project_id": request.project_id,
                        ":environment_id": request.environment_id,
                        ":environment_url": request.environment_url,
                        ":environment_scope_key": request.environment_scope_key,
                        ":requested_mode": request.requested_mode.as_str(),
                        ":web_focus": request.web_focus.map(|focus| focus.as_str()),
                        ":trigger": request.trigger.as_str(),
                        ":admission_class": request.admission_class.as_str(),
                        ":idempotency_key": request.idempotency_key,
                        ":request_fingerprint": request.request_fingerprint,
                        ":started_at": request.now_ms,
                        ":web_status": request.web_status.map(ScanComponentStatus::as_str),
                        ":web_detail": request.web_detail,
                        ":code_status": request.code_status.map(ScanComponentStatus::as_str),
                        ":code_detail": request.code_detail,
                        ":based_on_event_sequence": based_on_event_sequence,
                    },
                )
                .map_err(|error| ScanAdmissionError::Storage(error.to_string()))?;
                let execution_id = tx.last_insert_rowid();
                let execution = tx
                    .query_row(
                        &format!(
                            "SELECT {EXECUTION_COLUMNS}
                         FROM scan_executions
                         WHERE id = :execution_id"
                        ),
                        named_params! { ":execution_id": execution_id },
                        execution_from_row,
                    )
                    .map_err(|error| ScanAdmissionError::Storage(error.to_string()))?;
                tx.commit()
                    .map_err(|error| ScanAdmissionError::Storage(error.to_string()))?;
                Ok(ScanAdmissionOutcome {
                    execution,
                    reused: false,
                })
            },
        )
        .map_err(|error| ScanAdmissionError::Storage(error.to_string()))?
    }

    #[tracing::instrument(skip(self), fields(execution_id, component = ?component))]
    pub fn start_scan_execution_component(
        &self,
        execution_id: i64,
        component: ScanComponent,
    ) -> Result<(), DbError> {
        let status_column = match component {
            ScanComponent::Web => "web_status",
            ScanComponent::Code => "code_status",
        };
        self.execute(move |conn| {
            let changed = conn.execute(
                &format!(
                    "UPDATE scan_executions
                     SET status = 'running',
                         {status_column} = 'running'
                     WHERE id = :execution_id
                       AND {status_column} = 'planned'
                       AND status IN ('planned', 'running')"
                ),
                named_params! { ":execution_id": execution_id },
            )?;
            if changed == 0 {
                return Err(DbError::Other(format!(
                    "execution {execution_id} component {component:?} is not planned"
                )));
            }
            Ok(())
        })?
    }

    #[tracing::instrument(skip(self, detail), fields(execution_id, component = ?component, component_status = ?component_status))]
    pub fn finish_scan_execution_component(
        &self,
        execution_id: i64,
        component: ScanComponent,
        component_status: ScanComponentStatus,
        detail: Option<String>,
        completed_at: i64,
    ) -> Result<ScanExecutionRecord, DbError> {
        if component_status.is_unsettled() {
            return Err(DbError::Other(
                "finish_scan_execution_component requires a settled status".into(),
            ));
        }
        let (status_column, detail_column) = match component {
            ScanComponent::Web => ("web_status", "web_detail"),
            ScanComponent::Code => ("code_status", "code_detail"),
        };
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            let changed = tx.execute(
                &format!(
                    "UPDATE scan_executions
                     SET {status_column} = :component_status,
                         {detail_column} = :detail
                     WHERE id = :execution_id
                       AND status IN ('planned', 'running')"
                ),
                named_params! {
                    ":component_status": component_status.as_str(),
                    ":detail": detail,
                    ":execution_id": execution_id,
                },
            )?;
            if changed == 0 {
                return Err(DbError::Other(format!(
                    "execution {execution_id} is missing or already settled"
                )));
            }

            let (web_status, code_status): (Option<String>, Option<String>) = tx.query_row(
                "SELECT web_status, code_status
                 FROM scan_executions
                 WHERE id = :execution_id",
                named_params! { ":execution_id": execution_id },
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let statuses = [web_status.as_deref(), code_status.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            let unsettled = statuses
                .iter()
                .any(|status| matches!(*status, "planned" | "running"));
            if !unsettled {
                let completed = statuses
                    .iter()
                    .filter(|status| **status == "complete")
                    .count();
                let failed = statuses
                    .iter()
                    .filter(|status| **status == "failed")
                    .count();
                let cancelled = statuses
                    .iter()
                    .filter(|status| **status == "cancelled")
                    .count();
                let status = if completed > 0 && failed + cancelled == 0 {
                    ScanExecutionStatus::Complete
                } else if completed > 0 {
                    ScanExecutionStatus::Partial
                } else if cancelled > 0 && failed == 0 {
                    ScanExecutionStatus::Cancelled
                } else {
                    ScanExecutionStatus::Failed
                };
                tx.execute(
                    "UPDATE scan_executions
                     SET status = :status, completed_at = :completed_at
                     WHERE id = :execution_id",
                    named_params! {
                        ":status": status.as_str(),
                        ":completed_at": completed_at,
                        ":execution_id": execution_id,
                    },
                )?;
            }

            let execution = tx.query_row(
                &format!(
                    "SELECT {EXECUTION_COLUMNS}
                     FROM scan_executions
                     WHERE id = :execution_id"
                ),
                named_params! { ":execution_id": execution_id },
                execution_from_row,
            )?;
            tx.commit()?;
            Ok(execution)
        })?
    }

    /// Fail a fully planned execution before any collector starts. Reused or
    /// already-running executions are untouched.
    #[tracing::instrument(skip(self, detail), fields(execution_id))]
    pub fn release_scan_execution_before_start(
        &self,
        execution_id: i64,
        detail: String,
        completed_at: i64,
    ) -> Result<(), DbError> {
        self.execute(move |conn| {
            let changed = conn.execute(
                "UPDATE scan_executions
                 SET status = 'failed',
                     completed_at = :completed_at,
                     failure_summary = :detail,
                     web_status = CASE WHEN web_status = 'planned' THEN 'failed' ELSE web_status END,
                     code_status = CASE WHEN code_status = 'planned' THEN 'failed' ELSE code_status END
                 WHERE id = :execution_id AND status = 'planned'",
                named_params! {
                    ":completed_at": completed_at,
                    ":detail": detail,
                    ":execution_id": execution_id,
                },
            )?;
            if changed == 0 {
                return Err(DbError::Other(format!(
                    "execution {execution_id} is not eligible for reservation release"
                )));
            }
            Ok(())
        })?
    }

    /// Cancel a fully planned execution before collection starts. This is
    /// distinct from a failure: history shows the user's cancellation.
    #[tracing::instrument(skip(self, detail), fields(execution_id))]
    pub fn cancel_scan_execution_before_start(
        &self,
        execution_id: i64,
        detail: String,
        completed_at: i64,
    ) -> Result<ScanExecutionRecord, DbError> {
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            let changed = tx.execute(
                "UPDATE scan_executions
                 SET status = 'cancelled',
                     completed_at = :completed_at,
                     failure_summary = :detail,
                     web_status = CASE
                         WHEN web_status = 'planned' THEN 'cancelled'
                         ELSE web_status
                     END,
                     code_status = CASE
                         WHEN code_status = 'planned' THEN 'cancelled'
                         ELSE code_status
                     END
                 WHERE id = :execution_id AND status = 'planned'",
                named_params! {
                    ":completed_at": completed_at,
                    ":detail": detail,
                    ":execution_id": execution_id,
                },
            )?;
            if changed == 0 {
                return Err(DbError::Other(format!(
                    "execution {execution_id} is not eligible for pre-start cancellation"
                )));
            }
            let execution = tx.query_row(
                &format!(
                    "SELECT {EXECUTION_COLUMNS}
                     FROM scan_executions
                     WHERE id = :execution_id"
                ),
                named_params! { ":execution_id": execution_id },
                execution_from_row,
            )?;
            tx.commit()?;
            Ok(execution)
        })?
    }

    #[tracing::instrument(skip(self), fields(execution_id))]
    pub fn get_scan_execution(
        &self,
        execution_id: i64,
    ) -> Result<Option<ScanExecutionRecord>, DbError> {
        self.execute(move |conn| {
            conn.query_row(
                &format!(
                    "SELECT {EXECUTION_COLUMNS}
                     FROM scan_executions
                     WHERE id = :execution_id"
                ),
                named_params! { ":execution_id": execution_id },
                execution_from_row,
            )
            .optional()
            .map_err(DbError::from)
        })?
    }

    #[tracing::instrument(skip(self), fields(project_id, environment_scope_key = ?environment_scope_key, run_kind = ?run_kind, limit))]
    pub fn get_scan_execution_history(
        &self,
        project_id: Option<i64>,
        environment_scope_key: Option<String>,
        run_kind: Option<crate::core::normalized_scan::ScanRunKind>,
        limit: u32,
    ) -> Result<Vec<ScanExecutionSummary>, DbError> {
        self.execute(move |conn| {
            let mut statement = conn.prepare(
                "SELECT
                    execution.id,
                    execution.project_id,
                    execution.environment_id,
                    execution.environment_url,
                    execution.requested_mode,
                    execution.web_focus,
                    execution.trigger,
                    execution.status,
                    execution.started_at,
                    execution.completed_at,
                    score.overall,
                    score.critical_count,
                    score.high_count,
                    score.medium_count,
                    score.low_count,
                    execution.web_status,
                    execution.web_detail,
                    execution.code_status,
                    execution.code_detail,
                    (
                        SELECT run.id
                        FROM scan_runs AS run
                        WHERE run.execution_id = execution.id
                          AND run.source = 'web_scan'
                          AND run.run_kind = 'single'
                        ORDER BY run.id DESC
                        LIMIT 1
                    ) AS web_scan_id,
                    (
                        SELECT run.id
                        FROM scan_runs AS run
                        WHERE run.execution_id = execution.id
                          AND run.source = 'web_scan'
                          AND run.run_kind = 'multi_parent'
                        ORDER BY run.id DESC
                        LIMIT 1
                    ) AS web_session_id,
                    (
                        SELECT COUNT(*)
                        FROM scan_runs AS page_run
                        WHERE page_run.execution_id = execution.id
                          AND page_run.source = 'web_scan'
                          AND page_run.run_kind = 'page'
                    ) AS web_page_count,
                    (
                        SELECT run.id
                        FROM scan_runs AS run
                        WHERE run.execution_id = execution.id
                          AND run.source = 'code_scan'
                          AND run.run_kind = 'code'
                        ORDER BY run.id DESC
                        LIMIT 1
                    ) AS code_scan_id
                 FROM scan_executions AS execution
                 LEFT JOIN score_snapshots AS score ON score.id = execution.score_snapshot_id
                 WHERE execution.admission_class <> 'bounded_verification'
                   AND (:project_id IS NULL OR execution.project_id = :project_id)
                   AND (
                       :environment_scope_key IS NULL
                       OR execution.environment_scope_key = :environment_scope_key
                   )
                   AND (
                       :run_kind IS NULL
                       OR EXISTS (
                           SELECT 1 FROM scan_runs AS matching_run
                           WHERE matching_run.execution_id = execution.id
                             AND matching_run.run_kind = :run_kind
                       )
                   )
                 ORDER BY execution.started_at DESC, execution.id DESC
                 LIMIT :limit",
            )?;
            let rows = statement.query_map(
                named_params! {
                    ":project_id": project_id,
                    ":environment_scope_key": environment_scope_key,
                    ":run_kind": run_kind.map(|kind| kind.as_str()),
                    ":limit": limit,
                },
                |row| {
                    let execution_id: i64 = row.get(0)?;
                    let runs =
                        load_execution_run_summaries(conn, execution_id).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Integer,
                                Box::new(std::io::Error::other(error.to_string())),
                            )
                        })?;
                    Ok(ScanExecutionSummary {
                        id: execution_id,
                        project_id: row.get(1)?,
                        environment_id: row.get(2)?,
                        environment_url: row.get(3)?,
                        requested_mode: parse_required_enum(
                            4,
                            "scan_executions.requested_mode",
                            &row.get::<_, String>(4)?,
                        )?,
                        web_focus: parse_optional_enum_required(
                            5,
                            "scan_executions.web_focus",
                            row.get::<_, Option<String>>(5)?,
                        )?,
                        trigger: parse_required_enum(
                            6,
                            "scan_executions.trigger",
                            &row.get::<_, String>(6)?,
                        )?,
                        status: parse_required_enum(
                            7,
                            "scan_executions.status",
                            &row.get::<_, String>(7)?,
                        )?,
                        started_at: row.get(8)?,
                        completed_at: row.get(9)?,
                        score: row.get(10)?,
                        critical_count: row.get(11)?,
                        high_count: row.get(12)?,
                        medium_count: row.get(13)?,
                        low_count: row.get(14)?,
                        web_status: parse_optional_enum_required(
                            15,
                            "scan_executions.web_status",
                            row.get::<_, Option<String>>(15)?,
                        )?,
                        web_detail: row.get(16)?,
                        code_status: parse_optional_enum_required(
                            17,
                            "scan_executions.code_status",
                            row.get::<_, Option<String>>(17)?,
                        )?,
                        code_detail: row.get(18)?,
                        web_scan_id: row.get(19)?,
                        web_session_id: row.get(20)?,
                        web_page_count: row.get(21)?,
                        code_scan_id: row.get(22)?,
                        runs,
                    })
                },
            )?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?
    }

    #[tracing::instrument(skip(self, environment_url), fields(execution_id, project_id))]
    pub fn link_scan_execution_score_snapshot(
        &self,
        execution_id: i64,
        project_id: i64,
        environment_url: Option<&str>,
    ) -> Result<ScanExecutionRecord, DbError> {
        let environment_scope_key = normalize_env_url(environment_url);
        self.execute(move |conn| {
            conn.execute(
                "UPDATE scan_executions
                 SET score_snapshot_id = (
                     SELECT id
                     FROM score_snapshots
                     WHERE project_id = :project_id
                       AND environment_url = :environment_scope_key
                     ORDER BY id DESC
                     LIMIT 1
                 )
                 WHERE id = :execution_id",
                named_params! {
                    ":execution_id": execution_id,
                    ":project_id": project_id,
                    ":environment_scope_key": environment_scope_key,
                },
            )?;
            conn.query_row(
                &format!(
                    "SELECT {EXECUTION_COLUMNS}
                     FROM scan_executions
                     WHERE id = :execution_id"
                ),
                named_params! { ":execution_id": execution_id },
                execution_from_row,
            )
            .map_err(DbError::from)
        })?
    }
}

#[cfg(test)]
#[path = "scan_executions_tests.rs"]
mod tests;
