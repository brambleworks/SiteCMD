use serde::Serialize;
use tauri::AppHandle;
use tauri::State;
use ts_rs::TS;

use crate::db::Database;

use super::{emit_event, run_blocking, sanitize_error};

const ENABLE_DEV_UPDATE_CHECK_ENV: &str = "SITECMD_ENABLE_DEV_UPDATE_CHECK";

/// Typed update result that distinguishes signature failures from transient
/// network failures.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum UpdateCheckOutcome {
    Available {
        version: String,
        date: Option<String>,
        body: Option<String>,
        current_version: String,
    },
    UpToDate,
    /// Couldn't reach the update server. Transient; the frontend should
    /// fall back to a quiet "couldn't check" status, not a scary banner.
    NetworkUnavailable {
        message: String,
    },
    /// A manifest signature failure that must block download and remain visible.
    SignatureInvalid {
        message: String,
    },
    Unknown {
        message: String,
    },
}

/// Uses message fragments because the plugin exposes no stable error variants.
pub fn classify_update_check_error(message: &str) -> UpdateCheckOutcome {
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("signature")
        || lowered.contains("verification failed")
        || lowered.contains("minisign")
        || lowered.contains("public key")
    {
        return UpdateCheckOutcome::SignatureInvalid {
            message: message.to_string(),
        };
    }
    if lowered.contains("network")
        || lowered.contains("dns")
        || lowered.contains("timed out")
        || lowered.contains("timeout")
        || lowered.contains("connection")
        || lowered.contains("transport")
        || lowered.contains("request")
        || lowered.contains("io error")
    {
        return UpdateCheckOutcome::NetworkUnavailable {
            message: message.to_string(),
        };
    }
    UpdateCheckOutcome::Unknown {
        message: message.to_string(),
    }
}

fn should_skip_app_update_check(debug_assertions: bool, dev_update_check_enabled: bool) -> bool {
    debug_assertions && !dev_update_check_enabled
}

fn dev_update_check_enabled() -> bool {
    std::env::var_os(ENABLE_DEV_UPDATE_CHECK_ENV).is_some()
}

#[tracing::instrument(skip(path))]
pub(crate) async fn detect_updates_for_path(
    path: &std::path::Path,
) -> Result<crate::updates::types::UpdateReport, String> {
    if !path.is_dir() {
        return Err("Project path is not a valid directory".into());
    }

    let start = std::time::Instant::now();

    let path = path.to_path_buf();
    // The on-demand report lists what is observable now; partial-observation
    // handling (refresh-not-resolve) lives in the updates adapter poll.
    let packages =
        run_blocking(move || crate::updates::detect_dependencies(&path).packages).await?;
    let ecosystems_detected = crate::updates::detected_ecosystems(&packages);

    if packages.is_empty() {
        return Ok(crate::updates::types::UpdateReport {
            packages: Vec::new(),
            updates: Vec::new(),
            ecosystems_detected: Vec::new(),
            scan_duration_ms: start.elapsed().as_millis() as u64,
        });
    }

    tracing::info!(
        "updates: found {} packages across {} ecosystems, checking registries...",
        packages.len(),
        ecosystems_detected.len()
    );

    // The scan's install-script posture is surfaced through the updates
    // adapter's hourly poll (as a dependencies work item), not through this
    // on-demand report.
    let updates = crate::updates::registry::check_for_updates(&packages)
        .await
        .updates;

    tracing::info!(
        "updates: {} packages have updates available ({} ms)",
        updates.len(),
        start.elapsed().as_millis()
    );

    Ok(crate::updates::types::UpdateReport {
        packages,
        updates,
        ecosystems_detected,
        scan_duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Detect outdated dependencies in a project by parsing lockfiles and querying registries.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn detect_updates(
    app: AppHandle,
    db: State<'_, std::sync::Arc<Database>>,
    project_id: i64,
    project_path: Option<String>,
) -> Result<crate::updates::types::UpdateReport, String> {
    let path = {
        let db = db.inner().clone();
        run_blocking(move || -> Result<std::path::PathBuf, String> {
            let path = crate::project_paths::resolve_registered_project_dir(
                &db,
                project_id,
                project_path.as_deref(),
            )?;
            crate::core::code_scan::validate_project_path(&path)
        })
        .await??
    };
    let report = detect_updates_for_path(&path).await?;
    let refreshed_at = chrono::Utc::now().to_rfc3339();
    let report = {
        let db = db.inner().clone();
        run_blocking(move || {
            if let Err(error) =
                db.save_project_updates_snapshot(project_id, None, &report, &refreshed_at)
            {
                tracing::warn!("Failed to persist project updates snapshot: {}", error);
            }
            report
        })
        .await?
    };
    emit_event(
        &app,
        "project-signals-changed",
        serde_json::json!({
            "projectId": project_id,
            "url": null,
            "source": "updates",
            "updates": &report,
        }),
    );
    Ok(report)
}

/// Return a typed updater outcome for every success and failure branch.
#[tracing::instrument(skip(app))]
pub async fn check_app_update(app: AppHandle) -> Result<UpdateCheckOutcome, String> {
    use tauri_plugin_updater::UpdaterExt;

    if should_skip_app_update_check(cfg!(debug_assertions), dev_update_check_enabled()) {
        tracing::debug!(
            "Skipping app update check in debug build; set {}=1 to test updater locally",
            ENABLE_DEV_UPDATE_CHECK_ENV
        );
        return Ok(UpdateCheckOutcome::UpToDate);
    }

    let updater = app
        .updater()
        .map_err(|e| sanitize_error(format!("Updater not configured: {}", e)))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateCheckOutcome::Available {
            version: update.version,
            date: update.date.map(|d| d.to_string()),
            body: update.body,
            current_version: update.current_version,
        }),
        Ok(None) => Ok(UpdateCheckOutcome::UpToDate),
        Err(e) => {
            let message = e.to_string();
            let outcome = classify_update_check_error(&message);
            match &outcome {
                UpdateCheckOutcome::SignatureInvalid { .. } => {
                    // A bad signature may indicate a compromised update channel.
                    tracing::error!(
                        "Update manifest signature verification FAILED: {}",
                        sanitize_error(message.clone())
                    );
                }
                _ => {
                    tracing::warn!("Update check failed: {}", sanitize_error(message));
                }
            }
            Ok(outcome)
        }
    }
}

/// Event name carrying download progress while an app update installs. The
/// frontend subscribes to render a percentage. Payload is [`AppUpdateProgress`].
pub const APP_UPDATE_PROGRESS_EVENT: &str = "app-update://progress";

/// Download progress for an in-flight app update. `total` is `None` until the
/// server reports a content-length.
#[derive(Debug, Clone, Serialize)]
pub struct AppUpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Outcome of downloading and signature-verifying an app update.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum UpdateInstallOutcome {
    /// Update downloaded, signature-verified, and installed. The caller decides
    /// when to relaunch (immediately in manual mode, on the user's nod via the
    /// restart pill in auto mode).
    Installed {
        version: String,
    },
    /// Nothing newer than the running version; nothing was installed.
    UpToDate,
    /// Debug build with the dev override unset; the updater is intentionally
    /// inert so local runs never try to replace themselves.
    Skipped,
    NetworkUnavailable {
        message: String,
    },
    SignatureInvalid {
        message: String,
    },
    Unknown {
        message: String,
    },
}

/// Map a download/install error string onto the install outcome, keeping the
/// signature-verification failure loud (logged at error) exactly as the check
/// path does, so a tampered manifest can never be applied silently.
fn install_outcome_from_error(message: &str) -> UpdateInstallOutcome {
    match classify_update_check_error(message) {
        UpdateCheckOutcome::SignatureInvalid { message } => {
            tracing::error!(
                "Update install signature verification FAILED: {}",
                sanitize_error(message.clone())
            );
            UpdateInstallOutcome::SignatureInvalid {
                message: sanitize_error(message),
            }
        }
        UpdateCheckOutcome::NetworkUnavailable { message } => {
            tracing::warn!(
                "Update install network failure: {}",
                sanitize_error(message.clone())
            );
            UpdateInstallOutcome::NetworkUnavailable {
                message: sanitize_error(message),
            }
        }
        _ => {
            tracing::warn!(
                "Update install failed: {}",
                sanitize_error(message.to_string())
            );
            UpdateInstallOutcome::Unknown {
                message: sanitize_error(message.to_string()),
            }
        }
    }
}

/// Downloads and installs the available update while emitting
/// [`APP_UPDATE_PROGRESS_EVENT`]. The updater verifies its signature before
/// applying bytes; the frontend controls when the app relaunches.
#[tracing::instrument(skip(app))]
pub async fn download_and_install_app_update(
    app: AppHandle,
) -> Result<UpdateInstallOutcome, String> {
    use tauri_plugin_updater::UpdaterExt;

    if should_skip_app_update_check(cfg!(debug_assertions), dev_update_check_enabled()) {
        return Ok(UpdateInstallOutcome::Skipped);
    }

    let updater = app
        .updater()
        .map_err(|e| sanitize_error(format!("Updater not configured: {}", e)))?;

    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => return Ok(UpdateInstallOutcome::UpToDate),
        Err(e) => return Ok(install_outcome_from_error(&e.to_string())),
    };

    let version = update.version.clone();
    let mut downloaded: u64 = 0;
    let on_chunk = {
        let app = app.clone();
        move |chunk_len: usize, content_len: Option<u64>| {
            downloaded = downloaded.saturating_add(chunk_len as u64);
            emit_event(
                &app,
                APP_UPDATE_PROGRESS_EVENT,
                AppUpdateProgress {
                    downloaded,
                    total: content_len,
                },
            );
        }
    };

    match update.download_and_install(on_chunk, || {}).await {
        Ok(()) => Ok(UpdateInstallOutcome::Installed { version }),
        Err(e) => Ok(install_outcome_from_error(&e.to_string())),
    }
}

/// Relaunch the app to apply an installed update.
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::{
        classify_update_check_error, install_outcome_from_error, should_skip_app_update_check,
        UpdateCheckOutcome, UpdateInstallOutcome,
    };

    #[test]
    fn classify_signature_failure_returns_signature_invalid() {
        assert!(matches!(
            classify_update_check_error("Signature verification failed"),
            UpdateCheckOutcome::SignatureInvalid { .. }
        ));
        assert!(matches!(
            classify_update_check_error("minisign: public key mismatch"),
            UpdateCheckOutcome::SignatureInvalid { .. }
        ));
    }

    #[test]
    fn classify_network_failure_returns_network_unavailable() {
        for msg in [
            "Network error: dns lookup failed",
            "connection reset by peer",
            "request timed out after 30s",
            "transport error: tls handshake",
            "io error: broken pipe",
        ] {
            assert!(
                matches!(
                    classify_update_check_error(msg),
                    UpdateCheckOutcome::NetworkUnavailable { .. }
                ),
                "expected NetworkUnavailable for: {msg}"
            );
        }
    }

    #[test]
    fn classify_unknown_falls_back_when_message_matches_nothing() {
        assert!(matches!(
            classify_update_check_error("some entirely novel update plugin error"),
            UpdateCheckOutcome::Unknown { .. }
        ));
    }

    #[test]
    fn signature_check_runs_before_network_check_when_both_could_match() {
        // A message like "signature check timed out" must still be
        // classified as a signature failure, not a transient network one.
        assert!(matches!(
            classify_update_check_error("signature check timed out"),
            UpdateCheckOutcome::SignatureInvalid { .. }
        ));
    }

    #[test]
    fn app_update_check_skips_debug_builds_by_default() {
        assert!(should_skip_app_update_check(true, false));
    }

    #[test]
    fn app_update_check_can_be_enabled_for_debug_builds() {
        assert!(!should_skip_app_update_check(true, true));
    }

    #[test]
    fn app_update_check_runs_in_release_builds() {
        assert!(!should_skip_app_update_check(false, false));
    }

    #[test]
    fn install_error_keeps_signature_failures_loud_and_distinct() {
        assert!(matches!(
            install_outcome_from_error("minisign signature verification failed"),
            UpdateInstallOutcome::SignatureInvalid { .. }
        ));
        assert!(matches!(
            install_outcome_from_error("connection reset by peer"),
            UpdateInstallOutcome::NetworkUnavailable { .. }
        ));
        assert!(matches!(
            install_outcome_from_error("disk full while writing update"),
            UpdateInstallOutcome::Unknown { .. }
        ));
    }
}
