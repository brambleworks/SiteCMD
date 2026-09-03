//! What the analyzer does with a navigation its gate deferred.
//!
//! The gate refuses any host it has not validated and hands the URL to the
//! driver. Before the main frame commits a document, such a URL is a server
//! redirect hop: the driver validates it on the async runtime and
//! re-navigates. Once the main frame holds an admitted document, a deferred
//! URL came from a subframe (an ad or consent iframe) or from a navigation
//! the page started, and re-navigating would leave the analyzer grading that
//! document instead of the page. The host may still be admitted so later
//! loads of it succeed, but the main frame stays where it is.

use super::analyzer::NavigationGate;
use sitecmd_engine::browser::AdmittedDocuments;
use std::sync::{Arc, Mutex};
use tauri::webview::PageLoadEvent;

/// The document the main frame holds, recorded from the webview's page-load
/// events. `PageLoadEvent::Started` is WebKit's `didCommitNavigation` and
/// WebView2's `ContentLoading`: both are raised for the main frame only and
/// only after any server redirect, so a recorded URL means the main frame
/// committed that document. The blank start page is never recorded.
#[derive(Clone, Default)]
pub(crate) struct MainFrame(Arc<Mutex<Option<url::Url>>>);

impl MainFrame {
    pub(crate) fn record(&self, event: PageLoadEvent, url: &url::Url) {
        if event != PageLoadEvent::Started || url.as_str() == "about:blank" {
            return;
        }
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(url.clone());
        }
    }

    /// The committed document, or `None` while the main frame is still on
    /// the blank start page or inside a provisional navigation.
    pub(crate) fn committed(&self) -> Option<url::Url> {
        self.0.lock().ok()?.clone()
    }
}

/// `network_policy` reports a failed lookup with this prefix; every other
/// error it returns is a policy refusal.
const RESOLUTION_FAILURE_PREFIX: &str = "Could not resolve URL host";

/// What the driver did with one deferred navigation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HopOutcome {
    /// A redirect hop: validated, recorded as an admitted document, and
    /// re-navigated.
    Followed,
    /// The main frame already held an admitted document, so the target came
    /// from a subframe or a navigation the page started. The analyzer stayed
    /// on its document; `host_admitted` says whether later loads of the host
    /// will be allowed.
    StayedOnDocument { host_admitted: bool },
    /// The hop passed the inline checks but resolved to an address the policy
    /// refuses, so a public-looking hostname pointed into a private range.
    RefusedByPolicy,
    /// The hop's host did not resolve, so it can never be validated.
    Unresolvable,
    /// The cross-host redirect chain reached `MAX_REDIRECT_HOPS`.
    HopLimitReached,
}

impl HopOutcome {
    /// The analysis failure this outcome reports, or `None` when the analyzer
    /// still reaches its target. A hop the runtime cannot admit leaves the
    /// analyzer unable to reach the page, so the scan fails closed instead
    /// of reporting a completed browser run with missing metrics. A subframe
    /// the runtime keeps blocked is the page's problem, not the scan's.
    pub(crate) fn scan_failure(&self) -> Option<String> {
        match self {
            Self::Followed | Self::StayedOnDocument { .. } => None,
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
pub(crate) fn classify_admission_error(error: &str) -> HopOutcome {
    if error.starts_with(RESOLUTION_FAILURE_PREFIX) {
        HopOutcome::Unresolvable
    } else {
        HopOutcome::RefusedByPolicy
    }
}

/// Validate one deferred navigation on the async runtime. Before the main
/// frame commits it is a redirect hop: re-navigate to it and count it against
/// `MAX_REDIRECT_HOPS`, which is shared across the whole page load so a chain
/// of cross-host redirects cannot outlive the budget. After commit the main
/// frame keeps its document and only the host is admitted.
pub(crate) async fn admit_deferred_target(
    gate: &NavigationGate,
    main_frame: &MainFrame,
    target: url::Url,
    hops: &mut usize,
    admitted: &mut AdmittedDocuments,
    navigate: &mut impl FnMut(url::Url),
) -> HopOutcome {
    if main_frame.committed().is_some() {
        let host_admitted = match gate.admit_after_dns(&target).await {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(
                    "Analyzer kept a subframe navigation blocked: {}: {}",
                    crate::log_sanitizer::log_safe_url_target(target.as_str()),
                    error
                );
                false
            }
        };
        return HopOutcome::StayedOnDocument { host_admitted };
    }
    *hops += 1;
    if *hops > crate::constants::MAX_REDIRECT_HOPS {
        tracing::warn!(
            "Analyzer stopped following cross-host redirects after {} hops",
            *hops - 1
        );
        return HopOutcome::HopLimitReached;
    }
    match gate.admit_after_dns(&target).await {
        Ok(()) => {
            admitted.admit(&target);
            navigate(target);
            HopOutcome::Followed
        }
        Err(error) => {
            tracing::warn!(
                "Analyzer refused navigation to {}: {}",
                crate::log_sanitizer::log_safe_url_target(target.as_str()),
                error
            );
            classify_admission_error(&error)
        }
    }
}

#[cfg(test)]
#[path = "deferred_navigation_tests.rs"]
mod tests;
