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
    // Resolve and validate the target on the async runtime; the navigation
    // callback runs on the webview thread and must never touch DNS.
    let parsed_url = match url::Url::parse(url) {
        Ok(parsed_url) => parsed_url,
        Err(error) => {
            return WebviewAnalysis::failed(format!(
                "Refused to analyze URL: Invalid URL: {error}"
            ));
        }
    };
    if let Err(error) = crate::network_policy::validate_url(
        parsed_url.as_str(),
        crate::network_policy::UrlPolicy::Scan,
    )
    .await
    {
        return WebviewAnalysis::failed(format!("Refused to analyze URL: {}", error));
    }
    let allow_local_dev = crate::network_policy::scan_origin_allows_local_dev(&parsed_url);
    let (gate, mut deferred) = NavigationGate::new(&parsed_url, allow_local_dev);
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
        .on_navigation({
            let gate = gate.clone();
            move |target| {
                // Apply public redirect policy to every top-level navigation. Tauri
                // does not expose external subresource interception here; the
                // platform rules from install_private_network_rules cover
                // subresources on macOS and Windows.
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
    match tokio::time::timeout(crate::constants::WEBVIEW_RULES_INSTALL_WAIT, ready_receiver).await {
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

    if let Err(error) = wait_for_page_load(&webview, &gate, &mut deferred).await {
        let _ = webview.close();
        return WebviewAnalysis::failed(error);
    }
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

/// Wait for page completion while admitting deferred redirect hops, then allow
/// a short late-metric settle within the cap. Each deferred hop is
/// DNS-validated on this runtime and re-navigated; a hop that cannot be
/// admitted, and the hop past `MAX_REDIRECT_HOPS`, end the wait with the
/// failure the analysis reports.
async fn wait_for_page_load(
    webview: &tauri::WebviewWindow,
    gate: &NavigationGate,
    deferred: &mut tokio::sync::mpsc::UnboundedReceiver<url::Url>,
) -> Result<(), String> {
    // Each probe first reads the title (picking up the previous iteration's
    // eval), then issues the next readiness eval; eval results can't be read
    // back directly in Tauri v2.
    let deadline = tokio::time::Instant::now() + crate::constants::WEBVIEW_PAGE_LOAD_WAIT;
    let mut hops = 0usize;
    let ready = loop {
        if let Ok(title) = webview.title() {
            if title.starts_with(READY_TITLE_PREFIX) {
                break true;
            }
        }
        let _ = webview.eval(READY_PROBE_SCRIPT);
        if tokio::time::Instant::now() + crate::constants::WEBVIEW_POLL_INTERVAL > deadline {
            break false;
        }
        tokio::select! {
            _ = tokio::time::sleep(crate::constants::WEBVIEW_POLL_INTERVAL) => {}
            hop = deferred.recv() => match hop {
                Some(hop) => {
                    let outcome = follow_deferred_hop(gate, hop, &mut hops, &mut |target| {
                        let _ = webview.navigate(target);
                    })
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

/// Decides top-level navigations on the webview thread without DNS. IP
/// literals and local names are judged inline; a hostname is allowed only
/// once `analyze_url` has resolved and validated it on the async runtime,
/// so an unknown host is deferred through the channel, refused for now, and
/// re-navigated after validation.
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

/// `network_policy` reports a failed lookup with this prefix; every other
/// error it returns is a policy refusal.
const RESOLUTION_FAILURE_PREFIX: &str = "Could not resolve URL host";

/// What the drain did with one deferred hop.
#[derive(Debug, PartialEq, Eq)]
enum HopOutcome {
    Followed,
    /// The hop passed the inline checks but resolved to an address the policy
    /// refuses, so a public-looking hostname pointed into a private range.
    RefusedByPolicy,
    /// The hop's host did not resolve, so it can never be validated.
    Unresolvable,
    /// The cross-host redirect chain reached `MAX_REDIRECT_HOPS`.
    HopLimitReached,
}

impl HopOutcome {
    /// The analysis failure this outcome reports, or `None` when the hop was
    /// followed. A hop the runtime cannot admit leaves the analyzer unable to
    /// reach its target, so the scan fails closed instead of reporting a
    /// completed browser run with missing metrics.
    fn scan_failure(&self) -> Option<String> {
        match self {
            Self::Followed => None,
            Self::RefusedByPolicy => Some(
                "Analyzer refused a redirect that resolved to a private network address"
                    .to_string(),
            ),
            Self::Unresolvable => Some("Analyzer could not resolve a redirect target".to_string()),
            Self::HopLimitReached => Some(format!(
                "Analyzer stopped after {} cross-host redirects",
                crate::constants::MAX_REDIRECT_HOPS
            )),
        }
    }
}

/// Whether a refused admission was a failed lookup or a policy refusal.
fn classify_admission_error(error: &str) -> HopOutcome {
    if error.starts_with(RESOLUTION_FAILURE_PREFIX) {
        HopOutcome::Unresolvable
    } else {
        HopOutcome::RefusedByPolicy
    }
}

/// Validate one deferred hop on the async runtime and re-navigate to it. The
/// hop counter is shared across the whole page load, so a chain of cross-host
/// redirects cannot outlive `MAX_REDIRECT_HOPS`.
async fn follow_deferred_hop(
    gate: &NavigationGate,
    hop: url::Url,
    hops: &mut usize,
    navigate: &mut impl FnMut(url::Url),
) -> HopOutcome {
    *hops += 1;
    if *hops > crate::constants::MAX_REDIRECT_HOPS {
        tracing::warn!(
            "Analyzer stopped following cross-host redirects after {} hops",
            *hops - 1
        );
        return HopOutcome::HopLimitReached;
    }
    match gate.admit_after_dns(&hop).await {
        Ok(()) => {
            navigate(hop);
            HopOutcome::Followed
        }
        Err(error) => {
            tracing::warn!(
                "Analyzer refused navigation to {}: {}",
                crate::log_sanitizer::log_safe_url_target(hop.as_str()),
                error
            );
            classify_admission_error(&error)
        }
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
