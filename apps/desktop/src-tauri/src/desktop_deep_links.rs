//! Desktop deep-link handling.

use std::{path::Path, sync::Arc};

use serde::Serialize;
use tauri::{Emitter, Manager};

const CLI_IMPORT_EVENT: &str = "sitecmd-cli-imported";
const LICENSE_ACTIVATE_EVENT: &str = "sitecmd-license-activate-requested";

/// License-return scheme published through `product-facts.json` for Web parity.
pub const LICENSE_ACTIVATE_SCHEME: &str = "sitecmd://activate";
const LICENSE_KEY_MAX_LENGTH: usize = 256;

type CliImportEventPayload = crate::cli::CliImportSyncResult;

#[derive(Debug, Clone, Serialize)]
struct LicenseActivateEventPayload {
    key: String,
}

pub fn register_cli_import_handler(app: &mut tauri::App<tauri::Wry>) {
    use tauri_plugin_deep_link::DeepLinkExt;

    let app_handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            let url_str = url.to_string();

            if let Some(license_key) = decode_activate_deep_link(&url_str) {
                handle_activate_deep_link(&app_handle, license_key);
                continue;
            }
            if url_str.starts_with(LICENSE_ACTIVATE_SCHEME) {
                tracing::warn!("License-activate deep link was malformed");
                continue;
            }

            if let Some(destination) = classify_connected_deep_link(&url_str) {
                handle_connected_deep_link(&app_handle, destination);
                continue;
            }

            let Some(decoded_path) = decode_cli_import_deep_link(&url_str) else {
                if url_str.starts_with("sitecmd://import") {
                    tracing::warn!("CLI import deep link was invalid: {}", url_str);
                }
                continue;
            };
            let import_app = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                match handle_cli_import_deep_link(&import_app, decoded_path.as_path()).await {
                    Ok(payload) => {
                        tracing::info!(
                            "Imported CLI project '{}' ({}) scan_synced={}",
                            payload.name,
                            payload.url,
                            payload.imported_scan
                        );
                    }
                    Err(error) => {
                        tracing::error!("Failed to import CLI project: {}", error);
                    }
                }
            });
        }
    });
}

/// Destination kind for a `sitecmd://connected/...` link.
///
/// Only the kind is retained because connected ids and paths are not safe to
/// log. The frontend owns the full destination payload and routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectedDestination {
    /// `sitecmd://connected` - bring the window forward and nothing else.
    Refocus,
    /// `sitecmd://connected/alerts/{alert_id}` with an id inside its bounds.
    Alert,
    /// `sitecmd://connected/settings/{section}`.
    Settings,
    /// Unsupported path or invalid identifier.
    Unresolvable,
}

// Bound opaque IDs without depending on the service's current prefix.
const CONNECTED_ID_MAX_LENGTH: usize = 128;

fn is_bounded_connected_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= CONNECTED_ID_MAX_LENGTH
        && value
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '-' || char == '_')
}

/// Classify `sitecmd://connected[/path]` links, rejecting percent-encoded IDs.
pub fn classify_connected_deep_link(url_str: &str) -> Option<ConnectedDestination> {
    let parsed = url::Url::parse(url_str).ok()?;
    if parsed.scheme() != "sitecmd" || parsed.host_str() != Some("connected") {
        return None;
    }
    let segments: Vec<&str> = parsed
        .path()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    Some(match segments.as_slice() {
        [] => ConnectedDestination::Refocus,
        ["alerts", alert_id] if is_bounded_connected_id(alert_id) => ConnectedDestination::Alert,
        ["settings", "notifications" | "admins"] => ConnectedDestination::Settings,
        _ => ConnectedDestination::Unresolvable,
    })
}

fn handle_connected_deep_link(app: &tauri::AppHandle, destination: ConnectedDestination) {
    // Log only the validated destination kind, never untrusted path content.
    tracing::info!(
        ?destination,
        "Connected deep link received; refocusing main window"
    );
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// Parse `sitecmd://activate?key=<license-key>` and return the trimmed,
/// length-bounded key. Returns None for any other scheme, missing key, or
/// length overflow. Pure; tested directly.
pub fn decode_activate_deep_link(url_str: &str) -> Option<String> {
    let parsed = url::Url::parse(url_str).ok()?;
    if parsed.scheme() != "sitecmd" || parsed.host_str() != Some("activate") {
        return None;
    }
    let key = parsed
        .query_pairs()
        .find(|(name, _)| name == "key")
        .map(|(_, value)| value.trim().to_string())?;
    if key.is_empty() || key.len() > LICENSE_KEY_MAX_LENGTH {
        return None;
    }
    Some(key)
}

fn handle_activate_deep_link(app: &tauri::AppHandle, license_key: String) {
    // Don't log the key (sensitive). Just acknowledge the request.
    tracing::info!("License-activate deep link received; emitting to frontend");
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit(
        LICENSE_ACTIVATE_EVENT,
        LicenseActivateEventPayload { key: license_key },
    );
}

fn has_valid_percent_encoding(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
            continue;
        }
        index += 1;
    }
    true
}

fn decode_cli_import_deep_link(url_str: &str) -> Option<std::path::PathBuf> {
    let parsed = url::Url::parse(url_str).ok()?;
    if parsed.scheme() != "sitecmd" || parsed.host_str() != Some("import") {
        return None;
    }

    let encoded_path = parsed
        .query_pairs()
        .find(|(key, _)| key == "path")
        .map(|(_, value)| value.to_string())?;
    if !has_valid_percent_encoding(&encoded_path) {
        return None;
    }
    let decoded_path = urlencoding::decode(&encoded_path).ok()?;
    Some(std::path::PathBuf::from(decoded_path.into_owned()))
}

fn build_cli_import_confirmation(
    config: &crate::cli::CliConfig,
    project_path: &Path,
    updates_existing_project: bool,
) -> String {
    let action = if updates_existing_project {
        "update an existing SiteCMD project"
    } else {
        "create a new SiteCMD project"
    };
    format!(
        "This link will {action}.\n\nProject: {}\nURL: {}\nFolder: {}\n\nOnly continue if you initiated this import.",
        config.name,
        config.url,
        project_path.display()
    )
}

async fn handle_cli_import_deep_link(
    app: &tauri::AppHandle,
    project_path: &Path,
) -> Result<CliImportEventPayload, String> {
    // Bound externally supplied paths before import.
    let bounded_path = crate::core::code_scan::validate_project_path(project_path)?;
    let sitecmd_dir = bounded_path.join(".sitecmd");
    let config = crate::cli::read_config(&sitecmd_dir)?;
    let db = app.state::<Arc<crate::db::Database>>().inner().clone();
    let updates_existing_project = db.find_project_for_url(&config.url).is_some();
    let message = build_cli_import_confirmation(&config, &bounded_path, updates_existing_project);
    crate::commands::confirm_sensitive_action(
        app.clone(),
        "Import SiteCMD Project?",
        crate::commands::SensitiveActionTone::Warning,
        message,
        "Import Project",
    )
    .await?;

    let import_path = bounded_path.clone();
    let payload = crate::commands::run_blocking(move || {
        crate::cli::import_project_artifacts(db.as_ref(), &import_path)
    })
    .await??;

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }

    let _ = app.emit(CLI_IMPORT_EVENT, payload.clone());
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::{
        build_cli_import_confirmation, classify_connected_deep_link, decode_activate_deep_link,
        decode_cli_import_deep_link, has_valid_percent_encoding, ConnectedDestination,
    };
    use std::path::PathBuf;

    #[test]
    fn bare_connected_deep_link_still_only_refocuses() {
        assert_eq!(
            classify_connected_deep_link("sitecmd://connected"),
            Some(ConnectedDestination::Refocus),
        );
        assert_eq!(
            classify_connected_deep_link("sitecmd://connected/"),
            Some(ConnectedDestination::Refocus),
        );
    }

    #[test]
    fn classify_connected_deep_link_rejects_wrong_scheme_or_host() {
        assert_eq!(
            classify_connected_deep_link("https://sitecmd.com/connected"),
            None
        );
        assert_eq!(
            classify_connected_deep_link("sitecmd://activate?key=ABC"),
            None
        );
        assert_eq!(
            classify_connected_deep_link("sitecmd://import?path=%2Ffoo"),
            None
        );
        assert_eq!(classify_connected_deep_link("not-a-url"), None);
    }

    #[test]
    fn classify_connected_deep_link_reads_the_paths_the_service_emits() {
        assert_eq!(
            classify_connected_deep_link("sitecmd://connected/alerts/alr_0123456789abcdef01234567"),
            Some(ConnectedDestination::Alert),
        );
        assert_eq!(
            classify_connected_deep_link("sitecmd://connected/settings/notifications"),
            Some(ConnectedDestination::Settings),
        );
        assert_eq!(
            classify_connected_deep_link("sitecmd://connected/settings/admins"),
            Some(ConnectedDestination::Settings),
        );
    }

    #[test]
    fn classify_connected_deep_link_refuses_ids_outside_their_bounds() {
        // Separators, quotes, and percent-encoding all land on the not-found
        // state instead of travelling on as an id.
        for hostile in [
            "sitecmd://connected/alerts/%2e%2e%2f%2e%2e",
            "sitecmd://connected/alerts/alr_1'%20OR%201=1",
            "sitecmd://connected/alerts/",
            "sitecmd://connected/alerts",
            "sitecmd://connected/alerts/a/b",
        ] {
            assert_eq!(
                classify_connected_deep_link(hostile),
                Some(ConnectedDestination::Unresolvable),
                "{hostile} must not classify as a reachable alert",
            );
        }

        assert_eq!(
            classify_connected_deep_link("sitecmd://connected/alerts/.."),
            Some(ConnectedDestination::Refocus),
        );

        let oversized = "a".repeat(129);
        assert_eq!(
            classify_connected_deep_link(&format!("sitecmd://connected/alerts/{oversized}")),
            Some(ConnectedDestination::Unresolvable),
        );
    }

    #[test]
    fn classify_connected_deep_link_has_a_destination_for_paths_it_does_not_know() {
        assert_eq!(
            classify_connected_deep_link("sitecmd://connected/settings/whatever-ships-next"),
            Some(ConnectedDestination::Unresolvable),
        );
        assert_eq!(
            classify_connected_deep_link("sitecmd://connected/sites/site_1/notifications"),
            Some(ConnectedDestination::Unresolvable),
        );
    }

    #[test]
    fn decode_activate_deep_link_parses_a_valid_license_key() {
        assert_eq!(
            decode_activate_deep_link("sitecmd://activate?key=test-fixture-key-001"), // gitleaks:allow
            Some("test-fixture-key-001".to_string()),
        );
    }

    #[test]
    fn decode_activate_deep_link_url_decodes_and_trims_the_key() {
        assert_eq!(
            decode_activate_deep_link("sitecmd://activate?key=%20ABCD-1234%20"),
            Some("ABCD-1234".to_string()),
        );
    }

    #[test]
    fn decode_activate_deep_link_rejects_empty_or_missing_keys() {
        assert_eq!(decode_activate_deep_link("sitecmd://activate"), None);
        assert_eq!(decode_activate_deep_link("sitecmd://activate?key="), None);
        assert_eq!(
            decode_activate_deep_link("sitecmd://activate?other=ABC"),
            None
        );
    }

    #[test]
    fn decode_activate_deep_link_rejects_wrong_scheme_or_host() {
        assert_eq!(
            decode_activate_deep_link("https://sitecmd.com/activate?key=ABC"),
            None,
        );
        assert_eq!(decode_activate_deep_link("sitecmd://other?key=ABC"), None);
        assert_eq!(decode_activate_deep_link("sitecmd://import?key=ABC"), None);
    }

    #[test]
    fn decode_activate_deep_link_rejects_oversized_keys() {
        let oversized = "A".repeat(300);
        let url = format!("sitecmd://activate?key={oversized}");
        assert_eq!(decode_activate_deep_link(&url), None);
    }

    #[test]
    fn has_valid_percent_encoding_rejects_truncated_or_non_hex_sequences() {
        assert!(has_valid_percent_encoding("%2FUsers%2Fdev"));
        assert!(!has_valid_percent_encoding("%ZZ"));
        assert!(!has_valid_percent_encoding("%2"));
        assert!(!has_valid_percent_encoding("path%"));
    }

    #[test]
    fn decode_cli_import_deep_link_parses_encoded_project_paths() {
        let parsed = decode_cli_import_deep_link(
            "sitecmd://import?path=%2FUsers%2Fdev%2FProjects%2FWeb%2FSiteCMD%2Fapps%2Fsitecmd.com",
        );

        assert_eq!(
            parsed,
            Some(PathBuf::from(
                "/Users/dev/Projects/Web/SiteCMD/apps/sitecmd.com"
            ))
        );
    }

    #[test]
    fn decode_cli_import_deep_link_rejects_invalid_or_incomplete_urls() {
        assert_eq!(decode_cli_import_deep_link("sitecmd://import"), None);
        assert_eq!(
            decode_cli_import_deep_link("sitecmd://import?path=%ZZ"),
            None
        );
        assert_eq!(
            decode_cli_import_deep_link("sitecmd://open?page=dashboard"),
            None
        );
    }

    #[test]
    fn import_confirmation_identifies_existing_project_mutations() {
        let config = crate::cli::CliConfig::new("https://example.com", "Example");
        let message = build_cli_import_confirmation(
            &config,
            PathBuf::from("/Users/test/example").as_path(),
            true,
        );

        assert!(message.contains("update an existing SiteCMD project"));
        assert!(message.contains("Example"));
        assert!(message.contains("https://example.com"));
        assert!(message.contains("/Users/test/example"));
    }

    #[test]
    fn deep_link_paths_outside_home_are_rejected_before_import() {
        let decoded = decode_cli_import_deep_link("sitecmd://import?path=%2Fetc%2Fcron.d")
            .expect("URL decodes successfully - bounds check is the gate");
        assert_eq!(decoded, PathBuf::from("/etc/cron.d"));
        let result = crate::core::code_scan::validate_project_path(decoded.as_path());
        assert!(
            result.is_err(),
            "validate_project_path must reject /etc/cron.d so the deep-link handler refuses it"
        );
    }
}
