//! Multi-page scan session management.

use super::DbError;
use rusqlite::params;

#[cfg(test)]
use crate::checks::CheckResult;
use crate::checks::{CheckStatus, IssueConfidence, ScanCategory, Severity};

use super::from_row;
use super::helpers::{normalize_url, parse_required_enum};
use super::types::{ConsolidatedIssue, ConsolidatedIssueInstance, ScanSessionSummary, ScanSummary};
use super::Database;

impl Database {
    /// Canonical fixture helper for cross-page findings. Product multi-page
    /// scans complete the parent through `complete_multi_page_scan_run`.
    #[cfg(test)]
    pub fn save_session_issue_snapshot(
        &self,
        session_id: i64,
        issues: &[CheckResult],
    ) -> Result<(), DbError> {
        #[allow(clippy::type_complexity)]
        let (
            execution_id,
            project_id,
            site_id,
            environment_url,
            focus,
            started_at,
            raw_score,
            duration_ms,
            total_pages,
            axe_enabled,
            successful_page_urls,
        ): (
            i64,
            Option<i64>,
            i64,
            String,
            String,
            i64,
            Option<u32>,
            u64,
            usize,
            bool,
            Vec<String>,
        ) = self.execute(move |conn| {
            let header = conn.query_row(
                "SELECT execution_id, project_id, site_id, environment_url,
                        COALESCE(focus, 'health'), started_at, raw_score,
                        duration_ms, total_pages, COALESCE(axe_enabled, 0)
                 FROM scan_runs WHERE id = ?1 AND run_kind = 'multi_parent'",
                [session_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        super::from_row::row_u64(row, 7)?,
                        usize::try_from(row.get::<_, i64>(8)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                8,
                                rusqlite::types::Type::Integer,
                                Box::new(error),
                            )
                        })?,
                        row.get::<_, i64>(9)? != 0,
                    ))
                },
            )?;
            let mut statement = conn.prepare(
                "SELECT page_url FROM scan_runs
                 WHERE parent_run_id = ?1 AND status = 'complete'
                   AND page_url IS NOT NULL
                 ORDER BY id",
            )?;
            let pages = statement
                .query_map([session_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, DbError>((
                header.0, header.1, header.2, header.3, header.4, header.5, header.6, header.7,
                header.8, header.9, pages,
            ))
        })??;
        let focus: crate::core::scanner::ScanType = focus.parse().map_err(DbError::Other)?;
        let completed_at = started_at.saturating_add(duration_ms as i64);
        let batch = crate::core::normalized_scan::normalize_multi_page_parent(
            issues,
            execution_id,
            project_id,
            site_id,
            environment_url,
            successful_page_urls.clone(),
            successful_page_urls,
            total_pages,
            raw_score,
            duration_ms,
            started_at,
            completed_at,
            focus,
            axe_enabled,
            true,
        )?;
        self.complete_multi_page_scan_run(session_id, batch)
    }

    /// Canonical fixture helper. Product multi-page scans admit their execution
    /// first and call `start_multi_page_scan_run` directly.
    #[cfg(test)]
    #[tracing::instrument(skip(self), fields(site_id, total_pages, axe_enabled))]
    pub fn create_scan_session(
        &self,
        site_id: i64,
        total_pages: usize,
        axe_enabled: bool,
    ) -> Result<i64, DbError> {
        let (project_id, environment_url): (Option<i64>, String) =
            self.execute(move |conn| {
                conn.query_row(
                    "SELECT project_id, url FROM sites WHERE id = ?1",
                    [site_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(DbError::from)
            })??;
        let started_at = chrono::Utc::now().timestamp_millis();
        let action_key = super::scans::canonical_import_action_key("multi-web")?;
        let scope_key = normalize_url(&environment_url).0;
        let execution = self
            .admit_scan_execution(
                crate::core::scan_execution::NewScanExecution {
                    project_id,
                    environment_id: None,
                    environment_url: Some(environment_url.clone()),
                    environment_scope_key: scope_key,
                    requested_mode: crate::core::scan_execution::ScanExecutionMode::Web,
                    web_focus: Some(crate::core::scanner::ScanType::Health),
                    trigger: crate::core::scan_execution::ScanTrigger::Migration,
                    admission_class: crate::core::scan_execution::ScanAdmissionClass::SystemExempt,
                    idempotency_key: action_key.clone(),
                    request_fingerprint: format!("v1:{action_key}"),
                    now_ms: started_at,
                    web_status: Some(crate::core::scan_execution::ScanComponentStatus::Planned),
                    web_detail: None,
                    code_status: None,
                    code_detail: None,
                },
                crate::constants::SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
            )
            .map_err(|error| DbError::Other(error.to_string()))?
            .execution;
        self.start_scan_execution_component(
            execution.id,
            crate::core::scan_execution::ScanComponent::Web,
        )?;
        let selected_pages = (0..total_pages)
            .map(|index| format!("{environment_url}#fixture-page-{index}"))
            .collect::<Vec<_>>();
        self.start_multi_page_scan_run(
            execution.id,
            project_id,
            site_id,
            &environment_url,
            crate::core::scanner::ScanType::Health,
            &selected_pages,
            axe_enabled,
            started_at,
        )
    }

    #[cfg(test)]
    #[tracing::instrument(skip(self), fields(session_id, completed_pages))]
    pub fn update_scan_session_progress(
        &self,
        session_id: i64,
        completed_pages: usize,
    ) -> Result<(), DbError> {
        self.update_multi_page_scan_run_progress(session_id, completed_pages)
    }

    /// Mark a fixture parent complete, storing no completed score as NULL.
    #[cfg(test)]
    #[tracing::instrument(skip(self), fields(session_id, overall_score, duration_ms))]
    pub fn complete_scan_session(
        &self,
        session_id: i64,
        overall_score: Option<u32>,
        duration_ms: u64,
    ) -> Result<(), DbError> {
        let (execution_id, completed_at): (i64, i64) = self.execute(move |conn| {
            let (execution_id, started_at): (i64, i64) = conn.query_row(
                "SELECT execution_id, started_at FROM scan_runs WHERE id = ?1",
                [session_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let completed_at = started_at.saturating_add(duration_ms as i64);
            conn.execute(
                "UPDATE scan_runs
                 SET status = 'complete', completed_at = ?1, raw_score = ?2,
                     duration_ms = ?3, completed_pages = total_pages
                 WHERE id = ?4 AND run_kind = 'multi_parent'",
                params![
                    completed_at,
                    overall_score.map(i64::from),
                    duration_ms as i64,
                    session_id
                ],
            )?;
            Ok::<_, DbError>((execution_id, completed_at))
        })??;
        self.finish_scan_execution_component(
            execution_id,
            crate::core::scan_execution::ScanComponent::Web,
            crate::core::scan_execution::ScanComponentStatus::Complete,
            None,
            completed_at,
        )?;
        Ok(())
    }

    /// Mark a fixture multi-page run as failed.
    #[cfg(test)]
    #[tracing::instrument(skip(self), fields(session_id))]
    pub fn fail_scan_session(&self, session_id: i64) -> Result<(), DbError> {
        let execution_id: i64 = self.execute(move |conn| {
            conn.query_row(
                "SELECT execution_id FROM scan_runs WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .map_err(DbError::from)
        })??;
        let completed_at = chrono::Utc::now().timestamp_millis();
        self.fail_scan_run(session_id, completed_at, "fixture_failed")?;
        self.finish_scan_execution_component(
            execution_id,
            crate::core::scan_execution::ScanComponent::Web,
            crate::core::scan_execution::ScanComponentStatus::Failed,
            Some("fixture_failed".into()),
            completed_at,
        )?;
        Ok(())
    }

    /// Get consolidated issues from canonical parent and child findings,
    /// grouped by producer check_id and verdict. Cross-page findings carry no
    /// page_url, so an empty pages list renders as "All pages".
    #[tracing::instrument(skip(self), fields(session_id))]
    pub fn get_session_issues(&self, session_id: i64) -> Result<Vec<ConsolidatedIssue>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT category, check_id, severity, status, title, description,
                        fix_prompt, manual_fix, page_url, confidence,
                        confidence_reason, why_it_matters, raw_data
                 FROM (
                    SELECT finding.producer_category AS category,
                           finding.producer_check_id AS check_id,
                           finding.severity AS severity,
                           finding.verdict AS status,
                           finding.title AS title,
                           finding.description AS description,
                           finding.producer_fix_prompt AS fix_prompt,
                           finding.manual_fix AS manual_fix,
                           finding.page_url AS page_url,
                           finding.confidence AS confidence,
                           finding.confidence_reason AS confidence_reason,
                           finding.why_it_matters AS why_it_matters,
                           finding.raw_data AS raw_data,
                           finding.ordinal AS ordinal
                    FROM scan_findings finding
                    JOIN scan_runs run ON run.id = finding.run_id
                    WHERE run.id = ?1 OR run.parent_run_id = ?1
                 ) snapshots
                 ORDER BY check_id, status,
                          CASE severity
                              WHEN 'critical' THEN 0 WHEN 'high' THEN 1
                              WHEN 'medium' THEN 2 ELSE 3 END,
                          page_url, ordinal",
            )?;

            #[allow(clippy::type_complexity)]
            let rows: Vec<(
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            )> = stmt
                .query_map(params![session_id], |row| {
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

            let mut grouped: std::collections::HashMap<(String, String), ConsolidatedIssue> =
                std::collections::HashMap::new();

            for (
                cat,
                check_id,
                severity,
                status,
                title,
                description,
                fix_prompt,
                manual_fix,
                page_url,
                confidence,
                confidence_reason,
                why_it_matters,
                raw_data,
            ) in rows
            {
                // Legacy rows predate check_status and are backfilled as Fail;
                // new rows retain the producer's exact Fail/Warn verdict.
                let parsed_category: ScanCategory = parse_required_enum(0, "category", &cat)?;
                let parsed_status: CheckStatus = parse_required_enum(3, "check_status", &status)?;
                let key = (check_id.clone(), parsed_status.as_str().to_string());
                let parsed_severity: Severity = parse_required_enum(2, "severity", &severity)?;
                let parsed_confidence: IssueConfidence =
                    parse_required_enum(9, "confidence", &confidence)?;
                let parsed_raw_data = raw_data.as_deref().map(serde_json::from_str).transpose()?;
                let instance = ConsolidatedIssueInstance {
                    page_url: page_url.clone(),
                    category: parsed_category,
                    check_id: check_id.clone(),
                    severity: parsed_severity,
                    status: parsed_status,
                    title: title.clone(),
                    description: description.clone(),
                    fix_prompt: fix_prompt.clone(),
                    manual_fix: manual_fix.clone(),
                    raw_data: parsed_raw_data,
                    confidence: parsed_confidence,
                    confidence_reason: confidence_reason.clone(),
                    why_it_matters: why_it_matters.clone(),
                };
                let entry = grouped.entry(key).or_insert_with(|| ConsolidatedIssue {
                    category: parsed_category,
                    check_id: check_id.clone(),
                    severity: parsed_severity,
                    status: parsed_status,
                    title: title.clone(),
                    description: description.clone(),
                    fix_prompt: fix_prompt.clone(),
                    manual_fix: manual_fix.clone(),
                    confidence: parsed_confidence,
                    confidence_reason: confidence_reason.clone(),
                    why_it_matters: why_it_matters.clone(),
                    pages: Vec::new(),
                    page_count: 0,
                    instances: Vec::new(),
                });
                // The consolidated copy describes the highest-severity page in
                // the group. SQL ordering normally inserts it first; this
                // comparison makes that contract explicit and robust to query
                // refactors.
                if parsed_severity.sort_rank() < entry.severity.sort_rank() {
                    entry.category = parsed_category;
                    entry.severity = parsed_severity;
                    entry.title = title;
                    entry.description = description;
                    entry.fix_prompt = fix_prompt;
                    entry.manual_fix = manual_fix;
                    entry.confidence = parsed_confidence;
                    entry.confidence_reason = confidence_reason;
                    entry.why_it_matters = why_it_matters;
                }
                if let Some(url) = page_url {
                    if !entry.pages.contains(&url) {
                        entry.pages.push(url);
                    }
                }
                entry.instances.push(instance);
                entry.page_count = entry.pages.len();
            }

            let mut results: Vec<ConsolidatedIssue> = grouped.into_values().collect();
            let status_order = |status: CheckStatus| -> u8 {
                match status {
                    CheckStatus::Fail => 0,
                    CheckStatus::Warn => 1,
                    _ => 2,
                }
            };
            results.sort_by(|a, b| {
                status_order(a.status)
                    .cmp(&status_order(b.status))
                    .then(a.severity.sort_rank().cmp(&b.severity.sort_rank()))
                    .then(b.page_count.cmp(&a.page_count))
            });
            Ok(results)
        })?
    }

    /// Get session history for a URL with per-page scan details for each session.
    #[tracing::instrument(skip(self, url), fields(limit))]
    pub fn get_session_history(
        &self,
        url: &str,
        limit: u32,
    ) -> Result<Vec<ScanSessionSummary>, DbError> {
        let url = url.to_string();
        self.execute(move |conn| {
            let normalized = normalize_url(&url).0;

            let mut stmt = conn.prepare(
                "SELECT run.id, run.total_pages, run.completed_pages, run.status,
                        run.timestamp_text, run.raw_score, run.duration_ms
                 FROM scan_runs run
                 WHERE run.source = 'web_scan'
                   AND run.run_kind = 'multi_parent'
                   AND run.environment_scope_key = ?1
                 ORDER BY run.started_at DESC, run.id DESC
                 LIMIT ?2",
            )?;

            #[allow(clippy::type_complexity)]
            let sessions: Vec<(i64, i64, i64, String, String, Option<i64>, Option<i64>)> = stmt
                .query_map(
                    params![normalized, limit],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
                )
                ?
                .collect::<Result<Vec<_>, _>>()?;

            let mut results = Vec::new();
            for (session_id, total_pages, completed_pages, status, started_at, overall_score, duration_ms) in sessions {
                let mut page_stmt = conn
                    .prepare(
                        "SELECT run.id AS id,
                                COALESCE(run.page_url, run.environment_url, run.environment_scope_key) AS url,
                                COALESCE(run.mode, 'live') AS mode,
                                run.raw_score AS overall_score,
                                run.issues_total AS issues_total,
                                run.issues_critical AS issues_critical,
                                run.issues_high AS issues_high,
                                run.issues_medium AS issues_medium,
                                run.issues_low AS issues_low,
                                run.duration_ms AS duration_ms,
                                run.timestamp_text AS timestamp,
                                run.parent_run_id AS session_id,
                                run.page_url AS page_url,
                                COALESCE(run.focus, 'health') AS scan_type
                         FROM scan_runs run
                         WHERE run.parent_run_id = ?1 AND run.run_kind = 'page'
                         ORDER BY run.started_at ASC, run.id ASC",
                    )
                    ?;
                let page_scans: Vec<ScanSummary> = from_row::query_vec(&mut page_stmt, &[&session_id])?;

                results.push(ScanSessionSummary {
                    session_id, total_pages, completed_pages, status, started_at,
                    overall_score, duration_ms, page_scans,
                });
            }

            Ok(results)
        })?
    }

    /// Canonical fixture helper for a page child under a parent run.
    #[cfg(test)]
    #[tracing::instrument(skip(self, result, page_url), fields(site_id, session_id))]
    pub fn save_scan_with_session(
        &self,
        site_id: i64,
        session_id: i64,
        page_url: &str,
        result: &crate::core::scanner::ScanResult,
    ) -> Result<i64, DbError> {
        let (execution_id, project_id, environment_url, started_at): (
            i64,
            Option<i64>,
            String,
            i64,
        ) = self.execute(move |conn| {
            conn.query_row(
                "SELECT execution_id, project_id, environment_url, started_at
                 FROM scan_runs WHERE id = ?1 AND run_kind = 'multi_parent'",
                [session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(DbError::from)
        })??;
        let mut page_result = result.clone();
        page_result.url = page_url.to_string();
        let page_started_at =
            super::timestamp_text_to_ms(&page_result.timestamp).unwrap_or(started_at);
        let mut batch = crate::core::normalized_scan::normalize_web_scan(
            &page_result,
            execution_id,
            Some(session_id),
            project_id,
            site_id,
            crate::core::normalized_scan::ScanRunKind::Page,
            page_started_at,
        )?;
        batch.environment_url = Some(environment_url.clone());
        batch.environment_scope_key = normalize_url(&environment_url).0;
        batch.diagnostics.page_url = Some(page_url.to_string());
        self.persist_normalized_scan_run(batch)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::test_helpers::temp_db;

    #[test]
    fn session_history_fails_loudly_instead_of_dropping_a_malformed_row() {
        let db = temp_db();
        let url = "https://malformed-session.example.com";
        let site_id = db.get_or_create_site(url).expect("site");
        let session_id = db.create_scan_session(site_id, 1, false).expect("session");
        db.execute(move |conn| {
            conn.execute(
                "UPDATE scan_runs SET total_pages = 'not-an-integer' WHERE id = ?1",
                rusqlite::params![session_id],
            )
            .map(|_| ())
        })
        .expect("database worker")
        .expect("corrupt fixture row");

        let error = db
            .get_session_history(url, 20)
            .expect_err("malformed persisted history must not masquerade as an empty list");
        assert!(error.to_string().contains("Invalid column type"));
    }

    #[test]
    fn complete_scan_session_stores_none_as_null_not_a_red_zero() {
        // No completed score is NULL; a real zero remains zero.
        let db = temp_db();
        let site_id = db
            .get_or_create_site("https://unscored-session.example.com")
            .expect("site");

        let unscored = db.create_scan_session(site_id, 2, false).expect("session");
        db.complete_scan_session(unscored, None, 1234)
            .expect("complete unscored");
        let stored: Option<i64> = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT raw_score FROM scan_runs WHERE id = ?1",
                    rusqlite::params![unscored],
                    |row| row.get::<_, Option<i64>>(0),
                )
            })
            .expect("worker")
            .expect("read");
        assert_eq!(stored, None, "no-page-scored session persists NULL, not 0");

        // Negative control: a real 0 is preserved distinctly from NULL.
        let zero = db.create_scan_session(site_id, 1, false).expect("session");
        db.complete_scan_session(zero, Some(0), 10)
            .expect("complete zero");
        let stored_zero: Option<i64> = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT raw_score FROM scan_runs WHERE id = ?1",
                    rusqlite::params![zero],
                    |row| row.get::<_, Option<i64>>(0),
                )
            })
            .expect("worker")
            .expect("read");
        assert_eq!(stored_zero, Some(0));
    }

    #[test]
    fn fail_scan_session_marks_the_session_errored_with_an_end_time() {
        let db = temp_db();
        let site_id = db
            .get_or_create_site("https://aborted-session.example.com")
            .expect("site");
        let session_id = db.create_scan_session(site_id, 3, false).expect("session");

        db.fail_scan_session(session_id)
            .expect("mark session failed");

        let (status, completed_at) = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT status, completed_at FROM scan_runs WHERE id = ?1",
                    rusqlite::params![session_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
                )
            })
            .expect("database worker")
            .expect("read session");
        assert_eq!(status, "failed");
        assert!(
            completed_at.is_some(),
            "an aborted session must record when it ended"
        );
    }

    /// scan_multi's fail-closed abort paths must flip the canonical parent run
    /// to failed or history shows it running forever.
    #[test]
    fn scan_multi_wires_fail_scan_run_on_its_abort_path() {
        let source = include_str!("../commands/scan/multi_scan.rs");
        assert!(
            source.contains("db.fail_scan_run(") && source.contains("session_id,"),
            "scan_multi no longer marks aborted canonical runs failed"
        );
    }
}
