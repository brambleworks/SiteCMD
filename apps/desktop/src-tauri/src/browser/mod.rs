//! Headless Chrome transport for shared accessibility and Web Vitals payloads.

use crate::core::analysis_types::CoreWebVitals;
use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptions};
use serde::Serialize;
use sitecmd_engine::browser::{self, axe_run_script, parse_axe_report, AxeEvidenceCaps, AxeReport};
use std::ffi::OsStr;
use std::time::Duration;

/// Results from headless Chrome analysis
#[derive(Debug, Clone, Serialize)]
pub struct BrowserAnalysis {
    /// The axe run, or `None` when axe could not be run at all.
    pub axe: Option<AxeReport>,
    pub cwv: Option<CoreWebVitals>,
    pub skipped: bool,
    pub skip_reason: Option<String>,
}

/// CWV payloads - shared with the desktop webview and the hosted runner.
const CWV_OBSERVER_SCRIPT: &str = browser::CWV_OBSERVER_SCRIPT;
const CWV_READ_SCRIPT: &str = browser::CWV_READ_SCRIPT;

const CHROME_NO_SANDBOX_ENV: &str = "SITECMD_CHROME_NO_SANDBOX";

fn explicit_no_sandbox(value: Option<&OsStr>) -> bool {
    matches!(value.and_then(OsStr::to_str), Some("1"))
}

/// Disable Chrome's sandbox only for root or an explicit operator override.
fn sandbox_disabled(explicit_override: bool, euid_is_root: bool) -> bool {
    explicit_override || euid_is_root
}

/// Whether to launch Chrome with `--no-sandbox`, from the live environment.
fn sandbox_should_be_disabled() -> bool {
    let explicit_override = explicit_no_sandbox(std::env::var_os(CHROME_NO_SANDBOX_ENV).as_deref());
    #[cfg(unix)]
    // SAFETY: geteuid only reads the process's effective uid; it cannot fail or race.
    let euid_is_root = unsafe { libc::geteuid() } == 0;
    #[cfg(not(unix))]
    let euid_is_root = false;
    if explicit_override && !euid_is_root {
        tracing::warn!(
            "Chrome sandbox disabled by {}=1; use only in an isolated runner",
            CHROME_NO_SANDBOX_ENV
        );
    }
    sandbox_disabled(explicit_override, euid_is_root)
}

/// Run axe-core (and optionally CWV) against a URL using headless Chrome.
///
/// Returns `BrowserAnalysis` with `skipped: true` if Chrome is not found -
/// the caller should warn but not fail.
pub fn analyze_url(url: &str, cwv_enabled: bool) -> BrowserAnalysis {
    let skipped = |reason: String| BrowserAnalysis {
        axe: None,
        cwv: None,
        skipped: true,
        skip_reason: Some(reason),
    };

    // Root cannot start Chrome's sandbox; other callers must opt out explicitly.
    let browser = match Browser::new(LaunchOptions {
        headless: true,
        sandbox: !sandbox_should_be_disabled(),
        idle_browser_timeout: Duration::from_secs(60),
        ..LaunchOptions::default()
    }) {
        Ok(b) => b,
        Err(e) => {
            let msg = format!("{}", e);
            if msg.contains("not found") || msg.contains("No such file") {
                return skipped(
                    "Chrome/Chromium not found on PATH. Install Chrome or use --no-browser to skip browser-based checks.".into()
                );
            }
            return skipped(format!("Failed to launch Chrome: {}", e));
        }
    };

    let tab = match browser.new_tab() {
        Ok(t) => t,
        Err(e) => return skipped(format!("Failed to create browser tab: {}", e)),
    };

    // Register CWV observers for each document before navigation.
    if cwv_enabled {
        if let Err(e) = tab.call_method(Page::AddScriptToEvaluateOnNewDocument {
            source: CWV_OBSERVER_SCRIPT.to_string(),
            world_name: None,
            include_command_line_api: None,
            run_immediately: None,
        }) {
            tracing::warn!("Failed to register CWV observer: {}", e);
        }
    }

    if let Err(e) = tab.navigate_to(url) {
        return skipped(format!("Failed to navigate to {}: {}", url, e));
    }
    if let Err(e) = tab.wait_until_navigated() {
        return skipped(format!("Page load timed out for {}: {}", url, e));
    }

    // If CWV enabled, wait for metrics to accumulate
    let cwv = if cwv_enabled {
        std::thread::sleep(Duration::from_secs(8));
        collect_cwv(&tab)
    } else {
        None
    };

    // Inject axe-core and run accessibility scan
    let axe = match run_axe(&tab) {
        Ok(report) => Some(report),
        Err(error) => {
            tracing::error!("axe-core did not produce a report: {}", error);
            None
        }
    };

    BrowserAnalysis {
        axe,
        cwv,
        skipped: false,
        skip_reason: None,
    }
}

/// Collect Core Web Vitals from the injected observer script.
fn collect_cwv(tab: &headless_chrome::Tab) -> Option<CoreWebVitals> {
    let result = tab.evaluate(CWV_READ_SCRIPT, true).ok()?;
    let json_val = result.value?;
    let json_str = if let Some(s) = json_val.as_str() {
        s.to_string()
    } else {
        serde_json::to_string(&json_val).ok()?
    };
    if json_str == "null" {
        return None;
    }

    match serde_json::from_str::<CoreWebVitals>(&json_str) {
        Ok(cwv)
            if cwv.lcp_ms.is_some()
                || cwv.cls.is_some()
                || cwv.fcp_ms.is_some()
                || cwv.ttfb_ms.is_some()
                || cwv.observed_long_task_blocking_ms.is_some()
                || cwv.js_error_count.is_some() =>
        {
            tracing::info!(
                "CWV measured - LCP: {:?}ms, CLS: {:?}, FCP: {:?}ms, TTFB: {:?}ms",
                cwv.lcp_ms,
                cwv.cls,
                cwv.fcp_ms,
                cwv.ttfb_ms
            );
            Some(cwv)
        }
        Ok(_) => None,
        Err(e) => {
            tracing::warn!("Failed to parse CWV JSON: {}", e);
            None
        }
    }
}

/// Inject axe-core and run WCAG 2.0/2.1/2.2 A + AA accessibility audit.
fn run_axe(tab: &headless_chrome::Tab) -> Result<AxeReport, String> {
    // Inject axe-core itself (the same asset the desktop webview injects).
    tab.evaluate(browser::AXE_CORE_SCRIPT, false)
        .map_err(|error| format!("Failed to inject axe-core: {error}"))?;

    std::thread::sleep(crate::constants::AXE_INJECT_DELAY);

    // The `true` parameter awaits the payload's promise, so CDP returns the
    // JSON string the shared script hands back.
    let result = tab
        .evaluate(&axe_run_script(AxeEvidenceCaps::DEFAULT), true)
        .map_err(|error| format!("Failed to run axe-core: {error}"))?;

    let json = result
        .value
        .and_then(|value| match value.as_str() {
            Some(text) => Some(text.to_string()),
            None => serde_json::to_string(&value).ok(),
        })
        .ok_or_else(|| "axe-core returned no result".to_string())?;

    let report = parse_axe_report(&json)?;
    tracing::info!(
        "axe-core completed: {} violations, {} rules executed",
        report.violations.len(),
        report.executed_rules().len()
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{explicit_no_sandbox, sandbox_disabled};
    use std::ffi::OsStr;

    #[test]
    fn sandbox_stays_enabled_without_an_explicit_override_or_root() {
        assert!(
            !sandbox_disabled(false, false),
            "sandbox must stay ENABLED for a non-root end-user scan"
        );
        assert!(
            sandbox_disabled(true, false),
            "explicit override disables it"
        );
        assert!(sandbox_disabled(false, true), "root disables the sandbox");
        assert!(sandbox_disabled(true, true));
    }

    #[test]
    fn sandbox_override_requires_the_exact_opt_in_value() {
        assert!(explicit_no_sandbox(Some(OsStr::new("1"))));
        assert!(!explicit_no_sandbox(Some(OsStr::new("true"))));
        assert!(!explicit_no_sandbox(Some(OsStr::new("0"))));
        assert!(!explicit_no_sandbox(None));
    }
}
