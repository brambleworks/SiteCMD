//! Hidden-webview transport for shared accessibility and Web Vitals payloads.

use serde::{Deserialize, Serialize};
use sitecmd_engine::browser::{
    self, axe_run_script, parse_axe_report, AxeEvidenceCaps, AxeReport, AXE_RESULT_GLOBAL,
};
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};
use ts_rs::TS;

pub use crate::core::analysis_types::{AxeViolation, CoreWebVitals};

/// Results from Layer 2 webview analysis
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct WebviewAnalysis {
    pub cwv: Option<CoreWebVitals>,
    // None means accessibility analysis did not complete, not zero violations.
    pub accessibility: Option<AxeReport>,
    pub browser_ran: bool,
    pub browser_build: Option<String>,
    pub error: Option<String>,
}

impl WebviewAnalysis {
    /// Nothing measured, for the paths that refuse or fail before injection.
    fn failed(error: String) -> Self {
        Self {
            cwv: None,
            accessibility: None,
            browser_ran: false,
            browser_build: None,
            error: Some(error),
        }
    }
}

/// JavaScript injected immediately after webview creation to observe Core Web Vitals.
/// Must run early to catch LCP and layout-shift entries during page load.
const CWV_OBSERVER_SCRIPT: &str = browser::CWV_OBSERVER_SCRIPT;
const CWV_READ_SCRIPT: &str = browser::CWV_READ_SCRIPT;

/// Run Layer 2 analysis: load URL in hidden webview, capture CWV metrics, and
/// optionally run axe-core accessibility checks.
#[tracing::instrument(skip(app, url), fields(include_accessibility))]
pub async fn analyze_url(
    app: &AppHandle,
    url: &str,
    include_accessibility: bool,
) -> WebviewAnalysis {
    // Revalidate at this boundary so the parsed URL is the one loaded.
    let parsed_url = match url::Url::parse(url) {
        Ok(parsed_url) => parsed_url,
        Err(error) => {
            return WebviewAnalysis::failed(format!(
                "Refused to analyze URL: Invalid URL: {error}"
            ));
        }
    };
    if let Err(error) = crate::network_policy::validate_url_target_blocking(
        &parsed_url,
        crate::network_policy::UrlPolicy::Scan,
    ) {
        return WebviewAnalysis::failed(format!("Refused to analyze URL: {}", error));
    }
    let allow_local_dev = crate::network_policy::scan_origin_allows_local_dev(&parsed_url);
    let rules = super::private_network_rules::PrivateNetworkRules { allow_local_dev };

    let label = format!("analyzer-{}", chrono::Utc::now().timestamp_millis());
    let blank = url::Url::parse("about:blank").expect("about:blank parses"); // allow-expect: compile-time literal URL

    let webview = match WebviewWindowBuilder::new(app, &label, WebviewUrl::External(blank))
        .title("SiteCMD Analyzer")
        .inner_size(1280.0, 800.0)
        .focused(false)
        .visible(false)
        .decorations(false)
        .skip_taskbar(true)
        // Analyzer pages must not share cookies, storage, or cache state with
        // earlier scans. Incognito uses a non-persistent data store on macOS and
        // the equivalent isolated mode on supported Windows/Linux runtimes.
        .incognito(true)
        .initialization_script(CWV_OBSERVER_SCRIPT)
        .on_navigation(move |target| {
            // Apply public redirect policy to every top-level navigation. Tauri
            // does not expose external subresource interception here; the
            // platform rules from install_private_network_rules cover
            // subresources on macOS and Windows.
            let allowed = analyzer_navigation_allowed(target.as_str(), allow_local_dev);
            if !allowed {
                tracing::warn!(
                    "Analyzer refused navigation to a disallowed target: {}",
                    crate::log_sanitizer::log_safe_url_target(target.as_str())
                );
            }
            allowed
        })
        // The analyzer has no user-facing browsing surface. A scanned page must
        // not escape the guarded top-level navigation path through window.open.
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        // Downloads provide no analysis value and would let an untrusted page write
        // attacker-chosen content outside the analyzer's ephemeral web data store.
        .on_download(|_, _| false)
        .build()
    {
        Ok(w) => w,
        Err(e) => return WebviewAnalysis::failed(format!("Failed to create webview: {}", e)),
    };
    let _ = webview.hide();

    // Subresource rules must be live before the first byte of the target
    // loads, so the webview starts blank and navigates only once the platform
    // filter reports it is installed. A failed install fails the browser layer
    // closed rather than scanning with the user's LAN position exposed.
    let (ready_sender, ready_receiver) = tokio::sync::oneshot::channel::<bool>();
    if let Err(error) = super::private_network_rules::install_private_network_rules(
        &webview,
        rules,
        super::private_network_rules::RulesReady::new(ready_sender),
    ) {
        let _ = webview.close();
        return WebviewAnalysis::failed(format!(
            "Failed to install analyzer network rules: {error}"
        ));
    }
    match tokio::time::timeout(crate::constants::WEBVIEW_PAGE_LOAD_WAIT, ready_receiver).await {
        Ok(Ok(true)) => {}
        _ => {
            let _ = webview.close();
            return WebviewAnalysis::failed(
                "Analyzer private-network rules are unavailable".to_string(),
            );
        }
    }
    if let Err(error) = webview.navigate(parsed_url) {
        let _ = webview.close();
        return WebviewAnalysis::failed(format!("Failed to navigate analyzer: {error}"));
    }

    wait_for_page_load(&webview).await;
    let browser_build = collect_browser_build(&webview).await;

    // Collect CWV data before running axe (axe manipulates the DOM which could affect CLS)
    let cwv = collect_cwv(&webview).await;
    if let Some(ref c) = cwv {
        tracing::info!(
            "CWV measured - LCP: {:?}ms, CLS: {:?}, FCP: {:?}ms, TTFB: {:?}ms",
            c.lcp_ms,
            c.cls,
            c.fcp_ms,
            c.ttfb_ms
        );
    } else {
        tracing::info!("CWV measurement produced no supported metrics");
    }

    // Accessibility is optional. We still keep CWV collection for regular web
    // scans so Dashboard Web Vitals isn't gated behind the paid deep-scan.
    let (accessibility, error) = if include_accessibility {
        match run_axe_analysis(&webview).await {
            Ok(report) => (Some(report), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };

    let _ = webview.close();

    WebviewAnalysis {
        cwv,
        accessibility,
        browser_ran: true,
        browser_build,
        error,
    }
}

/// Poll immediately and then at `interval` until a value arrives or `cap` elapses.
async fn poll_webview<T>(
    interval: std::time::Duration,
    cap: std::time::Duration,
    mut probe: impl FnMut() -> Option<T>,
) -> Option<T> {
    let deadline = tokio::time::Instant::now() + cap;
    loop {
        if let Some(value) = probe() {
            return Some(value);
        }
        if tokio::time::Instant::now() + interval > deadline {
            return None;
        }
        tokio::time::sleep(interval).await;
    }
}

const READY_TITLE_PREFIX: &str = "___SHK_READY___";
/// The analyzer starts on `about:blank` while the private-network rules
/// compile, and that document is already complete, so readiness must exclude
/// it or the scan would measure the blank page instead of the target.
const READY_PROBE_SCRIPT: &str = "if (document.readyState === 'complete' && location.href !== 'about:blank') { document.title = '___SHK_READY___'; }";
const BROWSER_UA_TITLE_PREFIX: &str = "___SHK_BROWSER_UA___";
const BROWSER_UA_PROBE_SCRIPT: &str =
    "document.title = '___SHK_BROWSER_UA___' + navigator.userAgent;";

/// Wait for page completion, then allow a short late-metric settle within the cap.
async fn wait_for_page_load(webview: &tauri::WebviewWindow) {
    // Each probe first reads the title (picking up the previous iteration's
    // eval), then issues the next readiness eval; eval results can't be read
    // back directly in Tauri v2.
    let ready = poll_webview(
        crate::constants::WEBVIEW_POLL_INTERVAL,
        crate::constants::WEBVIEW_PAGE_LOAD_WAIT,
        || {
            if let Ok(title) = webview.title() {
                if title.starts_with(READY_TITLE_PREFIX) {
                    return Some(());
                }
            }
            let _ = webview.eval(READY_PROBE_SCRIPT);
            None
        },
    )
    .await;

    if ready.is_some() {
        tokio::time::sleep(crate::constants::WEBVIEW_POST_LOAD_SETTLE).await;
    }
}

async fn collect_browser_build(webview: &tauri::WebviewWindow) -> Option<String> {
    if webview.eval(BROWSER_UA_PROBE_SCRIPT).is_err() {
        return None;
    }
    let user_agent = poll_webview(
        crate::constants::WEBVIEW_POLL_INTERVAL,
        crate::constants::WEBVIEW_CWV_READ_TIMEOUT,
        || {
            webview
                .title()
                .ok()?
                .strip_prefix(BROWSER_UA_TITLE_PREFIX)
                .map(str::to_string)
        },
    )
    .await?;
    browser_build_from_user_agent(crate::core::engine_release::browser_engine(), &user_agent)
}

/// Read CWV data from the webview via document.title polling
async fn collect_cwv(webview: &tauri::WebviewWindow) -> Option<CoreWebVitals> {
    if let Err(e) = webview.eval(CWV_READ_SCRIPT) {
        tracing::warn!("Failed to read CWV data: {}", e);
        return None;
    }

    let json = poll_webview(
        crate::constants::WEBVIEW_POLL_INTERVAL,
        crate::constants::WEBVIEW_CWV_READ_TIMEOUT,
        || {
            let title = webview.title().ok()?;
            title
                .strip_prefix("___SHK_CWV___")
                .map(|json_str| json_str.to_string())
        },
    )
    .await?;

    match serde_json::from_str::<CoreWebVitals>(&json) {
        Ok(cwv) => {
            // Return Some only if at least one metric was captured
            if cwv.lcp_ms.is_some()
                || cwv.cls.is_some()
                || cwv.fcp_ms.is_some()
                || cwv.ttfb_ms.is_some()
                || cwv.observed_long_task_blocking_ms.is_some()
                || cwv.js_error_count.is_some()
            {
                return Some(cwv);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to parse CWV JSON: {} - raw: {}", e, json);
        }
    }

    None
}

async fn run_axe_analysis(webview: &tauri::WebviewWindow) -> Result<AxeReport, String> {
    // Inject axe before its runner; the payload also tolerates deferred script
    // evaluation.
    if let Err(e) = webview.eval(browser::AXE_CORE_SCRIPT) {
        return Err(format!("Failed to inject axe-core: {}", e));
    }
    if let Err(e) = webview.eval(axe_run_script(AxeEvidenceCaps::DEFAULT)) {
        return Err(format!("Failed to run axe: {}", e));
    }

    // External pages cannot use __TAURI__, so poll axe results through a
    // document-title bridge while complex pages finish analysis.
    let title_script = format!(
        "document.title = '___SHK___' + (JSON.stringify(window.{AXE_RESULT_GLOBAL} || null))"
    );
    let started = std::time::Instant::now();
    let payload = poll_webview(
        crate::constants::AXE_POLL_INTERVAL,
        crate::constants::AXE_RESULT_TIMEOUT,
        || {
            if let Ok(title) = webview.title() {
                if let Some(json) = title.strip_prefix("___SHK___") {
                    if json != "null" && serde_json::from_str::<serde_json::Value>(json).is_ok() {
                        return Some(json.to_string());
                    }
                }
            }
            let _ = webview.eval(&title_script);
            None
        },
    )
    .await;

    let Some(payload) = payload else {
        return Err(format!(
            "Axe-core timed out after {} seconds",
            crate::constants::AXE_RESULT_TIMEOUT.as_secs()
        ));
    };

    let report = parse_axe_report(&payload)?;
    tracing::info!(
        "Axe-core completed after {}ms: {} violations, {} rules executed",
        started.elapsed().as_millis(),
        report.violations.len(),
        report.executed_rules().len()
    );
    Ok(report)
}

/// Whether the hidden analyzer webview may perform a top-level navigation.
/// Only an explicitly local scan origin may navigate to loopback; a public
/// origin cannot gain that exception through a redirect.
fn analyzer_navigation_allowed(url: &str, allow_local_dev: bool) -> bool {
    if url == "about:blank" {
        return true;
    }
    crate::network_policy::validate_url_blocking(
        url,
        crate::network_policy::UrlPolicy::Redirect { allow_local_dev },
    )
    .is_ok()
}

fn browser_build_from_user_agent(engine: &str, user_agent: &str) -> Option<String> {
    let markers: &[&str] = match engine {
        "webview2" => &["Edg/", "Chrome/"],
        "chromium" => &["Chrome/", "Chromium/"],
        "webkit" | "webkitgtk" => &["AppleWebKit/"],
        _ => &[],
    };
    markers.iter().find_map(|marker| {
        user_agent
            .split_ascii_whitespace()
            .find_map(|token| token.strip_prefix(marker))
            .map(|version| {
                version
                    .trim_matches(|character: char| {
                        !character.is_ascii_alphanumeric() && character != '.' && character != '-'
                    })
                    .to_string()
            })
            .filter(|version| {
                !version.is_empty() && version.bytes().any(|byte| byte.is_ascii_digit())
            })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        analyzer_navigation_allowed, browser_build_from_user_agent, poll_webview,
        READY_PROBE_SCRIPT,
    };
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn poll_webview_returns_immediately_when_probe_is_ready() {
        let start = tokio::time::Instant::now();
        let result = poll_webview(Duration::from_millis(100), Duration::from_secs(8), || {
            Some(42)
        })
        .await;
        assert_eq!(result, Some(42));
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn poll_webview_returns_as_soon_as_probe_succeeds() {
        let start = tokio::time::Instant::now();
        let mut calls = 0;
        let result = poll_webview(Duration::from_millis(100), Duration::from_secs(8), || {
            calls += 1;
            (calls >= 5).then_some(())
        })
        .await;
        assert_eq!(result, Some(()));
        // 5th probe fires after 4 sleeps: 400ms, nowhere near the 8s cap.
        assert_eq!(start.elapsed(), Duration::from_millis(400));
    }

    #[tokio::test(start_paused = true)]
    async fn poll_webview_gives_up_at_the_cap() {
        let start = tokio::time::Instant::now();
        let result = poll_webview(Duration::from_millis(100), Duration::from_secs(1), || {
            None::<()>
        })
        .await;
        assert_eq!(result, None);
        assert!(start.elapsed() <= Duration::from_secs(1));
        assert!(start.elapsed() >= Duration::from_millis(900));
    }

    #[test]
    fn refuses_navigation_to_cloud_metadata_and_private_ranges() {
        // These targets stay forbidden even when the original scan is local.
        assert!(!analyzer_navigation_allowed(
            "http://169.254.169.254/latest/meta-data/",
            true
        ));
        assert!(!analyzer_navigation_allowed("http://192.168.1.1/", true));
        assert!(!analyzer_navigation_allowed("http://10.0.0.5/", true));
        assert!(!analyzer_navigation_allowed("http://172.16.0.1/", true));
    }

    #[test]
    fn public_scan_cannot_redirect_to_loopback() {
        assert!(!analyzer_navigation_allowed(
            "http://127.0.0.1:3000/",
            false
        ));
        assert!(!analyzer_navigation_allowed(
            "http://localhost:3000/",
            false
        ));
        assert!(!analyzer_navigation_allowed("http://[::1]:3000/", false));
    }

    #[test]
    fn readiness_probe_never_fires_on_the_blank_start_page() {
        assert!(READY_PROBE_SCRIPT.contains("location.href !== 'about:blank'"));
        assert!(READY_PROBE_SCRIPT.contains("document.readyState === 'complete'"));
    }

    #[test]
    fn blank_start_page_is_always_allowed() {
        assert!(analyzer_navigation_allowed("about:blank", false));
        assert!(analyzer_navigation_allowed("about:blank", true));
    }

    #[test]
    fn explicit_local_scan_can_retain_loopback_navigation() {
        assert!(analyzer_navigation_allowed("http://127.0.0.1:3000/", true));
        assert!(analyzer_navigation_allowed("http://localhost:3000/", true));
    }

    #[test]
    fn browser_build_is_derived_from_the_runtime_user_agent() {
        assert_eq!(
            browser_build_from_user_agent(
                "webkit",
                "Mozilla/5.0 AppleWebKit/621.1.15 (KHTML, like Gecko) Version/18.5 Safari/621.1.15",
            )
            .as_deref(),
            Some("621.1.15")
        );
        assert_eq!(
            browser_build_from_user_agent(
                "webview2",
                "Mozilla/5.0 Chrome/136.0.7103.49 Safari/537.36 Edg/136.0.3240.50",
            )
            .as_deref(),
            Some("136.0.3240.50")
        );
    }
}
