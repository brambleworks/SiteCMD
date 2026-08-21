use super::DbError;
use rusqlite::{params, OptionalExtension};

use crate::core::code_provenance::CodeCheckoutProvenance;
use crate::core::code_scan::{
    code_issue_domain, score_report, CodeIssue, CodeIssueView, CodeScanDomain, CodeScanReport,
};
use crate::core::normalized_scan::normalize_code_scan_with_provenance;
use crate::core::scan_execution::{
    NewScanExecution, ScanAdmissionClass, ScanComponent, ScanComponentStatus, ScanExecutionMode,
    ScanTrigger,
};

use super::from_row::{self, row_u64, FromRow};
use super::types::{CodeScanDomainSummary, CodeScanResult, CodeScanSummary};
use super::Database;

/// Columns shared by full and lean code-scan summary queries.
pub(super) const CODE_SCAN_SUMMARY_COLUMNS: &str =
    "id, project_id, environment_url, raw_score AS overall_score, \
     issues_total AS issue_count, issues_critical AS critical_count, \
     issues_high AS high_count, duration_ms, timestamp_text AS checked_at, framework";

/// Canonical immutable issue rows shared by Code Scan history views.
pub(super) const CODE_SCAN_ISSUE_ROWS_CTE: &str = "code_scan_issue_rows AS (
    SELECT finding.id AS row_id,
           finding.run_id AS scan_ref,
           finding.canonical_check_id AS check_id,
           finding.domain AS domain,
           finding.severity AS severity,
           finding.title AS title,
           finding.detail_json AS detail_json,
           finding.ordinal AS ordinal
    FROM scan_findings finding
    JOIN scan_runs run ON run.id = finding.run_id
    WHERE run.source = 'code_scan'
)";

fn code_issue_view_query(limit_one: bool) -> String {
    format!(
        "WITH {CODE_SCAN_ISSUE_ROWS_CTE}
         SELECT detail_json, domain, check_id, severity, title
         FROM code_scan_issue_rows
         WHERE scan_ref = ?1
         ORDER BY
            CASE severity
                WHEN 'critical' THEN 0
                WHEN 'high' THEN 1
                WHEN 'medium' THEN 2
                ELSE 3
            END,
            ordinal ASC{}",
        if limit_one { "\n         LIMIT 1" } else { "" }
    )
}

/// Shared domain-ranking CTE, optionally restricted by `scope`.
pub(super) fn ranked_domains_cte(scope: &str) -> String {
    format!(
        "{CODE_SCAN_ISSUE_ROWS_CTE},
ranked_domains AS (
    SELECT
        scan_ref AS code_scan_id,
        domain,
        issue_count,
        ROW_NUMBER() OVER (
            PARTITION BY scan_ref
            ORDER BY issue_count DESC, domain_rank ASC
        ) AS row_num
    FROM (
        SELECT
            scan_ref,
            domain,
            COUNT(*) AS issue_count,
            CASE domain
                WHEN 'database' THEN 0
                WHEN 'ai-safety' THEN 1
                WHEN 'security' THEN 2
                WHEN 'architecture' THEN 3
                WHEN 'operations' THEN 4
                WHEN 'supply-chain' THEN 5
                ELSE 6
            END AS domain_rank
        FROM code_scan_issue_rows
        WHERE 1 = 1{scope}
        GROUP BY scan_ref, domain
    )
)"
    )
}

/// Parses a snapshot or legacy work-item row into `CodeIssueView`. Lightweight
/// paths may strip rich fields; malformed data remains an explicit error.
fn code_issue_view_from_row(
    detail_json: Option<String>,
    domain_str: Option<String>,
    canonical_check_id: String,
    stored_severity: String,
    stored_title: String,
    strip_rich_fields: bool,
) -> Result<CodeIssueView, DbError> {
    let json = detail_json.ok_or_else(|| {
        DbError::Other("Code Scan issue snapshot is missing its issue_json payload".into())
    })?;
    let mut issue: CodeIssue = serde_json::from_str(&json)?;
    if canonical_check_id.is_empty() {
        return Err(DbError::Other(
            "Code Scan issue snapshot has an empty canonical_check_id".into(),
        ));
    }
    crate::core::code_scan::validate_canonical_check_id(&canonical_check_id)
        .map_err(DbError::Other)?;
    let derived_check_id = crate::core::code_scan::canonical_code_check_id(&issue.id);
    if canonical_check_id != derived_check_id {
        return Err(DbError::Other(format!(
            "Code Scan issue snapshot canonical identity mismatch: column '{canonical_check_id}', producer rule '{}' resolves to '{derived_check_id}'",
            crate::core::code_scan::code_producer_rule_id(&issue.id)
        )));
    }
    if !issue.check_id.is_empty() && issue.check_id != canonical_check_id {
        return Err(DbError::Other(format!(
            "Code Scan issue snapshot check_id mismatch: column '{canonical_check_id}', payload '{}'",
            issue.check_id
        )));
    }
    let parsed_severity: crate::checks::Severity = stored_severity.parse().map_err(|error| {
        DbError::Other(format!(
            "invalid persisted Code Scan severity '{stored_severity}': {error}"
        ))
    })?;
    if issue.severity != parsed_severity {
        return Err(DbError::Other(format!(
            "Code Scan issue snapshot severity mismatch for '{canonical_check_id}': column '{}', payload '{}'",
            parsed_severity.as_str(),
            issue.severity.as_str()
        )));
    }
    if issue.title != stored_title {
        return Err(DbError::Other(format!(
            "Code Scan issue snapshot title mismatch for '{canonical_check_id}'"
        )));
    }
    issue.check_id = canonical_check_id;
    if strip_rich_fields {
        issue.source_excerpt = None;
        issue.evidence = None;
        issue.why_now = None;
        issue.likely_fix = None;
        issue.verify_hint = None;
    }
    let domain = match domain_str.filter(|value| !value.is_empty()) {
        Some(value) => value.parse::<CodeScanDomain>().map_err(|error| {
            DbError::Other(format!(
                "invalid persisted Code Scan domain '{value}': {error}"
            ))
        })?,
        None => code_issue_domain(&issue),
    };
    Ok(CodeIssueView::from_issue_with_domain(issue, domain))
}

/// Per-scan domain aggregates for a set of scan ids, keyed by scan id.
/// Shared by the full history query and the two-scan snapshot hydration.
pub(super) fn load_domain_summaries_for_scans(
    conn: &rusqlite::Connection,
    scan_ids: &[i64],
) -> Result<std::collections::HashMap<i64, Vec<CodeScanDomainSummary>>, DbError> {
    let placeholders: String = scan_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "WITH {CODE_SCAN_ISSUE_ROWS_CTE}
         SELECT
            scan_ref AS code_scan_id,
            domain,
            COUNT(*) AS issue_count,
            SUM(CASE WHEN severity = 'critical' THEN 1 ELSE 0 END) AS critical_count,
            SUM(CASE WHEN severity = 'high' THEN 1 ELSE 0 END) AS high_count,
            SUM(CASE WHEN severity = 'medium' THEN 1 ELSE 0 END) AS medium_count,
            SUM(CASE WHEN severity = 'low' THEN 1 ELSE 0 END) AS low_count
         FROM code_scan_issue_rows
         WHERE scan_ref IN ({})
         GROUP BY scan_ref, domain
         ORDER BY scan_ref,
            CASE domain
                WHEN 'database' THEN 0
                WHEN 'ai-safety' THEN 1
                WHEN 'security' THEN 2
                WHEN 'architecture' THEN 3
                WHEN 'operations' THEN 4
                WHEN 'supply-chain' THEN 5
                ELSE 6
            END ASC",
        placeholders
    );
    let mut ds_stmt = conn.prepare(&sql)?;
    let params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = scan_ids
        .iter()
        .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|b| b.as_ref()).collect();
    let ds_rows = ds_stmt.query_map(param_refs.as_slice(), |row| {
        let scan_id: i64 = row.get("code_scan_id")?;
        Ok((scan_id, CodeScanDomainSummary::from_row(row)?))
    })?;
    let mut domain_map: std::collections::HashMap<i64, Vec<CodeScanDomainSummary>> =
        std::collections::HashMap::new();
    for row in ds_rows {
        let (scan_id, summary) = row?;
        domain_map.entry(scan_id).or_default().push(summary);
    }
    Ok(domain_map)
}

fn load_code_scan_domain_summaries(
    conn: &rusqlite::Connection,
    scan_id: i64,
) -> Result<Vec<CodeScanDomainSummary>, DbError> {
    let sql = format!(
        "WITH {CODE_SCAN_ISSUE_ROWS_CTE}
         SELECT
                domain,
                COUNT(*) AS issue_count,
                SUM(CASE WHEN severity = 'critical' THEN 1 ELSE 0 END) AS critical_count,
                SUM(CASE WHEN severity = 'high' THEN 1 ELSE 0 END) AS high_count,
                SUM(CASE WHEN severity = 'medium' THEN 1 ELSE 0 END) AS medium_count,
                SUM(CASE WHEN severity = 'low' THEN 1 ELSE 0 END) AS low_count
             FROM code_scan_issue_rows
             WHERE scan_ref = ?1
             GROUP BY domain
             ORDER BY
                CASE domain
                    WHEN 'database' THEN 0
                    WHEN 'ai-safety' THEN 1
                    WHEN 'security' THEN 2
                    WHEN 'architecture' THEN 3
                    WHEN 'operations' THEN 4
                    WHEN 'supply-chain' THEN 5
                    ELSE 6
                END ASC"
    );
    let mut stmt = conn.prepare(&sql)?;

    let rows = stmt.query_map(params![scan_id], CodeScanDomainSummary::from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

fn load_code_scan_issue_views(
    conn: &rusqlite::Connection,
    scan_id: i64,
) -> Result<Vec<CodeIssueView>, DbError> {
    let sql = code_issue_view_query(false);
    let mut stmt = conn.prepare(&sql)?;

    let rows = stmt.query_map(params![scan_id], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (detail_json, domain_str, canonical_check_id, stored_severity, stored_title) = row?;
        // Loud on purpose: silently dropping a row rendered an
        // incomplete issue list over a corrupt blob, so a malformed detail_json
        // is fatal here. The full loader keeps the rich fields (no strip).
        result.push(code_issue_view_from_row(
            detail_json,
            domain_str,
            canonical_check_id,
            stored_severity,
            stored_title,
            false,
        )?);
    }
    Ok(result)
}

fn load_lightweight_code_scan_issue_views(
    conn: &rusqlite::Connection,
    scan_id: i64,
) -> Result<Vec<CodeIssueView>, DbError> {
    // Lightweight: deserialize from detail_json but strip rich fields after parsing.
    let sql = code_issue_view_query(false);
    let mut stmt = conn.prepare(&sql)?;

    let rows = stmt.query_map(params![scan_id], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (detail_json, domain_str, canonical_check_id, stored_severity, stored_title) = row?;
        // Loud on purpose, matching load_top_code_scan_issue_view.
        result.push(code_issue_view_from_row(
            detail_json,
            domain_str,
            canonical_check_id,
            stored_severity,
            stored_title,
            true,
        )?);
    }
    Ok(result)
}

fn load_top_code_scan_issue_view(
    conn: &rusqlite::Connection,
    scan_id: i64,
) -> Result<Option<CodeIssueView>, DbError> {
    let sql = code_issue_view_query(true);
    let mut stmt = conn.prepare(&sql)?;

    let row = stmt
        .query_row(params![scan_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .optional()?;

    let Some((detail_json, domain_str, canonical_check_id, stored_severity, stored_title)) = row
    else {
        return Ok(None);
    };

    // Strip rich fields; a malformed detail_json is fatal (propagate).
    Ok(Some(code_issue_view_from_row(
        detail_json,
        domain_str,
        canonical_check_id,
        stored_severity,
        stored_title,
        true,
    )?))
}

fn validate_code_scan_report_counts(report: &CodeScanReport) -> Result<(), DbError> {
    let mut critical = 0usize;
    let mut high = 0usize;
    let mut medium = 0usize;
    let mut low = 0usize;
    for issue in &report.issues {
        match issue.severity {
            crate::checks::Severity::Critical => critical += 1,
            crate::checks::Severity::High => high += 1,
            crate::checks::Severity::Medium => medium += 1,
            crate::checks::Severity::Low => low += 1,
        }
    }
    let expected = (report.issues.len(), critical, high, medium, low);
    let declared = (
        report.issue_count,
        report.critical_count,
        report.high_count,
        report.medium_count,
        report.low_count,
    );
    if declared != expected {
        return Err(DbError::Other(format!(
            "Code Scan report count mismatch: declared total/critical/high/medium/low {declared:?}, actual {expected:?}"
        )));
    }
    Ok(())
}

impl Database {
    #[tracing::instrument(
        skip(self, report, project_path, environment_url),
        fields(project_id, duration_ms)
    )]
    pub fn save_code_scan(
        &self,
        project_id: i64,
        environment_url: Option<String>,
        project_path: String,
        report: &CodeScanReport,
        duration_ms: u64,
    ) -> Result<i64, DbError> {
        // Imported reports cannot establish the checkout observed at scan time,
        // so this path must not attach current Git provenance.
        let provenance = CodeCheckoutProvenance::default();
        self.save_code_scan_with_provenance(
            project_id,
            environment_url,
            project_path,
            report,
            duration_ms,
            provenance,
        )
    }

    /// Persist a report with checkout facts captured before the source walk.
    pub fn save_code_scan_with_provenance(
        &self,
        project_id: i64,
        environment_url: Option<String>,
        project_path: String,
        report: &CodeScanReport,
        duration_ms: u64,
        provenance: CodeCheckoutProvenance,
    ) -> Result<i64, DbError> {
        validate_code_scan_report_counts(report)?;
        let started_at = super::timestamp_text_to_ms(&report.checked_at)
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
        let environment_scope_key = environment_url
            .as_deref()
            .map(|url| super::helpers::normalize_url(url).0)
            .unwrap_or_else(|| format!("project:{project_id}"));
        let action_key = super::scans::canonical_import_action_key("code")?;
        let execution = self
            .admit_scan_execution(
                NewScanExecution {
                    project_id: Some(project_id),
                    environment_id: None,
                    environment_url: environment_url.clone(),
                    environment_scope_key: environment_scope_key.clone(),
                    requested_mode: ScanExecutionMode::Code,
                    web_focus: None,
                    trigger: ScanTrigger::Migration,
                    admission_class: ScanAdmissionClass::SystemExempt,
                    idempotency_key: action_key.clone(),
                    request_fingerprint: format!("v1:{action_key}"),
                    now_ms: started_at,
                    web_status: None,
                    web_detail: None,
                    code_status: Some(ScanComponentStatus::Planned),
                    code_detail: None,
                },
                crate::constants::SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
            )
            .map_err(|error| DbError::Other(error.to_string()))?
            .execution;
        self.start_scan_execution_component(execution.id, ScanComponent::Code)?;
        let batch = normalize_code_scan_with_provenance(
            report,
            execution.id,
            project_id,
            environment_url,
            environment_scope_key,
            project_path,
            score_report(report),
            duration_ms,
            started_at,
            provenance,
        )?;
        let run_id = match self.persist_normalized_scan_run(batch) {
            Ok(run_id) => run_id,
            Err(error) => {
                let _ = self.finish_scan_execution_component(
                    execution.id,
                    ScanComponent::Code,
                    ScanComponentStatus::Failed,
                    Some(error.to_string()),
                    chrono::Utc::now().timestamp_millis(),
                );
                return Err(error);
            }
        };
        self.finish_scan_execution_component(
            execution.id,
            ScanComponent::Code,
            ScanComponentStatus::Complete,
            None,
            started_at.saturating_add(duration_ms as i64),
        )?;
        Ok(run_id)
    }

    #[tracing::instrument(skip(self), fields(project_id, limit))]
    pub fn get_code_scan_history(
        &self,
        project_id: i64,
        limit: u32,
    ) -> Result<Vec<CodeScanSummary>, DbError> {
        self.execute(move |conn| {
            // Restrict domain aggregation to the paged scan IDs while sharing the
            // timeline backfill's canonical ranking CTE.
            let sql = format!(
                "WITH target_scans AS (
                        SELECT {columns}
                        FROM scan_runs
                        WHERE project_id = ?1
                          AND source = 'code_scan'
                          AND run_kind = 'code'
                          AND status = 'complete'
                        ORDER BY started_at DESC, id DESC
                        LIMIT ?2
                    ),
                    {ranked_domains}
                    SELECT
                        target_scans.id,
                        target_scans.project_id,
                        target_scans.environment_url,
                        target_scans.overall_score,
                        target_scans.issue_count,
                        target_scans.critical_count,
                        target_scans.high_count,
                        target_scans.duration_ms,
                        target_scans.checked_at,
                        target_scans.framework,
                        ranked_domains.domain AS top_domain,
                        COALESCE(ranked_domains.issue_count, 0) AS top_domain_count
                     FROM target_scans
                     LEFT JOIN ranked_domains
                       ON ranked_domains.code_scan_id = target_scans.id
                      AND ranked_domains.row_num = 1
                     ORDER BY target_scans.checked_at DESC, target_scans.id DESC",
                columns = CODE_SCAN_SUMMARY_COLUMNS,
                ranked_domains =
                    ranked_domains_cte(" AND scan_ref IN (SELECT id FROM target_scans)"),
            );
            let mut stmt = conn.prepare(&sql)?;

            let summaries =
                from_row::query_vec::<CodeScanSummary>(&mut stmt, &[&project_id, &limit])?;
            if summaries.is_empty() {
                return Ok(summaries);
            }
            let scan_ids: Vec<i64> = summaries.iter().map(|s| s.id).collect();
            let mut domain_map = load_domain_summaries_for_scans(conn, &scan_ids)?;
            let mut summaries = summaries;
            for summary in &mut summaries {
                summary.domain_summaries = domain_map.remove(&summary.id).unwrap_or_default();
            }
            Ok(summaries)
        })?
    }

    #[tracing::instrument(skip(self), fields(scan_id))]
    pub fn get_code_scan_detail(&self, scan_id: i64) -> Result<Option<CodeScanResult>, DbError> {
        self.execute(move |conn| {
            let scan_row = conn
                .query_row(
                    "SELECT id, project_id, environment_url, raw_score,
                            issues_total, issues_critical, issues_high,
                            issues_medium, issues_low, duration_ms,
                            timestamp_text, framework
                     FROM scan_runs
                     WHERE id = ?1 AND source = 'code_scan' AND run_kind = 'code'",
                    params![scan_id],
                    |row| {
                        Ok(CodeScanResult {
                            id: row.get(0)?,
                            project_id: row.get(1)?,
                            environment_url: row.get(2)?,
                            overall_score: row.get(3)?,
                            issue_count: row.get(4)?,
                            critical_count: row.get(5)?,
                            high_count: row.get(6)?,
                            medium_count: row.get(7)?,
                            low_count: row.get(8)?,
                            duration_ms: row_u64(row, 9)?,
                            checked_at: row.get(10)?,
                            framework: row.get(11)?,
                            domain_summaries: Vec::new(),
                            // Reloaded from history: the skip tally is a
                            // scan-time signal, not persisted, so it is empty.
                            skipped_scopes: Default::default(),
                            issues: Vec::new(),
                        })
                    },
                )
                .optional()?;

            let Some(mut result) = scan_row else {
                return Ok(None);
            };

            result.issues = load_code_scan_issue_views(conn, scan_id)?;
            result.domain_summaries = load_code_scan_domain_summaries(conn, scan_id)?;

            Ok(Some(result))
        })?
    }

    #[tracing::instrument(skip(self), fields(scan_id))]
    pub fn get_code_scan_issue_views(&self, scan_id: i64) -> Result<Vec<CodeIssueView>, DbError> {
        self.execute(move |conn| load_lightweight_code_scan_issue_views(conn, scan_id))?
    }

    #[tracing::instrument(skip(self), fields(scan_id))]
    pub fn get_top_code_scan_issue_view(
        &self,
        scan_id: i64,
    ) -> Result<Option<CodeIssueView>, DbError> {
        self.execute(move |conn| load_top_code_scan_issue_view(conn, scan_id))?
    }

    #[tracing::instrument(skip(self), fields(scan_id))]
    pub fn get_code_scan_overview(&self, scan_id: i64) -> Result<Option<CodeScanResult>, DbError> {
        self.execute(move |conn| {
            let scan_row = conn
                .query_row(
                    "SELECT id, project_id, environment_url, raw_score,
                            issues_total, issues_critical, issues_high,
                            issues_medium, issues_low, duration_ms,
                            timestamp_text, framework
                     FROM scan_runs
                     WHERE id = ?1 AND source = 'code_scan' AND run_kind = 'code'",
                    params![scan_id],
                    |row| {
                        Ok(CodeScanResult {
                            id: row.get(0)?,
                            project_id: row.get(1)?,
                            environment_url: row.get(2)?,
                            overall_score: row.get(3)?,
                            issue_count: row.get(4)?,
                            critical_count: row.get(5)?,
                            high_count: row.get(6)?,
                            medium_count: row.get(7)?,
                            low_count: row.get(8)?,
                            duration_ms: row_u64(row, 9)?,
                            checked_at: row.get(10)?,
                            framework: row.get(11)?,
                            domain_summaries: Vec::new(),
                            // Reloaded from history: the skip tally is a
                            // scan-time signal, not persisted, so it is empty.
                            skipped_scopes: Default::default(),
                            issues: Vec::new(),
                        })
                    },
                )
                .optional()?;

            let Some(mut result) = scan_row else {
                return Ok(None);
            };

            result.domain_summaries = load_code_scan_domain_summaries(conn, scan_id)?;

            Ok(Some(result))
        })?
    }
}

#[cfg(test)]
#[path = "code_scans_tests.rs"]
mod tests;
