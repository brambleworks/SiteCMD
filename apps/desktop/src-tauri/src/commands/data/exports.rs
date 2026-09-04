use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tempfile::NamedTempFile;

use super::ensure_within_home;
use crate::commands::{
    confirm_sensitive_action, run_blocking, sanitize_error, SensitiveActionTone,
};

pub(super) fn validate_export_write_path(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(
            "Export path must be an absolute path within the user's home directory.".to_string(),
        );
    }
    if let Some(parent) = p.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        let canonical = parent
            .canonicalize()
            .map_err(|_| "Export path parent directory does not exist.".to_string())?;
        ensure_within_home(&canonical)?;
    }

    if let Ok(metadata) = std::fs::symlink_metadata(p) {
        if metadata.file_type().is_symlink() {
            return Err("Export path cannot target a symlink.".to_string());
        }
        let canonical = p
            .canonicalize()
            .map_err(|_| "Cannot resolve export path.".to_string())?;
        ensure_within_home(&canonical)?;
    }

    Ok(())
}

fn validated_export_target(path: &str) -> Result<(PathBuf, PathBuf), String> {
    validate_export_write_path(path)?;
    let target = PathBuf::from(path);
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| "Export path parent directory does not exist.".to_string())?
        .canonicalize()
        .map_err(|_| "Export path parent directory does not exist.".to_string())?;
    Ok((target, parent))
}

fn persist_export_tempfile(
    temp: NamedTempFile,
    target: &Path,
    allow_overwrite: bool,
) -> Result<(), String> {
    if allow_overwrite {
        temp.persist(target).map_err(|e| {
            sanitize_error(format!(
                "Failed to finalize {}: {}",
                target.display(),
                e.error
            ))
        })?;
        return Ok(());
    }

    temp.persist_noclobber(target).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            "Export destination already exists. Choose a new file name.".to_string()
        } else {
            sanitize_error(format!(
                "Failed to finalize {}: {}",
                target.display(),
                e.error
            ))
        }
    })?;
    Ok(())
}

pub(super) fn write_export_contents<F>(
    path: &str,
    allow_overwrite: bool,
    writer: F,
) -> Result<(), String>
where
    F: FnOnce(&mut File) -> Result<(), String>,
{
    let (target, parent) = validated_export_target(path)?;
    let mut temp = NamedTempFile::new_in(&parent)
        .map_err(|e| sanitize_error(format!("Failed to prepare {}: {}", target.display(), e)))?;
    writer(temp.as_file_mut())?;
    temp.as_file_mut()
        .flush()
        .map_err(|e| sanitize_error(format!("Failed to flush {}: {}", target.display(), e)))?;
    temp.as_file_mut()
        .sync_all()
        .map_err(|e| sanitize_error(format!("Failed to sync {}: {}", target.display(), e)))?;
    persist_export_tempfile(temp, &target, allow_overwrite)
}

async fn confirm_export_write(app: AppHandle, path: &str) -> Result<bool, String> {
    validate_export_write_path(path)?;
    let target = Path::new(path);
    let allow_overwrite = target.exists();
    let (title, message, action) = if allow_overwrite {
        (
            "Replace existing file?",
            format!(
                "This will overwrite the existing file at {}.",
                target.display()
            ),
            "Replace File",
        )
    } else {
        (
            "Save export file?",
            format!(
                "This will create a new export file at {}.",
                target.display()
            ),
            "Save File",
        )
    };

    confirm_sensitive_action(app, title, SensitiveActionTone::Warning, message, action).await?;
    Ok(allow_overwrite)
}

/// Write text content to a file path (for export dialogs).
#[tracing::instrument(skip(app, path, content), fields(content_len = content.len()))]
pub async fn write_export_file(
    app: AppHandle,
    path: String,
    content: String,
) -> Result<(), String> {
    let allow_overwrite = confirm_export_write(app, &path).await?;
    // Disk I/O is offloaded to a blocking task so a slow filesystem cannot
    // stall Tauri's IPC executor (which serves scan progress events too).
    run_blocking(move || {
        write_export_contents(&path, allow_overwrite, |file| {
            file.write_all(content.as_bytes())
                .map_err(|e| sanitize_error(format!("Failed to write {}: {}", path, e)))
        })
    })
    .await?
}

/// Write binary content to a file path (for PDF export).
#[tracing::instrument(skip(app, path, bytes), fields(byte_len = bytes.len()))]
pub async fn write_export_bytes(
    app: AppHandle,
    path: String,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let allow_overwrite = confirm_export_write(app, &path).await?;
    run_blocking(move || {
        write_export_contents(&path, allow_overwrite, |file| {
            file.write_all(&bytes)
                .map_err(|e| sanitize_error(format!("Failed to write {}: {}", path, e)))
        })
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn validate_export_write_path_rejects_symlink_targets() {
        use std::os::unix::fs::symlink;

        let base = std::env::current_dir().expect("cwd");
        let dir = tempfile::Builder::new()
            .prefix("sitecmd-data-tests")
            .tempdir_in(&base)
            .expect("tempdir in workspace");
        let target = dir.path().join("real.txt");
        let symlink_path = dir.path().join("export.txt");
        std::fs::write(&target, "ok").expect("write target");
        symlink(&target, &symlink_path).expect("create symlink");

        let error = validate_export_write_path(symlink_path.to_str().expect("utf8 path"))
            .expect_err("symlink should be rejected");

        assert!(error.contains("symlink"));
    }

    #[test]
    fn validate_export_write_path_rejects_relative_paths() {
        let error = validate_export_write_path("report.html")
            .expect_err("relative export paths should be rejected");

        assert!(error.contains("absolute path"));
    }

    #[test]
    fn write_export_contents_refuses_existing_file_without_overwrite_approval() {
        let home = std::env::var("HOME").expect("home");
        let dir = tempfile::Builder::new()
            .prefix("sitecmd-export-tests")
            .tempdir_in(home)
            .expect("tempdir in home");
        let export_path = dir.path().join("report.md");
        std::fs::write(&export_path, "keep me").expect("seed existing file");

        let error =
            write_export_contents(export_path.to_str().expect("utf8 path"), false, |file| {
                file.write_all(b"replace me")
                    .map_err(|e| format!("write failed: {}", e))
            })
            .expect_err("existing file should not be overwritten without approval");

        assert!(error.contains("already exists"));
        assert_eq!(
            std::fs::read_to_string(&export_path).expect("read existing"),
            "keep me"
        );
    }

    #[test]
    fn write_export_contents_allows_explicit_overwrite() {
        let home = std::env::var("HOME").expect("home");
        let dir = tempfile::Builder::new()
            .prefix("sitecmd-export-tests")
            .tempdir_in(home)
            .expect("tempdir in home");
        let export_path = dir.path().join("report.md");
        std::fs::write(&export_path, "replace me").expect("seed existing file");

        write_export_contents(export_path.to_str().expect("utf8 path"), true, |file| {
            file.write_all(b"new contents")
                .map_err(|e| format!("write failed: {}", e))
        })
        .expect("overwrite after approval");

        assert_eq!(
            std::fs::read_to_string(&export_path).expect("read overwritten"),
            "new contents"
        );
    }
}
