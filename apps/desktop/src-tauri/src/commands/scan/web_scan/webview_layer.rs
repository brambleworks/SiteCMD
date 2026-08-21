//! Optional hidden-webview analysis layered onto the network scanner result.

use crate::core::scanner::{self, ScanError, ScanType};
use tauri::{AppHandle, Emitter};

use super::super::policy::{
    should_run_accessibility_webview_analysis, should_run_webview_analysis,
};

#[derive(Debug, Clone, Default)]
pub(super) struct BrowserRuntime {
    pub ran: bool,
    pub build: Option<String>,
}

/// Append axe-core / CWV findings from a hidden Tauri webview and emit progress
/// around the browser-analysis stage.
pub(super) async fn apply_webview_layer(
    app: &AppHandle,
    result: &mut scanner::ScanResult,
    url: &str,
    scan_type: ScanType,
    axe_enabled: Option<bool>,
) -> Result<BrowserRuntime, ScanError> {
    let parsed = url::Url::parse(url).map_err(|error| ScanError::InvalidUrl(error.to_string()))?;
    let is_local = crate::core::localhost::is_localhost(&parsed);
    if !should_run_webview_analysis(scan_type, is_local) {
        return Ok(BrowserRuntime::default());
    }
    let run_accessibility =
        should_run_accessibility_webview_analysis(scan_type, axe_enabled, is_local);

    let _ = app.emit(
        "scan-progress",
        scanner::ScanProgress {
            check_id: "browser-analysis".to_string(),
            category: crate::checks::ScanCategory::Performance,
            status: "running".to_string(),
            results_count: 0,
            checks_done: 0,
            checks_total: 0,
        },
    );
    let webview_result = crate::webview::analyzer::analyze_url(app, url, run_accessibility).await;
    // The analyzer reports a report only when axe genuinely ran, so passing it
    // straight through keeps the first-party checks as the coverage source
    // whenever the optional browser detector did not.
    let webview_result_count = webview_result
        .accessibility
        .as_ref()
        .map_or(0, |report| report.violations.len())
        + usize::from(webview_result.cwv.is_some());
    scanner::append_webview_results(
        result,
        webview_result.accessibility.as_ref(),
        webview_result.cwv.as_ref(),
    );
    let _ = app.emit(
        "scan-progress",
        scanner::ScanProgress {
            check_id: "browser-analysis".to_string(),
            category: crate::checks::ScanCategory::Performance,
            status: "complete".to_string(),
            results_count: webview_result_count,
            checks_done: 0,
            checks_total: 0,
        },
    );
    Ok(BrowserRuntime {
        ran: webview_result.browser_ran,
        build: webview_result.browser_build,
    })
}
