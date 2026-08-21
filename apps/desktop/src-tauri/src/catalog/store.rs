//! Atomic storage for the last verified catalog pack.
//!
//! Failed verification or interrupted writes never replace the active pack.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::schema::CatalogPack;

const ACTIVE_PACK_FILENAME: &str = "active.json";
const CATALOG_DIRNAME: &str = "catalog";

/// Highest release sequence ever activated. Keeping this monotonic floor
/// outside the pack prevents a missing or corrupt pack from reopening replay.
const HIGH_WATER_FILENAME: &str = "release-sequence";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("catalog storage is unavailable: {0}")]
    Io(#[from] std::io::Error),
    #[error("stored catalog pack is unreadable: {0}")]
    Corrupt(String),
}

/// The catalog directory inside an app data directory.
pub fn catalog_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(CATALOG_DIRNAME)
}

/// Read the active pack, distinguishing corruption from absence.
pub fn load_active(app_data_dir: &Path) -> Result<Option<CatalogPack>, StoreError> {
    let path = catalog_dir(app_data_dir).join(ACTIVE_PACK_FILENAME);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(StoreError::Io(error)),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| StoreError::Corrupt(error.to_string()))
}

/// Make `bytes` the active pack. The caller must have verified them first;
/// this function is the commit step, not a checkpoint.
pub fn activate(
    app_data_dir: &Path,
    bytes: &[u8],
    release_sequence: u64,
) -> Result<(), StoreError> {
    let dir = catalog_dir(app_data_dir);
    fs::create_dir_all(&dir)?;
    crate::app_identity::ensure_private_directory(&dir)?;

    // Raise the rollback floor before publishing so interruption leaves a
    // repairable missing-pack state, never a permissive floor.
    record_high_water(&dir, release_sequence)?;

    // The temporary file must share a directory with the destination: rename
    // is only atomic within one filesystem, and a system temp dir is often a
    // different mount.
    let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
    temp.write_all(bytes)?;
    // Flush the contents before the rename publishes the name. Without this a
    // crash can leave the new name pointing at zero bytes, which is precisely
    // the half-written state the rename is supposed to prevent.
    temp.as_file().sync_all()?;
    temp.persist(dir.join(ACTIVE_PACK_FILENAME))
        .map_err(|error| StoreError::Io(error.error))?;
    // The rename itself is only durable once the directory entry is. Without
    // this a power loss can lose the published name and leave the previous
    // pack, or nothing, in its place.
    sync_dir(&dir);
    Ok(())
}

/// Persist the high-water mark, never lowering it.
fn record_high_water(dir: &Path, release_sequence: u64) -> Result<(), StoreError> {
    if stored_high_water(dir).is_some_and(|stored| stored >= release_sequence) {
        return Ok(());
    }
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(release_sequence.to_string().as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(dir.join(HIGH_WATER_FILENAME))
        .map_err(|error| StoreError::Io(error.error))?;
    sync_dir(dir);
    Ok(())
}

/// The recorded high-water mark, or `None` when none has been written.
fn stored_high_water(dir: &Path) -> Option<u64> {
    fs::read_to_string(dir.join(HIGH_WATER_FILENAME))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Flush the directory when the platform supports opening it as a file.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(handle) = fs::File::open(dir) {
        let _ = handle.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Report whether the active pack is missing, corrupt, or below its persisted
/// rollback floor. The floor survives these states; only the same sequence may
/// repair the pack.
pub fn active_pack_needs_repair(app_data_dir: &Path) -> bool {
    let dir = catalog_dir(app_data_dir);
    match load_active(app_data_dir) {
        Ok(Some(pack)) => {
            stored_high_water(&dir).is_some_and(|floor| pack.release_sequence < floor)
        }
        Ok(None) => stored_high_water(&dir).is_some(),
        Err(_) => true,
    }
}

pub fn active_release_sequence(app_data_dir: &Path) -> Option<u64> {
    let dir = catalog_dir(app_data_dir);
    let from_pack = load_active(app_data_dir)
        .ok()
        .flatten()
        .map(|pack| pack.release_sequence);
    match (stored_high_water(&dir), from_pack) {
        (Some(high_water), Some(active)) => Some(high_water.max(active)),
        (Some(high_water), None) => Some(high_water),
        (None, active) => active,
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
