//! Issue links CRUD - tracks external tracker tickets linked to scan check results.

use super::DbError;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::from_row::{self, FromRow};
use super::Database;
use ts_rs::TS;

/// Column list for `IssueLink` reads, in the order `IssueLink::from_row`
/// (db/from_row.rs) expects. Shared by all three link queries so the SELECT
/// list and the mapper can't drift apart.
const ISSUE_LINK_COLUMNS: &str = "id, project_id, check_id, run_id AS scan_id, provider, \
     external_id, external_url, status, created_at, resolved_at";

/// A link between a scan check result and an external issue tracker ticket.
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct IssueLink {
    pub id: i64,
    pub project_id: i64,
    pub check_id: String,
    pub scan_id: i64,
    pub provider: String,
    pub external_id: String,
    pub external_url: String,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

impl Database {
    /// Prevent pairing a Web run with another project's tracker credentials.
    pub fn scan_run_belongs_to_project(
        &self,
        project_id: i64,
        run_id: i64,
    ) -> Result<bool, DbError> {
        self.execute(move |conn| {
            conn.query_row(
                "SELECT EXISTS(
                     SELECT 1
                     FROM scan_runs
                     WHERE id = ?1 AND project_id = ?2 AND source = 'web_scan'
                 )",
                params![run_id, project_id],
                |row| row.get(0),
            )
            .map_err(DbError::from)
        })?
    }

    /// Create a new issue link against the canonical evidence run.
    #[tracing::instrument(skip(self, external_url), fields(project_id, check_id = %check_id, run_id, provider = %provider, external_id = %external_id))]
    pub fn create_issue_link(
        &self,
        project_id: i64,
        check_id: &str,
        run_id: i64,
        provider: &str,
        external_id: &str,
        external_url: &str,
    ) -> Result<i64, DbError> {
        let check_id = check_id.to_string();
        let provider = provider.to_string();
        let external_id = external_id.to_string();
        let external_url = external_url.to_string();
        let created_at = Utc::now().to_rfc3339();

        self.execute(move |conn| {
            conn.execute(
                "INSERT INTO issue_links
                    (project_id, check_id, run_id, provider, external_id, external_url, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7)",
                params![project_id, check_id, run_id, provider, external_id, external_url, created_at],
            )
            ?;

            Ok(conn.last_insert_rowid())
        })?
    }

    /// Get all issue links for a project, ordered newest first.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_issue_links(&self, project_id: i64) -> Result<Vec<IssueLink>, DbError> {
        self.execute(move |conn| {
            let sql = format!(
                "SELECT {ISSUE_LINK_COLUMNS}
                     FROM issue_links
                     WHERE project_id = ?1
                     ORDER BY created_at DESC"
            );
            let mut stmt = conn.prepare(&sql)?;
            from_row::query_vec::<IssueLink>(&mut stmt, &[&project_id])
        })?
    }

    /// Get only open issue links for a project.
    #[tracing::instrument(skip(self), fields(project_id))]
    pub fn get_open_issue_links(&self, project_id: i64) -> Result<Vec<IssueLink>, DbError> {
        self.execute(move |conn| {
            let sql = format!(
                "SELECT {ISSUE_LINK_COLUMNS}
                     FROM issue_links
                     WHERE project_id = ?1 AND status = 'open'
                     ORDER BY created_at DESC"
            );
            let mut stmt = conn.prepare(&sql)?;
            from_row::query_vec::<IssueLink>(&mut stmt, &[&project_id])
        })?
    }

    /// Mark an issue link as resolved with the current timestamp.
    #[tracing::instrument(skip(self), fields(link_id))]
    pub fn resolve_issue_link(&self, link_id: i64) -> Result<(), DbError> {
        let resolved_at = Utc::now().to_rfc3339();
        self.execute(move |conn| {
            conn.execute(
                "UPDATE issue_links
                 SET status = 'resolved', resolved_at = ?1
                 WHERE id = ?2",
                params![resolved_at, link_id],
            )?;
            Ok(())
        })?
    }

    /// Returns the newest link for an exact project, check, scan, and provider
    /// identity so filing retries remain idempotent.
    #[tracing::instrument(skip(self), fields(project_id, check_id = %check_id, scan_id))]
    pub fn get_issue_link_for_attempt(
        &self,
        project_id: i64,
        check_id: &str,
        scan_id: i64,
        provider: &str,
    ) -> Result<Option<IssueLink>, DbError> {
        let check_id = check_id.to_string();
        let provider = provider.to_string();
        self.execute(move |conn| {
            let sql = format!(
                "SELECT {ISSUE_LINK_COLUMNS}
                     FROM issue_links
                     WHERE project_id = ?1 AND check_id = ?2 AND run_id = ?3 AND provider = ?4
                     ORDER BY created_at DESC
                     LIMIT 1"
            );
            let mut stmt = conn.prepare(&sql)?;
            stmt.query_row(
                params![project_id, check_id, scan_id, provider],
                IssueLink::from_row,
            )
            .optional()
            .map_err(DbError::from)
        })?
    }

    /// Get the most recent issue link for a specific check on a project.
    #[tracing::instrument(skip(self), fields(project_id, check_id = %check_id))]
    pub fn get_issue_link_for_check(
        &self,
        project_id: i64,
        check_id: &str,
    ) -> Result<Option<IssueLink>, DbError> {
        let check_id = check_id.to_string();
        self.execute(move |conn| {
            let sql = format!(
                "SELECT {ISSUE_LINK_COLUMNS}
                     FROM issue_links
                     WHERE project_id = ?1 AND check_id = ?2
                     ORDER BY created_at DESC
                     LIMIT 1"
            );
            let mut stmt = conn.prepare(&sql)?;
            stmt.query_row(params![project_id, check_id], IssueLink::from_row)
                .optional()
                .map_err(DbError::from)
        })?
    }
}
