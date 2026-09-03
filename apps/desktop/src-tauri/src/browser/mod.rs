//! Headless Chrome transport for shared accessibility and Web Vitals payloads.

use crate::core::analysis_types::CoreWebVitals;
use headless_chrome::protocol::cdp::types::Event;
use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptions};
use serde::Serialize;
use sitecmd_engine::browser::{
    self, axe_report_from_value, axe_run_script, AdmittedDocuments, AxeEvidenceCaps, AxeReport,
    DocumentMismatch,
};
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

    // Every main-frame document this tab commits, with the loader that
    // committed it. `Tab::get_url` cannot be used for this: it returns the
    // cached `targetInfo.url` with no loader and no navigation reason, so it
    // cannot tell the requested navigation's landing from a page-initiated
    // hop, and `wait_until_navigated` waits on one flag that a new document's
    // `init` lifecycle event raises again, so the wait spans such a hop.
    let commits = MainFrameCommits::default();
    let listener = match tab.add_event_listener(commits.listener()) {
        Ok(listener) => listener,
        Err(e) => return skipped(format!("Failed to observe navigation for {}: {}", url, e)),
    };
    if let Err(e) = tab.navigate_to(url) {
        return skipped(format!("Failed to navigate to {}: {}", url, e));
    }
    if let Err(e) = tab.wait_until_navigated() {
        return skipped(format!("Page load timed out for {}: {}", url, e));
    }
    let _ = tab.remove_event_listener(&listener);
    let commits = commits.observed();
    // With no commit reported, the record holds only the requested URL and
    // says nothing about what Chrome actually loaded. A payload that then
    // fails to match is an unconfirmed navigation, not a page that moved
    // itself, and the reason has to say which of the two it is.
    let navigation_confirmed = !commits.is_empty();
    let admitted = match navigation_documents(url, &commits) {
        Some(admitted) => admitted,
        None => return skipped(format!("Could not identify the document loaded for {url}")),
    };

    // If CWV enabled, wait for metrics to accumulate
    let cwv = if cwv_enabled {
        std::thread::sleep(Duration::from_secs(8));
        match collect_cwv(&tab, &admitted) {
            Ok(cwv) => cwv,
            Err(mismatch) => return skipped(document_failure(url, navigation_confirmed, mismatch)),
        }
    } else {
        None
    };

    // Inject axe-core and run accessibility scan
    let axe = match run_axe(&tab, &admitted) {
        Ok(report) => Some(report),
        Err(AxeFailure::OtherDocument(mismatch)) => {
            return skipped(document_failure(url, navigation_confirmed, mismatch))
        }
        Err(AxeFailure::NoReport(error)) => {
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

/// Why this run measured nothing, in the reader's terms. A mismatch means the
/// same thing to the payload either way, so the reason has to separate a page
/// that moved the browser off the requested document from a navigation this
/// transport could not confirm in the first place.
fn document_failure(url: &str, navigation_confirmed: bool, mismatch: DocumentMismatch) -> String {
    match mismatch {
        DocumentMismatch::OtherDocument { origin } if !navigation_confirmed => format!(
            "Could not confirm which document was loaded for {url}: the browser reported no \
             main-frame navigation, and the page identified itself as {origin}"
        ),
        DocumentMismatch::OtherDocument { origin } => format!(
            "{url} moved the browser to another document before it could be measured ({origin})"
        ),
        DocumentMismatch::Unidentified => {
            format!("The browser payload for {url} did not identify the document it was read from")
        }
    }
}

/// One `Page.frameNavigated` for the main frame: the document a loader
/// committed, and the loader that committed it.
#[derive(Debug, Clone)]
struct MainFrameCommit {
    loader_id: String,
    url: String,
}

/// The main-frame commits observed on one tab, in the order Chrome reported
/// them. The CDP event handler runs on the transport's own thread, so the
/// record is shared.
#[derive(Debug, Clone, Default)]
struct MainFrameCommits(std::sync::Arc<std::sync::Mutex<Vec<MainFrameCommit>>>);

impl MainFrameCommits {
    /// A CDP event listener that records main-frame commits. The blank start
    /// page is skipped so it can never be mistaken for the requested
    /// navigation's own commit.
    fn listener(
        &self,
    ) -> std::sync::Arc<dyn headless_chrome::browser::tab::EventListener<Event> + Send + Sync> {
        let commits = self.0.clone();
        std::sync::Arc::new(move |event: &Event| {
            let Event::PageFrameNavigated(navigated) = event else {
                return;
            };
            let frame = &navigated.params.frame;
            if frame.parent_id.is_some() || frame.url == "about:blank" {
                return;
            }
            if let Ok(mut commits) = commits.lock() {
                commits.push(MainFrameCommit {
                    loader_id: frame.loader_id.clone(),
                    url: frame.url.clone(),
                });
            }
        })
    }

    /// The commits recorded so far, in order.
    fn observed(&self) -> Vec<MainFrameCommit> {
        self.0
            .lock()
            .map(|commits| commits.clone())
            .unwrap_or_default()
    }
}

/// The documents this navigation may be graded from: the URL the scan was
/// asked about plus every main-frame document the requested navigation's own
/// loader committed.
///
/// A server redirect chain is one navigation, so its landing commits under the
/// same loader id as the request. A meta refresh or a script hop starts a new
/// navigation, so its document commits under a different loader id. Chrome's
/// commit order settles which loader is the requested one: a document cannot
/// navigate itself before it commits, so the first main-frame commit after the
/// navigate is always the request's. Every later loader is the page moving
/// itself and is not the page this scan was asked about.
fn navigation_documents(requested: &str, commits: &[MainFrameCommit]) -> Option<AdmittedDocuments> {
    let mut admitted = AdmittedDocuments::new(&url::Url::parse(requested).ok()?);
    if let Some(requested_loader) = commits.first().map(|commit| commit.loader_id.as_str()) {
        for commit in commits
            .iter()
            .filter(|commit| commit.loader_id == requested_loader)
        {
            if let Ok(landed) = url::Url::parse(&commit.url) {
                admitted.admit(&landed);
            }
        }
    }
    Some(admitted)
}

/// Why one axe run produced no report.
enum AxeFailure {
    /// axe ran against a document this navigation never landed on.
    OtherDocument(DocumentMismatch),
    /// axe could not be injected, run, or read back.
    NoReport(String),
}

/// Collect Core Web Vitals from the injected observer script. `Err` means the
/// sample describes a document this navigation never landed on, which ends
/// the run; `Ok(None)` means the page reported no supported metric.
fn collect_cwv(
    tab: &headless_chrome::Tab,
    admitted: &AdmittedDocuments,
) -> Result<Option<CoreWebVitals>, DocumentMismatch> {
    let Some(payload) = read_payload(tab, CWV_READ_SCRIPT) else {
        return Ok(None);
    };
    admitted.verify_payload(&payload)?;

    match serde_json::from_value::<CoreWebVitals>(payload) {
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
            Ok(Some(cwv))
        }
        Ok(_) => Ok(None),
        Err(e) => {
            tracing::warn!("Failed to parse CWV JSON: {}", e);
            Ok(None)
        }
    }
}

/// Evaluate one shared payload script and decode its JSON. The scripts hand
/// back a JSON string; a runtime that returns the object directly is decoded
/// the same way.
fn read_payload(tab: &headless_chrome::Tab, script: &str) -> Option<serde_json::Value> {
    let value = tab.evaluate(script, true).ok()?.value?;
    match value.as_str() {
        Some("null") => None,
        Some(text) => serde_json::from_str(text).ok(),
        None => Some(value),
    }
}

/// Inject axe-core and run WCAG 2.0/2.1/2.2 A + AA accessibility audit
/// against a document this navigation is entitled to grade.
fn run_axe(
    tab: &headless_chrome::Tab,
    admitted: &AdmittedDocuments,
) -> Result<AxeReport, AxeFailure> {
    // Inject axe-core itself (the same asset the desktop webview injects).
    tab.evaluate(browser::AXE_CORE_SCRIPT, false)
        .map_err(|error| AxeFailure::NoReport(format!("Failed to inject axe-core: {error}")))?;

    std::thread::sleep(crate::constants::AXE_INJECT_DELAY);

    // The `true` parameter awaits the payload's promise, so CDP returns the
    // JSON string the shared script hands back.
    let payload = read_payload(tab, &axe_run_script(AxeEvidenceCaps::DEFAULT))
        .ok_or_else(|| AxeFailure::NoReport("axe-core returned no result".to_string()))?;
    admitted
        .verify_payload(&payload)
        .map_err(AxeFailure::OtherDocument)?;

    let report = axe_report_from_value(payload).map_err(AxeFailure::NoReport)?;
    tracing::info!(
        "axe-core completed: {} violations, {} rules executed",
        report.violations.len(),
        report.executed_rules().len()
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{
        document_failure, explicit_no_sandbox, navigation_documents, sandbox_disabled,
        MainFrameCommit,
    };
    use sitecmd_engine::browser::DocumentMismatch;
    use std::ffi::OsStr;

    fn commit(loader_id: &str, url: &str) -> MainFrameCommit {
        MainFrameCommit {
            loader_id: loader_id.into(),
            url: url.into(),
        }
    }

    #[test]
    fn the_requested_loaders_landing_is_graded_and_a_later_loader_is_not() {
        // A server redirect chain is one navigation, so its landing commits
        // under the loader the request started. A meta refresh or a script hop
        // during the wait commits a new document under a new loader, and
        // `wait_until_navigated` returns on that second document's idle: this
        // is the case a settled tab URL cannot tell apart.
        let admitted = navigation_documents(
            "http://example.com/",
            &[
                commit("L1", "https://www.example.com/"),
                commit("L2", "https://interstitial.example.net/consent"),
            ],
        )
        .expect("target");
        assert_eq!(
            admitted.verify_payload(&serde_json::json!({
                "document_url": "https://www.example.com/home"
            })),
            Ok(()),
            "the requested navigation's own landing is the page that was asked about"
        );
        assert_eq!(
            admitted.verify_payload(&serde_json::json!({ "document_url": "http://example.com/" })),
            Ok(())
        );
        assert_eq!(
            admitted.verify_payload(&serde_json::json!({
                "document_url": "https://interstitial.example.net/consent"
            })),
            Err(DocumentMismatch::OtherDocument {
                origin: "https://interstitial.example.net".into()
            }),
            "a document a later loader committed is the page moving itself"
        );
    }

    #[test]
    fn every_document_the_requested_loader_committed_is_graded() {
        // The record is keyed on the loader, not on a commit count: one
        // navigation is one loader, and whatever that loader committed is
        // that navigation. A same-document hop reuses the loader but fires
        // `Page.navigatedWithinDocument`, so it never reaches this record at
        // all; the filter does not assume how many commits arrive.
        let admitted = navigation_documents(
            "https://example.com/",
            &[
                commit("L1", "https://example.com/"),
                commit("L1", "https://example.com/en/"),
                commit("L9", "https://tracker.example.net/"),
            ],
        )
        .expect("target");
        assert_eq!(
            admitted
                .verify_payload(&serde_json::json!({ "document_url": "https://example.com/en/" })),
            Ok(())
        );
        assert!(admitted
            .verify_payload(&serde_json::json!({ "document_url": "https://tracker.example.net/" }))
            .is_err());
    }

    #[test]
    fn an_unconfirmed_navigation_reads_differently_from_a_page_that_moved_itself() {
        // The disclosed limit of this transport: with no observed commit the
        // record holds only the requested URL, so a genuine server redirect
        // reads as a mismatch. A reader has to be able to tell that apart
        // from the page navigating away, which is a real finding.
        let elsewhere = DocumentMismatch::OtherDocument {
            origin: "https://www.example.com".into(),
        };
        let unconfirmed = document_failure("https://example.com/", false, elsewhere.clone());
        assert!(
            unconfirmed.contains("Could not confirm which document was loaded")
                && unconfirmed.contains("no main-frame navigation")
                && unconfirmed.contains("https://www.example.com"),
            "{unconfirmed}"
        );

        let moved = document_failure("https://example.com/", true, elsewhere);
        assert!(
            moved.contains("moved the browser to another document")
                && moved.contains("https://www.example.com"),
            "{moved}"
        );
        assert!(
            !moved.contains("Could not confirm"),
            "a confirmed navigation must not report the transport's own limit"
        );

        // A payload with no identity says so whether or not a commit was seen.
        for navigation_confirmed in [true, false] {
            let unidentified = document_failure(
                "https://example.com/",
                navigation_confirmed,
                DocumentMismatch::Unidentified,
            );
            assert!(
                unidentified.contains("did not identify the document"),
                "{unidentified}"
            );
        }
    }

    #[test]
    fn a_payload_without_identity_is_not_graded_by_the_headless_transport() {
        let admitted = navigation_documents(
            "https://example.com/",
            &[commit("L1", "https://example.com/")],
        )
        .expect("target");
        assert_eq!(
            admitted.verify_payload(&serde_json::json!({ "violations": [] })),
            Err(DocumentMismatch::Unidentified)
        );
    }

    #[test]
    fn an_unparseable_target_admits_no_document() {
        assert!(navigation_documents("not a url", &[]).is_none());
        // No observed commit leaves the requested target as the only admitted
        // document rather than admitting whatever the tab ended up on.
        let admitted = navigation_documents("https://example.com/", &[]).expect("target");
        assert_eq!(
            admitted.verify_payload(&serde_json::json!({ "document_url": "https://example.com/" })),
            Ok(())
        );
        assert!(admitted
            .verify_payload(&serde_json::json!({ "document_url": "https://other.example/" }))
            .is_err());
    }

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
