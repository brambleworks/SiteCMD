//! Automatic `.sitecmd` artifact export for linked projects.

use crate::core::scanner;

pub(super) fn auto_export_sitecmd_scan(project_path: Option<&str>, result: &scanner::ScanResult) {
    let Some(project_path) = project_path.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    let sitecmd_dir = std::path::Path::new(project_path).join(".sitecmd");
    if !sitecmd_dir.is_dir() {
        return;
    }

    if let Err(error) = crate::cli::export::export_scan(&sitecmd_dir, result) {
        tracing::warn!("Failed to auto-export desktop scan: {}", error);
    } else {
        tracing::info!("Auto-exported desktop scan");
    }
}
