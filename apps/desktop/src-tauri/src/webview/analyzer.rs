//! Hidden-webview transport for shared accessibility and Web Vitals payloads.

use serde::{Deserialize, Serialize};

use sitecmd_engine::browser::{
    self, axe_report_from_value, axe_run_script, payload_document_url, AdmittedDocuments,
    AxeEvidenceCaps, AxeReport, DocumentMismatch, AXE_RESULT_GLOBAL, CWV_RESULT_GLOBAL,
};

use super::deferred_navigation::{admit_deferred_target, MainFrame};
use super::title_bridge::{poll_webview, read_bridged_json, BridgeReadError, TitleBridge};
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

    /// The browser ran, but a payload describes a document the analyzer
    /// never admitted. Nothing is attached: the page grades as
    /// browser-unavailable rather than as some other document.
    fn other_document(browser_build: Option<String>, mismatch: DocumentMismatch) -> Self {
        Self {
            cwv: None,
            accessibility: None,
            browser_ran: true,
            browser_build,
            error: Some(mismatch.to_string()),
        }
    }
}

/// The scan was cancelled while the analyzer was running. The webview has been
/// closed and nothing was measured, so the caller must neither report the run
/// as complete nor persist a result for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisCancelled;

/// Cancellation probe shared with the network scanner.
pub type CancelCheck = crate::scan_runtime::CancelFn;

/// Await `work`, abandoning it as soon as `cancel` reports the scan was
/// cancelled. The analyzer's waits are timer polls over the document-title
/// bridge, so dropping the future costs nothing beyond the reading in flight.
async fn until_cancelled<T>(
    cancel: &CancelCheck,
    work: impl std::future::Future<Output = T>,
) -> Result<T, AnalysisCancelled> {
    if cancel() {
        return Err(AnalysisCancelled);
    }
    tokio::pin!(work);
    loop {
        tokio::select! {
            biased;
            value = &mut work => return Ok(value),
            _ = tokio::time::sleep(crate::constants::WEBVIEW_POLL_INTERVAL) => {
                if cancel() {
                    return Err(AnalysisCancelled);
                }
            }
        }
    }
}

/// JavaScript injected immediately after webview creation to observe Core Web Vitals.
/// Must run early to catch LCP and layout-shift entries during page load.
const CWV_OBSERVER_SCRIPT: &str = browser::CWV_OBSERVER_SCRIPT;
const CWV_READ_SCRIPT: &str = browser::CWV_READ_SCRIPT;
/// Title markers under which the page frames its chunked JSON payloads; the
/// bridge module documents why a payload never crosses as one title.
const CWV_TITLE_MARKER: &str = "___SHK_CWV___";
const AXE_TITLE_MARKER: &str = "___SHK_AXE___";

/// Run Layer 2 analysis without cancellation, for the standalone analyzer
/// commands that have no scan request to cancel.
pub async fn analyze_url(
    app: &AppHandle,
    url: &str,
    include_accessibility: bool,
) -> WebviewAnalysis {
    let never_cancelled = || false;
    analyze_url_cancellable(app, url, include_accessibility, &never_cancelled)
        .await
        .unwrap_or_else(|AnalysisCancelled| {
            WebviewAnalysis::failed("Browser analysis cancelled".to_string())
        })
}

/// Run Layer 2 analysis: load URL in hidden webview, capture CWV metrics, and
/// optionally run axe-core accessibility checks. Returns `Err` as soon as
/// `cancel` reports the scan was cancelled, with the webview closed.
#[tracing::instrument(skip(app, url, cancel), fields(include_accessibility))]
pub async fn analyze_url_cancellable(
    app: &AppHandle,
    url: &str,
    include_accessibility: bool,
    cancel: &CancelCheck,
) -> Result<WebviewAnalysis, AnalysisCancelled> {
    if cancel() {
        return Err(AnalysisCancelled);
    }
    // Resolve and validate the target on the async runtime; the navigation
    // callback runs on the webview thread and must never touch DNS.
    let parsed_url = match url::Url::parse(url) {
        Ok(parsed_url) => parsed_url,
        Err(error) => {
            return Ok(WebviewAnalysis::failed(format!(
                "Refused to analyze URL: Invalid URL: {error}"
            )));
        }
    };
    if let Err(error) = until_cancelled(
        cancel,
        crate::network_policy::validate_url(
            parsed_url.as_str(),
            crate::network_policy::UrlPolicy::Scan,
        ),
    )
    .await?
    {
        return Ok(WebviewAnalysis::failed(format!(
            "Refused to analyze URL: {}",
            error
        )));
    }
    let allow_local_dev =
        crate::network_policy::LocalOrigin::classify(&parsed_url).allows_local_dev();
    let (gate, mut deferred) = NavigationGate::new(&parsed_url, allow_local_dev);
    let rules = super::private_network_rules::PrivateNetworkRules { allow_local_dev };
    let titles = TitleBridge::default();
    let main_frame = MainFrame::default();

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
        // WebRTC and WebTransport bypass the resource loader the subresource
        // rules sit on, so their constructors are removed in every frame
        // before the page's own scripts run.
        .initialization_script_for_all_frames(super::private_network_rules::WEBRTC_LOCKDOWN_SCRIPT)
        // Every read-back from the page arrives as a document title; see
        // TitleBridge for why the window title cannot be polled instead.
        .on_document_title_changed({
            let titles = titles.clone();
            move |_webview, title| titles.record(title)
        })
        // The main frame's commits decide whether a deferred navigation is a
        // redirect hop to follow or a subframe to leave where it is.
        .on_page_load({
            let main_frame = main_frame.clone();
            move |_webview, payload| main_frame.record(payload.event(), payload.url())
        })
        .on_navigation({
            let gate = gate.clone();
            move |target| {
                // Apply public redirect policy to every navigation the webview
                // makes. The platform hook is frame-blind: WebKit's policy
                // delegate and WebView2's NavigationStarting hand a subframe's
                // navigation to this same closure, so an iframe's URL arrives
                // here looking exactly like a redirect hop. The driver tells
                // them apart by main-frame commit state; see
                // deferred_navigation.rs. Tauri does not expose external
                // subresource interception here; the platform rules from
                // install_private_network_rules block private-network
                // subresources on macOS and Windows, and the fallback arm
                // fails the browser layer closed everywhere else.
                let allowed = gate.decide(target);
                if !allowed {
                    tracing::warn!(
                        "Analyzer refused navigation to a disallowed target: {}",
                        crate::log_sanitizer::log_safe_url_target(target.as_str())
                    );
                }
                allowed
            }
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
        Err(e) => {
            return Ok(WebviewAnalysis::failed(format!(
                "Failed to create webview: {}",
                e
            )))
        }
    };
    let _ = webview.hide();

    // One close covers every outcome from here, cancellation included.
    let analysis = drive_analyzer(
        &webview,
        &titles,
        &gate,
        &main_frame,
        &mut deferred,
        rules,
        parsed_url,
        include_accessibility,
        cancel,
    )
    .await;
    let _ = webview.close();
    analysis
}

/// Drive one prepared analyzer webview from network-rule install through axe.
/// The caller owns the webview and closes it on every outcome, so cancellation
/// here never leaves a hidden window behind.
async fn drive_analyzer(
    webview: &tauri::WebviewWindow,
    titles: &TitleBridge,
    gate: &NavigationGate,
    main_frame: &MainFrame,
    deferred: &mut tokio::sync::mpsc::UnboundedReceiver<url::Url>,
    rules: super::private_network_rules::PrivateNetworkRules,
    target: url::Url,
    include_accessibility: bool,
    cancel: &CancelCheck,
) -> Result<WebviewAnalysis, AnalysisCancelled> {
    // Subresource rules must be live before the first byte of the target
    // loads, so the webview starts blank and navigates only once the platform
    // filter reports it is installed. A failed install fails the browser layer
    // closed rather than scanning with the user's LAN position exposed.
    let (ready_sender, ready_receiver) = tokio::sync::oneshot::channel::<bool>();
    if let Err(error) = super::private_network_rules::install_private_network_rules(
        webview,
        rules,
        super::private_network_rules::RulesReady::new(ready_sender),
    ) {
        return Ok(WebviewAnalysis::failed(format!(
            "Failed to install analyzer network rules: {error}"
        )));
    }
    match until_cancelled(
        cancel,
        tokio::time::timeout(crate::constants::WEBVIEW_RULES_INSTALL_WAIT, ready_receiver),
    )
    .await?
    {
        Ok(Ok(true)) => {}
        _ => {
            return Ok(WebviewAnalysis::failed(
                super::private_network_rules::rules_unavailable_reason().to_string(),
            ));
        }
    }
    // A hidden page is timer-throttled like a background tab, which slows
    // axe and the page itself; a failure here only costs speed.
    if let Err(error) = super::hidden_page_timers::unthrottle_hidden_page_timers(webview) {
        tracing::warn!("Analyzer keeps hidden-page timer throttling: {error}");
    }
    let mut admitted = AdmittedDocuments::new(&target);
    if let Err(error) = webview.navigate(target) {
        return Ok(WebviewAnalysis::failed(format!(
            "Failed to navigate analyzer: {error}"
        )));
    }

    if let Err(error) = until_cancelled(
        cancel,
        wait_for_page_load(webview, titles, gate, main_frame, deferred, &mut admitted),
    )
    .await?
    {
        return Ok(WebviewAnalysis::failed(error));
    }
    let browser_build = until_cancelled(cancel, collect_browser_build(webview, titles)).await?;
    // The gate judges a navigation by host, so a same-host scheme or port
    // change is allowed inline and never arrives as a deferred hop. The
    // commit says which transport the analyzer really loaded; it can only
    // refine a host the runtime already admitted, never add one.
    if let Some(committed) = main_frame.committed() {
        admitted.observe_commit(&committed);
    }

    // Collect CWV data before running axe (axe manipulates the DOM which could affect CLS)
    let cwv = until_cancelled(cancel, collect_cwv(webview, titles)).await?;
    // Every payload names the document it was read from; one from a document
    // the analyzer never admitted grades nothing, and there is no point
    // spending the axe budget on it either.
    if let Some((_, document_url)) = &cwv {
        if let Err(mismatch) = admitted.verify(document_url.as_deref()) {
            return Ok(WebviewAnalysis::other_document(browser_build, mismatch));
        }
    }
    let cwv = cwv.map(|(cwv, _)| cwv);
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
        match until_cancelled(cancel, run_axe_analysis(webview, titles)).await? {
            Ok((report, document_url)) => match admitted.verify(document_url.as_deref()) {
                Ok(()) => (Some(report), None),
                Err(mismatch) => {
                    return Ok(WebviewAnalysis::other_document(browser_build, mismatch))
                }
            },
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };

    Ok(WebviewAnalysis {
        cwv,
        accessibility,
        browser_ran: true,
        browser_build,
        error,
    })
}

const READY_TITLE_PREFIX: &str = "___SHK_READY___";
/// The analyzer starts on `about:blank` while the private-network rules
/// compile, and that document is already complete, so readiness must exclude
/// it or the scan would measure the blank page instead of the target.
const READY_PROBE_SCRIPT: &str = "if (document.readyState === 'complete' && location.href !== 'about:blank') { document.title = '___SHK_READY___'; }";
const BROWSER_UA_TITLE_PREFIX: &str = "___SHK_BROWSER_UA___";
const BROWSER_UA_PROBE_SCRIPT: &str =
    "document.title = '___SHK_BROWSER_UA___' + navigator.userAgent;";

/// Wait for page completion while handling deferred navigations, then allow
/// a short late-metric settle within the cap. A deferred target before the
/// main frame commits is a redirect hop: DNS-validated on this runtime,
/// recorded in `admitted`, and re-navigated; a hop that cannot be admitted,
/// and the hop past `MAX_REDIRECT_HOPS`, end the wait with the failure the
/// analysis reports. A deferred target after commit came from a subframe or
/// the page itself and never moves the main frame.
async fn wait_for_page_load(
    webview: &tauri::WebviewWindow,
    titles: &TitleBridge,
    gate: &NavigationGate,
    main_frame: &MainFrame,
    deferred: &mut tokio::sync::mpsc::UnboundedReceiver<url::Url>,
    admitted: &mut AdmittedDocuments,
) -> Result<(), String> {
    // Each probe first reads the title (picking up the previous iteration's
    // eval), then issues the next readiness eval; eval results can't be read
    // back directly in Tauri v2.
    let deadline = tokio::time::Instant::now() + crate::constants::WEBVIEW_PAGE_LOAD_WAIT;
    let mut hops = 0usize;
    let ready = loop {
        if titles.read_prefixed(READY_TITLE_PREFIX).is_some() {
            break true;
        }
        let _ = webview.eval(READY_PROBE_SCRIPT);
        if tokio::time::Instant::now() + crate::constants::WEBVIEW_POLL_INTERVAL > deadline {
            break false;
        }
        tokio::select! {
            _ = tokio::time::sleep(crate::constants::WEBVIEW_POLL_INTERVAL) => {}
            hop = deferred.recv() => match hop {
                Some(hop) => {
                    let outcome = admit_deferred_target(
                        gate,
                        main_frame,
                        hop,
                        &mut hops,
                        admitted,
                        &mut |target| {
                            let _ = webview.navigate(target);
                        },
                    )
                    .await;
                    if let Some(failure) = outcome.scan_failure() {
                        return Err(failure);
                    }
                    // A local name is admitted without ever reaching the
                    // resolver, so this arm can complete without suspending;
                    // yield so a burst of hops cannot starve the runtime.
                    tokio::task::yield_now().await;
                }
                None => tokio::time::sleep(crate::constants::WEBVIEW_POLL_INTERVAL).await,
            }
        }
    };

    if ready {
        tokio::time::sleep(crate::constants::WEBVIEW_POST_LOAD_SETTLE).await;
    }
    Ok(())
}

async fn collect_browser_build(
    webview: &tauri::WebviewWindow,
    titles: &TitleBridge,
) -> Option<String> {
    if webview.eval(BROWSER_UA_PROBE_SCRIPT).is_err() {
        return None;
    }
    let user_agent = poll_webview(
        crate::constants::WEBVIEW_POLL_INTERVAL,
        crate::constants::WEBVIEW_CWV_READ_TIMEOUT,
        || titles.read_prefixed(BROWSER_UA_TITLE_PREFIX),
    )
    .await?;
    browser_build_from_user_agent(crate::core::engine_release::browser_engine(), &user_agent)
}

/// Read CWV data from the webview via the document-title bridge, with the
/// `document_url` the payload recorded so the caller can verify which
/// document the sample describes.
async fn collect_cwv(
    webview: &tauri::WebviewWindow,
    titles: &TitleBridge,
) -> Option<(CoreWebVitals, Option<String>)> {
    if let Err(e) = webview.eval(CWV_READ_SCRIPT) {
        tracing::warn!("Failed to read CWV data: {}", e);
        return None;
    }

    let json = match read_bridged_json(
        webview,
        titles,
        CWV_RESULT_GLOBAL,
        CWV_TITLE_MARKER,
        crate::constants::WEBVIEW_POLL_INTERVAL,
        crate::constants::WEBVIEW_CWV_READ_TIMEOUT,
    )
    .await
    {
        Ok(json) => json,
        Err(error) => {
            tracing::warn!("Failed to read CWV data: {}", error);
            return None;
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&json) {
        Ok(value) => value,
        Err(e) => {
            tracing::warn!("Failed to parse CWV JSON: {} - raw: {}", e, json);
            return None;
        }
    };
    let document_url = payload_document_url(&value);
    match serde_json::from_value::<CoreWebVitals>(value) {
        Ok(cwv) => {
            // Return Some only if at least one metric was captured
            if cwv.lcp_ms.is_some()
                || cwv.cls.is_some()
                || cwv.fcp_ms.is_some()
                || cwv.ttfb_ms.is_some()
                || cwv.observed_long_task_blocking_ms.is_some()
                || cwv.js_error_count.is_some()
            {
                return Some((cwv, document_url));
            }
        }
        Err(e) => {
            tracing::warn!("Failed to parse CWV JSON: {} - raw: {}", e, json);
        }
    }

    None
}

/// Run axe in the main frame and read its report back, with the
/// `document_url` the payload recorded.
async fn run_axe_analysis(
    webview: &tauri::WebviewWindow,
    titles: &TitleBridge,
) -> Result<(AxeReport, Option<String>), String> {
    // Inject axe before its runner; the payload also tolerates deferred script
    // evaluation.
    if let Err(e) = webview.eval(browser::AXE_CORE_SCRIPT) {
        return Err(format!("Failed to inject axe-core: {}", e));
    }
    if let Err(e) = webview.eval(axe_run_script(AxeEvidenceCaps::DEFAULT)) {
        return Err(format!("Failed to run axe: {}", e));
    }

    // External pages cannot use __TAURI__, so the report crosses the
    // document-title bridge in chunks once axe parks it in its global.
    let started = std::time::Instant::now();
    let payload = read_bridged_json(
        webview,
        titles,
        AXE_RESULT_GLOBAL,
        AXE_TITLE_MARKER,
        crate::constants::AXE_POLL_INTERVAL,
        crate::constants::AXE_RESULT_TIMEOUT,
    )
    .await
    .map_err(|error| match error {
        BridgeReadError::NotReady => format!(
            "Axe-core timed out after {} seconds",
            crate::constants::AXE_RESULT_TIMEOUT.as_secs()
        ),
        other => format!("Axe-core results could not be read from the analyzer: {other}"),
    })?;

    let value: serde_json::Value = serde_json::from_str(&payload)
        .map_err(|error| format!("axe payload was not valid JSON: {error}"))?;
    let document_url = payload_document_url(&value);
    let report = axe_report_from_value(value)?;
    tracing::info!(
        "Axe-core completed after {}ms: {} violations, {} rules executed",
        started.elapsed().as_millis(),
        report.violations.len(),
        report.executed_rules().len()
    );
    Ok((report, document_url))
}

/// Decides navigations on the webview thread without DNS. The platform hook
/// this sits behind is frame-blind, so a subframe's navigation reaches the
/// gate the same way a top-level one does; the gate judges the URL and the
/// driver decides what a deferred URL means (see `deferred_navigation.rs`:
/// before the main frame commits it is a redirect hop to follow, after commit
/// it is a subframe or a page-initiated navigation and the main frame stays
/// put). IP literals and local names are judged inline; a hostname is allowed
/// only once the async runtime has resolved and validated it, so an unknown
/// host is deferred through the channel and refused for now.
pub(crate) struct NavigationGate {
    allow_local_dev: bool,
    allowed_hosts: std::sync::Mutex<std::collections::HashSet<String>>,
    deferred: tokio::sync::mpsc::UnboundedSender<url::Url>,
}

impl NavigationGate {
    pub(crate) fn new(
        origin: &url::Url,
        allow_local_dev: bool,
    ) -> (
        std::sync::Arc<Self>,
        tokio::sync::mpsc::UnboundedReceiver<url::Url>,
    ) {
        let (deferred, receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut allowed_hosts = std::collections::HashSet::new();
        if let Some(host) = normalized_domain(origin) {
            allowed_hosts.insert(host);
        }
        (
            std::sync::Arc::new(Self {
                allow_local_dev,
                allowed_hosts: std::sync::Mutex::new(allowed_hosts),
                deferred,
            }),
            receiver,
        )
    }

    pub(crate) fn decide(&self, target: &url::Url) -> bool {
        if target.as_str() == "about:blank" {
            return true;
        }
        let policy = crate::network_policy::UrlPolicy::Redirect {
            allow_local_dev: self.allow_local_dev,
        };
        if crate::network_policy::validate_redirect_target_nonblocking(target, policy).is_err() {
            return false;
        }
        let Some(host) = normalized_domain(target) else {
            // An IP literal that passed the inline check above.
            return target.host().is_some();
        };
        if self
            .allowed_hosts
            .lock()
            .map(|hosts| hosts.contains(&host))
            .unwrap_or(false)
        {
            return true;
        }
        let _ = self.deferred.send(target.clone());
        false
    }

    /// Async DNS validation for a deferred hop. On success the host joins the
    /// allow-set and the caller re-navigates.
    pub(crate) async fn admit_after_dns(&self, target: &url::Url) -> Result<(), String> {
        crate::network_policy::validate_url(
            target.as_str(),
            crate::network_policy::UrlPolicy::Redirect {
                allow_local_dev: self.allow_local_dev,
            },
        )
        .await?;
        if let Some(host) = normalized_domain(target) {
            self.allow_host(&host);
        }
        Ok(())
    }

    pub(crate) fn allow_host(&self, host: &str) {
        if let Ok(mut hosts) = self.allowed_hosts.lock() {
            hosts.insert(host.to_ascii_lowercase());
        }
    }
}

fn normalized_domain(url: &url::Url) -> Option<String> {
    match url.host()? {
        url::Host::Domain(domain) => Some(domain.trim_end_matches('.').to_ascii_lowercase()),
        url::Host::Ipv4(_) | url::Host::Ipv6(_) => None,
    }
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
#[path = "analyzer_tests.rs"]
mod tests;
