use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, State};

use super::ensure_within_home;
use super::exports::write_export_contents;
use crate::commands::{
    confirm_sensitive_action, run_blocking, sanitize_error, SensitiveActionTone,
};
use crate::{api_cache, db::Database};

fn validate_existing_read_path(path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    let metadata =
        std::fs::symlink_metadata(p).map_err(|_| "Source path does not exist.".to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("Source path cannot be a symlink.".to_string());
    }
    if !metadata.is_file() {
        return Err("Source path must be a file.".to_string());
    }
    let canonical = p
        .canonicalize()
        .map_err(|_| "Cannot resolve source path.".to_string())?;
    ensure_within_home(&canonical)?;
    Ok(canonical)
}

fn read_sqlite_header(path: &Path) -> Result<[u8; 16], String> {
    let mut header = [0_u8; 16];
    let mut file = File::open(path)
        .map_err(|e| sanitize_error(format!("Failed to read {}: {}", path.display(), e)))?;
    file.read_exact(&mut header)
        .map_err(|_| "Not a valid SQLite database file".to_string())?;
    Ok(header)
}

fn export_database_to_path(
    db: &Database,
    dest_path: &str,
    allow_overwrite: bool,
) -> Result<u64, String> {
    let src = db.path().to_string();
    // Checkpoint WAL into main DB file for a clean copy
    db.execute(move |conn| {
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .map_err(|e| format!("WAL checkpoint failed: {}", e))
    })
    .map_err(sanitize_error)?
    .map_err(sanitize_error)?;

    write_export_contents(dest_path, allow_overwrite, |file| {
        let mut src_file = File::open(&src)
            .map_err(|e| sanitize_error(format!("Failed to read database backup source: {}", e)))?;
        std::io::copy(&mut src_file, file)
            .map_err(|e| sanitize_error(format!("Failed to copy database: {}", e)))?;
        Ok(())
    })?;

    Ok(std::fs::metadata(dest_path).map(|m| m.len()).unwrap_or(0))
}

fn format_import_database_result(size: u64, warnings: &[String]) -> String {
    if warnings.is_empty() {
        return format!("{} bytes", size);
    }

    let warning = warnings.join("; ");
    format!("{} bytes; restored with warning: {}", size, warning)
}

/// Export the database to a user-chosen file path.
/// Runs a WAL checkpoint first to ensure the backup is self-contained.
#[tracing::instrument(skip(app, db, dest_path), fields(dest_path_len = dest_path.len()))]
pub async fn export_database(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    dest_path: String,
) -> Result<String, String> {
    let allow_overwrite = Path::new(&dest_path).exists();
    let audit_detail = serde_json::json!({ "overwrite": allow_overwrite });
    if let Err(e) = confirm_sensitive_action(
        app,
        "Export SiteCMD database?",
        SensitiveActionTone::Warning,
        if allow_overwrite {
            format!(
                "This will export a full SiteCMD database backup and replace the existing file at {}.",
                dest_path
            )
        } else {
            "This will export a full SiteCMD database backup to the selected file.".to_string()
        },
        if allow_overwrite {
            "Replace and Export"
        } else {
            "Export Database"
        },
    )
    .await
    {
        crate::audit_log::record("data.export", audit_detail, "fail");
        return Err(e.into());
    }
    let db_ref = db.inner().clone();
    let dest_path_for_export = dest_path.clone();
    let size = match run_blocking(move || {
        export_database_to_path(db_ref.as_ref(), &dest_path_for_export, allow_overwrite)
    })
    .await?
    {
        Ok(s) => s,
        Err(e) => {
            crate::audit_log::record("data.export", audit_detail, "fail");
            return Err(e);
        }
    };
    tracing::info!("Database exported ({} bytes)", size);
    crate::audit_log::record(
        "data.export",
        serde_json::json!({ "overwrite": allow_overwrite, "size_bytes": size }),
        "ok",
    );
    Ok(format!("{} bytes", size))
}

/// Import a database from a backup file, replacing the current one.
/// Restores through the live SQLite connection so the new data is available immediately.
#[tracing::instrument(skip(app, db, src_path), fields(src_path_len = src_path.len()))]
pub async fn import_database(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    src_path: String,
) -> Result<String, String> {
    let audit_detail = serde_json::json!({});
    let source_path = match validate_existing_read_path(&src_path) {
        Ok(p) => p,
        Err(e) => {
            crate::audit_log::record("data.import", audit_detail, "fail");
            return Err(e);
        }
    };
    // Validate that the source file looks like a SQLite database
    let header = match read_sqlite_header(&source_path) {
        Ok(h) => h,
        Err(e) => {
            crate::audit_log::record("data.import", audit_detail, "fail");
            return Err(e);
        }
    };
    if &header != b"SQLite format 3\0" {
        crate::audit_log::record("data.import", audit_detail, "fail");
        return Err("Not a valid SQLite database file".to_string());
    }

    if let Err(e) = confirm_sensitive_action(
        app.clone(),
        "Restore SiteCMD backup?",
        SensitiveActionTone::Warning,
        "This will replace the current SiteCMD database with the selected backup file. Current projects, scans, settings, and integrations may change.".to_string(),
        "Restore Backup",
    )
    .await
    {
        crate::audit_log::record("data.import", audit_detail, "fail");
        return Err(e.into());
    }

    {
        // Serialize restore with validation so stale writes cannot restore replaced state.
        let _license_generation = crate::licensing::commands::license_mutation().lock().await;
        let restore_result = {
            let db = db.inner().clone();
            let source_path = source_path.clone();
            run_blocking(move || db.restore_from_backup(source_path))
                .await
                .and_then(|inner| inner)
        };
        if let Err(e) = restore_result.map_err(sanitize_error) {
            // Failed restores may still replace rows, so always discard caches.
            api_cache::clear_all();
            crate::audit_log::record("data.import", audit_detail, "fail");
            return Err(e);
        }
        crate::licensing::commands::note_license_rows_replaced();
    }

    // Sanitize imported credentials and clear caches before reading the new dataset.
    api_cache::clear_all();
    let warnings = {
        let db = db.inner().clone();
        run_blocking(move || {
            let mut warnings = Vec::new();
            if let Err(e) = crate::keyring::migrate_restored_credentials(&app, db.as_ref()) {
                warnings.push(sanitize_error(e));
            }
            if let Err(e) = crate::keyring::mark_legacy_key_migration_complete(&app) {
                warnings.push(sanitize_error(e));
            }
            warnings
        })
        .await?
    };

    let size = std::fs::metadata(db.path()).map(|m| m.len()).unwrap_or(0);
    tracing::info!(
        "Database imported ({} bytes) - live connection restored",
        size
    );
    crate::audit_log::record(
        "data.import",
        serde_json::json!({ "size_bytes": size, "warnings": warnings.len() }),
        "ok",
    );
    if !warnings.is_empty() {
        let warning = warnings.join("; ");
        tracing::warn!(
            "Database restored with credential cleanup warning: {}",
            warning
        );
        return Ok(format_import_database_result(size, &warnings));
    }
    Ok(format_import_database_result(size, &warnings))
}

/// Get the filesystem path to the SQLite database file.
#[tracing::instrument(skip(db))]
pub async fn get_db_path(db: State<'_, Arc<Database>>) -> Result<String, String> {
    Ok(db.path().to_string())
}

/// Get the database file size in bytes.
#[tauri::command]
#[tracing::instrument(skip(db))]
pub async fn get_db_size(db: State<'_, Arc<Database>>) -> Result<u64, String> {
    let path = db.path();
    std::fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| sanitize_error(format!("Failed to get file size: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::temp_db;

    #[test]
    fn read_sqlite_header_only_reads_signature_prefix() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("backup.sqlite");
        let mut bytes = b"SQLite format 3\0".to_vec();
        bytes.extend_from_slice(b"rest-of-file-contents");
        std::fs::write(&path, bytes).expect("write sqlite header");

        let header = read_sqlite_header(&path).expect("read header");

        assert_eq!(&header, b"SQLite format 3\0");
    }

    #[test]
    fn export_database_to_path_writes_restorable_backup() {
        let db = temp_db();
        db.upsert_project("Export Test", "/tmp/export-test", Some("astro"))
            .expect("seed project");

        let home = std::env::var("HOME").expect("home");
        let dir = tempfile::Builder::new()
            .prefix("sitecmd-export-tests")
            .tempdir_in(home)
            .expect("tempdir in home");
        let export_path = dir.path().join("backup.sqlite");

        let size = export_database_to_path(&db, export_path.to_str().expect("utf8 path"), false)
            .expect("export database");

        assert!(size > 0);

        let exported = Database::open(export_path).expect("open exported database");
        let projects = exported.get_projects().expect("projects from exported db");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Export Test");
        assert_eq!(projects[0].framework.as_deref(), Some("astro"));
    }

    #[test]
    fn format_import_database_result_reports_warning_without_claiming_failure() {
        let result = format_import_database_result(
            4096,
            &[
                "credentials sanitized".to_string(),
                "legacy key marker skipped".to_string(),
            ],
        );

        assert_eq!(
            result,
            "4096 bytes; restored with warning: credentials sanitized; legacy key marker skipped"
        );
        assert!(!result.contains("failed"));
    }

    #[test]
    fn format_import_database_result_without_warnings_is_plain_size() {
        assert_eq!(format_import_database_result(2048, &[]), "2048 bytes");
    }
}
