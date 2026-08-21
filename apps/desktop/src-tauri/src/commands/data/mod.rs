use std::path::{Path, PathBuf};

pub mod backup;
pub mod diagnostics;
pub mod exports;
pub mod scan_deletion;

pub use backup::*;
pub use diagnostics::*;
pub use exports::*;
pub use scan_deletion::*;

/// Resolve the canonical home directory used for path validation.
/// Shared by export-write and backup-read validation paths so both
/// agree on the user-home boundary.
pub(super) fn canonical_home_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "Cannot determine home directory for path validation.".to_string())?;
    home.canonicalize()
        .map_err(|_| "Cannot resolve home directory for path validation.".to_string())
}

/// Reject paths that escape the user's home directory.
pub(super) fn ensure_within_home(path: &Path) -> Result<(), String> {
    let home = canonical_home_dir()?;
    if !path.starts_with(&home) {
        return Err("Export path must be within the user's home directory.".to_string());
    }
    Ok(())
}
