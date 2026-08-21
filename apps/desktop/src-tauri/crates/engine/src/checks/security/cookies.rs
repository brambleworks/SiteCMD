//! Checks readable Set-Cookie headers without persisting cookie values.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

/// Capability-family prefix for dynamically named per-cookie verdicts.
pub const CHECK_ID_PREFIX: &str = "security.cookies.";

/// Checks cookie security flags (Secure, HttpOnly, SameSite)
pub struct CookieSecurityCheck;

/// Attribute name before `=`, with browser-tolerated whitespace removed.
fn attr_key(attr: &str) -> &str {
    attr.split_once('=')
        .map(|(key, _)| key.trim_end())
        .unwrap_or(attr)
}

/// Validate a cookie name against the HTTP token grammar.
fn valid_cookie_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte > 0x20
                && byte < 0x7f
                && !matches!(
                    byte,
                    b'(' | b')'
                        | b'<'
                        | b'>'
                        | b'@'
                        | b','
                        | b';'
                        | b':'
                        | b'\\'
                        | b'"'
                        | b'/'
                        | b'['
                        | b']'
                        | b'?'
                        | b'='
                        | b'{'
                        | b'}'
                )
        })
}

impl Check for CookieSecurityCheck {
    fn id(&self) -> &str {
        "security.cookies"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let cookie_headers = ctx.response_headers.get_all("set-cookie");
        let total_header_count = cookie_headers.iter().count();
        let set_cookie_headers: Vec<&str> = cookie_headers
            .iter()
            .filter_map(|value| value.to_str().ok())
            .collect();
        let unreadable_header_count = total_header_count.saturating_sub(set_cookie_headers.len());

        if total_header_count == 0 {
            return vec![CheckResult {
                check_id: "security.cookies".into(),
                category: ScanCategory::Security,
                title: "Cookie security".into(),
                description: "No Set-Cookie header was observed on this response. This does not inspect cookies set by JavaScript, other routes, redirects, or later authenticated responses.".into(),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        let mut results = Vec::new();
        if unreadable_header_count > 0 {
            results.push(CheckResult {
                check_id: "security.cookies.unreadable_headers".into(),
                category: ScanCategory::Security,
                title: "Set-Cookie headers could not be inspected".into(),
                description: format!(
                    "{} of {} Set-Cookie {} could not be represented as visible text by the HTTP client, so their attributes were not graded. No cookie value is retained in evidence.",
                    unreadable_header_count,
                    total_header_count,
                    if unreadable_header_count == 1 { "header value" } else { "header values" }
                ),
                status: CheckStatus::Warn,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: Some("Inspect the raw response with an HTTP client that preserves header bytes, identify malformed or non-text Set-Cookie values, and make each header conform to the cookie grammar before re-running the scan. Do not paste live cookie values into tickets or logs.".into()),
                raw_data: Some(serde_json::json!({
                    "set_cookie_header_count": total_header_count,
                    "unreadable_header_count": unreadable_header_count,
                    "cookie_values_redacted": true,
                })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The client directly rejected these header values as text, but SiteCMD intentionally does not preserve their raw bytes or infer which attributes they contain.".into()),
                why_it_matters: Some("An uninspectable cookie header can be malformed or client-specific, but its security impact cannot be determined without safely reviewing the original response.".into()),
            });
        }

        for cookie_str in &set_cookie_headers {
            let Some((raw_name, _)) = cookie_str.split_once('=') else {
                results.push(CheckResult {
                    check_id: "security.cookies.malformed_header".into(),
                    category: ScanCategory::Security,
                    title: "Malformed Set-Cookie header".into(),
                    description: "A Set-Cookie header has no cookie-pair '=' delimiter, so SiteCMD could not reliably separate its name/value from attributes. The header value is redacted from evidence.".into(),
                    status: CheckStatus::Warn,
                    severity: Severity::Low,
                    fix_prompt: None,
                    manual_fix: Some("Inspect the response at its origin, correct the Set-Cookie syntax to a valid cookie-pair followed by semicolon-delimited attributes, and test the target browsers. Keep live cookie values out of logs and issue trackers.".into()),
                    raw_data: Some(serde_json::json!({"cookie_value_redacted": true})),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: Some("Malformed cookie headers can be ignored or parsed differently by clients; this check does not assume whether the intended cookie was stored.".into()),
                });
                continue;
            };
            let lower = cookie_str.to_lowercase();
            let name = raw_name.trim();
            if !valid_cookie_name(name) {
                results.push(CheckResult {
                    check_id: "security.cookies.malformed_header".into(),
                    category: ScanCategory::Security,
                    title: "Malformed Set-Cookie cookie name".into(),
                    description: "A Set-Cookie header has an empty or invalid cookie name, so SiteCMD did not grade its attributes. The complete header and cookie value are redacted from evidence.".into(),
                    status: CheckStatus::Warn,
                    severity: Severity::Low,
                    fix_prompt: None,
                    manual_fix: Some("Emit a non-empty cookie name that follows the HTTP token grammar, then inspect the deployed response in supported clients. Keep the live cookie value out of logs and issue trackers.".into()),
                    raw_data: Some(serde_json::json!({"cookie_value_redacted": true})),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: Some("A malformed cookie pair can be ignored rather than stored; this check does not assume whether the application depends on it.".into()),
                });
                continue;
            }

            // Parse attribute tokens rather than matching names or values that
            // merely contain `secure`; trim whitespace around attribute keys.
            let attributes: Vec<&str> =
                lower.split(';').skip(1).map(|piece| piece.trim()).collect();
            let has_secure = attributes.contains(&"secure");
            let has_httponly = attributes.contains(&"httponly");
            // When a value-bearing attribute repeats, RFC6265bis processing
            // uses the last successfully parsed occurrence. Looking from the
            // end avoids grading a stale first SameSite/Path declaration.
            let samesite_value = attributes.iter().rev().find_map(|attr| {
                attr.split_once('=').and_then(|(key, value)| {
                    key.trim()
                        .eq_ignore_ascii_case("samesite")
                        .then(|| value.trim().to_ascii_lowercase())
                })
            });
            let has_samesite = samesite_value
                .as_deref()
                .is_some_and(|value| matches!(value, "lax" | "strict" | "none"));
            let has_samesite_attribute = attributes
                .iter()
                .any(|attr| attr_key(attr).eq_ignore_ascii_case("samesite"));
            let secure_origin = ctx.url.scheme() == "https" || ctx.is_localhost;
            let has_partitioned = attributes.contains(&"partitioned");
            let removal_cookie = attributes
                .iter()
                .rev()
                .find_map(|attr| {
                    attr.split_once('=').and_then(|(key, value)| {
                        key.trim()
                            .eq_ignore_ascii_case("max-age")
                            .then(|| value.trim().parse::<i64>().ok())
                            .flatten()
                    })
                })
                .is_some_and(|seconds| seconds <= 0);

            // RFC6265bis user-agent processing applies these prefix matches
            // case-insensitively, even though server authoring guidance uses
            // the conventional __Host-/__Secure- spelling.
            let name_lower = name.to_ascii_lowercase();
            let mut prefix_violations: Vec<&str> = Vec::new();
            if name_lower.starts_with("__host-") {
                if !has_secure {
                    prefix_violations.push("the Secure flag is missing");
                }
                if attributes.iter().any(|attr| {
                    attr.split_once('=')
                        .map(|(key, value)| key.trim_end() == "domain" && !value.trim().is_empty())
                        .unwrap_or(false)
                }) {
                    prefix_violations.push("a Domain attribute is set (not allowed)");
                }
                let has_root_path = attributes
                    .iter()
                    .rev()
                    .find_map(|attr| {
                        attr.split_once('=').and_then(|(key, value)| {
                            (key.trim_end() == "path").then(|| value.trim())
                        })
                    })
                    .is_some_and(|value| value == "/");
                if !has_root_path {
                    prefix_violations.push("Path=/ is missing");
                }
            } else if name_lower.starts_with("__secure-") && !has_secure {
                prefix_violations.push("the Secure flag is missing");
            }
            if (name_lower.starts_with("__host-") || name_lower.starts_with("__secure-"))
                && !secure_origin
            {
                prefix_violations.push("the response came from a non-secure origin");
            }

            if !prefix_violations.is_empty() {
                let prefix = if name_lower.starts_with("__host-") {
                    "__Host-"
                } else {
                    "__Secure-"
                };
                results.push(CheckResult {
                    check_id: format!("{CHECK_ID_PREFIX}{name}"),
                    category: ScanCategory::Security,
                    title: format!("Cookie '{}' violates its {} prefix contract", name, prefix),
                    description: format!(
                        "Cookie '{}' uses the {} name prefix, but {}. User agents implementing RFC6265bis prefix processing reject a cookie that violates this contract. SiteCMD cannot tell whether this cookie is authentication-critical or which client versions and policies are in scope.",
                        name,
                        prefix,
                        prefix_violations.join(", "),
                    ),
                    status: CheckStatus::Fail,
                    severity: Severity::Medium,
                    fix_prompt: None,
                    manual_fix: Some(format!(
                        "Fix the Set-Cookie attributes so the {} contract holds: {} requires the Secure flag{}. Alternatively, drop the prefix from the cookie name if you cannot meet its rules.",
                        prefix,
                        prefix,
                        if prefix == "__Host-" {
                            ", Path=/, and no Domain attribute"
                        } else {
                            ""
                        },
                    )),
                    raw_data: Some(serde_json::json!({
                        "cookie_name": name,
                        "cookie_value_redacted": true,
                        "attribute_names": attributes.iter().map(|attr| attr_key(attr)).collect::<Vec<_>>(),
                        "prefix": prefix,
                        "violations": prefix_violations,
                    })),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: Some(
                        "Conforming clients reject prefix-violating cookies. User-visible impact depends on the cookie's purpose and the clients that must be supported.".into(),
                    ),
                });
                continue;
            }

            if has_secure && !secure_origin {
                results.push(CheckResult {
                    check_id: format!("{CHECK_ID_PREFIX}{name}"),
                    category: ScanCategory::Security,
                    title: format!("Secure cookie '{}' came from an HTTP origin", name),
                    description: format!("Cookie '{}' carries the Secure attribute but was observed on a non-HTTPS, non-localhost response. Conforming user agents do not accept a Secure cookie from a non-secure origin; this scan does not establish whether the same cookie is also set correctly over HTTPS.", name),
                    status: CheckStatus::Fail,
                    severity: Severity::Medium,
                    fix_prompt: None,
                    manual_fix: Some("Set production cookies only from the canonical HTTPS response, retain the Secure attribute, and redirect or disable the HTTP origin. Re-test the actual authentication/session flow over HTTPS; do not remove Secure merely to make the cookie work over HTTP.".into()),
                    raw_data: Some(serde_json::json!({
                        "cookie_name": name,
                        "cookie_value_redacted": true,
                        "response_scheme": ctx.url.scheme(),
                    })),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: Some("If the cookie is required, conforming clients can ignore this Set-Cookie response. The feature impact depends on whether a correct HTTPS response sets it elsewhere.".into()),
                });
                continue;
            }

            if has_partitioned && !has_secure {
                results.push(CheckResult {
                    check_id: format!("{CHECK_ID_PREFIX}{name}"),
                    category: ScanCategory::Security,
                    title: format!("Partitioned cookie '{}' lacks Secure", name),
                    description: format!("Cookie '{}' requests partitioned storage without the Secure attribute. User agents supporting the Partitioned cookie contract require Secure and can ignore this cookie; legacy clients may ignore the Partitioned attribute itself.", name),
                    status: CheckStatus::Fail,
                    severity: Severity::Medium,
                    fix_prompt: None,
                    manual_fix: Some(format!("Add Secure to the Set-Cookie header for '{}' and serve it only over HTTPS, or remove Partitioned if the cookie is not intended for partitioned cross-site storage. Test the actual embedded/top-level contexts and supported browsers.", name)),
                    raw_data: Some(serde_json::json!({
                        "cookie_name": name,
                        "cookie_value_redacted": true,
                        "violations": ["Partitioned requires Secure"],
                    })),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: Some("Supporting clients can reject a Partitioned cookie without Secure, but functional impact depends on whether this cookie is required in partitioned contexts.".into()),
                });
                continue;
            }

            // RFC6265bis rejects SameSite=None without Secure.
            let samesite_none = samesite_value.as_deref() == Some("none");
            if samesite_none && !has_secure {
                results.push(CheckResult {
                    check_id: format!("{CHECK_ID_PREFIX}{name}"),
                    category: ScanCategory::Security,
                    title: format!(
                        "Cookie '{}' sets SameSite=None without Secure",
                        name
                    ),
                    description: format!(
                        "Cookie '{}' sets SameSite=None without the Secure flag. Conforming RFC6265bis user agents ignore this cookie; legacy clients and product-specific cookie policies can differ. SiteCMD cannot infer whether the cookie supports an embed, federated sign-in flow, widget, or another cross-site use case.",
                        name,
                    ),
                    status: CheckStatus::Fail,
                    severity: Severity::Medium,
                    fix_prompt: None,
                    manual_fix: Some(format!(
                        "Add the Secure flag to the Set-Cookie header for '{}' (SameSite=None requires it), or drop SameSite=None if the cookie never needs to be sent cross-site.",
                        name,
                    )),
                    raw_data: Some(serde_json::json!({
                        "cookie_name": name,
                        "cookie_value_redacted": true,
                        "attribute_names": attributes.iter().map(|attr| attr_key(attr)).collect::<Vec<_>>(),
                        "violations": ["SameSite=None requires the Secure flag"],
                    })),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: Some(
                        "Conforming clients reject SameSite=None cookies without Secure. Functional impact depends on whether this cookie is required and which clients the product supports.".into(),
                    ),
                });
                continue;
            }

            if removal_cookie {
                results.push(CheckResult {
                    check_id: format!("{CHECK_ID_PREFIX}{name}"),
                    category: ScanCategory::Security,
                    title: format!("Cookie '{}' removal response", name),
                    description: format!("Cookie '{}' has a valid non-positive Max-Age, so this response requests immediate removal rather than persistence. SiteCMD still checked prefix, Secure-origin, Partitioned, and SameSite=None contracts above, but does not treat missing HttpOnly or an omitted SameSite attribute on this deletion header as a new persistent-cookie weakness.", name),
                    status: CheckStatus::Pass,
                    severity: Severity::Low,
                    fix_prompt: None,
                    manual_fix: None,
                    raw_data: Some(serde_json::json!({
                        "cookie_name": name,
                        "cookie_value_redacted": true,
                        "removal_cookie": true,
                    })),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: None,
                });
                continue;
            }

            if has_samesite_attribute && !has_samesite {
                results.push(CheckResult {
                    check_id: format!("{CHECK_ID_PREFIX}{name}"),
                    category: ScanCategory::Security,
                    title: format!("Cookie '{}' has an invalid SameSite value", name),
                    description: format!("Cookie '{}' includes a SameSite attribute, but its value is not Lax, Strict, or None. User-agent fallback behavior can vary, so the intended cross-site policy is not reliably expressed.", name),
                    status: CheckStatus::Warn,
                    severity: Severity::Low,
                    fix_prompt: None,
                    manual_fix: Some(format!("Choose the policy for '{}' from the flow requirements: Strict for same-site-only use, Lax for common first-party navigation flows, or None together with Secure for intentional cross-site use. Test the actual sign-in, embed, payment, and callback paths before changing it.", name)),
                    raw_data: Some(serde_json::json!({
                        "cookie_name": name,
                        "cookie_value_redacted": true,
                        "samesite_value": samesite_value,
                    })),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: Some("An unrecognized SameSite value does not communicate a portable cookie-sending policy; actual fallback and impact depend on the client and flow.".into()),
                });
                continue;
            }

            let mut issues = Vec::new();
            let missing_secure = !has_secure && !ctx.is_localhost;
            if missing_secure {
                issues.push("the Secure flag");
            }
            if !has_httponly {
                issues.push("the HttpOnly flag");
            }
            if !has_samesite {
                issues.push("an explicit valid SameSite attribute");
            }

            if issues.is_empty() {
                results.push(CheckResult {
                    check_id: format!("{CHECK_ID_PREFIX}{name}"),
                    category: ScanCategory::Security,
                    title: format!("Cookie '{}' attributes", name),
                    description: format!("Cookie '{}' includes Secure, HttpOnly, and a recognized SameSite value. This attribute check does not establish whether the chosen SameSite policy fits the flow, whether JavaScript access is intentionally unnecessary, or whether Domain, Path, lifetime, and server-side session controls are correct.", name),
                    status: CheckStatus::Pass,
                    severity: Severity::Low,
                    fix_prompt: None,
                    manual_fix: None,
                    raw_data: Some(serde_json::json!({
                        "cookie_name": name,
                        "cookie_value_redacted": true,
                        "has_secure": has_secure,
                        "has_httponly": has_httponly,
                        "samesite_value": samesite_value,
                    })),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: None,
                });
            } else {
                results.push(CheckResult {
                    check_id: format!("{CHECK_ID_PREFIX}{name}"),
                    category: ScanCategory::Security,
                    title: format!("Cookie '{}' attributes need review", name),
                    description: format!("Cookie '{}' is missing {}. Secure is normally required for sensitive cookies on public HTTPS sites; HttpOnly is appropriate only when client-side JavaScript does not need the cookie; SameSite must match the actual first-party or cross-site flow. Cookie purpose is not observable from this response.", name, issues.join(", ")),
                    status: CheckStatus::Warn,
                    severity: if missing_secure { Severity::Medium } else { Severity::Low },
                    fix_prompt: None,
                    manual_fix: Some(format!("Classify what '{}' stores and which flows send/read it. Add Secure for a public HTTPS cookie unless a documented localhost-only exception applies. Add HttpOnly when JavaScript must not read it (but keep intentional script-readable tokens/preferences documented). Choose SameSite=Strict, Lax, or None from tested navigation/embed/auth requirements; None also requires Secure. Re-test login, logout, callbacks, embeds, and supported browsers.", name)),
                    raw_data: Some(serde_json::json!({
                        "cookie_name": name,
                        "cookie_value_redacted": true,
                        "has_secure": has_secure,
                        "has_httponly": has_httponly,
                        "has_samesite": has_samesite,
                    })),
                    confidence: crate::checks::IssueConfidence::NeedsReview,
                    confidence_reason: Some("The missing attributes are directly observed, but the cookie's sensitivity, JavaScript access requirements, cross-site flows, client defaults, and server-side session controls are not visible in this response.".into()),
                    why_it_matters: Some(
                        "For a sensitive session cookie, missing Secure can expose it to cleartext requests and missing HttpOnly can increase theft impact after script injection. SameSite influences cross-site sending. Those consequences depend on the cookie's purpose and surrounding controls.".into(),
                    ),
                });
            }
        }

        results
    }

    fn skip_in_predeploy(&self) -> bool {
        false // We still check HttpOnly and SameSite on localhost
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, PageContext};
    use http::header::{HeaderMap, HeaderValue};

    fn ctx_with_headers(body: &str, headers: HeaderMap) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("https://example.com").unwrap(),
            response_headers: headers,
            status_code: 200,
            body: body.to_string(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn test_cookies_no_cookies_pass() {
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", HeaderMap::new());
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].severity, Severity::Low);
        assert!(results[0].description.contains("other routes"));
    }

    #[test]
    fn test_cookies_secure_cookie_pass() {
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("session=abc123; Secure; HttpOnly; SameSite=Strict"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn test_cookies_insecure_cookie_warns_for_context_review() {
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("session=abc123; Path=/"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].description.contains("missing"));
    }

    #[test]
    fn test_cookies_partial_flags_warn() {
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("token=xyz; Secure; Path=/"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        // Should mention HttpOnly and SameSite are missing
        assert!(results[0].description.contains("HttpOnly"));
        assert!(results[0].description.contains("SameSite"));
    }

    #[test]
    fn cookie_name_or_value_containing_secure_does_not_count_as_secure_flag() {
        for cookie in [
            // Name contains "secure"
            "secure_session=abc123; Path=/",
            // Value contains "secure"
            "token=mysecuretoken; Path=/",
            // Value contains "insecure"
            "token=insecure123; Path=/",
        ] {
            let mut h = HeaderMap::new();
            h.insert("set-cookie", HeaderValue::from_str(cookie).unwrap());
            let check = CookieSecurityCheck;
            let ctx = ctx_with_headers("", h);
            let results = check.run(&ctx);
            assert_eq!(results.len(), 1, "cookie: {cookie}");
            assert_eq!(
                results[0].status,
                CheckStatus::Warn,
                "cookie {cookie:?} lacks the Secure attribute and must warn; substring matching previously passed it"
            );
            assert!(
                results[0].description.contains("Secure"),
                "cookie {cookie:?} should report missing Secure flag, got: {}",
                results[0].description
            );
        }
    }

    #[test]
    fn host_prefix_violations_fail_medium_without_assuming_cookie_purpose() {
        // __Host- requires Secure, Path=/, and no Domain. Conforming clients
        // reject violations, but the scanner cannot infer business impact.
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static(
                "__Host-session=abc; HttpOnly; SameSite=Lax; Path=/app; Domain=example.com",
            ),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, crate::checks::Severity::Medium);
        assert!(results[0].description.contains("Secure"));
        assert!(results[0].description.contains("Domain"));
        assert!(results[0].description.contains("Path=/"));
    }

    #[test]
    fn valid_host_prefix_cookie_passes() {
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("__Host-session=abc; Secure; HttpOnly; SameSite=Lax; Path=/"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn secure_prefix_without_secure_flag_fails() {
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("__Secure-token=abc; HttpOnly; SameSite=Lax; Path=/"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert!(results[0].title.contains("__Secure-"));
    }

    #[test]
    fn samesite_none_without_secure_is_a_rejected_cookie() {
        // Attribute processing is specified by RFC6265bis; do not pin the
        // product copy to browser-version claims that will age out.
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("widget_session=abc; HttpOnly; SameSite=None; Path=/"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, crate::checks::Severity::Medium);
        assert!(results[0].title.contains("SameSite=None"));
        assert!(
            results[0].description.contains("Conforming RFC6265bis"),
            "rejection must be attributed to the processing contract: {}",
            results[0].description
        );
        assert!(
            results[0].description.contains("legacy clients"),
            "copy must hedge for differing clients and policies: {}",
            results[0].description
        );
    }

    #[test]
    fn samesite_with_whitespace_around_equals_counts_as_present() {
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("session=abc123; Secure; HttpOnly; SameSite = Lax"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "spaced SameSite must parse as present: {}",
            results[0].description
        );
    }

    #[test]
    fn host_prefix_tolerates_whitespace_around_path_equals() {
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static(
                "__Host-session=abc; Secure; HttpOnly; SameSite=Lax; Path = /",
            ),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "spaced Path = / must satisfy the __Host- contract: {}",
            results[0].description
        );
    }

    #[test]
    fn missing_flag_fix_requires_flow_classification() {
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("session=abc123; Path=/"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        let fix = results[0].manual_fix.as_deref().unwrap_or_default();
        assert!(
            fix.contains("Classify what") && fix.contains("SameSite=Strict, Lax, or None"),
            "fix should make the attribute choice contextual: {}",
            fix
        );
    }

    #[test]
    fn samesite_none_with_secure_is_valid() {
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("widget_session=abc; Secure; HttpOnly; SameSite=None"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn cookie_with_httponly_in_value_does_not_count_as_httponly_flag() {
        // Same class of regression as the secure-flag case.
        let mut h = HeaderMap::new();
        h.insert(
            "set-cookie",
            HeaderValue::from_static("token=httponlyplaceholder; Path=/"),
        );
        let check = CookieSecurityCheck;
        let ctx = ctx_with_headers("", h);
        let results = check.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(
            results[0].description.contains("HttpOnly"),
            "should report missing HttpOnly even when value contains the word; got: {}",
            results[0].description
        );
    }
}

#[cfg(test)]
mod additional_tests;
