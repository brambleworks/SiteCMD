//! Optional hidden-webview analysis layered onto the network scanner result.

use crate::core::scanner::{self, ScanError, ScanType};
use tauri::{AppHandle, Emitter};

use super::super::policy::webview_analysis_profile;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::commands::scan) struct BrowserRuntime {
    pub(in crate::commands::scan) ran: bool,
    pub(in crate::commands::scan) axe_ran: bool,
    pub(in crate::commands::scan) build: Option<String>,
}

impl BrowserRuntime {
    pub(in crate::commands::scan) fn for_scope(runtimes: &[Self], selected_pages: usize) -> Self {
        let complete_scope = selected_pages > 0 && runtimes.len() == selected_pages;
        let ran = complete_scope && runtimes.iter().all(|runtime| runtime.ran);
        let axe_ran = complete_scope && runtimes.iter().all(|runtime| runtime.axe_ran);
        let build = ran
            .then(|| runtimes.first().and_then(|runtime| runtime.build.clone()))
            .flatten()
            .filter(|candidate| {
                runtimes
                    .iter()
                    .all(|runtime| runtime.build.as_deref() == Some(candidate.as_str()))
            });
        Self {
            ran,
            axe_ran,
            build,
        }
    }
}

/// Append axe-core / CWV findings from a hidden Tauri webview and emit progress
/// around the browser-analysis stage.
pub(in crate::commands::scan) async fn apply_webview_layer(
    app: &AppHandle,
    result: &mut scanner::ScanResult,
    url: &str,
    scan_type: ScanType,
    axe_enabled: Option<bool>,
) -> Result<BrowserRuntime, ScanError> {
    let parsed = url::Url::parse(url).map_err(|error| ScanError::InvalidUrl(error.to_string()))?;
    let (run_browser, run_accessibility) =
        webview_analysis_profile(scan_type, axe_enabled, &parsed);
    if !run_browser {
        return Ok(BrowserRuntime::default());
    }

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
        axe_ran: run_accessibility && webview_result.accessibility.is_some(),
        build: webview_result.browser_build,
    })
}
