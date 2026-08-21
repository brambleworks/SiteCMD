//! Grade CORS response headers and reflected-origin probes.
//! The desktop executes the foreign-origin request; this module owns its plan and verdict.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

/// Checks for CORS misconfiguration in response headers
pub struct CorsCheck;

impl Check for CorsCheck {
    fn id(&self) -> &str {
        "security.cors"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let acao = ctx
            .response_headers
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let acac = ctx
            .response_headers
            .get("access-control-allow-credentials")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let is_wildcard = acao == "*";
        // Sandboxed frames, file URLs, and some redirects send `Origin: null`,
        // so treating it as a trusted specific origin is unsafe.
        let is_null_origin = acao.eq_ignore_ascii_case("null");
        let credentials_allowed = acac.eq_ignore_ascii_case("true");

        let (status, severity, title, description, manual_fix) = if is_wildcard
            && credentials_allowed
        {
            // Configuration-error signal: browsers reject the combination,
            // so nothing observed here is exploitable as-is.
            (
                CheckStatus::Fail,
                Severity::Medium,
                "Invalid credentialed CORS combination",
                "The response combines Access-Control-Allow-Origin: * with Access-Control-Allow-Credentials: true. Browsers reject that combination, so it does not grant credentialed cross-origin reads; it can instead break the cross-origin flow the server appears to intend.".to_string(),
                Some("Decide whether this response is public or credentialed. For public, non-user-specific content, keep '*' and remove Access-Control-Allow-Credentials. If credentials are required, compare Origin against an explicit trusted-origin allowlist, return only the matched origin, add Vary: Origin, and test the real browser flow.".to_string()),
            )
        } else if is_wildcard {
            (
                CheckStatus::Warn,
                Severity::Low,
                "CORS allows any origin",
                "Access-Control-Allow-Origin: * permits browser scripts on other origins to read qualifying non-credentialed responses from this URL. This can be intentional for a public API or static asset. The sampled response does not establish whether the content is public or whether authenticated or user-specific routes share this policy, so verify those boundaries before changing it.".to_string(),
                Some("If this response is intentionally public and non-user-specific, the wildcard can remain. If this route exposes private or user-specific content, return an explicitly allowlisted Origin and add Vary: Origin; review other routes separately because CORS policy may be route-specific.".to_string()),
            )
        } else if is_null_origin {
            (
                CheckStatus::Warn,
                Severity::Medium,
                "CORS allows the null origin",
                "CORS is configured with Access-Control-Allow-Origin: null. The null origin is shared by sandboxed documents, local files, and some opaque-origin contexts, so it cannot identify one trusted application. A hostile sandboxed page can also send Origin: null; exposure depends on whether this response contains non-public data and whether the browser sends any required authorization.".to_string(),
                Some("Replace Access-Control-Allow-Origin: null with the explicit origins that need access (or '*' if the content is genuinely public). If a local file or sandboxed frame needs the API, give it a real origin instead of relying on null.".to_string()),
            )
        } else {
            (
                CheckStatus::Pass,
                Severity::Low,
                "CORS configuration",
                if acao.is_empty() {
                    "No Access-Control-Allow-Origin header was observed. Browser CORS therefore does not grant scripts on other origins permission to read this response. This check does not assess JSONP, postMessage, embeds, non-browser clients, other routes, or runtime/proxy variants.".to_string()
                } else {
                    format!("CORS returned one specific origin: {}. No wildcard or null-origin policy was observed in this response; the active reflection probe separately checks whether arbitrary origins are echoed.", acao)
                },
                None,
            )
        };

        vec![CheckResult {
            check_id: "security.cors".into(),
            category: ScanCategory::Security,
            title: title.into(),
            description,
            status,
            severity,
            fix_prompt: None,
            manual_fix,
            raw_data: if !acao.is_empty() {
                Some(serde_json::json!({
                    "access_control_allow_origin": acao,
                    "access_control_allow_credentials": acac,
                }))
            } else {
                None
            },
            confidence: if is_wildcard && !credentials_allowed || is_null_origin {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: if is_wildcard && !credentials_allowed {
                Some("The wildcard header is directly observed, but whether the response is intentionally public and whether other routes use the same policy require review.".into())
            } else if is_null_origin {
                Some("The null-origin policy is directly observed, but exploitability depends on response sensitivity, authorization, credential behavior, and the requesting context.".into())
            } else {
                None
            },
            why_it_matters: if is_wildcard && credentials_allowed {
                Some(
                    "Browsers reject this combination. The immediate impact is a broken or ineffective credentialed cross-origin policy, not a demonstrated data disclosure."
                        .into(),
                )
            } else if is_wildcard {
                Some("Fine for public content; a data leak only if authenticated or user-specific responses share this policy.".into())
            } else if is_null_origin {
                Some("The null origin is not a stable application identity. If this response exposes non-public data without another effective authorization boundary, an untrusted opaque-origin context may be able to read it.".into())
            } else {
                None
            },
        }]
    }
}

/// A fixed origin no legitimate allowlist would contain (.example is an
/// IANA-reserved TLD), so an echo of it proves the server reflects
/// arbitrary origins rather than matching a list.
pub const PROBE_ORIGIN: &str = "https://sitecmd-cors-probe.example";

/// The probe request: re-request the page with a foreign Origin header and
/// read only the response headers.
pub fn reflection_probe_request(page_url: &str) -> crate::probe::ProbeRequest {
    crate::probe::ProbeRequest::get(page_url)
        .body(crate::probe::BodyPolicy::None)
        .header("Origin", PROBE_ORIGIN)
}

/// The localhost-preview result: dev servers (Vite, webpack-dev-server) are
/// deliberately permissive about origins, so grade this on a deployed target.
pub fn reflection_localhost_skip_result() -> Vec<CheckResult> {
    reflection_result(
        CheckStatus::Skipped,
        Severity::Low,
        "CORS origin reflection",
        "Skipped on localhost preview. Local dev servers are deliberately permissive about origins, so check this on a deployed target.".into(),
        None,
        Some(serde_json::json!({"reason": "localhost_preview_server"})),
        None,
    )
}

/// Grade a reflection probe's response headers. Split from the request so
/// the verdict logic is testable without a live server.
fn reflection_verdict(acao: Option<&str>, acac: Option<&str>) -> Option<(CheckStatus, Severity)> {
    let reflected = acao.map(|v| v.trim() == PROBE_ORIGIN).unwrap_or(false);
    if !reflected {
        return None;
    }
    let credentials = acac
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if credentials {
        Some((CheckStatus::Fail, Severity::High))
    } else {
        Some((CheckStatus::Warn, Severity::Low))
    }
}

fn reflection_result(
    status: CheckStatus,
    severity: Severity,
    title: &str,
    description: String,
    manual_fix: Option<String>,
    raw_data: Option<serde_json::Value>,
    why_it_matters: Option<String>,
) -> Vec<CheckResult> {
    vec![CheckResult {
        check_id: "security.cors_reflection".into(),
        category: ScanCategory::Security,
        title: title.into(),
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix,
        raw_data,
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters,
    }]
}

/// Grade the reflection probe outcome.
pub fn evaluate_reflection(outcome: crate::probe::ProbeOutcome) -> Vec<CheckResult> {
    let crate::probe::ProbeOutcome::Response(response) = outcome else {
        return reflection_result(
            CheckStatus::Skipped,
            Severity::Low,
            "CORS origin reflection",
            "Could not probe for CORS origin reflection (the request with a foreign Origin header failed).".into(),
            None,
            None,
            None,
        );
    };
    let acao = response
        .header("access-control-allow-origin")
        .map(String::from);
    let acac = response
        .header("access-control-allow-credentials")
        .map(String::from);

    match reflection_verdict(acao.as_deref(), acac.as_deref()) {
        Some((CheckStatus::Fail, severity)) => reflection_result(
            CheckStatus::Fail,
            severity,
            "CORS reflects arbitrary origins while allowing credentials",
            format!(
                "The server echoed the arbitrary probe Origin ({}) in Access-Control-Allow-Origin and also returned Access-Control-Allow-Credentials: true. This authorizes that origin to read the response when the browser includes eligible ambient credentials. Actual disclosure depends on the endpoint returning sensitive data and on cookie SameSite, third-party-cookie, HTTP-auth, and other browser credential rules.",
                PROBE_ORIGIN
            ),
            Some("Replace the reflect-the-Origin logic with an explicit allowlist comparison: only echo the Origin back when it exactly matches one of your own trusted origins, and never combine reflection with Access-Control-Allow-Credentials: true.".into()),
            Some(serde_json::json!({
                "probe_origin": PROBE_ORIGIN,
                "access_control_allow_origin": acao,
                "access_control_allow_credentials": acac,
            })),
            Some("If a sensitive endpoint shares this policy and the browser sends its ambient credentials cross-site, JavaScript on an arbitrary origin can read that endpoint's response.".into()),
        ),
        Some((status, severity)) => reflection_result(
            status,
            severity,
            "CORS reflects any origin",
            format!(
                "The server echoed the arbitrary probe Origin ({}) in Access-Control-Allow-Origin. Without Access-Control-Allow-Credentials, browser JavaScript at arbitrary origins can read responses available without ambient credentials. That can be intentional for public APIs; verify the response is public and that other routes do not add credential support to the same reflection policy.",
                PROBE_ORIGIN
            ),
            Some("If this content is public, prefer Access-Control-Allow-Origin: * over reflection. If any endpoint on this backend allows credentials, switch the reflection to an explicit origin allowlist.".into()),
            Some(serde_json::json!({
                "probe_origin": PROBE_ORIGIN,
                "access_control_allow_origin": acao,
                "access_control_allow_credentials": acac,
            })),
            Some("Arbitrary-origin read access is appropriate only for public responses. Route-specific middleware can change the effective policy, so review any authenticated or user-specific endpoints separately.".into()),
        ),
        None => reflection_result(
            CheckStatus::Pass,
            Severity::Low,
            "CORS origin reflection",
            "The server does not echo arbitrary origins back in Access-Control-Allow-Origin.".into(),
            None,
            None,
            None,
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::CorsCheck;
    use crate::checks::{Check, CheckStatus, PageContext, Severity};

    fn ctx_with_headers(headers: &[(&str, &str)]) -> PageContext {
        let mut map = http::header::HeaderMap::new();
        for (name, value) in headers {
            map.append(
                http::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("https://example.com").unwrap(),
            response_headers: map,
            status_code: 200,
            body: String::new(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn wildcard_with_credentials_fails_high() {
        let results = CorsCheck.run(&ctx_with_headers(&[
            ("access-control-allow-origin", "*"),
            ("access-control-allow-credentials", "true"),
        ]));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, Severity::Medium);
        assert!(results[0].description.contains("Browsers reject"));
        assert!(!results[0].description.contains("one edit away"));
        assert!(!results[0].description.contains("framework upgrade"));
        let why = results[0].why_it_matters.as_deref().unwrap_or_default();
        assert!(
            !why.contains("Any website can make authenticated requests"),
            "why_it_matters must not contradict the description: {}",
            why
        );
    }

    #[test]
    fn wildcard_without_credentials_is_a_low_warn() {
        let results = CorsCheck.run(&ctx_with_headers(&[("access-control-allow-origin", "*")]));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0]
            .description
            .contains("qualifying non-credentialed responses"));
        assert!(!results[0]
            .description
            .contains("any website can read responses"));
    }

    #[test]
    fn null_origin_is_a_warn_not_a_praised_pass() {
        let results = CorsCheck.run(&ctx_with_headers(&[(
            "access-control-allow-origin",
            "null",
        )]));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert!(
            results[0].description.contains("Origin: null"),
            "null-origin copy should explain who can send Origin: null: {}",
            results[0].description
        );
        assert!(!results[0].description.contains("any attacker"));
        assert!(!results[0].description.contains("recommended approach"));
    }

    #[test]
    fn specific_origin_passes() {
        let results = CorsCheck.run(&ctx_with_headers(&[(
            "access-control-allow-origin",
            "https://app.example.com",
        )]));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("https://app.example.com"));
        assert!(!results[0].description.contains("recommended approach"));
    }

    #[test]
    fn reflected_origin_with_credentials_is_the_exploitable_fail() {
        assert_eq!(
            super::reflection_verdict(Some(super::PROBE_ORIGIN), Some("true")),
            Some((CheckStatus::Fail, Severity::High))
        );
        // Case-insensitive credentials value still counts.
        assert_eq!(
            super::reflection_verdict(Some(super::PROBE_ORIGIN), Some("True")),
            Some((CheckStatus::Fail, Severity::High))
        );
    }

    #[test]
    fn reflected_origin_without_credentials_is_a_low_warn() {
        assert_eq!(
            super::reflection_verdict(Some(super::PROBE_ORIGIN), None),
            Some((CheckStatus::Warn, Severity::Low))
        );
    }

    #[test]
    fn allowlisted_wildcard_or_absent_acao_is_not_reflection() {
        // A fixed allowlisted origin, a wildcard, or no CORS at all: the
        // probe origin was NOT echoed, so reflection is not happening.
        assert_eq!(
            super::reflection_verdict(Some("https://app.example.com"), Some("true")),
            None
        );
        assert_eq!(super::reflection_verdict(Some("*"), None), None);
        assert_eq!(super::reflection_verdict(None, Some("true")), None);
    }

    #[test]
    fn absent_cors_headers_report_only_the_browser_cors_observation() {
        let results = CorsCheck.run(&ctx_with_headers(&[]));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0]
            .description
            .contains("No Access-Control-Allow-Origin header"));
        assert!(results[0].description.contains("does not assess"));
        assert!(!results[0].description.contains("secure"));
    }
}
