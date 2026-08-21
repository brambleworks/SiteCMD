//! Environment-scoped workflow state for unified issues.
//!
//! [`IssueLifecycle`] couples each status to its valid payload so invalid state
//! combinations cannot reach the database.

use super::DbError;
use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use super::helpers::normalize_env_url;
use super::Database;
use crate::core::types_work_items::{IssueStatus, VerifiedBy};

/// (status, snooze_until, block_reason, verified_by) for one issue's lifecycle
/// overlay. `verified_by` is present exactly when the status is `verified`.
pub type IssueStateRow = (IssueStatus, Option<i64>, Option<String>, Option<VerifiedBy>);
pub type IssueStateMap = HashMap<String, IssueStateRow>;

/// Lifecycle state and its status-specific payload.
/// Regression is scan-observed and therefore has no caller-facing constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueLifecycle {
    /// Active again: the issue is back on the user's list.
    Active,
    Snoozed {
        until: i64,
    },
    Ignored,
    Blocked {
        reason: Option<String>,
    },
    /// Fixed, according to `by`. The two provenances are different facts and
    /// the store keeps them apart; see [`VerifiedBy`].
    Verified {
        by: VerifiedBy,
    },
}

impl IssueLifecycle {
    pub fn status(&self) -> IssueStatus {
        match self {
            IssueLifecycle::Active => IssueStatus::New,
            IssueLifecycle::Snoozed { .. } => IssueStatus::Snoozed,
            IssueLifecycle::Ignored => IssueStatus::Ignored,
            IssueLifecycle::Blocked { .. } => IssueStatus::Blocked,
            IssueLifecycle::Verified { .. } => IssueStatus::Verified,
        }
    }

    fn snooze_until(&self) -> Option<i64> {
        match self {
            IssueLifecycle::Snoozed { until } => Some(*until),
            _ => None,
        }
    }

    fn block_reason(&self) -> Option<&str> {
        match self {
            IssueLifecycle::Blocked { reason } => reason.as_deref(),
            _ => None,
        }
    }

    fn verified_by(&self) -> Option<VerifiedBy> {
        match self {
            IssueLifecycle::Verified { by } => Some(*by),
            _ => None,
        }
    }
}

/// Decode the constrained status column strictly into the shared enum.
fn parse_status_column(raw: String) -> Result<IssueStatus, rusqlite::Error> {
    raw.parse().map_err(|e: String| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        )
    })
}

/// Same contract as [`parse_status_column`] for the provenance column.
fn parse_verified_by_column(raw: Option<String>) -> Result<Option<VerifiedBy>, rusqlite::Error> {
    raw.map(|value| {
        value.parse().map_err(|e: String| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            )
        })
    })
    .transpose()
}

/// Shared lifecycle upsert for local and connected decisions.
/// `env_key` must already be canonicalized.
pub(super) fn write_lifecycle_row(
    conn: &Connection,
    project_id: i64,
    env_key: &str,
    check_id: &str,
    lifecycle: &IssueLifecycle,
    now_ms: i64,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO project_issue_states (
            project_id, env_url, check_id, status, snooze_until, block_reason,
            verified_by, last_status_changed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(project_id, env_url, check_id) DO UPDATE SET
            status = excluded.status,
            snooze_until = excluded.snooze_until,
            block_reason = excluded.block_reason,
            verified_by = excluded.verified_by,
            last_status_changed_at = excluded.last_status_changed_at",
        params![
            project_id,
            env_key,
            check_id,
            lifecycle.status().as_str(),
            lifecycle.snooze_until(),
            lifecycle.block_reason(),
            lifecycle.verified_by().map(|by| by.as_str()),
            now_ms,
        ],
    )?;
    Ok(())
}

/// Apply re-observation semantics shared by both issue projection paths.
/// Scan-verified issues regress; user-claimed fixes reactivate. Blocked and
/// snoozed issues remain suppressed, while ignored issues reactivate.
pub(crate) fn reconcile_reobserved_lifecycle<'a>(
    tx: &Transaction<'_>,
    project_id: i64,
    env_url: &str,
    observed_at: i64,
    check_ids: impl Iterator<Item = &'a str>,
) -> Result<(), DbError> {
    let check_ids: std::collections::BTreeSet<&str> = check_ids.collect();
    if check_ids.is_empty() {
        return Ok(());
    }
    tx.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS temp_reobserved_check_ids (
             check_id TEXT PRIMARY KEY
         ) WITHOUT ROWID;
         DELETE FROM temp_reobserved_check_ids;",
    )?;
    for check_id in check_ids {
        tx.execute(
            "INSERT OR IGNORE INTO temp_reobserved_check_ids(check_id) VALUES (?1)",
            params![check_id],
        )?;
    }

    for verified_by in VerifiedBy::ALL {
        let next = if verified_by.proves_absence() {
            IssueStatus::Regressed
        } else {
            IssueStatus::New
        };
        tx.execute(
            "UPDATE project_issue_states
             SET status = ?1,
                 verified_by = NULL,
                 last_status_changed_at = ?2
             WHERE project_id = ?3 AND env_url = ?4
               AND status = ?5 AND verified_by = ?6
               AND check_id IN (SELECT check_id FROM temp_reobserved_check_ids)",
            params![
                next.as_str(),
                observed_at,
                project_id,
                env_url,
                IssueStatus::Verified.as_str(),
                verified_by.as_str(),
            ],
        )?;
    }

    tx.execute(
        "UPDATE project_issue_states
         SET status = ?1,
             snooze_until = NULL,
             block_reason = NULL,
             last_status_changed_at = ?2
         WHERE project_id = ?3 AND env_url = ?4 AND status = ?5
           AND check_id IN (SELECT check_id FROM temp_reobserved_check_ids)",
        params![
            IssueStatus::New.as_str(),
            observed_at,
            project_id,
            env_url,
            IssueStatus::Ignored.as_str(),
        ],
    )?;
    tx.execute("DELETE FROM temp_reobserved_check_ids", [])?;
    Ok(())
}

impl Database {
    #[tracing::instrument(skip(self, env_url), fields(project_id))]
    pub fn get_issue_state_map(
        &self,
        project_id: i64,
        env_url: Option<&str>,
    ) -> Result<IssueStateMap, DbError> {
        let env_key = normalize_env_url(env_url);
        if env_key.is_empty() {
            // Project-wide grouping can combine multiple environments under the same
            // check_id, so applying env-scoped state here would be ambiguous.
            return Ok(HashMap::new());
        }

        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT check_id, status, snooze_until, block_reason, verified_by
                     FROM project_issue_states
                     WHERE project_id = ?1 AND env_url = ?2",
            )?;

            let rows = stmt.query_map(params![project_id, env_key], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    parse_status_column(row.get::<_, String>(1)?)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    parse_verified_by_column(row.get::<_, Option<String>>(4)?)?,
                ))
            })?;

            let mut by_check_id: IssueStateMap = HashMap::new();
            for row in rows {
                let (check_id, status, snooze_until, block_reason, verified_by) = row?;
                by_check_id.insert(check_id, (status, snooze_until, block_reason, verified_by));
            }
            Ok(by_check_id)
        })?
    }

    /// Single-row lifecycle lookup for one (project, env, check_id), returning
    /// (status, snooze_until, block_reason, verified_by). Returns None for a
    /// project-wide (empty env) lookup since env-scoped state is ambiguous there.
    #[tracing::instrument(skip(self, env_url), fields(project_id, check_id = %check_id))]
    pub fn get_issue_state(
        &self,
        project_id: i64,
        env_url: Option<&str>,
        check_id: &str,
    ) -> Result<Option<IssueStateRow>, DbError> {
        let env_key = normalize_env_url(env_url);
        if env_key.is_empty() {
            return Ok(None);
        }
        let check_id = check_id.to_string();
        self.execute(move |conn| {
            conn.query_row(
                "SELECT status, snooze_until, block_reason, verified_by
                 FROM project_issue_states
                 WHERE project_id = ?1 AND env_url = ?2 AND check_id = ?3",
                params![project_id, env_key, check_id],
                |row| {
                    Ok((
                        parse_status_column(row.get::<_, String>(0)?)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        parse_verified_by_column(row.get::<_, Option<String>>(3)?)?,
                    ))
                },
            )
            .optional()
            .map_err(DbError::from)
        })?
    }

    #[tracing::instrument(skip(self, env_url), fields(project_id, check_id = %check_id, lifecycle = ?lifecycle, now_ms))]
    pub fn set_issue_state(
        &self,
        project_id: i64,
        env_url: &str,
        check_id: &str,
        lifecycle: IssueLifecycle,
        now_ms: i64,
    ) -> Result<(), DbError> {
        let env_key = normalize_env_url(Some(env_url));
        if env_key.is_empty() {
            return Err("env_url is required for issue state updates".into());
        }

        let check_id = check_id.to_string();
        self.execute(move |conn| {
            write_lifecycle_row(conn, project_id, &env_key, &check_id, &lifecycle, now_ms)
        })?
    }

    /// Apply one lifecycle state to the canonical issue group. Code locations
    /// already share a rule-level check id, so this is one direct write; no
    /// sibling discovery or per-location fan-out is allowed here.
    #[tracing::instrument(skip(self, env_url), fields(project_id, check_id = %check_id, lifecycle = ?lifecycle))]
    pub fn set_issue_group_state(
        &self,
        project_id: i64,
        env_url: &str,
        check_id: &str,
        lifecycle: IssueLifecycle,
        now_ms: i64,
    ) -> Result<(), DbError> {
        crate::core::code_scan::validate_canonical_check_id(check_id).map_err(DbError::Other)?;
        self.set_issue_state(project_id, env_url, check_id, lifecycle, now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::Severity;
    use crate::db::test_helpers::temp_db;
    use crate::db::work_items::WorkItemMetadata;

    #[test]
    fn issue_state_is_scoped_by_environment() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Issue State Test", "/tmp/issue-state-test", Some("nextjs"))
            .expect("upsert");

        db.set_issue_state(
            project_id,
            "https://example.com",
            "security.csp",
            IssueLifecycle::Ignored,
            1_000,
        )
        .expect("set prod state");
        db.set_issue_state(
            project_id,
            "https://staging.example.com",
            "security.csp",
            IssueLifecycle::Blocked {
                reason: Some("waiting on deploy".to_string()),
            },
            2_000,
        )
        .expect("set staging state");

        let prod = db
            .get_issue_state_map(project_id, Some("https://example.com"))
            .expect("load prod states");
        let staging = db
            .get_issue_state_map(project_id, Some("https://staging.example.com"))
            .expect("load staging states");

        assert_eq!(
            prod.get("security.csp"),
            Some(&(IssueStatus::Ignored, None, None, None))
        );
        assert_eq!(
            staging.get("security.csp"),
            Some(&(
                IssueStatus::Blocked,
                None,
                Some("waiting on deploy".to_string()),
                None
            ))
        );
    }

    #[test]
    fn every_writable_lifecycle_round_trips_through_the_store() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Round Trip", "/tmp/round-trip", Some("nextjs"))
            .expect("upsert");

        let lifecycles = [
            IssueLifecycle::Active,
            IssueLifecycle::Snoozed { until: 9_000 },
            IssueLifecycle::Ignored,
            IssueLifecycle::Blocked {
                reason: Some("intended".to_string()),
            },
            IssueLifecycle::Verified {
                by: VerifiedBy::UserClaim,
            },
            IssueLifecycle::Verified {
                by: VerifiedBy::LocalScan,
            },
        ];
        let mut written = std::collections::HashSet::new();
        for (index, lifecycle) in lifecycles.into_iter().enumerate() {
            let check_id = format!("security.check-{index}");
            db.set_issue_state(
                project_id,
                "https://example.com",
                &check_id,
                lifecycle.clone(),
                1_000,
            )
            .unwrap_or_else(|e| panic!("set {lifecycle:?}: {e}"));
            let got = db
                .get_issue_state(project_id, Some("https://example.com"), &check_id)
                .expect("get state")
                .expect("state exists");
            assert_eq!(got.0, lifecycle.status(), "{lifecycle:?} must round-trip");
            assert_eq!(got.1, lifecycle.snooze_until());
            assert_eq!(got.2.as_deref(), lifecycle.block_reason());
            assert_eq!(
                got.3,
                lifecycle.verified_by(),
                "{lifecycle:?} must round-trip who verified it"
            );
            written.insert(lifecycle.status());
        }
        let writable: std::collections::HashSet<IssueStatus> = IssueStatus::ALL
            .into_iter()
            .filter(|status| *status != IssueStatus::Regressed)
            .collect();
        assert_eq!(
            written, writable,
            "every status except Regressed must be writable through IssueLifecycle"
        );
    }

    /// The CHECK constraint is the last line of defense against a raw write
    /// bypassing the enum (there should be none, but SQL is stringly-typed).
    #[test]
    fn check_constraint_rejects_unknown_status_strings() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Check Reject", "/tmp/check-reject", Some("nextjs"))
            .expect("upsert");

        let result = db.execute(move |conn| {
            conn.execute(
                "INSERT INTO project_issue_states (
                    project_id, env_url, check_id, status, last_status_changed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    project_id,
                    "https://example.com",
                    "security.csp",
                    "wontfix",
                    1_000
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        });
        let err = result
            .map_err(String::from)
            .and_then(|inner: Result<(), String>| inner)
            .expect_err("unknown status must violate the CHECK constraint");
        assert!(
            err.contains("CHECK") || err.contains("constraint"),
            "expected a CHECK constraint violation, got: {err}"
        );
    }

    /// A raw write is the one way an unprovenanced verification could still
    /// appear - the reconcilers ARE raw writes - so the invariant lives in the
    /// schema as well as in the type.
    #[test]
    fn the_schema_refuses_a_verified_row_with_no_prover() {
        let db = temp_db();
        let project_id = db
            .upsert_project("No Prover", "/tmp/no-prover", Some("nextjs"))
            .expect("upsert");

        let err = raw_insert(&db, project_id, "verified", None)
            .expect_err("a verified row must name who verified it");
        assert!(
            err.contains("CHECK") || err.contains("constraint"),
            "expected a CHECK constraint violation, got: {err}"
        );
    }

    /// The other direction: a prover that outlives the verification it
    /// described would let a regressed or reopened row still look proven.
    #[test]
    fn the_schema_refuses_a_prover_on_a_row_that_is_not_verified() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Stale Prover", "/tmp/stale-prover", Some("nextjs"))
            .expect("upsert");

        let err = raw_insert(&db, project_id, "regressed", Some("local_scan"))
            .expect_err("only a verified row may carry a prover");
        assert!(
            err.contains("CHECK") || err.contains("constraint"),
            "expected a CHECK constraint violation, got: {err}"
        );
    }

    /// A claim followed by real evidence is an upgrade, not a second row: the
    /// scan overwrites the provenance so the issue stops being described as
    /// merely claimed.
    #[test]
    fn a_scan_verification_replaces_the_users_claim_on_the_same_row() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Upgrade", "/tmp/upgrade", Some("nextjs"))
            .expect("upsert");

        for by in [VerifiedBy::UserClaim, VerifiedBy::LocalScan] {
            db.set_issue_state(
                project_id,
                "https://example.com",
                "security.csp",
                IssueLifecycle::Verified { by },
                1_000,
            )
            .expect("set verified");
        }

        let got = db
            .get_issue_state(project_id, Some("https://example.com"), "security.csp")
            .expect("get")
            .expect("row exists");
        assert_eq!(got.0, IssueStatus::Verified);
        assert_eq!(got.3, Some(VerifiedBy::LocalScan));
    }

    /// Raw INSERT bypassing the write API, returning the SQL error text.
    fn raw_insert(
        db: &crate::db::Database,
        project_id: i64,
        status: &str,
        verified_by: Option<&str>,
    ) -> Result<(), String> {
        let status = status.to_string();
        let verified_by = verified_by.map(str::to_string);
        db.execute(move |conn| {
            conn.execute(
                "INSERT INTO project_issue_states (
                    project_id, env_url, check_id, status, verified_by,
                    last_status_changed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    project_id,
                    "https://example.com",
                    "security.csp",
                    status,
                    verified_by,
                    1_000
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })
        .map_err(String::from)
        .and_then(|inner: Result<(), String>| inner)
    }

    #[test]
    fn project_wide_issue_state_lookup_returns_empty_map() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Issue State Test", "/tmp/issue-state-test", Some("nextjs"))
            .expect("upsert");

        db.set_issue_state(
            project_id,
            "https://example.com",
            "security.csp",
            IssueLifecycle::Ignored,
            1_000,
        )
        .expect("set state");

        let states = db
            .get_issue_state_map(project_id, None)
            .expect("load project-wide states");
        assert!(
            states.is_empty(),
            "project-wide lookups should not merge env-scoped issue state"
        );
    }

    #[test]
    fn get_single_issue_state_round_trips() {
        let db = temp_db();
        let project_id = db
            .upsert_project("t", "/tmp/t", Some("nextjs"))
            .expect("upsert");
        db.set_issue_state(
            project_id,
            "https://example.com",
            "security.csp",
            IssueLifecycle::Blocked {
                reason: Some("intended".to_string()),
            },
            1_000,
        )
        .expect("set");

        let got = db
            .get_issue_state(project_id, Some("https://example.com"), "security.csp")
            .expect("get");
        assert_eq!(
            got,
            Some((
                IssueStatus::Blocked,
                None,
                Some("intended".to_string()),
                None
            ))
        );

        let missing = db
            .get_issue_state(project_id, Some("https://example.com"), "seo.title")
            .expect("get missing");
        assert_eq!(missing, None);
    }

    use crate::db::work_items::WorkItemInput;

    fn code_item(signal: &str, check_id: &str) -> WorkItemInput {
        WorkItemInput {
            project_id: 1,
            env_url: "https://example.com".into(),
            source: "code_scan".into(),
            signal_id: signal.into(),
            check_id: check_id.into(),
            category: "code_quality".into(),
            severity: Severity::Medium,
            title: "Database query inside a loop creates an N+1 problem".into(),
            description: "N+1 query".into(),
            detail_json: None,
            scan_ref: None,
            page_url: None,
            fix_prompt: None,
            manual_fix: None,
            why_it_matters: None,
            observed_at: 1_000,
            metadata: WorkItemMetadata::default(),
        }
    }

    /// Multiple Code occurrences share one canonical id, so a lifecycle action
    /// writes exactly one state row without sibling discovery or fan-out.
    #[test]
    fn code_group_lifecycle_is_one_direct_canonical_write() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Group Block", "/tmp/group-block", Some("astro"))
            .expect("upsert");

        // Two locations of one rule (one issues-list row) plus an unrelated web
        // issue that must stay active.
        db.upsert_work_items_diff(
            "code_scan",
            project_id,
            "https://example.com",
            vec![
                code_item("code_scan:nplus:a", "code_scan.n-plus-one-query"),
                code_item("code_scan:nplus:b", "code_scan.n-plus-one-query"),
            ],
            1_000,
        )
        .expect("seed code items");

        db.set_issue_group_state(
            project_id,
            "https://example.com",
            "code_scan.n-plus-one-query",
            IssueLifecycle::Blocked {
                reason: Some("not relevant".to_string()),
            },
            2_000,
        )
        .expect("block group");

        let states = db
            .get_issue_state_map(project_id, Some("https://example.com"))
            .expect("load states");
        assert_eq!(
            states
                .get("code_scan.n-plus-one-query")
                .map(|row| row.0.as_str()),
            Some("blocked"),
            "the canonical group is blocked"
        );
        assert_eq!(states.len(), 1, "one canonical group writes one state row");

        db.set_issue_group_state(
            project_id,
            "https://example.com",
            "code_scan.n-plus-one-query",
            IssueLifecycle::Active,
            3_000,
        )
        .expect("reopen group");
        let states = db
            .get_issue_state_map(project_id, Some("https://example.com"))
            .expect("reload states");
        assert_eq!(
            states
                .get("code_scan.n-plus-one-query")
                .map(|row| row.0.as_str()),
            Some("new"),
            "reopening updates the canonical group"
        );
    }

    #[test]
    fn lifecycle_rejects_path_bearing_code_identity() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Canonical State", "/tmp/canonical-state", Some("astro"))
            .expect("upsert");
        let error = db
            .set_issue_group_state(
                project_id,
                "https://example.com",
                "code_scan.n-plus-one-query:src/db.ts",
                IssueLifecycle::Ignored,
                1_000,
            )
            .expect_err("lifecycle must not normalize legacy identities");
        assert!(error.to_string().contains("is not canonical"));
    }
}
