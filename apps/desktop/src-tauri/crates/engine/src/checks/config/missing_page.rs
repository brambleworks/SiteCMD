//! Portable custom-404 probe plan and verdicts.

use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};
use crate::probe::{BodyPolicy, ProbeOutcome, ProbeRequest};

/// Reproducible path that is deliberately unlikely to be a real route.
pub const MISSING_PAGE_PATH: &str = "/this-page-definitely-does-not-exist-shk-test";

/// The probe request: the status line is the primary evidence and the body
/// only refines it (404-page substance), so the body is read regardless of
/// status and its absence degrades rather than reclassifying.
pub fn missing_page_probe_request(origin_with_port: &str) -> ProbeRequest {
    ProbeRequest::get(format!("{origin_with_port}{MISSING_PAGE_PATH}")).body(BodyPolicy::Always)
}

/// The localhost-preview result: local preview servers often return their
/// own generic 404 page, so the deployed target is what matters.
pub fn localhost_skip_result() -> CheckResult {
    CheckResult {
        check_id: "config.custom_404".into(),
        category: ScanCategory::Polish,
        title: "Custom 404 page".into(),
        description: "Skipped on localhost preview. Local preview servers often return their own generic 404 page, so this is worth checking on a deployed preview or staging URL instead.".into(),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({"reason": "localhost_preview_server"})),
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

/// Classification of a probe for a definitely missing path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPageKind {
    /// 404 with a body long enough to look like a branded page.
    CustomPage,
    /// 404 with a short body: a bare server-default error page.
    ServerDefault,
    /// 2xx for the generated missing-looking path: a likely soft 404.
    Soft404,
    /// Some other non-404 status (301, 500,...).
    WrongStatus,
}

pub fn missing_page_kind(status_code: u16, body_len: usize) -> MissingPageKind {
    if status_code == 404 {
        if body_len > 500 {
            MissingPageKind::CustomPage
        } else {
            MissingPageKind::ServerDefault
        }
    } else if (200..300).contains(&status_code) {
        MissingPageKind::Soft404
    } else {
        MissingPageKind::WrongStatus
    }
}

/// Grade the missing-page probe outcome.
pub fn evaluate_missing_page(outcome: ProbeOutcome) -> Vec<CheckResult> {
    let ProbeOutcome::Response(response) = outcome else {
        return vec![CheckResult {
            check_id: "config.custom_404".into(),
            category: ScanCategory::Polish,
            title: "Custom 404 page".into(),
            description: "Could not test 404 page.".into(),
            status: CheckStatus::Skipped,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }];
    };

    let status_code = response.status;
    let body_len = response.body.as_ref().map_or(0, |body| body.text.len());
    let kind = missing_page_kind(status_code, body_len);
    let is_custom = kind == MissingPageKind::CustomPage;
    // A 2xx for the generated missing-looking path is a likely soft 404. One
    // probe cannot establish how every unknown URL behaves or whether an
    // unusual wildcard route is intentional.
    let is_soft_404 = kind == MissingPageKind::Soft404;
    let title = if is_custom {
        "404 response includes a substantial error page".to_string()
    } else if status_code == 404 {
        "404 response body is minimal".to_string()
    } else if is_soft_404 {
        format!(
            "Missing pages return HTTP {} (soft 404) instead of 404",
            status_code
        )
    } else {
        format!("Missing page returns HTTP {} instead of 404", status_code)
    };
    vec![CheckResult {
        check_id: "config.custom_404".into(),
        category: ScanCategory::Polish,
        title,
        description: if is_custom {
            "The generated missing-looking path returned HTTP 404 with a response body over 500 bytes. That confirms the status and a substantial body, but body length alone does not establish that the page is branded, accessible, useful, or consistent with the rest of the site.".into()
        } else if status_code == 404 {
            "The generated missing-looking path returned HTTP 404 with a short response body. It may be an intentional minimal API response or a generic server page; inspect it in a browser before deciding that a richer recovery experience is appropriate.".into()
        } else if is_soft_404 {
            format!(
                "The generated missing-looking path returned HTTP {} instead of 404, which is a likely soft-404 signal. This one probe does not establish that every unknown URL behaves this way or that search engines will classify the response as a soft 404. If the path is genuinely absent, a success status can confuse users and automated link/crawl handling and may cause search engines to exclude the response or spend crawl capacity on non-existent URLs.",
                status_code
            )
        } else {
            format!(
                "A page that does not exist returned HTTP {} instead of a 404.",
                status_code
            )
        },
        status: if is_custom {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: if is_custom {
            None
        } else if is_soft_404 {
            Some("Make unknown paths return a real HTTP 404 (or 410) status, then style that 404 response as a branded page. On SPA hosts, configure the platform's 404 handling instead of a blanket 200 catch-all: Netlify/Vercel/Cloudflare Pages all support a real 404 for unmatched routes.".into())
        } else {
            Some("Add a real 404 page with your normal navigation, a short explanation, and one or two clear ways back to working content.".into())
        },
        raw_data: Some(serde_json::json!({
            "status_code": status_code,
            "body_length": body_len,
            "soft_404": is_soft_404,
        })),
        confidence: if is_soft_404 {
            crate::checks::IssueConfidence::NeedsReview
        } else {
            crate::checks::IssueConfidence::High
        },
        confidence_reason: if is_soft_404 {
            Some("One generated missing-looking path returned a 2xx response; wildcard routing intent, response meaning, and other unknown paths were not evaluated.".into())
        } else {
            None
        },
        why_it_matters: if is_custom {
            None
        } else if is_soft_404 {
            Some("For genuinely absent content, an accurate 404 or 410 status helps users, crawlers, caches, and link checkers distinguish missing resources from successful pages.".into())
        } else {
            Some("A minimal 404 can be correct for an API, while a user-facing site may benefit from clear recovery links; the right treatment depends on the route and audience.".into())
        },
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_hundred_on_missing_path_is_a_soft_404() {
        // The soft-404 case  called out: a catch-all 200
        // means broken-link checks never see a failure.
        assert_eq!(missing_page_kind(200, 4000), MissingPageKind::Soft404);
        assert_eq!(missing_page_kind(204, 0), MissingPageKind::Soft404);
    }

    #[test]
    fn real_404s_are_classified_by_body_length_not_as_soft() {
        assert_eq!(missing_page_kind(404, 1200), MissingPageKind::CustomPage);
        assert_eq!(missing_page_kind(404, 80), MissingPageKind::ServerDefault);
    }

    #[test]
    fn other_statuses_are_wrong_status_not_soft() {
        assert_eq!(missing_page_kind(301, 0), MissingPageKind::WrongStatus);
        assert_eq!(missing_page_kind(500, 0), MissingPageKind::WrongStatus);
    }
}
