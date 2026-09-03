//! Plan and grade HTTP-to-HTTPS enforcement probes.
//! HSTS separately covers protection before the initial insecure request.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::probe::{BodyPolicy, ProbeOutcome, ProbeRequest, RedirectPolicy};

/// What the runtime should do after reading the scanned URL: either the
/// verdict is already complete (nothing probeable), or one bounded origin
/// root needs a no-follow request.
///
/// The two probes ask opposite questions and are graded by different
/// functions, so they are different variants rather than one variant whose
/// meaning a caller has to infer from the URL it is holding. Pair each with
/// the grader its doc names; nothing else will produce a correct verdict.
pub enum HttpsEnforcementStep {
    Done(Vec<CheckResult>),
    /// The scan ran over HTTPS. Fetch this HTTP origin root to see whether
    /// cleartext is redirected, and grade it with [`evaluate_http_downgrade`].
    ProbeHttpOrigin {
        url: url::Url,
    },
    /// The scan ran over cleartext HTTP. Fetch this HTTPS origin root to find
    /// out whether HTTPS exists to redirect to, and grade it with
    /// [`evaluate_https_availability`].
    ProbeHttpsOrigin {
        url: url::Url,
    },
}

/// The probe for an origin root: one no-follow request whose status and
/// Location are the whole evidence, so no body is read. Both steps use it.
pub fn origin_root_request(url: &url::Url) -> ProbeRequest {
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

/// Decide which origin root this scan can probe.
///
/// `page_is_local` is the caller's own local-environment verdict, the same
/// `PageContext::is_localhost` that gates the localhost skips in
/// `config.custom_404` and `security.cors_reflection`. It is a parameter
/// rather than something inferred from the URL here so this check cannot
/// disagree with those: a `.test`, `.ddev.site`, or `.local` dev host is local
/// to all three or to none of them. What that flag does NOT yet cover is a
/// bare private-LAN literal, which every one of the three treats as public.
pub fn plan_https_enforcement(page_url: &url::Url, page_is_local: bool) -> HttpsEnforcementStep {
    if page_url.scheme() != "https" {
        // A local preview server serving cleartext is not a defect in the
        // deployed site, and there is no deployed host here to ask.
        if page_is_local {
            return HttpsEnforcementStep::Done(skipped(
                "Skipped on localhost preview. A local preview server serving plain HTTP says nothing about whether the deployed site offers HTTPS or redirects to it.",
                Some("Re-run the scan against the deployed URL to verify that HTTPS is available and that the public HTTP origin redirects to it."),
                "A local preview server's transport does not establish the deployed site's HTTPS behavior.",
                Some(serde_json::json!({ "reason": "localhost_preview_server" })),
                None,
            ));
        }
        // The page this scan graded arrived over cleartext and was not
        // redirected to HTTPS on the way, because `PageContext::url` is the
        // URL the fetch finished on. Probe the HTTPS origin root to find out
        // whether HTTPS exists to redirect to.
        return match origin_root_probe_url(page_url, "https") {
            Some(url) => HttpsEnforcementStep::ProbeHttpsOrigin { url },
            None => HttpsEnforcementStep::Done(skipped(
                "Could not construct HTTPS URL for testing.",
                None,
                "The scanner could not construct the bounded public HTTPS-origin probe URL from the scanned URL.",
                None,
                None,
            )),
        };
    }

    // Probe the public HTTP origin root, not the scanned page URL. Copying
    // userinfo, a query token, or a secret-bearing path into a cleartext
    // request would create the exposure this check is meant to prevent.
    match origin_root_probe_url(page_url, "http") {
        Some(url) => HttpsEnforcementStep::ProbeHttpOrigin { url },
        None => HttpsEnforcementStep::Done(skipped(
            "Could not construct HTTP URL for testing.",
            None,
            "The scanner could not construct the bounded public HTTP-origin probe URL from the scanned URL.",
            None,
            None,
        )),
    }
}

/// Grade the HTTPS origin root of [`HttpsEnforcementStep::ProbeHttpsOrigin`].
///
/// The failure is established by the scan itself, not by this probe: the page
/// this scan graded was delivered over cleartext HTTP and the fetch was not
/// redirected to HTTPS. The probe only refines the wording, and it cannot
/// establish more than "a response arrived" or "none did" - a 502 from a
/// terminator whose backend is down still proves HTTPS is listening, and a
/// timeout or reset does not prove HTTPS is absent.
pub fn evaluate_https_availability(probe_url: &str, outcome: ProbeOutcome) -> Vec<CheckResult> {
    let safe_probe_url = crate::log_sanitizer::log_safe_url_target(probe_url);
    let https_answered = matches!(outcome, ProbeOutcome::Response(_));
    let https_status = match &outcome {
        ProbeOutcome::Response(response) => Some(response.status),
        ProbeOutcome::Failure(_) => None,
    };

    let (title, description, manual_fix) = if let Some(status) = https_status {
        (
            "HTTP does not redirect to HTTPS",
            format!(
                "This scan fetched the page over cleartext HTTP and was never redirected to HTTPS, and the HTTPS origin root ({}) answered with status {}. HTTPS is reachable for this host, so plain-http visitors are being served over cleartext when they could be sent to it.",
                safe_probe_url, status
            ),
            "At the public edge, redirect the HTTP origin to the equivalent HTTPS origin with an intentional permanent status, preserving paths and safe query semantics. Use 308 where non-GET method preservation matters, test representative routes/methods, and configure HSTS on HTTPS only after confirming the HTTPS estate is ready.",
        )
    } else {
        (
            // Not "the site has no HTTPS": one unanswered probe cannot
            // separate an absent listener from a timeout, a reset, or a
            // network in between. What is certain is the cleartext delivery
            // this scan already observed, so the title says only that.
            "No HTTPS response observed; site served over HTTP",
            format!(
                // Nor "the probe returned no response": a caller that planned
                // this probe and never executed it hands the same failure
                // outcome in (`evaluation::unexecuted_probe`), and
                // saying a request came back empty would then be describing a
                // request nobody made. State the absence, not its cause.
                "This scan fetched the page over cleartext HTTP, and no HTTPS response was observed for the HTTPS origin root ({}). An unanswered probe, a timeout, a reset, a filtered port, and an origin with no HTTPS at all are indistinguishable here; what is established is that this page was served in cleartext.",
                safe_probe_url
            ),
            "Obtain a certificate for the host (a free automated one from Let's Encrypt or your host's built-in provisioning is enough), serve the site over HTTPS, then redirect the HTTP origin to it with a permanent status. Confirm from a public network before adding HSTS.",
        )
    };

    vec![CheckResult {
        check_id: "security.https_enforcement".into(),
        category: ScanCategory::Security,
        title: title.into(),
        description,
        status: CheckStatus::Fail,
        severity: Severity::High,
        fix_prompt: None,
        manual_fix: Some(manual_fix.into()),
        raw_data: Some(serde_json::json!({
            "scanned_over_http": true,
            "https_probe_url": safe_probe_url,
            "https_status": https_status,
            "https_answered": https_answered,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: Some("Anything sent over cleartext HTTP can be read or modified by anyone on the network path between the visitor and the server, and browsers mark these pages as not secure.".into()),
    }]
}

/// Grade the HTTP origin root of [`HttpsEnforcementStep::ProbeHttpOrigin`].
pub fn evaluate_http_downgrade(probe_url: &str, outcome: ProbeOutcome) -> Vec<CheckResult> {
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

fn origin_root_probe_url(source: &url::Url, scheme: &str) -> Option<url::Url> {
    let mut target = source.clone();
    target.set_scheme(scheme).ok()?;
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
        let HttpsEnforcementStep::ProbeHttpOrigin { url } = plan_https_enforcement(&source, false)
        else {
            panic!("an https page must plan a downgrade probe");
        };
        assert_eq!(url.as_str(), "http://example.com/");
    }

    #[test]
    fn an_http_scan_probes_the_https_origin_root_instead_of_giving_up() {
        let source =
            url::Url::parse("http://user:password@example.com/private/page?token=secret#frag")
                .expect("test URL");
        let HttpsEnforcementStep::ProbeHttpsOrigin { url } = plan_https_enforcement(&source, false)
        else {
            panic!("an http page must plan an HTTPS availability probe");
        };
        assert_eq!(url.as_str(), "https://example.com/");
    }

    #[test]
    fn an_http_scan_whose_host_serves_https_is_a_missing_redirect_not_a_skip() {
        let outcome = ProbeOutcome::Response(crate::probe::ProbeResponse {
            status: 200,
            final_url: "https://example.com/".into(),
            content_type: None,
            content_length: None,
            headers: Vec::new(),
            body: None,
        });
        let results = evaluate_https_availability("https://example.com/", outcome);
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, Severity::High);
        assert_eq!(results[0].title, "HTTP does not redirect to HTTPS");
    }

    #[test]
    fn an_unanswered_https_probe_reports_what_was_seen_not_that_https_is_absent() {
        // A timeout, a reset, and a host with no TLS listener are the same
        // outcome here, and neverssl proves the difference matters: its HTTPS
        // origin answers on some attempts and sends an empty reply on others.
        // Whichever way that lands, the title must not assert HTTP-only.
        for class in [
            crate::probe::ProbeFailureClass::Transport,
            crate::probe::ProbeFailureClass::Timeout,
        ] {
            let outcome = ProbeOutcome::Failure(crate::probe::ProbeFailure {
                class,
                detail: "error sending request".into(),
            });
            let results = evaluate_https_availability("https://neverssl.com/", outcome);
            assert_eq!(results[0].status, CheckStatus::Fail);
            assert_eq!(results[0].severity, Severity::High);
            assert_eq!(
                results[0].title,
                "No HTTPS response observed; site served over HTTP"
            );
            assert!(
                !results[0].title.contains("only"),
                "{class:?}: one unanswered probe does not establish that the site has no HTTPS"
            );
            assert!(
                !results[0].description.contains("HTTPS was not tested"),
                "the cleartext delivery is observed, not untested"
            );
        }
    }

    #[test]
    fn a_probe_the_caller_never_ran_is_not_described_as_a_request_that_came_back_empty() {
        // The hosted lane synthesizes this exact outcome for a probe it
        // planned and did not execute. The Fail is still sound, because the
        // cleartext delivery comes from the page artifact rather than from
        // this probe, but the description must not narrate a request nobody
        // made.
        let results = evaluate_https_availability(
            "https://example.com/",
            crate::evaluation::unexecuted_probe(),
        );
        assert_eq!(results[0].status, CheckStatus::Fail);
        for claim in [
            "returned no response",
            "within the probe limits",
            "from this network",
        ] {
            assert!(
                !results[0].description.contains(claim),
                "an unexecuted probe cannot support `{claim}`, got {}",
                results[0].description
            );
        }
        assert!(results[0]
            .description
            .contains("no HTTPS response was observed"));
    }

    #[test]
    fn any_https_response_counts_as_reachable_including_a_gateway_error() {
        // A 502 from a terminator whose backend is down still proves HTTPS is
        // listening, which is what this branch claims and all it claims.
        let outcome = ProbeOutcome::Response(crate::probe::ProbeResponse {
            status: 502,
            final_url: "https://example.com/".into(),
            content_type: None,
            content_length: None,
            headers: Vec::new(),
            body: None,
        });
        let results = evaluate_https_availability("https://example.com/", outcome);
        assert_eq!(results[0].title, "HTTP does not redirect to HTTPS");
        assert!(
            results[0].description.contains("answered with status 502"),
            "the status is reported rather than characterized, got {}",
            results[0].description
        );
    }

    #[test]
    fn a_local_http_scan_keeps_the_preview_skip_on_the_callers_own_verdict() {
        // Whatever the runtime calls local is skipped here, whether or not
        // this file would have recognized it: a `.test` or `.ddev.site` dev
        // host skips for the same reason it skips in config.custom_404, which
        // reads the same flag. (Which hosts get the flag is the runtime's
        // decision, not this check's, and the desktop shell's tests pin that
        // side, including a private-LAN literal it does not yet cover.)
        for target in [
            "http://localhost:4321/",
            "http://127.0.0.1:4321/page",
            "http://[::1]:8080/",
            "http://my-app.ddev.site/",
            "http://shop.test/checkout",
            "http://192.168.1.40:8080/",
        ] {
            let source = url::Url::parse(target).expect("test URL");
            let HttpsEnforcementStep::Done(results) = plan_https_enforcement(&source, true) else {
                panic!("{target} must not plan a probe when the runtime calls it local");
            };
            assert_eq!(results[0].status, CheckStatus::Skipped);
            assert!(results[0].description.contains("localhost preview"));
        }
    }

    #[test]
    fn a_public_http_scan_is_not_skipped_as_local() {
        let source = url::Url::parse("http://neverssl.com/").expect("test URL");
        assert!(
            matches!(
                plan_https_enforcement(&source, false),
                HttpsEnforcementStep::ProbeHttpsOrigin { .. }
            ),
            "a public cleartext page must be probed, not skipped"
        );
    }

    #[test]
    fn the_planned_request_never_follows_redirects_or_reads_a_body() {
        let url = url::Url::parse("http://example.com/").expect("test URL");
        let request = origin_root_request(&url);
        assert_eq!(request.redirects, RedirectPolicy::None);
        assert_eq!(request.body, BodyPolicy::None);
    }
}
