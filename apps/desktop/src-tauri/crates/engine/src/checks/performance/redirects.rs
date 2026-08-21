//! Portable redirect walker and `performance.redirect_chain` verdict.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::probe::{BodyPolicy, ProbeOutcome, ProbeRequest, RedirectPolicy};

pub const REDIRECT_HOP_LIMIT: usize = 10;

/// One observed redirect from the shared per-scan redirect walk: the URL
/// that responded, where it pointed, and the status code it used.
#[derive(Debug, Clone)]
pub struct RedirectHop {
    pub from: String,
    pub to: String,
    pub status: u16,
}

/// Why the bounded redirect walk stopped. Keeping transport/protocol failures
/// distinct from a final response prevents a failed probe from becoming the
/// false pass "no redirects detected."
#[derive(Debug, Clone)]
pub enum RedirectWalkTermination {
    FinalResponse { url: String, status: u16 },
    Loop { url: String },
    NetworkError { url: String },
    MissingLocation { url: String, status: u16 },
    InvalidLocation { url: String, status: u16 },
    HopLimitReached { url: String, limit: usize },
}

#[derive(Debug, Clone)]
pub struct RedirectWalk {
    pub hops: Vec<RedirectHop>,
    pub termination: RedirectWalkTermination,
}

/// The walk in progress: feed each probe outcome back to advance it.
#[derive(Debug)]
pub struct RedirectWalker {
    current: String,
    hops: Vec<RedirectHop>,
    visited: Vec<String>,
}

/// What the runtime should do after one observed response.
#[derive(Debug)]
pub enum RedirectWalkStep {
    /// Probe the walker's next `request`.
    Continue(RedirectWalker),
    Done(RedirectWalk),
}

impl RedirectWalker {
    pub fn new(start_url: &url::Url) -> Self {
        let current = start_url.to_string();
        Self {
            visited: vec![current.clone()],
            current,
            hops: Vec::new(),
        }
    }

    /// The probe for the walker's current position: a no-follow, no-body
    /// GET whose 3xx answer is the hop being observed.
    pub fn request(&self) -> ProbeRequest {
        ProbeRequest::get(&self.current)
            .body(BodyPolicy::None)
            .redirects(RedirectPolicy::None)
    }

    /// Advance the walk by one observed outcome.
    pub fn observe(mut self, outcome: &ProbeOutcome) -> RedirectWalkStep {
        let done = |hops, termination| RedirectWalkStep::Done(RedirectWalk { hops, termination });
        let response = match outcome {
            ProbeOutcome::Response(response) => response,
            ProbeOutcome::Failure(_) => {
                return done(
                    self.hops,
                    RedirectWalkTermination::NetworkError { url: self.current },
                );
            }
        };
        let status = response.status;
        if !(300..400).contains(&status) {
            return done(
                self.hops,
                RedirectWalkTermination::FinalResponse {
                    url: self.current,
                    status,
                },
            );
        }
        let Some(location) = response.header("location") else {
            return done(
                self.hops,
                RedirectWalkTermination::MissingLocation {
                    url: self.current,
                    status,
                },
            );
        };
        let Some(next) = resolve_location(&self.current, location) else {
            return done(
                self.hops,
                RedirectWalkTermination::InvalidLocation {
                    url: self.current,
                    status,
                },
            );
        };
        let is_revisit = self.visited.contains(&next);
        self.hops.push(RedirectHop {
            from: std::mem::replace(&mut self.current, next.clone()),
            to: next.clone(),
            status,
        });
        if is_revisit {
            return done(self.hops, RedirectWalkTermination::Loop { url: next });
        }
        if self.hops.len() >= REDIRECT_HOP_LIMIT {
            return done(
                self.hops,
                RedirectWalkTermination::HopLimitReached {
                    url: next,
                    limit: REDIRECT_HOP_LIMIT,
                },
            );
        }
        self.visited.push(next);
        RedirectWalkStep::Continue(self)
    }
}

/// Resolves a Location header with browser-compatible URL joining, preserving
/// ports and supporting scheme-relative and bare-relative targets.
fn resolve_location(current: &str, location: &str) -> Option<String> {
    let base = url::Url::parse(current).ok()?;
    base.join(location.trim()).ok().map(|u| u.to_string())
}

fn safe_url(url: &str) -> String {
    crate::log_sanitizer::evidence_safe_page_url(url)
}

/// Returns an inconclusive verdict when the requested URL is unavailable.
/// Starting from the response URL would hide redirects already followed.
pub fn redirect_chain_unrecorded_start() -> CheckResult {
    CheckResult {
        check_id: "performance.redirect_chain".into(),
        category: ScanCategory::Performance,
        title: "Redirect walk had no starting URL".into(),
        description: "This scan did not record the URL it requested, so the redirect walk has no start. The URL a response came from is the END of any chain, and walking from it would report no redirects even for a site that redirects, so no claim is made about this URL's chain.".into(),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "termination": "unrecorded_start",
        })),
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some(
            "The page record carries the URL the body came from, after redirects, and nothing about what was requested."
                .into(),
        ),
        why_it_matters: None,
    }
}

/// Grade the `performance.redirect_chain` outcome from the completed walk.
pub fn evaluate_redirect_chain(start_url: &str, walk: &RedirectWalk) -> CheckResult {
    let redirect_count = walk.hops.len();
    let safe_start = safe_url(start_url);
    let safe_hops: Vec<serde_json::Value> = walk
        .hops
        .iter()
        .map(|hop| {
            serde_json::json!({
                "from": safe_url(&hop.from),
                "to": safe_url(&hop.to),
                "status": hop.status,
            })
        })
        .collect();

    let (status, severity, title, description, manual_fix, confidence, confidence_reason, why_it_matters, termination, final_url, final_status, loop_url) =
        match &walk.termination {
            RedirectWalkTermination::Loop { url } => (
                CheckStatus::Fail,
                Severity::Medium,
                "Redirect loop detected".to_string(),
                format!(
                    "The redirect walk revisited {} after {} observed hop{}. This loop never resolves to a final response.",
                    safe_url(url),
                    redirect_count,
                    if redirect_count == 1 { "" } else { "s" },
                ),
                Some("Break the redirect cycle by finding the rule that points back to an earlier URL and targeting the intended final resource. Test scheme/host normalization, path/query handling, CDN and origin rules, and representative request methods after the change.".to_string()),
                IssueConfidence::High,
                None,
                Some("A redirect loop prevents the URL from reaching final content; clients eventually stop following the cycle and report a redirect error.".to_string()),
                "loop",
                None,
                None,
                Some(safe_url(url)),
            ),
            RedirectWalkTermination::HopLimitReached { url, limit } => (
                CheckStatus::Fail,
                Severity::Medium,
                format!("At least {limit} redirects observed"),
                format!("The bounded walk observed {redirect_count} redirects and reached {} but did not reach a final response within the {limit}-hop probe limit. The chain may continue beyond the observed evidence.", safe_url(url)),
                Some("Trace the full browser-facing chain and remove unnecessary or conflicting rules. Preserve deliberate method, locale, authentication, and consent transitions, then confirm the URL reaches the intended final response well within normal client redirect limits.".to_string()),
                IssueConfidence::High,
                None,
                Some("A very long redirect chain adds repeated round trips and can exceed client limits before final content is reached.".to_string()),
                "hop_limit_reached",
                None,
                None,
                None,
            ),
            RedirectWalkTermination::MissingLocation { url, status } => (
                CheckStatus::Fail,
                Severity::Medium,
                "Redirect response has no Location header".to_string(),
                format!("{} returned HTTP {} without a usable Location header, so the redirect walk could not continue to a destination.", safe_url(url), status),
                Some("Fix the response so it either returns the intended non-redirect status/content or includes one valid Location target. Test relative and absolute targets, query handling, cache behavior, and relevant request methods through every CDN/proxy/origin layer.".to_string()),
                IssueConfidence::High,
                None,
                Some("A redirect response without a usable destination can leave clients on an unintended response instead of the expected resource.".to_string()),
                "missing_location",
                None,
                Some(*status),
                None,
            ),
            RedirectWalkTermination::InvalidLocation { url, status } => (
                CheckStatus::Fail,
                Severity::Medium,
                "Redirect response has an invalid Location".to_string(),
                format!("{} returned HTTP {} with a Location value that could not be resolved against the current URL, so the redirect walk could not continue.", safe_url(url), status),
                Some("Replace the malformed Location with a valid absolute or correctly resolved relative URL. Test encoding, scheme-relative and relative paths, ports, query strings, and relevant request methods through the public response path.".to_string()),
                IssueConfidence::High,
                None,
                Some("A malformed redirect destination can prevent clients from reaching the intended resource.".to_string()),
                "invalid_location",
                None,
                Some(*status),
                None,
            ),
            RedirectWalkTermination::NetworkError { url } => (
                CheckStatus::Skipped,
                Severity::Low,
                "Redirect walk was inconclusive".to_string(),
                format!("The redirect probe became inconclusive at {} after {} observed hop{}. No claim is made about whether the chain resolves.", safe_url(url), redirect_count, if redirect_count == 1 { "" } else { "s" }),
                None,
                IssueConfidence::NeedsReview,
                Some("The probe did not receive a final HTTP response, so redirect count and resolution could not be fully graded.".to_string()),
                None,
                "network_error",
                None,
                None,
                None,
            ),
            RedirectWalkTermination::FinalResponse { url, status } => {
                let (check_status, check_severity) = if redirect_count >= 4 {
                    (CheckStatus::Fail, Severity::Medium)
                } else if redirect_count >= 2 {
                    (CheckStatus::Warn, Severity::Low)
                } else {
                    (CheckStatus::Pass, Severity::Low)
                };
                let title = if redirect_count >= 2 {
                    format!("{} redirects before final response", redirect_count)
                } else {
                    "Redirect chain".to_string()
                };
                let description = match redirect_count {
                    0 => format!("No redirects were observed. {} returned final HTTP {}.", safe_start, status),
                    1 => format!("One redirect was observed before {} returned final HTTP {}. A single canonicalization or content redirect can be intentional.", safe_url(url), status),
                    _ => format!("{} redirects were observed before {} returned final HTTP {}. Each hop adds another request/response round trip before final content; actual delay depends on network, cache, connection reuse, and server behavior.", redirect_count, safe_url(url), status),
                };
                (
                    check_status,
                    check_severity,
                    title,
                    description,
                    (redirect_count >= 2).then(|| "Review each observed hop by purpose and owning layer, collapse only unnecessary transitions, and update controlled internal links to the intended final URL where direct navigation is correct. Preserve deliberate authentication, consent, locale, and method-sensitive redirects, then retest the public chain.".to_string()),
                    IssueConfidence::High,
                    None,
                    (redirect_count >= 2).then(|| "Each redirect adds another request/response round trip before final content and can delay navigation; the actual cost depends on network, cache, connection reuse, and server behavior.".to_string()),
                    "final_response",
                    Some(safe_url(url)),
                    Some(*status),
                    None,
                )
            }
        };

    CheckResult {
        check_id: "performance.redirect_chain".into(),
        category: ScanCategory::Performance,
        title,
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix,
        raw_data: Some(serde_json::json!({
            "start_url": safe_start,
            "redirect_count": redirect_count,
            "hops": safe_hops,
            "termination": termination,
            "final_url": final_url,
            "final_status": final_status,
            "loop_url": loop_url,
        })),
        confidence,
        confidence_reason,
        why_it_matters,
    }
}

#[cfg(test)]
#[path = "redirects_tests.rs"]
mod tests;
