//! Portable www/non-www consistency check.
//! Alternate-host probes preserve redirects because a 3xx is the evidence.

use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};
use crate::probe::{BodyPolicy, ProbeOutcome, ProbeRequest, RedirectPolicy};

/// The alternate host for a page host: `www.` added when absent, stripped
/// when present.
pub fn alternate_host(host: &str) -> String {
    match host.strip_prefix("www.") {
        Some(bare) => bare.to_string(),
        None => format!("www.{host}"),
    }
}

/// The probe request: status only, redirects deliberately not followed.
pub fn alt_host_probe_request(scheme: &str, alt_host: &str) -> ProbeRequest {
    ProbeRequest::get(format!("{scheme}://{alt_host}/"))
        .body(BodyPolicy::None)
        .redirects(RedirectPolicy::None)
}

/// Classification of the alternate host's direct response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltHostResponse {
    /// 3xx: the alternate host redirects to a canonical host.
    Redirects,
    /// 2xx: both hosts serve the site directly - a duplicate-content risk.
    ServesSite,
    /// 4xx/5xx: the alternate host answers but does not duplicate the site.
    ErrorStatus,
}

pub fn alt_host_response(status: u16) -> AltHostResponse {
    if (300..400).contains(&status) {
        AltHostResponse::Redirects
    } else if (200..300).contains(&status) {
        AltHostResponse::ServesSite
    } else {
        AltHostResponse::ErrorStatus
    }
}

/// Grade the alternate-host probe outcome.
pub fn evaluate_alt_host(alt_host: &str, outcome: ProbeOutcome) -> Vec<CheckResult> {
    let ProbeOutcome::Response(response) = outcome else {
        // The probe cannot distinguish a missing host from a timeout, so neither
        // can support a clean consistency verdict.
        return vec![CheckResult {
            check_id: "config.www_redirect".into(),
            category: ScanCategory::Seo,
            title: "www/non-www probe did not complete".into(),
            description: format!(
                "The request to {} did not complete, so this scan produced no www/non-www consistency verdict. A host that does not exist at all is a common and harmless reason for that, but so is a timeout at a host that does serve the site.",
                alt_host
            ),
            status: CheckStatus::Skipped,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "alt_host": alt_host,
                "reason": "request_failed",
            })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some(
                "The alternate host returned no response, which does not establish whether it serves the site, redirects, or does not exist.".into(),
            ),
            why_it_matters: None,
        }];
    };

    let status_code = response.status;
    let verdict = alt_host_response(status_code);
    let duplicate = verdict == AltHostResponse::ServesSite;
    vec![CheckResult {
        check_id: "config.www_redirect".into(),
        category: ScanCategory::Seo,
        title: if duplicate {
            "No redirect between www and non-www".into()
        } else {
            "www/non-www consistency".into()
        },
        description: match verdict {
            AltHostResponse::Redirects => format!(
                "{} redirects correctly, so search engines and visitors land on one canonical host.",
                alt_host
            ),
            AltHostResponse::ServesSite => "Both the www and non-www versions serve the site directly (each answers with a success status). Pick one host and redirect the other to it so you are not splitting authority across two copies of the site.".to_string(),
            AltHostResponse::ErrorStatus => format!(
                "{} answers with HTTP {} rather than serving the site, so there is no duplicate copy. Redirecting it to your canonical host would help visitors who type that variant, but nothing is being split.",
                alt_host, status_code
            ),
        },
        status: if duplicate {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        },
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: if duplicate {
            Some("Choose the host you want to keep, then add a permanent redirect from the other variant at the CDN, reverse proxy, or hosting layer.".into())
        } else {
            None
        },
        raw_data: Some(serde_json::json!({
            "alt_host": alt_host,
            "alt_status_code": status_code,
        })),
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: if duplicate {
            Some("If both hosts stay live, backlinks, canonical signals, and crawl budget get split between two versions of the same site.".into())
        } else {
            None
        },
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_statuses_classify_as_redirects() {
        for status in [301u16, 302, 307, 308] {
            assert_eq!(alt_host_response(status), AltHostResponse::Redirects);
        }
    }

    #[test]
    fn success_statuses_mean_the_alternate_host_serves_the_site() {
        assert_eq!(alt_host_response(200), AltHostResponse::ServesSite);
        assert_eq!(alt_host_response(204), AltHostResponse::ServesSite);
    }

    #[test]
    fn error_statuses_are_not_a_duplicate_content_risk() {
        for status in [400u16, 403, 404, 410, 500, 503] {
            assert_eq!(alt_host_response(status), AltHostResponse::ErrorStatus);
        }
    }

    #[test]
    fn alternate_host_toggles_the_www_prefix() {
        assert_eq!(alternate_host("example.com"), "www.example.com");
        assert_eq!(alternate_host("www.example.com"), "example.com");
    }

    #[test]
    fn a_failed_probe_declines_to_grade_rather_than_passing() {
        use crate::probe::{ProbeFailure, ProbeFailureClass};
        let results = evaluate_alt_host(
            "www.example.com",
            ProbeOutcome::Failure(ProbeFailure {
                class: ProbeFailureClass::Transport,
                detail: "connection refused".into(),
            }),
        );
        assert_ne!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].title.contains("did not complete"));
    }
}
