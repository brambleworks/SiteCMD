//! Grades temporary statuses in observed canonical redirect walks.

use crate::checks::performance::redirects::{RedirectHop, RedirectWalk, RedirectWalkTermination};
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

/// True for redirect statuses browsers and crawlers treat as temporary.
fn is_temporary_status(status: u16) -> bool {
    matches!(status, 302 | 303 | 307)
}

/// Return whether a hop changes only scheme or www/apex host.
fn is_canonicalizing_hop(from: &str, to: &str) -> bool {
    let (Ok(from), Ok(to)) = (url::Url::parse(from), url::Url::parse(to)) else {
        return false;
    };
    let (Some(from_host), Some(to_host)) = (from.host_str(), to.host_str()) else {
        return false;
    };
    let apex = |host: &str| {
        host.strip_prefix("www.")
            .map(str::to_string)
            .unwrap_or_else(|| host.to_string())
    };
    let same_site = apex(from_host) == apex(to_host);
    let same_page = from.path() == to.path() && from.query() == to.query();
    let form_changed = from.scheme() != to.scheme() || from_host != to_host;
    same_site && same_page && form_changed
}

/// First hop in the chain that canonicalizes the URL with a temporary
/// status. One finding is enough: the fix (changing the rule to 301/308)
/// is the same wherever it sits in the chain.
fn first_temporary_canonicalizing_hop(hops: &[RedirectHop]) -> Option<&RedirectHop> {
    hops.iter()
        .find(|hop| is_temporary_status(hop.status) && is_canonicalizing_hop(&hop.from, &hop.to))
}

fn safe_url(url: &str) -> String {
    crate::log_sanitizer::evidence_safe_page_url(url)
}

/// Build the result. Passes when the chain has no temporary canonicalizing
/// hop (including when there is no redirect at all).
fn temporary_redirect_result(hop: Option<&RedirectHop>) -> CheckResult {
    let (status, title, description, fix_prompt, manual_fix, raw_data, why_it_matters) = match hop {
        Some(hop) => {
            let from = safe_url(&hop.from);
            let to = safe_url(&hop.to);
            (
                CheckStatus::Warn,
                "URL scheme or host normalization uses a temporary status".to_string(),
                format!(
                    "{} redirects to {} with HTTP {}, a temporary status. If this is the intended long-term canonical scheme and hostname, a permanent 301 or 308 communicates that intent more accurately. Keep the temporary status when the transition is genuinely conditional or expected to revert.",
                    from, to, hop.status
                ),
                Some(format!(
                    "The redirect from {} to {} responds with HTTP {}. Confirm whether the destination is the permanent canonical form and review the methods that can reach this rule. If it is permanent, use 301 for GET/HEAD-only navigation or 308 where the method and body must be preserved; otherwise document and keep the temporary status.",
                    from, to, hop.status
                )),
                Some("Confirm the redirect is intended to be permanent, then update the server, CDN, or framework rule to 301 or to 308 when it must preserve the request method and body. Test representative methods, query strings, cache behavior, and the final canonical URL. Leave a temporary status in place when that accurately reflects product behavior.".to_string()),
                Some(serde_json::json!({
                    "from": from,
                    "to": to,
                    "status": hop.status,
                })),
                Some("A temporary status communicates that the source may remain authoritative, while a permanent status is a clearer canonicalization signal for clients and crawlers. The status alone does not determine search presentation or ranking.".to_string()),
            )
        }
        None => (
            CheckStatus::Pass,
            "Redirect statuses".to_string(),
            "No canonicalizing redirect uses a temporary status.".to_string(),
            None,
            None,
            None,
            None,
        ),
    };

    CheckResult {
        check_id: "seo.temporary_redirect".into(),
        category: ScanCategory::Seo,
        title,
        description,
        status,
        severity: Severity::Low,
        fix_prompt,
        manual_fix,
        raw_data,
        confidence: if status == CheckStatus::Pass {
            IssueConfidence::High
        } else {
            IssueConfidence::NeedsReview
        },
        confidence_reason: if status == CheckStatus::Pass {
            None
        } else {
            Some("Redirect status and URL change were observed; whether the destination is intentionally permanent requires product context.".into())
        },
        why_it_matters,
    }
}

/// Return a skipped verdict when the redirect walk has no recorded start URL.
pub fn temporary_redirect_unrecorded_start() -> CheckResult {
    CheckResult {
        check_id: "seo.temporary_redirect".into(),
        category: ScanCategory::Seo,
        title: "Redirect status review had no starting URL".into(),
        description: "This scan did not record the URL it requested, so the redirect walk these statuses are read from has no start. A chain nothing observed looks identical to a chain with no temporary canonicalization, so no pass is issued.".into(),
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

/// Grade the `seo.temporary_redirect` outcome from the completed walk.
pub fn evaluate_temporary_redirect(walk: &RedirectWalk) -> CheckResult {
    if let Some(hop) = first_temporary_canonicalizing_hop(&walk.hops) {
        return temporary_redirect_result(Some(hop));
    }
    if matches!(
        walk.termination,
        RedirectWalkTermination::FinalResponse { .. }
    ) {
        return temporary_redirect_result(None);
    }

    let (termination, safe_url, detail) = match &walk.termination {
        RedirectWalkTermination::Loop { url } => {
            ("loop", safe_url(url), "the walk entered a redirect loop")
        }
        RedirectWalkTermination::NetworkError { url } => (
            "network_error",
            safe_url(url),
            "the probe did not receive a final HTTP response",
        ),
        RedirectWalkTermination::MissingLocation { url, .. } => (
            "missing_location",
            safe_url(url),
            "a redirect response had no usable Location header",
        ),
        RedirectWalkTermination::InvalidLocation { url, .. } => (
            "invalid_location",
            safe_url(url),
            "a redirect response had an invalid Location value",
        ),
        RedirectWalkTermination::HopLimitReached { url, .. } => (
            "hop_limit_reached",
            safe_url(url),
            "the bounded walk reached its hop limit",
        ),
        RedirectWalkTermination::FinalResponse { .. } => unreachable!("handled above"),
    };

    CheckResult {
        check_id: "seo.temporary_redirect".into(),
        category: ScanCategory::Seo,
        title: "Redirect status review was inconclusive".into(),
        description: format!(
            "No temporary scheme/host canonicalization was observed in {} redirect hop{}, but the walk was inconclusive at {} because {}; no pass is issued for the unobserved remainder.",
            walk.hops.len(),
            if walk.hops.len() == 1 { "" } else { "s" },
            safe_url,
            detail,
        ),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "observed_hops": walk.hops.len(),
            "termination": termination,
            "at_url": safe_url,
        })),
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some(
            "The observed hops did not contain a temporary canonicalization, but the redirect walk did not reach a final response, so later status codes were not inspected."
                .into(),
        ),
        why_it_matters: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hop(from: &str, to: &str, status: u16) -> RedirectHop {
        RedirectHop {
            from: from.into(),
            to: to.into(),
            status,
        }
    }

    #[test]
    fn inconclusive_walk_does_not_emit_a_false_pass() {
        let walk = RedirectWalk {
            hops: Vec::new(),
            termination: RedirectWalkTermination::NetworkError {
                url: "https://example.com/".into(),
            },
        };
        let result = evaluate_temporary_redirect(&walk);
        assert_eq!(result.status, CheckStatus::Skipped);
        assert!(result.description.contains("inconclusive"));
        assert!(!result.description.starts_with("No canonicalizing"));
    }

    #[test]
    fn https_upgrade_via_302_is_a_temporary_canonicalizing_hop() {
        let hops = [hop(
            "http://example.com/page",
            "https://example.com/page",
            302,
        )];
        assert!(first_temporary_canonicalizing_hop(&hops).is_some());
    }

    #[test]
    fn https_upgrade_via_301_is_fine() {
        let hops = [hop(
            "http://example.com/page",
            "https://example.com/page",
            301,
        )];
        assert!(first_temporary_canonicalizing_hop(&hops).is_none());
    }

    #[test]
    fn www_swap_via_307_is_a_temporary_canonicalizing_hop() {
        let hops = [hop("https://www.example.com/", "https://example.com/", 307)];
        assert!(first_temporary_canonicalizing_hop(&hops).is_some());
    }

    #[test]
    fn combined_scheme_and_www_hop_via_303_is_flagged() {
        let hops = [hop("http://example.com/", "https://www.example.com/", 303)];
        assert!(first_temporary_canonicalizing_hop(&hops).is_some());
    }

    #[test]
    fn cross_domain_302_is_not_canonicalizing() {
        let hops = [hop(
            "https://old.example.com/",
            "https://other.example.net/",
            302,
        )];
        assert!(first_temporary_canonicalizing_hop(&hops).is_none());
    }

    #[test]
    fn path_change_302_is_a_content_redirect_not_canonicalizing() {
        let hops = [hop(
            "https://example.com/old",
            "https://example.com/new",
            302,
        )];
        assert!(first_temporary_canonicalizing_hop(&hops).is_none());
    }

    #[test]
    fn chain_without_temporary_hops_yields_a_passing_result() {
        let result = temporary_redirect_result(None);
        assert_eq!(result.check_id, "seo.temporary_redirect");
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn flagged_hop_result_names_the_status_code() {
        let hops = [hop("http://example.com/", "https://example.com/", 302)];
        let flagged = first_temporary_canonicalizing_hop(&hops);
        let result = temporary_redirect_result(flagged);
        assert_eq!(result.status, CheckStatus::Warn);
        assert!(result.description.contains("HTTP 302"));
        assert_eq!(result.confidence, IssueConfidence::NeedsReview);
        assert!(result.title.contains("scheme or host"));
        assert!(result
            .description
            .contains("If this is the intended long-term"));
        assert!(!result.description.contains("ranking signals"));
        assert!(result
            .manual_fix
            .as_deref()
            .is_some_and(|fix| fix.contains("Confirm") && fix.contains("preserve")));
        assert!(!result
            .why_it_matters
            .as_deref()
            .unwrap_or_default()
            .contains("consolidate slower"));
    }
}
