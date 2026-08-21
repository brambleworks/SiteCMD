//! Durable producer identity, submission ordering, and scan-start pull basis.

use rusqlite::{params, Connection, OptionalExtension};

use super::helpers::lifecycle_env_url;
use super::{Database, DbError};

/// A durably allocated submission sequence paired with its installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionTicket {
    installation_id: String,
    sequence: i64,
}

impl SubmissionTicket {
    /// The stable installation identity this counter is keyed to.
    pub fn installation_id(&self) -> &str {
        &self.installation_id
    }

    /// The installation's monotonic submission sequence.
    pub fn sequence(&self) -> i64 {
        self.sequence
    }
}

/// Producer ordering state for display, including the last allocated sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerIdentity {
    pub installation_id: String,
    pub last_submission_sequence: i64,
    pub minted_at: i64,
}

/// Mint a stable random installation identity that survives credential rotation.
fn mint_installation_id() -> Result<String, DbError> {
    super::helpers::mint_local_id("inst_")
}

/// Last pulled event sequence for an environment, or protocol genesis `0`.
pub(super) fn site_event_watermark(
    conn: &Connection,
    project_id: Option<i64>,
    environment_scope_key: &str,
) -> Result<i64, DbError> {
    let Some(project_id) = project_id else {
        return Ok(0);
    };
    let env_url = lifecycle_env_url(environment_scope_key);
    Ok(conn
        .query_row(
            "SELECT event_sequence FROM connected_site_watermarks
             WHERE project_id = ?1 AND env_url = ?2",
            params![project_id, env_url],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0))
}

/// Raise a site's pulled event watermark within an existing transaction.
pub(super) fn raise_site_event_watermark(
    conn: &Connection,
    project_id: i64,
    environment_scope_key: &str,
    event_sequence: i64,
    pulled_at: i64,
) -> Result<i64, DbError> {
    conn.execute(
        "INSERT INTO connected_site_watermarks
            (project_id, env_url, event_sequence, pulled_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(project_id, env_url) DO UPDATE SET
             event_sequence = excluded.event_sequence,
             pulled_at = excluded.pulled_at
         WHERE excluded.event_sequence > connected_site_watermarks.event_sequence",
        params![project_id, environment_scope_key, event_sequence, pulled_at],
    )?;
    conn.query_row(
        "SELECT event_sequence FROM connected_site_watermarks
         WHERE project_id = ?1 AND env_url = ?2",
        params![project_id, environment_scope_key],
        |row| row.get::<_, i64>(0),
    )
    .map_err(DbError::from)
}

impl Database {
    /// Persist and return the next submission sequence, minting the installation
    /// identity when needed. Crashes may leave gaps but never reuse a number.
    #[tracing::instrument(skip(self))]
    pub fn allocate_submission_sequence(&self, now_ms: i64) -> Result<SubmissionTicket, DbError> {
        // Mint before the transaction so allocation remains a single insert/update path.
        let minted = mint_installation_id()?;
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO connected_producer
                    (id, installation_id, submission_sequence, minted_at)
                 VALUES (1, ?1, 0, ?2)
                 ON CONFLICT(id) DO NOTHING",
                params![minted, now_ms],
            )?;
            let (installation_id, sequence) = tx.query_row(
                "UPDATE connected_producer
                    SET submission_sequence = submission_sequence + 1
                  WHERE id = 1
                  RETURNING installation_id, submission_sequence",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )?;
            tx.commit()?;
            Ok(SubmissionTicket {
                installation_id,
                sequence,
            })
        })?
    }

    /// The producer's ordering state, or `None` when this installation has
    /// never allocated a submission and therefore has no identity yet.
    #[tracing::instrument(skip(self))]
    pub fn get_producer_identity(&self) -> Result<Option<ProducerIdentity>, DbError> {
        self.execute(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT installation_id, submission_sequence, minted_at
                     FROM connected_producer WHERE id = 1",
                    [],
                    |row| {
                        Ok(ProducerIdentity {
                            installation_id: row.get(0)?,
                            last_submission_sequence: row.get(1)?,
                            minted_at: row.get(2)?,
                        })
                    },
                )
                .optional()?)
        })?
    }

    /// Advance the installation's event watermark without allowing regression.
    #[tracing::instrument(skip(self, environment_scope_key))]
    pub fn record_pulled_event_sequence(
        &self,
        project_id: i64,
        environment_scope_key: &str,
        event_sequence: i64,
        pulled_at: i64,
    ) -> Result<i64, DbError> {
        if event_sequence < 0 {
            return Err(DbError::Other(
                "a pulled event sequence cannot be negative".to_string(),
            ));
        }
        let env_url = lifecycle_env_url(environment_scope_key);
        self.execute(move |conn| {
            raise_site_event_watermark(conn, project_id, &env_url, event_sequence, pulled_at)
        })?
    }

    /// The basis a scan started under: the watermark stamped on its execution.
    #[tracing::instrument(skip(self), fields(execution_id))]
    pub fn get_execution_event_basis(&self, execution_id: i64) -> Result<Option<i64>, DbError> {
        self.execute(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT based_on_event_sequence FROM scan_executions WHERE id = ?1",
                    params![execution_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?)
        })?
    }
}

#[cfg(test)]
#[path = "connected_producer_tests.rs"]
mod tests;
