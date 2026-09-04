use crate::core::scanner::ScanType;
use crate::db::normalize_scan_retention;
use tauri_plugin_store::StoreExt;

pub(super) const DEFAULT_HISTORY_QUERY_LIMIT: u32 = 100;
pub(super) const MAX_HISTORY_QUERY_LIMIT: u32 = 500;

pub(super) fn validate_issue_link_provider(provider: &str) -> Result<(), String> {
    crate::commands::issue_links::resolve_issue_link_provider(provider).map(|_| ())
}

/// Bound the size of a history query.
pub(super) fn sanitize_history_limit(requested_limit: Option<u32>) -> u32 {
    requested_limit
        .unwrap_or(DEFAULT_HISTORY_QUERY_LIMIT)
        .min(MAX_HISTORY_QUERY_LIMIT)
}

/// Clamp the per-environment scan-retention setting.
pub(crate) fn scan_retention(requested: Option<u32>) -> u32 {
    normalize_scan_retention(requested)
}

pub(crate) fn resolve_scan_retention(requested: Option<u32>, configured: u32) -> u32 {
    scan_retention(requested.or(Some(configured)))
}

fn scan_retention_from_settings_value(settings: Option<&serde_json::Value>) -> u32 {
    let requested = settings
        .and_then(|value| value.get("retentionLimit"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    scan_retention(requested)
}

pub(crate) fn configured_scan_retention(app: &tauri::AppHandle) -> u32 {
    let settings = app
        .store(crate::commands::app_settings::APP_SETTINGS_FILE)
        .ok()
        .and_then(|store| store.get("scan-prefs"));
    scan_retention_from_settings_value(settings.as_ref())
}

#[tracing::instrument(fields(scan_type = %scan_type, is_local))]
pub(crate) fn should_run_webview_analysis(scan_type: ScanType, is_local: bool) -> bool {
    !is_local && scan_type != ScanType::Security
}

#[tracing::instrument(fields(scan_type = %scan_type, axe_enabled, is_local))]
pub(crate) fn should_run_accessibility_webview_analysis(
    scan_type: ScanType,
    axe_enabled: Option<bool>,
    is_local: bool,
) -> bool {
    if !should_run_webview_analysis(scan_type, is_local) {
        return false;
    }

    if scan_type == ScanType::Accessibility {
        true
    } else {
        axe_enabled.unwrap_or(false)
    }
}

pub(crate) fn webview_analysis_profile(
    scan_type: ScanType,
    axe_enabled: Option<bool>,
    url: &url::Url,
) -> (bool, bool) {
    let is_local = crate::network_policy::LocalOrigin::classify(url).is_local_environment();
    (
        should_run_webview_analysis(scan_type, is_local),
        should_run_accessibility_webview_analysis(scan_type, axe_enabled, is_local),
    )
}

#[cfg(test)]
mod retention_tests {
    use super::*;
    use crate::db::MAX_SCAN_RETENTION;

    #[test]
    fn retention_honors_the_setting_up_to_the_hard_max() {
        assert_eq!(scan_retention(Some(50)), 50);
        assert_eq!(scan_retention(Some(5)), 5);
        assert!(scan_retention(Some(0)) >= 1);
        assert_eq!(scan_retention(Some(10_000)), MAX_SCAN_RETENTION);
    }

    #[test]
    fn retention_falls_back_to_the_durable_setting() {
        assert_eq!(resolve_scan_retention(Some(20), 30), 20);
        assert_eq!(resolve_scan_retention(None, 30), 30);
        assert_eq!(resolve_scan_retention(None, 10_000), MAX_SCAN_RETENTION);
    }

    #[test]
    fn configured_retention_reads_the_durable_scan_preference() {
        let settings = serde_json::json!({ "retentionLimit": 30 });

        assert_eq!(MAX_SCAN_RETENTION, 100);
        assert_eq!(scan_retention_from_settings_value(Some(&settings)), 30);
        assert_eq!(scan_retention_from_settings_value(None), 50);
        assert_eq!(
            scan_retention_from_settings_value(Some(&serde_json::json!({
                "retentionLimit": 10_000
            }))),
            100
        );
    }

    #[test]
    fn webview_profile_treats_every_loopback_address_as_local() {
        let url = url::Url::parse("http://127.0.0.2:5173").expect("loopback URL");

        assert_eq!(
            webview_analysis_profile(ScanType::Health, Some(true), &url),
            (false, false)
        );
    }
}
