//! Optional hidden-webview analysis layered onto the network scanner result.

use crate::core::scanner::{self, ScanError, ScanType};
use tauri::{AppHandle, Emitter};

use super::super::policy::webview_analysis_profile;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BrowserRuntime {
    pub(in crate::commands::scan) ran: bool,
    pub(in crate::commands::scan) axe_ran: bool,
    pub(in crate::commands::scan) build: Option<String>,
    pub(in crate::commands::scan) failure: Option<String>,
}

impl BrowserRuntime {
    fn from_analysis(
        analysis: &crate::webview::analyzer::WebviewAnalysis,
        accessibility_requested: bool,
    ) -> Self {
        Self {
            ran: analysis.browser_ran,
            axe_ran: accessibility_requested && analysis.accessibility.is_some(),
            build: analysis.browser_build.clone(),
            failure: analysis.error.as_ref().map(crate::commands::sanitize_error),
        }
    }

    fn progress_status(&self) -> &'static str {
        if self.failure.is_some() {
            "error"
        } else {
            "complete"
        }
    }

    pub(in crate::commands::scan) fn incomplete_detail(&self) -> Option<String> {
        self.failure
            .as_ref()
            .map(|failure| format!("Browser analysis failed: {failure}"))
    }

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
            failure: runtimes.iter().find_map(|runtime| runtime.failure.clone()),
        }
    }
}

/// Append axe-core / CWV findings from a hidden Tauri webview and emit progress
/// around the browser-analysis stage. A cancelled scan returns
/// [`ScanError::Cancelled`] so the caller never persists the run.
pub(in crate::commands::scan) async fn apply_webview_layer(
    app: &AppHandle,
    result: &mut scanner::ScanResult,
    url: &str,
    scan_type: ScanType,
    axe_enabled: Option<bool>,
    cancel: &crate::webview::analyzer::CancelCheck,
) -> Result<BrowserRuntime, ScanError> {
    if cancel() {
        return Err(ScanError::Cancelled);
    }
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
    let webview_result =
        crate::webview::analyzer::analyze_url_cancellable(app, url, run_accessibility, cancel)
            .await
            .map_err(|crate::webview::analyzer::AnalysisCancelled| ScanError::Cancelled)?;
    let browser_runtime = BrowserRuntime::from_analysis(&webview_result, run_accessibility);
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
            status: browser_runtime.progress_status().to_string(),
            results_count: webview_result_count,
            checks_done: 0,
            checks_total: 0,
        },
    );
    Ok(browser_runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_browser_layer_refuses_a_cancelled_run_before_it_opens_a_webview() {
        let source = include_str!("webview_layer.rs");
        let entry = source
            .find("pub(in crate::commands::scan) async fn apply_webview_layer(")
            .expect("the browser layer entry point must exist");
        let body = &source[entry..];
        let build = body
            .find("analyze_url_cancellable(")
            .expect("the browser layer must drive the analyzer");
        let before = &body[..build];
        assert!(
            before.contains("if cancel()"),
            "a cancelled run must be refused before the analyzer opens a webview"
        );
        assert!(
            before.contains("return Err(ScanError::Cancelled);"),
            "the refusal must be the shared cancellation error the callers already skip persistence on"
        );
    }

    #[test]
    fn a_cancelled_browser_layer_short_circuits_persistence() {
        let source = include_str!("../web_scan.rs");
        let layer = source
            .find("apply_webview_layer(")
            .expect("web_scan must run the browser layer");
        let persist = source
            .find("let outcome = post_scan_persist(")
            .expect("web_scan must persist through post_scan_persist");
        assert!(
            layer < persist,
            "the browser layer must run before persistence so its cancellation error skips the save"
        );
        let between = &source[layer..persist];
        assert!(
            between.contains(".await?;"),
            "the browser layer result must propagate with `?`, so a cancelled analysis never reaches post_scan_persist"
        );
    }

    #[test]
    fn cancellation_is_checked_immediately_before_the_scan_is_persisted() {
        let source = include_str!("../web_scan.rs");
        let persist = source
            .find("let outcome = post_scan_persist(")
            .expect("web_scan must persist through post_scan_persist");
        let gate = source[..persist]
            .rfind("if cancel_fn() {")
            .expect("a cancellation check must sit before the save");
        let between = &source[gate..persist];
        assert!(
            between.contains("return Err(scanner::ScanError::Cancelled);"),
            "the check before the save must abandon the run"
        );
        assert!(
            !between.contains(".await"),
            "nothing may run between the last cancellation check and the save, so a cancel arriving during the browser layer never leaves a saved run reported as complete"
        );
    }

    #[test]
    fn analyzer_failures_remain_visible_to_the_scan_execution() {
        let runtime = BrowserRuntime::from_analysis(
            &crate::webview::analyzer::WebviewAnalysis {
                cwv: None,
                accessibility: None,
                browser_ran: false,
                browser_build: None,
                error: Some("Failed to create webview: unavailable".into()),
            },
            true,
        );

        assert_eq!(
            runtime.failure.as_deref(),
            Some("Failed to create webview: unavailable")
        );
        assert_eq!(runtime.progress_status(), "error");
        assert_eq!(
            runtime.incomplete_detail().as_deref(),
            Some("Browser analysis failed: Failed to create webview: unavailable")
        );
        assert!(!runtime.ran);
        assert!(!runtime.axe_ran);
    }
}
