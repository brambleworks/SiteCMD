//! Plan and grade HTTP-to-HTTPS enforcement probes.
//! HSTS separately covers protection before the initial insecure request.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::probe::{BodyPolicy, ProbeOutcome, ProbeRequest, RedirectPolicy};

/// What the runtime should do after reading the scanned URL: either the
/// verdict is already complete (nothing probeable), or the bounded HTTP
/// origin root needs one no-follow request.
pub enum HttpsEnforcementStep {
    Done(Vec<CheckResult>),
    Probe { url: url::Url },
}

/// The probe for the HTTP origin root: one no-follow request whose status
/// and Location are the whole evidence, so no body is read.
pub fn http_origin_request(url: &url::Url) -> ProbeRequest {
    ProbeRequest::get(url.as_str())
        .body(BodyPolicy::None)
        .redirects(RedirectPolicy::None)
}

fn skipped(
    description: &str,
    manual_fix: Option<&str>,
    confidence_reason: &str,
    raw_data: Option<serde_json::Value>,
    why_it_matters: Option<&str>,
) -> Vec<CheckResult> {
    vec![CheckResult {
        check_id: "security.https_enforcement".into(),
        category: ScanCategory::Security,
        title: "HTTPS enforcement".into(),
        description: description.into(),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: manual_fix.map(str::to_string),
        raw_data,
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some(confidence_reason.into()),
        why_it_matters: why_it_matters.map(str::to_string),
    }]
}

/// Decide whether the scanned URL supports an HTTP-downgrade probe.
pub fn plan_https_enforcement(page_url: &url::Url) -> HttpsEnforcementStep {
    if page_url.scheme() != "https" {
        // An HTTP scan does not prove whether the same host supports HTTPS.
        return HttpsEnforcementStep::Done(skipped(
            "Site was scanned over HTTP. HTTPS was not tested. Re-scan with an https:// URL to verify HTTPS is available and that HTTP redirects to it.",
            Some("Re-run the scan with the canonical `https://` URL. Then verify the public HTTP origin redirects to HTTPS with an intentional permanent status (commonly 301 for GET/HEAD navigation or 308 when method preservation matters), and review HSTS separately."),
            "We can't tell whether HTTPS works for this host without a separate scan over HTTPS.",
            None,
            Some("If HTTPS isn't available or HTTP isn't redirected, login sessions, form submissions, and page content can all be intercepted or modified in transit."),
        ));
    }

    // Probe the public HTTP origin root, not the scanned page URL. Copying
    // userinfo, a query token, or a secret-bearing path into a cleartext
    // request would create the exposure this check is meant to prevent.
    match http_origin_probe_url(page_url) {
        Some(url) => HttpsEnforcementStep::Probe { url },
        None => HttpsEnforcementStep::Done(skipped(
            "Could not construct HTTP URL for testing.",
            None,
            "The scanner could not construct the bounded public HTTP-origin probe URL from the scanned URL.",
            None,
            None,
        )),
    }
}

/// Grade the HTTP-origin probe outcome.
pub fn evaluate_https_enforcement(probe_url: &str, outcome: ProbeOutcome) -> Vec<CheckResult> {
    let safe_probe_url = crate::log_sanitizer::log_safe_url_target(probe_url);
    let response = match outcome {
        ProbeOutcome::Response(response) => response,
        // A request failure has multiple indistinguishable causes;
        // preserve it as skipped rather than turning it into a pass.
        ProbeOutcome::Failure(_) => {
            return skipped(
                "The scanner could not obtain an HTTP response from the origin root within the probe limits. HTTP may be disabled, filtered, unreachable from this network, or temporarily failing; no redirect or successful cleartext response was verified.",
                Some("Repeat the root request from a representative public network with redirects disabled, then distinguish an intentional closed HTTP port from DNS, firewall, regional edge, or transient failures. No change is needed if HTTP is intentionally unavailable and supported clients use HTTPS directly."),
                "A request failure cannot distinguish intentional HTTP disablement from network, DNS, firewall, regional edge, or transient conditions.",
                Some(serde_json::json!({
                    "http_probe_url": safe_probe_url,
                    "outcome": "request_failed",
                    "error_detail_redacted": true,
                })),
                None,
            );
        }
    };

    let status = response.status;
    let location = response.header("location").map(str::to_string);
    let safe_location = location
        .as_deref()
        .map(crate::log_sanitizer::evidence_safe_url_reference);
    let grade = classify_http_redirect(status, location.as_deref());

    let (result_status, severity, title, description, manual_fix, why_it_matters) = match grade {
        HttpRedirectGrade::PermanentToHttps => (
            CheckStatus::Pass,
            Severity::Low,
            "HTTPS enforcement",
            format!(
                "The HTTP origin root returned status {} with a direct HTTPS Location ({}). This verifies the first response only; the initial HTTP request is still unauthenticated unless the client already knows an HSTS/preload policy, which is assessed separately.",
                status,
                safe_location.as_deref().unwrap_or("not provided")
            ),
            None,
            None,
        ),
        HttpRedirectGrade::TemporaryToHttps => (
            CheckStatus::Warn,
            Severity::Low,
            "HTTP redirects to HTTPS with a temporary redirect",
            format!(
                "The HTTP origin root returned temporary redirect status {} with a direct HTTPS Location ({}). This reaches HTTPS in the observed first hop, but does not express permanent canonicalization; cache and crawler treatment depends on the status and response headers. The initial HTTP request has the same transport exposure as any server-side redirect and is covered separately by HSTS.",
                status,
                safe_location.as_deref().unwrap_or("not provided")
            ),
            Some("If HTTP-to-HTTPS is intended to be permanent, change the edge/server rule to an appropriate permanent status: commonly 301 for GET/HEAD navigation or 308 when the request method and body must be preserved. Test representative methods and cache headers, and keep the temporary status when the destination is genuinely temporary.".to_string()),
            Some("A temporary status communicates weaker canonical intent than a permanent redirect. It does not by itself prove repeated insecure hops, split indexing, or a user-visible failure.".to_string()),
        ),
        HttpRedirectGrade::RedirectElsewhere => (
            CheckStatus::Warn,
            Severity::Medium,
            "First HTTP redirect is not directly HTTPS",
            format!(
                "The HTTP origin root returned status {} with Location {}. The first hop is not an absolute HTTPS target (or the Location is absent/malformed). Because this probe intentionally stops after one response, it does not establish whether a later hop reaches HTTPS.",
                status,
                safe_location.as_deref().unwrap_or("not provided")
            ),
            Some("Follow the complete redirect chain as a logged-out client and verify it terminates on the intended HTTPS canonical URL without an HTTP intermediate. Prefer a direct permanent HTTP-to-HTTPS hop when product behavior allows it, and test representative hosts, paths, methods, and query handling before changing the edge rule.".to_string()),
            Some("An HTTP intermediate or malformed redirect can extend cleartext exposure or strand clients, but this first-hop probe does not prove the final chain outcome.".to_string()),
        ),
        HttpRedirectGrade::AcceptsHttpRequest => (
            CheckStatus::Fail,
            Severity::High,
            "HTTP does not redirect to HTTPS",
            format!(
                "The HTTP origin root accepted the request with status {} instead of redirecting to HTTPS. This is direct evidence that this route can complete over cleartext HTTP; the probe did not inspect authenticated routes or every path.",
                status
            ),
            Some("At the public edge, redirect the HTTP origin to the equivalent HTTPS origin with an intentional permanent status, preserving paths and safe query semantics. Use 308 where non-GET method preservation matters, test representative routes/methods, and configure HSTS on HTTPS only after confirming the HTTPS estate is ready.".to_string()),
            Some("A successful cleartext HTTP response lets an on-path attacker observe or modify that request and response. Impact depends on the route and data; this root probe does not prove that credentials or authenticated content are exposed.".to_string()),
        ),
        HttpRedirectGrade::NoRedirectError => (
            // A non-success response is not proof of a successful
            // insecure copy. Its body and path/client variance are
            // still unknown, so describe only the observed root.
            CheckStatus::Warn,
            Severity::Low,
            "HTTP origin returns a non-success response instead of HTTPS",
            format!(
                "The HTTP origin root returned status {} without a recognized redirect. No successful HTTP page response was observed, but plain-http navigation may stop at this response rather than reach HTTPS; response bodies and behavior can vary by client or path.",
                status
            ),
            Some("Verify the public HTTP response from the intended regions and clients. If ordinary HTTP navigation should reach the site, add a direct permanent HTTPS redirect at the edge; keep a deliberate deny/error response only when that behavior is documented and compatible with the site's clients.".to_string()),
            Some("The observed root request did not serve a successful page over HTTP, but users or crawlers following an http:// URL may not reach the HTTPS site. This probe does not characterize every path or response body.".to_string()),
        ),
    };

    vec![CheckResult {
        check_id: "security.https_enforcement".into(),
        category: ScanCategory::Security,
        title: title.into(),
        description,
        status: result_status,
        severity,
        fix_prompt: None,
        manual_fix,
        raw_data: Some(serde_json::json!({
            "http_status": status,
            "http_probe_url": safe_probe_url,
            "location": safe_location,
            "redirects_to_https": !matches!(
                grade,
                HttpRedirectGrade::RedirectElsewhere
                    | HttpRedirectGrade::AcceptsHttpRequest
                    | HttpRedirectGrade::NoRedirectError
            ),
            "permanent_redirect": matches!(grade, HttpRedirectGrade::PermanentToHttps),
            "first_hop_only": true,
            "hsts_assessed_separately": true,
        })),
        confidence: if matches!(grade, HttpRedirectGrade::RedirectElsewhere) {
            IssueConfidence::NeedsReview
        } else {
            IssueConfidence::High
        },
        confidence_reason: if matches!(grade, HttpRedirectGrade::RedirectElsewhere) {
            Some("The first response is directly observed, but the no-redirect client did not follow a possible later chain to determine its final scheme or status.".into())
        } else {
            None
        },
        why_it_matters,
    }]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpRedirectGrade {
    /// 301/308 to an absolute https:// URL.
    PermanentToHttps,
    /// 302/303/307 to an absolute https:// URL.
    TemporaryToHttps,
    /// A recognized redirect whose first Location is not absolute HTTPS.
    RedirectElsewhere,
    /// No redirect and a 2xx: the root request completed over plain HTTP.
    AcceptsHttpRequest,
    /// No redirect and a non-2xx (404/403/5xx): port 80 answers, but
    /// plain-http visits dead-end on an error instead of reaching HTTPS.
    NoRedirectError,
}

fn http_origin_probe_url(source: &url::Url) -> Option<url::Url> {
    let mut target = source.clone();
    target.set_scheme("http").ok()?;
    target.set_username("").ok()?;
    target.set_password(None).ok()?;
    target.set_port(None).ok()?;
    target.set_path("/");
    target.set_query(None);
    target.set_fragment(None);
    Some(target)
}

fn classify_http_redirect(status: u16, location: Option<&str>) -> HttpRedirectGrade {
    // 304 is a conditional-request response, not an automatic redirect; 300
    // does not express a single selected target. Grade only the redirect
    // statuses this check can interpret unambiguously.
    let is_redirect = matches!(status, 301 | 302 | 303 | 307 | 308);
    if !is_redirect {
        return if (200..300).contains(&status) {
            HttpRedirectGrade::AcceptsHttpRequest
        } else {
            HttpRedirectGrade::NoRedirectError
        };
    }
    let to_https = location
        .and_then(|loc| url::Url::parse(loc.trim()).ok())
        .is_some_and(|target| target.scheme().eq_ignore_ascii_case("https"));
    if !to_https {
        return HttpRedirectGrade::RedirectElsewhere;
    }
    if matches!(status, 301 | 308) {
        HttpRedirectGrade::PermanentToHttps
    } else {
        HttpRedirectGrade::TemporaryToHttps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_redirect_permanence_is_graded() {
        assert_eq!(
            classify_http_redirect(301, Some("https://example.com/")),
            HttpRedirectGrade::PermanentToHttps
        );
        assert_eq!(
            classify_http_redirect(308, Some("https://example.com/")),
            HttpRedirectGrade::PermanentToHttps
        );
        assert_eq!(
            classify_http_redirect(302, Some("https://example.com/")),
            HttpRedirectGrade::TemporaryToHttps
        );
        assert_eq!(
            classify_http_redirect(307, Some("https://example.com/")),
            HttpRedirectGrade::TemporaryToHttps
        );
        assert_eq!(
            classify_http_redirect(302, Some("http://www.example.com/")),
            HttpRedirectGrade::RedirectElsewhere
        );
        assert_eq!(
            classify_http_redirect(301, None),
            HttpRedirectGrade::RedirectElsewhere
        );
        assert_eq!(
            classify_http_redirect(200, None),
            HttpRedirectGrade::AcceptsHttpRequest
        );
    }

    #[test]
    fn non_success_without_redirect_is_a_dead_end_not_insecure_content() {
        for status in [403u16, 404, 410, 500, 503] {
            assert_eq!(
                classify_http_redirect(status, None),
                HttpRedirectGrade::NoRedirectError,
                "status {} must classify as a dead-end, not served content",
                status
            );
        }
        assert_eq!(
            classify_http_redirect(204, None),
            HttpRedirectGrade::AcceptsHttpRequest
        );
        assert_eq!(
            classify_http_redirect(304, Some("https://example.com/")),
            HttpRedirectGrade::NoRedirectError,
            "304 is not an automatic redirect"
        );
    }

    #[test]
    fn downgrade_probe_never_copies_credentials_path_query_or_https_port() {
        let source = url::Url::parse(
            "https://user:password@example.com:8443/private/reset?token=secret#fragment",
        )
        .expect("test URL");
        let HttpsEnforcementStep::Probe { url } = plan_https_enforcement(&source) else {
            panic!("an https page must plan a downgrade probe");
        };
        assert_eq!(url.as_str(), "http://example.com/");
    }

    #[test]
    fn an_http_scan_target_is_skipped_without_a_probe() {
        let source = url::Url::parse("http://example.com/page").expect("test URL");
        let HttpsEnforcementStep::Done(results) = plan_https_enforcement(&source) else {
            panic!("an http page must not plan a downgrade probe");
        };
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("HTTPS was not tested"));
    }

    #[test]
    fn the_planned_request_never_follows_redirects_or_reads_a_body() {
        let url = url::Url::parse("http://example.com/").expect("test URL");
        let request = http_origin_request(&url);
        assert_eq!(request.redirects, RedirectPolicy::None);
        assert_eq!(request.body, BodyPolicy::None);
    }
}
