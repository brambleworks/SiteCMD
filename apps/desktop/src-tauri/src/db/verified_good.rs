//! Transactional storage for the engine-owned verified-good profile.
//!
//! Read, decision, and write remain atomic so revision guards cannot race scans.

use super::{Database, DbError};
use rusqlite::{params, Transaction};
use sitecmd_engine::profile::{
    DecisionError, DriftRecord, FieldRecord, FieldState, FieldValue, Observation, ProfileField,
    RecordOrigin, VerifiedGoodProfile,
};

/// Which decision a person made about a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineDecision {
    /// This is correct now: move the baseline, with provenance.
    Accept,
    /// Stop telling me: leave the baseline where it is.
    Dismiss,
}

/// What came of a decision. A refusal is an outcome, not a database failure:
/// the profile moving under someone is ordinary, and the caller renders it.
#[derive(Debug, Clone)]
pub enum BaselineDecisionOutcome {
    Applied { revision: i64 },
    Refused(DecisionError),
}

impl Database {
    /// A site's baseline, or an empty profile when nothing has been recorded.
    #[tracing::instrument(skip(self), fields(site_id))]
    pub fn get_verified_good_profile(&self, site_id: i64) -> Result<VerifiedGoodProfile, DbError> {
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let profile = read_profile(&tx, site_id)?;
            Ok(profile)
        })?
    }

    /// Apply observed families without changing unobserved baseline fields.
    #[tracing::instrument(skip(self, observation), fields(site_id, families = observation.values.len()))]
    pub fn apply_verified_good_observation(
        &self,
        site_id: i64,
        observation: Observation,
        observed_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<VerifiedGoodProfile, DbError> {
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let current = read_profile(&tx, site_id)?;
            let update = current.observe(&observation, observed_at);
            if !update.changed {
                return Ok(update.profile);
            }
            write_profile(&tx, site_id, &update.profile)?;
            tx.commit()?;
            Ok(update.profile)
        })?
    }

    /// Accept or dismiss the current change on one family.
    #[tracing::instrument(skip(self, expected_digest), fields(site_id, field = field.as_str()))]
    pub fn decide_verified_good(
        &self,
        site_id: i64,
        field: ProfileField,
        based_on_revision: u64,
        expected_digest: String,
        decision: BaselineDecision,
        decided_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<BaselineDecisionOutcome, DbError> {
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            let current = read_profile(&tx, site_id)?;
            let decided = match decision {
                BaselineDecision::Accept => {
                    current.accept(field, based_on_revision, &expected_digest, decided_at)
                }
                BaselineDecision::Dismiss => {
                    current.dismiss(field, based_on_revision, &expected_digest)
                }
            };
            let update = match decided {
                Ok(update) => update,
                Err(error) => return Ok(BaselineDecisionOutcome::Refused(error)),
            };
            write_profile(&tx, site_id, &update.profile)?;
            tx.commit()?;
            Ok(BaselineDecisionOutcome::Applied {
                revision: update.profile.revision as i64,
            })
        })?
    }
}

fn read_profile(tx: &Transaction<'_>, site_id: i64) -> Result<VerifiedGoodProfile, DbError> {
    let revision: i64 = tx
        .query_row(
            "SELECT profile_revision FROM sites WHERE id = ?1",
            params![site_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let mut stmt = tx.prepare(
        "SELECT field, good_value_json, good_digest, good_profile_version, good_recorded_at,
                good_source_scan_id, good_origin, drift_value_json, drift_digest,
                drift_first_seen_at, drift_last_seen_at, drift_source_scan_id, drift_dismissed
           FROM site_verified_good
          WHERE site_id = ?1",
    )?;
    let rows = stmt.query_map(params![site_id], |row| {
        Ok(StoredField {
            field: row.get(0)?,
            good_value_json: row.get(1)?,
            good_digest: row.get(2)?,
            good_profile_version: row.get(3)?,
            good_recorded_at: row.get(4)?,
            good_source_scan_id: row.get(5)?,
            good_origin: row.get(6)?,
            drift_value_json: row.get(7)?,
            drift_digest: row.get(8)?,
            drift_first_seen_at: row.get(9)?,
            drift_last_seen_at: row.get(10)?,
            drift_source_scan_id: row.get(11)?,
            drift_dismissed: row.get::<_, i64>(12)? != 0,
        })
    })?;
    let mut profile = VerifiedGoodProfile {
        revision: revision.max(0) as u64,
        ..Default::default()
    };
    for row in rows {
        let stored = row?;
        // Skip fields from newer profile versions instead of guessing.
        let Some((field, state)) = stored.into_state()? else {
            continue;
        };
        profile.fields.insert(field, state);
    }
    Ok(profile)
}

struct StoredField {
    field: String,
    good_value_json: String,
    good_digest: String,
    good_profile_version: i64,
    good_recorded_at: i64,
    good_source_scan_id: Option<i64>,
    good_origin: String,
    drift_value_json: Option<String>,
    drift_digest: Option<String>,
    drift_first_seen_at: Option<i64>,
    drift_last_seen_at: Option<i64>,
    drift_source_scan_id: Option<i64>,
    drift_dismissed: bool,
}

impl StoredField {
    fn into_state(self) -> Result<Option<(ProfileField, FieldState)>, DbError> {
        let (Some(field), Some(origin)) = (
            ProfileField::parse(&self.field),
            RecordOrigin::parse(&self.good_origin),
        ) else {
            return Ok(None);
        };
        let good = FieldRecord {
            value: serde_json::from_str::<FieldValue>(&self.good_value_json)?,
            digest: self.good_digest,
            profile_version: self.good_profile_version.max(0) as u16,
            recorded_at: from_millis(self.good_recorded_at),
            source_scan_id: self.good_source_scan_id,
            origin,
        };
        let drift = match (self.drift_value_json, self.drift_digest) {
            (Some(value_json), Some(digest)) => Some(DriftRecord {
                value: serde_json::from_str::<FieldValue>(&value_json)?,
                digest,
                first_seen_at: from_millis(self.drift_first_seen_at.unwrap_or_default()),
                last_seen_at: from_millis(self.drift_last_seen_at.unwrap_or_default()),
                source_scan_id: self.drift_source_scan_id,
                dismissed: self.drift_dismissed,
            }),
            _ => None,
        };
        Ok(Some((field, FieldState { good, drift })))
    }
}

fn write_profile(
    tx: &Transaction<'_>,
    site_id: i64,
    profile: &VerifiedGoodProfile,
) -> Result<(), DbError> {
    for (field, state) in &profile.fields {
        let drift = state.drift.as_ref();
        tx.execute(
            "INSERT INTO site_verified_good (
                 site_id, field, good_value_json, good_digest, good_profile_version,
                 good_recorded_at, good_source_scan_id, good_origin, drift_value_json,
                 drift_digest, drift_first_seen_at, drift_last_seen_at,
                 drift_source_scan_id, drift_dismissed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(site_id, field) DO UPDATE SET
                 good_value_json = excluded.good_value_json,
                 good_digest = excluded.good_digest,
                 good_profile_version = excluded.good_profile_version,
                 good_recorded_at = excluded.good_recorded_at,
                 good_source_scan_id = excluded.good_source_scan_id,
                 good_origin = excluded.good_origin,
                 drift_value_json = excluded.drift_value_json,
                 drift_digest = excluded.drift_digest,
                 drift_first_seen_at = excluded.drift_first_seen_at,
                 drift_last_seen_at = excluded.drift_last_seen_at,
                 drift_source_scan_id = excluded.drift_source_scan_id,
                 drift_dismissed = excluded.drift_dismissed",
            params![
                site_id,
                field.as_str(),
                serde_json::to_string(&state.good.value)?,
                state.good.digest,
                state.good.profile_version as i64,
                state.good.recorded_at.timestamp_millis(),
                state.good.source_scan_id,
                state.good.origin.as_str(),
                drift
                    .map(|drift| serde_json::to_string(&drift.value))
                    .transpose()?,
                drift.map(|drift| drift.digest.clone()),
                drift.map(|drift| drift.first_seen_at.timestamp_millis()),
                drift.map(|drift| drift.last_seen_at.timestamp_millis()),
                drift.and_then(|drift| drift.source_scan_id),
                drift.is_some_and(|drift| drift.dismissed) as i64,
            ],
        )?;
    }
    tx.execute(
        "UPDATE sites SET profile_revision = ?2 WHERE id = ?1",
        params![site_id, profile.revision as i64],
    )?;
    Ok(())
}

fn from_millis(millis: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_millis(millis).unwrap_or_default()
}

#[cfg(test)]
#[path = "verified_good_tests.rs"]
mod tests;
