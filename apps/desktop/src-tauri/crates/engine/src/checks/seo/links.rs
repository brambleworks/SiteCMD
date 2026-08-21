//! Portable plans and verdicts for broken links.
//! The desktop owns transport and network policy; this module owns sampling and grading.

use crate::checks::{CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use crate::probe::{ProbeOutcome, ProbeRequest};
use serde_json::{json, Value};
use url::Url;

// Coverage ratchet: the "we check your dead links" claim depends on these
// sample sizes. They may grow, never shrink (raise concurrency instead if
// scans feel slow); `link_sample_caps_never_shrink` pins the floor.

/// Canonical IDs for the portable link verdicts.
pub const INTERNAL_CHECK_ID: &str = "seo.broken_links";
pub const EXTERNAL_CHECK_ID: &str = "seo.broken_external_links";

/// How many same-origin links the broken-links check probes per page. Same
/// bound as the cross-page collector's per-page link cap, so "we check your
/// dead links" holds on real navigation-heavy pages, not just small ones.
pub const BROKEN_LINK_INTERNAL_SAMPLE: usize = 100;

/// How many external links the broken-links check probes per page. External
/// hosts are slower and rate-limit unfamiliar clients, so the sample stays
/// smaller than the internal one.
pub const BROKEN_LINK_EXTERNAL_SAMPLE: usize = 30;

/// Concurrent probes for same-origin links (the origin just served the page,
/// so it can absorb this burst).
pub const BROKEN_LINK_INTERNAL_CONCURRENCY: usize = 10;

/// Concurrent probes across external hosts, kept small so third-party sites
/// see a shallow burst, not a hammering.
pub const BROKEN_LINK_EXTERNAL_CONCURRENCY: usize = 8;

#[derive(Debug)]
pub struct LinkTargets {
    pub anchor_href_count: usize,
    pub internal: Vec<Url>,
    pub external: Vec<Url>,
    pub excluded_target_count: usize,
    pub effective_base_url: String,
}

fn decode_href(href: &str) -> String {
    crate::checks::html_attrs::decode_url_character_references(href)
}

/// Anchor href values in the initial HTML, excluding inert examples in
/// comments, scripts, and styles. This intentionally does not claim to see
/// links inserted into the rendered DOM by JavaScript.
pub fn collect_hrefs(body: &str) -> Vec<String> {
    let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(body, " ");
    let lower = scannable.to_ascii_lowercase();
    crate::checks::html_attrs::tag_slices(&scannable, &lower, "a")
        .into_iter()
        .filter_map(|tag| crate::checks::html_attrs::attr_value(tag, "href"))
        .map(|href| decode_href(&href))
        .collect()
}

/// The base every relative anchor resolves against: the first document
/// `<base href>` when it is a usable HTTP(S) URL, otherwise the page URL.
pub fn effective_base_url(ctx: &PageContext) -> Url {
    let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
    let lower = scannable.to_ascii_lowercase();
    let first_href = crate::checks::html_attrs::tag_slices(&scannable, &lower, "base")
        .into_iter()
        .find_map(|tag| crate::checks::html_attrs::attr_value(tag, "href"));
    first_href
        .as_deref()
        .map(decode_href)
        .and_then(|href| ctx.url.join(href.trim()).ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .unwrap_or_else(|| ctx.url.clone())
}

/// Resolve and classify every anchor destination. `allow_target` is the
/// runtime's network policy: a target it rejects is counted as excluded and
/// never enters a sample, so the policy gate cannot be bypassed by a plan.
pub fn resolve_link_targets(ctx: &PageContext, allow_target: impl Fn(&Url) -> bool) -> LinkTargets {
    let base = effective_base_url(ctx);
    let base_host = ctx.url.host_str().unwrap_or_default().trim_end_matches('.');
    let hrefs = collect_hrefs(&ctx.body);
    let anchor_href_count = hrefs.len();
    let mut internal = Vec::new();
    let mut external = Vec::new();
    let mut excluded_target_count = 0;

    for href in hrefs {
        let href = href.trim();
        if href.is_empty() || href.starts_with('#') {
            excluded_target_count += 1;
            continue;
        }
        let Ok(mut resolved) = base.join(href) else {
            excluded_target_count += 1;
            continue;
        };
        if !matches!(resolved.scheme(), "http" | "https") || !allow_target(&resolved) {
            excluded_target_count += 1;
            continue;
        }
        resolved.set_fragment(None);
        if resolved
            .host_str()
            .is_some_and(|host| host.trim_end_matches('.').eq_ignore_ascii_case(base_host))
        {
            internal.push(resolved);
        } else {
            external.push(resolved);
        }
    }

    internal.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
    internal.dedup_by(|a, b| a.as_str() == b.as_str());
    external.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
    external.dedup_by(|a, b| a.as_str() == b.as_str());

    LinkTargets {
        anchor_href_count,
        internal,
        external,
        excluded_target_count,
        effective_base_url: base.as_str().to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOutcomeKind {
    Responded,
    Missing,
    Inconclusive,
}

#[derive(Debug)]
pub struct LinkObservation {
    url: String,
    kind: ProbeOutcomeKind,
    status: Option<u16>,
    method: Option<&'static str>,
    note: Option<&'static str>,
}

impl LinkObservation {
    pub fn kind(&self) -> ProbeOutcomeKind {
        self.kind
    }
}

/// The first request for one destination: a status-only HEAD.
pub fn link_head_request(url: &Url) -> ProbeRequest {
    ProbeRequest::head(url.as_str())
}

/// The confirmation request after an error or failed HEAD.
pub fn link_get_request(url: &Url) -> ProbeRequest {
    ProbeRequest::get(url.as_str()).body(crate::probe::BodyPolicy::None)
}

/// Any error response from HEAD gets one GET attempt. Some frameworks do not
/// implement HEAD faithfully, and a 404/410 is not considered confirmed until
/// GET returns the same class of outcome.
pub fn head_status_needs_get_retry(status: u16) -> bool {
    status >= 400
}

/// Whether the HEAD outcome requires the GET confirmation request.
pub fn head_needs_get(head: &ProbeOutcome) -> bool {
    match head {
        ProbeOutcome::Failure(_) => true,
        ProbeOutcome::Response(response) => head_status_needs_get_retry(response.status),
    }
}

pub fn classify_probe_status(status: u16) -> ProbeOutcomeKind {
    match status {
        404 | 410 => ProbeOutcomeKind::Missing,
        100..=399 => ProbeOutcomeKind::Responded,
        _ => ProbeOutcomeKind::Inconclusive,
    }
}

/// Link evidence is persisted into issues and may later be exported. Retain
/// ordinary paths so the user can identify the link, while removing URL
/// credentials, query values, fragments, and token-shaped path segments.
pub fn evidence_url(raw_url: &str) -> String {
    crate::log_sanitizer::evidence_safe_page_url(raw_url)
}

/// Build one destination's observation from its HEAD outcome and, when the
/// HEAD required confirmation, the GET outcome.
pub fn observe_link(url: &Url, head: &ProbeOutcome, get: Option<&ProbeOutcome>) -> LinkObservation {
    let head_status = match head {
        ProbeOutcome::Response(response) => Some(response.status),
        ProbeOutcome::Failure(_) => None,
    };
    if let Some(status) = head_status {
        if !head_status_needs_get_retry(status) {
            return LinkObservation {
                url: url.to_string(),
                kind: classify_probe_status(status),
                status: Some(status),
                method: Some("HEAD"),
                note: None,
            };
        }
    }
    match get {
        Some(ProbeOutcome::Response(response)) => LinkObservation {
            url: url.to_string(),
            kind: classify_probe_status(response.status),
            status: Some(response.status),
            method: Some("GET"),
            note: None,
        },
        _ => LinkObservation {
            url: url.to_string(),
            kind: ProbeOutcomeKind::Inconclusive,
            status: head_status,
            method: head_status.map(|_| "HEAD"),
            note: Some(if head_status.is_some() {
                "GET confirmation request failed"
            } else {
                "HEAD and GET requests failed"
            }),
        },
    }
}

#[derive(Debug, Default)]
pub struct ProbeSummary {
    pub attempted_count: usize,
    pub responded_count: usize,
    pub broken: Vec<Value>,
    pub broken_labels: Vec<String>,
    pub inconclusive: Vec<Value>,
    pub inconclusive_labels: Vec<String>,
}

fn observation_json(observation: &LinkObservation) -> Value {
    json!({
        "url": evidence_url(&observation.url),
        "http_status": observation.status,
        "method": observation.method,
        "outcome": match observation.kind {
            ProbeOutcomeKind::Responded => "responded_without_404_or_410",
            ProbeOutcomeKind::Missing => "confirmed_404_or_410",
            ProbeOutcomeKind::Inconclusive => "inconclusive",
        },
        "note": observation.note,
    })
}

fn observation_label(observation: &LinkObservation) -> String {
    let safe_url = evidence_url(&observation.url);
    match (observation.status, observation.method, observation.note) {
        (Some(status), Some(method), Some(note)) => {
            format!("{safe_url} (HTTP {status} via {method}; {note})")
        }
        (Some(status), Some(method), None) => format!("{safe_url} (HTTP {status} via {method})"),
        (_, _, Some(note)) => format!("{safe_url} ({note})"),
        _ => format!("{safe_url} (probe outcome unavailable)"),
    }
}

/// Fold every destination's observation into the summary the verdict reads.
pub fn summarize_link_probes(
    attempted_count: usize,
    observations: Vec<LinkObservation>,
) -> ProbeSummary {
    let mut summary = ProbeSummary {
        attempted_count,
        ..ProbeSummary::default()
    };
    for observation in observations {
        match observation.kind {
            ProbeOutcomeKind::Responded => summary.responded_count += 1,
            ProbeOutcomeKind::Missing => {
                summary.broken_labels.push(observation_label(&observation));
                summary.broken.push(observation_json(&observation));
            }
            ProbeOutcomeKind::Inconclusive => {
                summary
                    .inconclusive_labels
                    .push(observation_label(&observation));
                summary.inconclusive.push(observation_json(&observation));
            }
        }
    }
    summary.broken_labels.sort_unstable();
    summary.inconclusive_labels.sort_unstable();
    summary.broken.sort_unstable_by_key(|a| a.to_string());
    summary.inconclusive.sort_unstable_by_key(|a| a.to_string());
    summary
}

/// Broken-link URLs kept in the human-readable description; the full list
/// always lives in raw_data. Keeps descriptions readable now that the sample
/// caps allow a link-rotted page to surface dozens of dead links.
const BROKEN_PREVIEW_LIMIT: usize = 10;

pub fn broken_preview(broken: &[String]) -> String {
    if broken.len() <= BROKEN_PREVIEW_LIMIT {
        return broken.join(", ");
    }
    format!(
        "{}, and {} more (full list in the issue details)",
        broken[..BROKEN_PREVIEW_LIMIT].join(", "),
        broken.len() - BROKEN_PREVIEW_LIMIT
    )
}

#[derive(Debug, Clone, Copy)]
pub enum LinkScope {
    Internal,
    External,
}

impl LinkScope {
    fn label(self) -> &'static str {
        match self {
            Self::Internal => "same-host",
            Self::External => "different-host",
        }
    }

    fn title_label(self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::External => "external",
        }
    }
}

fn link_probe_raw_data(
    targets: &LinkTargets,
    eligible_candidate_count: usize,
    sampled_candidate_count: usize,
    sample_limit: usize,
    summary: &ProbeSummary,
) -> Value {
    json!({
        "source_scope": "initial_html_anchor_href",
        "anchor_href_count": targets.anchor_href_count,
        "excluded_target_count": targets.excluded_target_count,
        "effective_base_target": evidence_url(&targets.effective_base_url),
        "eligible_candidate_count": eligible_candidate_count,
        "sample_limit": sample_limit,
        "sampled_candidate_count": sampled_candidate_count,
        "sample_truncated": eligible_candidate_count > sampled_candidate_count,
        "attempted_count": summary.attempted_count,
        "responded_without_404_or_410_count": summary.responded_count,
        "broken": summary.broken,
        "inconclusive": summary.inconclusive,
        "confirmed_missing_statuses": [404, 410],
        "soft_404_assessed": false,
        "destination_content_assessed": false,
        "fragment_targets_assessed": false,
        "authenticated_session_used": false,
        "rendered_dom_links_assessed": false
    })
}

pub fn no_link_targets_result(
    check_id: &str,
    severity: Severity,
    scope: LinkScope,
    targets: &LinkTargets,
    sample_limit: usize,
) -> CheckResult {
    let summary = ProbeSummary::default();
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Seo,
        title: format!("No eligible {} link targets", scope.title_label()),
        description: format!(
            "No {} HTTP(S) anchor destinations were eligible in the initial HTML. Fragment-only links, non-HTTP(S) schemes, disallowed network targets, and links inserted into the rendered DOM are outside this probe.",
            scope.label()
        ),
        status: CheckStatus::Pass,
        severity,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(link_probe_raw_data(targets, 0, 0, sample_limit, &summary)),
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

pub fn link_probe_result(
    check_id: &str,
    severity: Severity,
    scope: LinkScope,
    targets: &LinkTargets,
    eligible_candidate_count: usize,
    sampled_candidate_count: usize,
    sample_limit: usize,
    summary: ProbeSummary,
) -> CheckResult {
    let broken_count = summary.broken.len();
    let inconclusive_count = summary.inconclusive.len();
    let status = if broken_count > 0 {
        CheckStatus::Fail
    } else if inconclusive_count > 0 {
        CheckStatus::Skipped
    } else {
        CheckStatus::Pass
    };
    let sample_context = if eligible_candidate_count > sampled_candidate_count {
        format!(
            " {} of {} eligible destination{} {} sampled.",
            sampled_candidate_count,
            eligible_candidate_count,
            if eligible_candidate_count == 1 {
                ""
            } else {
                "s"
            },
            if sampled_candidate_count == 1 {
                "was"
            } else {
                "were"
            }
        )
    } else {
        format!(
            " {} eligible destination{} {} sampled.",
            sampled_candidate_count,
            if sampled_candidate_count == 1 {
                ""
            } else {
                "s"
            },
            if sampled_candidate_count == 1 {
                "was"
            } else {
                "were"
            }
        )
    };
    let description = match status {
        CheckStatus::Fail => {
            let inconclusive_note = if inconclusive_count == 0 {
                String::new()
            } else {
                format!(
                    " {} additional sampled destination{} had inconclusive responses and were not counted as broken.",
                    inconclusive_count,
                    if inconclusive_count == 1 { "" } else { "s" }
                )
            };
            format!(
                "At scan time, {} of {} sampled {} anchor destination{} returned HTTP 404 or 410 on a GET confirmation request: {}.{}{}",
                broken_count,
                sampled_candidate_count,
                scope.label(),
                if sampled_candidate_count == 1 { "" } else { "s" },
                broken_preview(&summary.broken_labels),
                inconclusive_note,
                sample_context
            )
        }
        CheckStatus::Skipped => format!(
            "The probe was inconclusive for {} of {} sampled {} anchor destination{}; the remaining {} responded without HTTP 404 or 410. No inconclusive outcome was counted as broken, and this result does not establish that every destination is valid.{}",
            inconclusive_count,
            sampled_candidate_count,
            scope.label(),
            if sampled_candidate_count == 1 { "" } else { "s" },
            summary.responded_count,
            sample_context
        ),
        CheckStatus::Pass => format!(
            "No HTTP 404 or 410 was observed among {} sampled {} anchor destination{} at scan time. This bounded status probe does not assess soft 404s, authenticated behavior, fragments, destination content, or links inserted after rendering.{}",
            sampled_candidate_count,
            scope.label(),
            if sampled_candidate_count == 1 { "" } else { "s" },
            sample_context
        ),
        CheckStatus::Warn => unreachable!("link probe does not emit Warn"),
    };
    let raw_data = link_probe_raw_data(
        targets,
        eligible_candidate_count,
        sampled_candidate_count,
        sample_limit,
        &summary,
    );

    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Seo,
        title: match status {
            CheckStatus::Fail => format!("Confirmed missing {} links", scope.title_label()),
            CheckStatus::Skipped => format!("{} link probe inconclusive", scope.title_label()),
            CheckStatus::Pass => format!("{} link status sample", scope.title_label()),
            CheckStatus::Warn => unreachable!("link probe does not emit Warn"),
        },
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix: (broken_count > 0).then(|| match scope {
            LinkScope::Internal => "Open each reported source link and confirm its intended destination. Correct the href, restore the route, add an appropriate permanent redirect if the page moved, or remove the link if the destination no longer exists. Re-run the probe after deployment; do not treat unrelated inconclusive responses as broken.".into(),
            LinkScope::External => "Open each reported outbound link and confirm the intended reference. Replace it with the provider's current destination or remove it when no trustworthy replacement exists. Re-run the probe after deployment; do not remove links solely because a separate response was inconclusive.".into(),
        }),
        raw_data: Some(raw_data),
        confidence: if status == CheckStatus::Skipped {
            crate::checks::IssueConfidence::NeedsReview
        } else {
            crate::checks::IssueConfidence::High
        },
        confidence_reason: match status {
            CheckStatus::Fail => Some("Each reported missing destination returned HTTP 404 or 410 to a GET confirmation request at scan time. Availability can still vary by time, geography, authentication, or request handling, so verify intent before changing content.".into()),
            CheckStatus::Skipped => Some("One or more sampled destinations returned a non-404/410 error or could not complete both probe attempts. Those observations cannot establish whether the links work for an intended user.".into()),
            CheckStatus::Pass | CheckStatus::Warn => None,
        },
        why_it_matters: (broken_count > 0).then(|| match scope {
            LinkScope::Internal => "A same-host anchor that returns 404 or 410 sends users and crawlers to a missing destination at scan time. Actual impact depends on the link's visibility, purpose, and traffic.".into(),
            LinkScope::External => "A different-host anchor that returns 404 or 410 sends users to a missing referenced destination at scan time. Actual impact depends on the link's context and importance.".into(),
        }),
    }
}

#[cfg(test)]
#[path = "links_tests.rs"]
mod tests;
