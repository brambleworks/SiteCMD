//! Durable outbox for connected lifecycle decisions.
//!
//! A user decision updates local lifecycle and outbound intent atomically.
//! Scan-proved verification and regression remain evidence, not mutations.

use rusqlite::{params, OptionalExtension};

use super::connected_sites::{connected_site, group_revision, raise_group_revision};
use super::helpers::{lifecycle_env_url, mint_local_id};
use super::issue_states::write_lifecycle_row;
use super::{Database, DbError, IssueLifecycle};
use crate::core::types_work_items::VerifiedBy;

/// A user lifecycle decision and its outbound payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupDecision {
    /// Explicitly return the issue to the active list.
    Reopen,
    Snooze {
        until: i64,
    },
    Ignore,
    Block {
        reason: Option<String>,
    },
    /// Record the user's claim that the issue is fixed.
    ClaimFixed,
}

impl GroupDecision {
    /// The local lifecycle written by this decision.
    pub fn lifecycle(&self) -> IssueLifecycle {
        match self {
            GroupDecision::Reopen => IssueLifecycle::Active,
            GroupDecision::Snooze { until } => IssueLifecycle::Snoozed { until: *until },
            GroupDecision::Ignore => IssueLifecycle::Ignored,
            GroupDecision::Block { reason } => IssueLifecycle::Blocked {
                reason: reason.clone(),
            },
            GroupDecision::ClaimFixed => IssueLifecycle::Verified {
                by: VerifiedBy::UserClaim,
            },
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            GroupDecision::Reopen => "reopen",
            GroupDecision::Snooze { .. } => "snooze",
            GroupDecision::Ignore => "ignore",
            GroupDecision::Block { .. } => "block",
            GroupDecision::ClaimFixed => "claim_fixed",
        }
    }

    fn snooze_until(&self) -> Option<i64> {
        match self {
            GroupDecision::Snooze { until } => Some(*until),
            _ => None,
        }
    }

    fn block_reason(&self) -> Option<&str> {
        match self {
            GroupDecision::Block { reason } => reason.as_deref(),
            _ => None,
        }
    }

    /// Decode a persisted decision strictly; schema and enum drift is an error.
    fn from_row(
        decision: &str,
        snooze_until: Option<i64>,
        block_reason: Option<String>,
    ) -> rusqlite::Result<Self> {
        let invalid = |what: &str| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    what.to_string(),
                )),
            )
        };
        match decision {
            "reopen" => Ok(GroupDecision::Reopen),
            "snooze" => snooze_until
                .map(|until| GroupDecision::Snooze { until })
                .ok_or_else(|| invalid("a snooze decision with no deadline")),
            "ignore" => Ok(GroupDecision::Ignore),
            "block" => Ok(GroupDecision::Block {
                reason: block_reason,
            }),
            "claim_fixed" => Ok(GroupDecision::ClaimFixed),
            other => Err(invalid(&format!("unknown recorded decision '{other}'"))),
        }
    }
}

/// Service state returned when a recorded decision is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationConflict {
    pub state: String,
    pub revision: i64,
    pub at: i64,
}

/// One undelivered decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMutation {
    pub id: i64,
    pub check_id: String,
    pub decision: GroupDecision,
    /// The group revision in force when the user decided, never a later one.
    pub based_on_revision: i64,
    /// Minted at decision time so a retry after a crash mid-send is the same
    /// request rather than a second decision.
    pub idempotency_key: String,
    pub decided_at: i64,
    pub conflict: Option<MutationConflict>,
}

/// What recording a decision did beyond writing the lifecycle row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionRecord {
    /// This environment is not connected, so nothing is owed to anyone.
    LocalOnly,
    /// Connected, but bootstrap has not committed. The bootstrap payload
    /// carries this group's state already.
    CarriedByBootstrap,
    Recorded(PendingMutation),
}

const OUTBOX_COLUMNS: &str = "id, check_id, decision, snooze_until, block_reason, \
     based_on_revision, idempotency_key, decided_at, conflicted_at, conflict_state, \
     conflict_revision";

fn pending_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PendingMutation> {
    let conflicted_at: Option<i64> = row.get(8)?;
    Ok(PendingMutation {
        id: row.get(0)?,
        check_id: row.get(1)?,
        decision: GroupDecision::from_row(
            &row.get::<_, String>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<String>>(4)?,
        )?,
        based_on_revision: row.get(5)?,
        idempotency_key: row.get(6)?,
        decided_at: row.get(7)?,
        conflict: conflicted_at
            .map(|at| -> rusqlite::Result<MutationConflict> {
                Ok(MutationConflict {
                    state: row.get(9)?,
                    revision: row.get(10)?,
                    at,
                })
            })
            .transpose()?,
    })
}

impl Database {
    /// Record the local lifecycle decision and its pending connected mutation.
    /// Capture the revision at decision time, and replace any unsent decision
    /// for the same group with a fresh idempotency key.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id, check_id = %check_id, decision = ?decision))]
    pub fn record_group_decision(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        check_id: &str,
        decision: GroupDecision,
        now_ms: i64,
    ) -> Result<DecisionRecord, DbError> {
        crate::core::code_scan::validate_canonical_check_id(check_id).map_err(DbError::Other)?;
        let env_url = lifecycle_env_url(environment_scope_key);
        if env_url.is_empty() {
            return Err("an environment is required for a lifecycle decision".into());
        }
        let check_id = check_id.to_string();
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let lifecycle = decision.lifecycle();
            write_lifecycle_row(&tx, project_id, &env_url, &check_id, &lifecycle, now_ms)?;

            let Some(site) = connected_site(&tx, project_id, &env_url)? else {
                tx.commit()?;
                return Ok(DecisionRecord::LocalOnly);
            };
            if !site.accepts_mutations() {
                tx.commit()?;
                return Ok(DecisionRecord::CarriedByBootstrap);
            }

            let based_on_revision = group_revision(&tx, project_id, &env_url, &check_id)?;
            let idempotency_key = mint_local_id("mut_")?;
            let id: i64 = tx.query_row(
                "INSERT INTO connected_mutation_outbox
                    (project_id, env_url, check_id, decision, snooze_until, block_reason,
                     based_on_revision, idempotency_key, decided_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(project_id, env_url, check_id) DO UPDATE SET
                     decision = excluded.decision,
                     snooze_until = excluded.snooze_until,
                     block_reason = excluded.block_reason,
                     based_on_revision = excluded.based_on_revision,
                     idempotency_key = excluded.idempotency_key,
                     decided_at = excluded.decided_at,
                     conflicted_at = NULL,
                     conflict_state = NULL,
                     conflict_revision = NULL
                 RETURNING id",
                params![
                    project_id,
                    env_url,
                    check_id,
                    decision.as_str(),
                    decision.snooze_until(),
                    decision.block_reason(),
                    based_on_revision,
                    idempotency_key,
                    now_ms,
                ],
                |row| row.get(0),
            )?;
            tx.commit()?;
            Ok(DecisionRecord::Recorded(PendingMutation {
                id,
                check_id,
                decision,
                based_on_revision,
                idempotency_key,
                decided_at: now_ms,
                conflict: None,
            }))
        })?
    }

    /// Return undelivered decisions in order, including conflicts awaiting user action.
    #[tracing::instrument(skip(self, environment_scope_key), fields(project_id))]
    pub fn pending_group_mutations(
        &self,
        project_id: i64,
        environment_scope_key: &str,
    ) -> Result<Vec<PendingMutation>, DbError> {
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {OUTBOX_COLUMNS} FROM connected_mutation_outbox
                 WHERE project_id = ?1 AND env_url = ?2 ORDER BY id"
            ))?;
            let rows = stmt.query_map(params![project_id, env_url], pending_from_row)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })?
    }

    /// Records the service revision and removes only the decision matching the
    /// acknowledged idempotency key. Returns whether a row was settled.
    #[tracing::instrument(skip(self, idempotency_key), fields(id, server_revision))]
    pub fn settle_group_mutation(
        &self,
        id: i64,
        idempotency_key: &str,
        server_revision: i64,
        now_ms: i64,
    ) -> Result<bool, DbError> {
        if server_revision < 0 {
            return Err("a mutation receipt revision cannot be negative".into());
        }
        let idempotency_key = idempotency_key.to_string();
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            let target: Option<(i64, String, String)> = tx
                .query_row(
                    "SELECT project_id, env_url, check_id FROM connected_mutation_outbox
                     WHERE id = ?1 AND idempotency_key = ?2",
                    params![id, idempotency_key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let Some((project_id, env_url, check_id)) = target else {
                return Ok(false);
            };
            raise_group_revision(
                &tx,
                project_id,
                &env_url,
                &check_id,
                server_revision,
                now_ms,
            )?;
            let deleted = tx.execute(
                "DELETE FROM connected_mutation_outbox WHERE id = ?1 AND idempotency_key = ?2",
                params![id, idempotency_key],
            )?;
            tx.commit()?;
            Ok(deleted > 0)
        })?
    }

    /// Record a stale-decision conflict and advance to the service's revision.
    /// The idempotency key must match the pending mutation.
    #[tracing::instrument(skip(self, idempotency_key, server_state), fields(id, server_revision))]
    pub fn record_mutation_conflict(
        &self,
        id: i64,
        idempotency_key: &str,
        server_state: &str,
        server_revision: i64,
        now_ms: i64,
    ) -> Result<bool, DbError> {
        if server_state.is_empty() {
            return Err("a conflict must name the state the service reported".into());
        }
        let idempotency_key = idempotency_key.to_string();
        let server_state = server_state.to_string();
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let target: Option<(i64, String, String)> = tx
                .query_row(
                    "SELECT project_id, env_url, check_id FROM connected_mutation_outbox
                     WHERE id = ?1 AND idempotency_key = ?2",
                    params![id, idempotency_key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let Some((project_id, env_url, check_id)) = target else {
                return Ok(false);
            };
            tx.execute(
                "UPDATE connected_mutation_outbox
                    SET conflicted_at = ?3, conflict_state = ?4, conflict_revision = ?5
                  WHERE id = ?1 AND idempotency_key = ?2",
                params![id, idempotency_key, now_ms, server_state, server_revision],
            )?;
            raise_group_revision(
                &tx,
                project_id,
                &env_url,
                &check_id,
                server_revision,
                now_ms,
            )?;
            tx.commit()?;
            Ok(true)
        })?
    }
}

#[cfg(test)]
#[path = "connected_outbox_tests.rs"]
mod tests;
