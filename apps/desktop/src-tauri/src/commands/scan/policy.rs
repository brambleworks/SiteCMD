use crate::core::scanner::ScanType;
use crate::db::normalize_scan_retention;

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

#[tracing::instrument(fields(scan_type = %scan_type, is_local))]
pub(super) fn should_run_webview_analysis(scan_type: ScanType, is_local: bool) -> bool {
    !is_local && scan_type != ScanType::Security
}

#[tracing::instrument(fields(scan_type = %scan_type, axe_enabled, is_local))]
pub(super) fn should_run_accessibility_webview_analysis(
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
}
