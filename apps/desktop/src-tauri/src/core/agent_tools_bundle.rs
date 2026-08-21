//! Install the bundled MCP server into persistent, owner-only application data.
//! Tauri resource paths can be temporary, including AppImage mounts.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MCP_DIR_NAME: &str = "sitecmd-mcp";
const MCP_ENTRY_FILE: &str = "sitecmd-mcp.mjs";
static BUNDLE_INSTALL_LOCK: Mutex<()> = Mutex::new(());

const MCP_BUNDLE_FILES: [&str; 5] = [
    "causal_graph.json",
    "fix_locations.json",
    "impact_score.json",
    "license_constants.json",
    MCP_ENTRY_FILE,
];

pub fn installed_bundle_dir() -> Result<PathBuf, String> {
    crate::app_identity::default_storage_dir()
        .map(|root| root.join(MCP_DIR_NAME))
        .ok_or_else(|| "could not resolve the persistent SiteCMD data directory".to_string())
}

pub fn installed_script_path() -> Result<PathBuf, String> {
    installed_bundle_dir().map(|dir| dir.join(MCP_ENTRY_FILE))
}

fn work_directory(destination_dir: &Path, suffix: &str) -> Result<PathBuf, String> {
    let name = destination_dir.file_name().ok_or_else(|| {
        format!(
            "persistent MCP directory {} has no directory name",
            destination_dir.display()
        )
    })?;
    Ok(destination_dir.with_file_name(format!(".{}.sitecmd-{suffix}", name.to_string_lossy())))
}

fn real_directory_exists(path: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(format!(
            "persistent MCP path {} must be a real directory",
            path.display()
        )),
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "could not inspect persistent MCP path {}: {error}",
            path.display()
        )),
    }
}

fn remove_real_directory(path: &Path) -> Result<(), String> {
    if real_directory_exists(path)? {
        std::fs::remove_dir_all(path).map_err(|error| {
            format!(
                "could not remove stale MCP directory {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn recover_interrupted_swap(destination_dir: &Path, backup_dir: &Path) -> Result<(), String> {
    let destination_exists = real_directory_exists(destination_dir)?;
    if !real_directory_exists(backup_dir)? {
        return Ok(());
    }

    if destination_exists {
        remove_real_directory(backup_dir)
    } else {
        std::fs::rename(backup_dir, destination_dir).map_err(|error| {
            format!(
                "could not restore the previous MCP bundle at {}: {error}",
                destination_dir.display()
            )
        })
    }
}

fn read_source_bundle(source_dir: &Path) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    MCP_BUNDLE_FILES
        .into_iter()
        .map(|file_name| {
            let source = source_dir.join(file_name);
            std::fs::read(&source)
                .map(|contents| (file_name, contents))
                .map_err(|error| {
                    format!(
                        "could not read bundled MCP asset {}: {error}",
                        source.display()
                    )
                })
        })
        .collect()
}

fn bundle_is_current(destination_dir: &Path, assets: &[(&str, Vec<u8>)]) -> bool {
    assets.iter().all(|(file_name, contents)| {
        std::fs::read(destination_dir.join(file_name))
            .map(|existing| existing == *contents)
            .unwrap_or(false)
    })
}

fn install_bundle_from(source_dir: &Path, destination_dir: &Path) -> Result<(), String> {
    let _install_guard = BUNDLE_INSTALL_LOCK
        .lock()
        .map_err(|_| "the MCP bundle installer lock is unavailable".to_string())?;
    let parent = destination_dir.parent().ok_or_else(|| {
        format!(
            "persistent MCP directory {} has no parent",
            destination_dir.display()
        )
    })?;
    crate::app_identity::ensure_private_directory(parent).map_err(|error| {
        format!(
            "could not create the persistent MCP parent directory {}: {error}",
            parent.display()
        )
    })?;

    let staging_dir = work_directory(destination_dir, "staging")?;
    let backup_dir = work_directory(destination_dir, "backup")?;
    recover_interrupted_swap(destination_dir, &backup_dir)?;
    remove_real_directory(&staging_dir)?;

    // Validate the source generation before replacing the installed bundle.
    let assets = read_source_bundle(source_dir)?;
    if real_directory_exists(destination_dir)? && bundle_is_current(destination_dir, &assets) {
        return Ok(());
    }

    crate::app_identity::ensure_private_directory(&staging_dir).map_err(|error| {
        format!(
            "could not create MCP staging directory {}: {error}",
            staging_dir.display()
        )
    })?;
    for (file_name, contents) in &assets {
        let staged = staging_dir.join(file_name);
        if let Err(error) = crate::app_identity::write_private_file(&staged, contents) {
            let _ = remove_real_directory(&staging_dir);
            return Err(format!(
                "could not stage MCP asset {}: {error}",
                staged.display()
            ));
        }
    }

    let had_previous_bundle = real_directory_exists(destination_dir)?;
    if had_previous_bundle {
        if let Err(error) = std::fs::rename(destination_dir, &backup_dir) {
            let _ = remove_real_directory(&staging_dir);
            return Err(format!(
                "could not preserve the previous MCP bundle {}: {error}",
                destination_dir.display()
            ));
        }
    }

    if let Err(install_error) = std::fs::rename(&staging_dir, destination_dir) {
        let restore_error = if had_previous_bundle {
            std::fs::rename(&backup_dir, destination_dir).err()
        } else {
            None
        };
        let _ = remove_real_directory(&staging_dir);
        return match restore_error {
            Some(restore_error) => Err(format!(
                "could not publish the staged MCP bundle: {install_error}; could not restore the previous bundle: {restore_error}"
            )),
            None => Err(format!(
                "could not publish the staged MCP bundle at {}: {install_error}",
                destination_dir.display()
            )),
        };
    }

    // Publication is already live, so stale-directory cleanup is best effort
    // and retries on the next refresh.
    if had_previous_bundle {
        let _ = remove_real_directory(&backup_dir);
    }

    Ok(())
}

#[cfg(feature = "desktop")]
pub fn refresh_bundled_server(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    let source_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("could not resolve the app resource directory: {error}"))?
        .join(MCP_DIR_NAME);
    install_bundle_from(&source_dir, &installed_bundle_dir()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_no_swap_directories(destination_dir: &Path) {
        for suffix in ["staging", "backup"] {
            let path = work_directory(destination_dir, suffix).expect("work path");
            assert!(
                !path.exists(),
                "successful installation must clean {}",
                path.display()
            );
        }
    }

    #[test]
    fn bundle_install_copies_every_asset_and_replaces_the_generation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let destination = temp.path().join("installed");
        std::fs::create_dir_all(&source).expect("source dir");

        for (index, file_name) in MCP_BUNDLE_FILES.iter().enumerate() {
            std::fs::write(source.join(file_name), format!("asset-{index}"))
                .expect("seed source asset");
        }

        install_bundle_from(&source, &destination).expect("install bundle");

        for (index, file_name) in MCP_BUNDLE_FILES.iter().enumerate() {
            assert_eq!(
                std::fs::read_to_string(destination.join(file_name)).expect("installed asset"),
                format!("asset-{index}")
            );
        }

        std::fs::write(source.join(MCP_ENTRY_FILE), "refreshed-entry")
            .expect("update source entry");
        install_bundle_from(&source, &destination).expect("refresh bundle");
        assert_eq!(
            std::fs::read_to_string(destination.join(MCP_ENTRY_FILE)).expect("refreshed entry"),
            "refreshed-entry"
        );
        assert_no_swap_directories(&destination);
    }

    #[test]
    fn bundle_install_leaves_the_whole_previous_bundle_when_an_asset_is_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let destination = temp.path().join("installed");
        std::fs::create_dir_all(&source).expect("source dir");
        std::fs::create_dir_all(&destination).expect("destination dir");

        for (index, file_name) in MCP_BUNDLE_FILES.iter().enumerate() {
            std::fs::write(destination.join(file_name), format!("known-good-{index}"))
                .expect("seed installed asset");
        }

        for file_name in MCP_BUNDLE_FILES {
            if file_name != "impact_score.json" {
                std::fs::write(source.join(file_name), "new").expect("seed source asset");
            }
        }

        assert!(install_bundle_from(&source, &destination).is_err());
        for (index, file_name) in MCP_BUNDLE_FILES.iter().enumerate() {
            assert_eq!(
                std::fs::read_to_string(destination.join(file_name))
                    .expect("installed asset survives"),
                format!("known-good-{index}"),
                "{file_name} must remain on the previous generation"
            );
        }
        assert_no_swap_directories(&destination);
    }

    #[test]
    fn bundle_install_recovers_a_previous_generation_after_an_interrupted_swap() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("missing-source");
        let destination = temp.path().join("installed");
        let backup = work_directory(&destination, "backup").expect("backup path");
        std::fs::create_dir_all(&backup).expect("backup dir");

        for (index, file_name) in MCP_BUNDLE_FILES.iter().enumerate() {
            std::fs::write(backup.join(file_name), format!("known-good-{index}"))
                .expect("seed backup asset");
        }

        assert!(install_bundle_from(&source, &destination).is_err());
        assert!(
            destination.is_dir(),
            "the previous generation must be restored"
        );
        assert!(!backup.exists(), "the recovered backup must become live");
        for (index, file_name) in MCP_BUNDLE_FILES.iter().enumerate() {
            assert_eq!(
                std::fs::read_to_string(destination.join(file_name))
                    .expect("restored installed asset"),
                format!("known-good-{index}")
            );
        }
    }
}
