//! Registered project path resolution.
//!
//! Renderer-provided filesystem paths are hints only. Security-sensitive local
//! operations resolve the authoritative project folder from SQLite by project ID.

use std::path::{Path, PathBuf};

use crate::db::Database;

pub fn canonicalize_project_dir(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Project folder is not linked.".to_string());
    }
    // Bound renderer-supplied project paths to the user's home before they can
    // become trusted roots for editor and reveal operations.
    let canonical = crate::core::code_scan::validate_project_path(Path::new(trimmed))?;
    if !canonical.is_dir() {
        return Err("Project path is not a folder.".to_string());
    }
    Ok(canonical)
}

pub fn resolve_registered_project_dir(
    db: &Database,
    project_id: i64,
    renderer_path_hint: Option<&str>,
) -> Result<PathBuf, String> {
    let stored_path = db
        .get_project_path(project_id)
        .ok_or_else(|| "Link a local project folder before running this action.".to_string())?;
    let registered = canonicalize_project_dir(&stored_path)?;

    if let Some(hint) = renderer_path_hint.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    }) {
        let hinted = canonicalize_project_dir(hint)?;
        if hinted != registered {
            return Err(
                "Requested project folder does not match the registered project.".to_string(),
            );
        }
    }

    Ok(registered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_project_dir_rejects_system_paths_outside_home() {
        // /etc exists on every Unix; on macOS it is also outside $HOME.
        // validate_project_path should reject it.
        let result = canonicalize_project_dir("/etc");
        assert!(
            result.is_err(),
            "canonicalize_project_dir must reject /etc (outside home)"
        );

        // /var/log similarly outside $HOME on every Unix the app supports.
        let result = canonicalize_project_dir("/var/log");
        assert!(
            result.is_err(),
            "canonicalize_project_dir must reject /var/log (outside home)"
        );
    }

    #[test]
    fn canonicalize_project_dir_rejects_empty_or_missing() {
        assert!(canonicalize_project_dir("").is_err());
        assert!(canonicalize_project_dir("   ").is_err());
        assert!(canonicalize_project_dir("/this/does/not/exist/anywhere").is_err());
    }

    #[test]
    fn canonicalize_project_dir_accepts_a_real_subdir_of_home() {
        // Use the home dir itself - canonicalize, ensure inside home.
        if let Ok(home) = std::env::var("HOME") {
            let result = canonicalize_project_dir(&home);
            assert!(
                result.is_ok(),
                "canonicalize_project_dir must accept $HOME itself: {result:?}"
            );
        }
    }
}
