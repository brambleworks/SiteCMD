//! Snapshot-oriented Code Scan summaries, domain hydration, and count keys.

use super::DbError;
use rusqlite::params;

use crate::core::code_scan::{code_issue_domain, CodeIssue, CodeIssueCountKey, CodeScanDomain};

use super::code_scans::{
    load_domain_summaries_for_scans, CODE_SCAN_ISSUE_ROWS_CTE, CODE_SCAN_SUMMARY_COLUMNS,
};
use super::from_row;
use super::types::CodeScanSummary;
use super::Database;

/// Recover a missing legacy domain from the immutable payload.
fn legacy_row_domain(conn: &rusqlite::Connection, row_id: i64) -> Result<CodeScanDomain, DbError> {
    let (domain, detail_json) = conn.query_row(
        "SELECT domain, detail_json FROM scan_findings WHERE id = ?1",
        params![row_id],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        },
    )?;
    if let Some(value) = domain.filter(|value| !value.is_empty()) {
        return value.parse::<CodeScanDomain>().map_err(|error| {
            DbError::Other(format!(
                "invalid domain '{value}' on migrated Code Scan finding {row_id}: {error}"
            ))
        });
    }
    let json = detail_json.ok_or_else(|| {
        DbError::Other(format!(
            "migrated Code Scan finding {row_id} has neither a domain nor detail_json"
        ))
    })?;
    let issue: CodeIssue = serde_json::from_str(&json)?;
    Ok(code_issue_domain(&issue))
}

impl Database {
    /// Read canonical Code Scan summaries without domain aggregation.
    /// Callers can hydrate domains only for the selected snapshots.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_code_scan_summaries_lean(
        &self,
        project_id: i64,
        limit: u32,
    ) -> Result<Vec<CodeScanSummary>, DbError> {
        self.execute(move |conn| {
            let sql = format!(
                "SELECT {columns}, NULL AS top_domain, 0 AS top_domain_count
                     FROM scan_runs
                     WHERE project_id = ?1
                       AND source = 'code_scan'
                       AND run_kind = 'code'
                       AND status = 'complete'
                     ORDER BY started_at DESC, id DESC
                     LIMIT ?2",
                columns = CODE_SCAN_SUMMARY_COLUMNS,
            );
            let mut stmt = conn.prepare(&sql)?;
            from_row::query_vec::<CodeScanSummary>(&mut stmt, &[&project_id, &limit])
        })?
    }

    /// Hydrate domain summaries and top domains for selected scans.
    #[tracing::instrument(skip(self, summaries), fields(count = summaries.len()))]
    pub fn hydrate_code_scan_domain_data(
        &self,
        mut summaries: Vec<CodeScanSummary>,
    ) -> Result<Vec<CodeScanSummary>, DbError> {
        if summaries.is_empty() {
            return Ok(summaries);
        }
        self.execute(move |conn| {
            let scan_ids: Vec<i64> = summaries.iter().map(|s| s.id).collect();
            let mut domain_map = load_domain_summaries_for_scans(conn, &scan_ids)?;
            for summary in &mut summaries {
                summary.domain_summaries = domain_map.remove(&summary.id).unwrap_or_default();
                // Keep the highest count, preserving rank order on ties.
                let mut top: Option<(CodeScanDomain, u32)> = None;
                for candidate in &summary.domain_summaries {
                    if top.is_none_or(|(_, count)| candidate.issue_count > count) {
                        top = Some((candidate.domain, candidate.issue_count));
                    }
                }
                if let Some((domain, count)) = top {
                    summary.top_domain = Some(domain);
                    summary.top_domain_count = count;
                }
            }
            Ok(summaries)
        })?
    }

    /// Read grouped count keys from snapshot columns.
    /// Legacy rows without a parseable domain fall back to `detail_json`.
    #[tracing::instrument(skip(self), fields(scan_id))]
    pub fn get_code_scan_issue_count_keys(
        &self,
        scan_id: i64,
    ) -> Result<Vec<CodeIssueCountKey>, DbError> {
        self.execute(move |conn| {
            let sql = format!(
                "WITH {CODE_SCAN_ISSUE_ROWS_CTE}
                 SELECT row_id, check_id, severity, title, domain
                 FROM code_scan_issue_rows
                 WHERE scan_ref = ?1
                 ORDER BY ordinal"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![scan_id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let mut result = Vec::with_capacity(rows.len());
            for (row_id, check_id, severity, title, domain_str) in rows {
                let domain = match domain_str.filter(|value| !value.is_empty()) {
                    Some(value) => value.parse::<CodeScanDomain>().map_err(|error| {
                        DbError::Other(format!(
                            "invalid domain '{value}' in Code Scan {scan_id} issue snapshot: {error}"
                        ))
                    })?,
                    None => legacy_row_domain(conn, row_id)?,
                };
                let severity = severity.parse().map_err(|error: String| {
                    DbError::Other(format!(
                        "invalid severity in Code Scan {scan_id} issue snapshot: {error}"
                    ))
                })?;
                result.push(CodeIssueCountKey {
                    check_id,
                    domain,
                    severity,
                    title,
                });
            }
            Ok(result)
        })?
    }
}
