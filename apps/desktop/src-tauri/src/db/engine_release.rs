//! Stores immutable release stamps and check inventories.
//!
//! A build's inventory is inserted atomically with its first stamped run.

use rusqlite::{named_params, params, Connection, OptionalExtension};
use sitecmd_engine::manifest::CompareDimension;
use sitecmd_engine::release::{CheckInventory, ExecutionProfile, InventoryEntry, ReleaseStamp};

use super::{Database, DbError};

/// A run's provenance as it was stored: what the build stated, and what that
/// build could produce.
#[derive(Debug, Clone)]
pub struct StoredBasis {
    pub stamp: ReleaseStamp,
    pub inventory: CheckInventory,
}

impl StoredBasis {
    pub fn basis(&self) -> sitecmd_engine::release::ObservationBasis<'_> {
        sitecmd_engine::release::ObservationBasis {
            stamp: &self.stamp,
            inventory: &self.inventory,
        }
    }
}

/// Record the inventory for a stamp, once. Called inside the transaction that
/// persists a run, so the inventory a run points at is committed with it or
/// not at all.
pub(super) fn record_inventory(
    conn: &Connection,
    stamp: &ReleaseStamp,
    inventory: &CheckInventory,
    recorded_at: i64,
) -> Result<(), DbError> {
    let already_recorded: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM engine_releases
             WHERE engine_release = ?1 AND manifest_digest = ?2",
            params![stamp.engine_release, stamp.manifest_digest],
            |row| row.get(0),
        )
        .optional()?;
    if already_recorded.is_some() {
        return Ok(());
    }

    conn.execute(
        "INSERT INTO engine_releases (
            engine_release, manifest_digest, manifest_schema, canonicalizer,
            crawl_profile, recorded_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            stamp.engine_release,
            stamp.manifest_digest,
            i64::from(sitecmd_engine::manifest::SCHEMA_VERSION),
            i64::from(stamp.canonicalizer),
            i64::from(stamp.crawl_profile),
            recorded_at,
        ],
    )?;

    let mut insert = conn.prepare(
        "INSERT INTO engine_release_checks (
            engine_release, manifest_digest, check_id, contract, compare_on, family
         ) VALUES (:engine_release, :manifest_digest, :check_id, :contract, :compare_on, :family)",
    )?;
    for (check_id, entry) in inventory.iter() {
        insert.execute(named_params! {
            ":engine_release": stamp.engine_release,
            ":manifest_digest": stamp.manifest_digest,
            ":check_id": check_id,
            ":contract": entry.contract,
            ":compare_on": serde_json::to_string(&entry.compare_on)?,
            ":family": i64::from(entry.family),
        })?;
    }
    Ok(())
}

/// Read a run's release stamp without backfilling pre-stamp runs.
pub(super) fn read_stamp(conn: &Connection, run_id: i64) -> Result<Option<ReleaseStamp>, DbError> {
    let row = conn
        .query_row(
            "SELECT engine_release, manifest_digest, canonicalizer, crawl_profile,
                    execution_profile_json
             FROM scan_runs WHERE id = ?1",
            params![run_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((Some(engine_release), Some(manifest_digest), canonicalizer, crawl_profile, profile)) =
        row
    else {
        return Ok(None);
    };
    let execution: ExecutionProfile = profile
        .as_deref()
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();
    Ok(Some(ReleaseStamp {
        engine_release,
        manifest_digest,
        canonicalizer: canonicalizer.unwrap_or_default() as u16,
        crawl_profile: crawl_profile.unwrap_or_default() as u16,
        execution,
    }))
}

fn read_inventory(
    conn: &Connection,
    engine_release: &str,
    manifest_digest: &str,
) -> Result<Option<CheckInventory>, DbError> {
    let mut statement = conn.prepare(
        "SELECT check_id, contract, compare_on, family
         FROM engine_release_checks
         WHERE engine_release = ?1 AND manifest_digest = ?2",
    )?;
    let rows = statement
        .query_map(params![engine_release, manifest_digest], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if rows.is_empty() {
        return Ok(None);
    }
    let entries = rows
        .into_iter()
        .map(|(check_id, contract, compare_on, family)| {
            let compare_on: Vec<CompareDimension> =
                serde_json::from_str(&compare_on).unwrap_or_default();
            (
                check_id,
                InventoryEntry {
                    contract,
                    compare_on,
                    family: family != 0,
                },
            )
        });
    Ok(Some(CheckInventory::from_entries(entries)))
}

impl Database {
    /// Persist the current inventory for non-scan callers such as connected
    /// sync and tests. Scan persistence records it transactionally with the run.
    #[tracing::instrument(skip(self))]
    pub fn record_current_engine_release(&self, recorded_at: i64) -> Result<(), DbError> {
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            record_inventory(
                &tx,
                &crate::core::engine_release::stamp(
                    crate::core::engine_release::ObservedSurface::Web,
                    None,
                    false,
                    None,
                ),
                &crate::core::engine_release::CURRENT_INVENTORY,
                recorded_at,
            )?;
            tx.commit()?;
            Ok(())
        })?
    }

    /// Return a run's stamp and inventory, or `None` when either is unknown.
    #[tracing::instrument(skip(self), fields(run_id))]
    pub fn run_release_basis(&self, run_id: i64) -> Result<Option<StoredBasis>, DbError> {
        self.execute(move |conn| {
            let Some(stamp) = read_stamp(conn, run_id)? else {
                return Ok(None);
            };
            let Some(inventory) =
                read_inventory(conn, &stamp.engine_release, &stamp.manifest_digest)?
            else {
                return Ok(None);
            };
            Ok(Some(StoredBasis { stamp, inventory }))
        })?
    }
}

#[cfg(test)]
#[path = "engine_release_tests.rs"]
mod tests;
