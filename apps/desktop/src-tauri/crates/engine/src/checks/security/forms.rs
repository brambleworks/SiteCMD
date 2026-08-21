//! Detects form actions that submit over insecure transport.

use crate::checks::html_attrs::{attr_value, tag_slices};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

fn form_tags(body: &str) -> Vec<&str> {
    let lower = body.to_ascii_lowercase();
    tag_slices(body, &lower, "form")
}

fn form_actions(body: &str) -> Vec<String> {
    form_tags(body)
        .into_iter()
        .filter_map(|tag| attr_value(tag, "action"))
        .filter(|action| !action.trim().is_empty())
        .collect()
}

/// Check for forms that submit to HTTP endpoints.
pub struct InsecureFormCheck;

impl Check for InsecureFormCheck {
    fn id(&self) -> &str {
        "security.insecure_form"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        if ctx.is_localhost && !ctx.url.scheme().eq_ignore_ascii_case("https") {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Form submission transport".into(),
                description: "Skipped on localhost preview. Local preview servers commonly use plain HTTP, so verify form transport security on a deployed HTTPS environment.".into(),
                status: CheckStatus::Skipped,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({"reason": "localhost_preview_server"})),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        if !ctx.url.scheme().eq_ignore_ascii_case("https") {
            // No forms on the page: the missing-HTTPS transport problem is
            // https_enforcement's finding. Emitting a second Critical here
            // double-counted one root cause.
            let forms = form_tags(&ctx.body);
            if forms.is_empty() {
                return vec![CheckResult {
                    check_id: self.id().into(),
                    category: self.category(),
                    title: "Form submission transport".into(),
                    description: "Skipped: the site is not served over HTTPS, but this page contains no forms. The transport problem itself is reported by the HTTPS enforcement check.".into(),
                    status: CheckStatus::Skipped,
                    severity: Severity::Low,
                    fix_prompt: None,
                    manual_fix: None,
                    raw_data: Some(serde_json::json!({"reason": "no_forms_on_non_https_page"})),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: None,
                }];
            }
            let form_count = forms.len();
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Forms are delivered over HTTP".into(),
                description: format!(
                    "This page contains {} form{} and its form markup is delivered over HTTP. A network attacker can alter the destination or fields before submission; an HTTPS action alone does not protect the integrity of a page delivered over HTTP.",
                    form_count,
                    if form_count == 1 { "" } else { "s" }
                ),
                status: CheckStatus::Fail,
                severity: Severity::High,
                fix_prompt: None,
                manual_fix: Some("Serve the page and all of its navigations over HTTPS, redirect HTTP requests to HTTPS, and then verify each form's resolved submission destination in the deployed page.".into()),
                raw_data: Some(serde_json::json!({ "form_count": form_count })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: Some(
                    "Without authenticated page delivery, a person on the network path can modify what the form displays and where entered data is sent. The scan does not establish that the forms collect sensitive data.".into(),
                ),
            }];
        }

        let mut insecure_targets: Vec<String> = Vec::new();

        for action in form_actions(&ctx.body) {
            let Ok(target) = ctx.url.join(action.trim()) else {
                continue;
            };
            if target.scheme().eq_ignore_ascii_case("http") {
                insecure_targets.push(crate::log_sanitizer::evidence_safe_page_url(
                    target.as_str(),
                ));
            }
        }

        insecure_targets.sort_unstable();
        insecure_targets.dedup();

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if insecure_targets.is_empty() {
                "Form submission transport".into()
            } else {
                "Forms submit to insecure HTTP endpoints".into()
            },
            description: if insecure_targets.is_empty() {
                format!(
                    "No observed form action resolved to plain HTTP. This source check reviewed {} form element{}; it does not validate runtime action changes or server-side handling.",
                    form_tags(&ctx.body).len(),
                    if form_tags(&ctx.body).len() == 1 { "" } else { "s" }
                )
            } else {
                format!(
                    "{} {} to insecure HTTP endpoints: {}",
                    insecure_targets.len(),
                    if insecure_targets.len() == 1 {
                        "form submits"
                    } else {
                        "forms submit"
                    },
                    insecure_targets
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
            status: if insecure_targets.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: if insecure_targets.is_empty() {
                None
            } else {
                Some("Change each surfaced action to a working HTTPS endpoint, then submit non-sensitive test data from the deployed page and confirm the final request stays on HTTPS. Also check client scripts do not replace the action at runtime.".into())
            },
            raw_data: if insecure_targets.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "insecure_targets": insecure_targets }))
            },
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if insecure_targets.is_empty() {
                None
            } else {
                Some("Values sent to an HTTP endpoint can be observed or modified on the network path. The source check does not establish which fields are populated or whether they contain sensitive data.".into())
            },
        }]
    }
}

/// Check for forms posting to external/third-party domains (potential phishing)
pub struct FormActionHijackCheck;

/// Match an exact domain or dot-delimited subdomain, never a suffix lookalike.
fn host_matches_domain(host: &str, domain: &str) -> bool {
    let domain = domain.trim_start_matches('.');
    host == domain || host.ends_with(&format!(".{}", domain))
}

/// Whether hosts are equal or one is a boundary-matched subdomain of the other.
fn same_site(a: &str, b: &str) -> bool {
    let a = a.trim_end_matches('.').to_ascii_lowercase();
    let b = b.trim_end_matches('.').to_ascii_lowercase();
    if a == b {
        return true;
    }
    match (psl::domain_str(&a), psl::domain_str(&b)) {
        (Some(a_domain), Some(b_domain)) => a_domain.eq_ignore_ascii_case(b_domain),
        _ => a.ends_with(&format!(".{b}")) || b.ends_with(&format!(".{a}")),
    }
}

impl Check for FormActionHijackCheck {
    fn id(&self) -> &str {
        "security.form_action_hijack"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let host = ctx.url.host_str().unwrap_or("");
        let mut external_targets: Vec<String> = Vec::new();

        // Known payment and identity providers, matched on dot-anchored domain
        // boundaries so lookalike hosts cannot pass.
        let safe_domains = [
            // Payments
            "paypal.com",
            "stripe.com",
            "checkout.stripe.com",
            // Google (sign-in + recaptcha + identity)
            "google.com",
            "accounts.google.com",
            "myaccount.google.com",
            "recaptcha.net",
            // Microsoft / Apple identity
            "login.microsoftonline.com",
            "appleid.apple.com",
            // Auth providers use tenant subdomains, so match their canonical suffixes.
            ".auth0.com",
            ".okta.com",
            ".oktapreview.com",
            ".amazoncognito.com",
            ".clerk.accounts.dev",
            "clerk.dev",
            ".stytch.com",
            ".workos.com",
            ".frontegg.com",
            ".fusionauth.io",
            // GitHub / GitLab OAuth
            "github.com",
            "gitlab.com",
        ];

        for action in form_actions(&ctx.body) {
            if let Ok(parsed) = ctx.url.join(action.trim()) {
                if !matches!(parsed.scheme(), "http" | "https") {
                    continue;
                }
                let action_host = parsed.host_str().unwrap_or("");
                // Check if it's a different domain
                if !action_host.is_empty() && !same_site(action_host, host) {
                    // Not a known safe payment/auth provider
                    let is_safe = safe_domains
                        .iter()
                        .any(|d| host_matches_domain(action_host, d));
                    if !is_safe {
                        external_targets.push(crate::log_sanitizer::evidence_safe_page_url(
                            parsed.as_str(),
                        ));
                    }
                }
            }
        }

        external_targets.sort_unstable();
        external_targets.dedup();

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if external_targets.is_empty() {
                "Form Action Domains".into()
            } else {
                "Forms submit to unexpected external domains".into()
            },
            description: if external_targets.is_empty() {
                "No HTTP(S) form action outside the scanned registrable site or the check's limited list of common payment and identity-provider domains was observed. This does not validate provider account ownership, runtime action changes, or server-side form handling.".into()
            } else {
                format!(
                    "{} {} to unexpected external domains: {}",
                    external_targets.len(),
                    if external_targets.len() == 1 {
                        "form submits"
                    } else {
                        "forms submit"
                    },
                    external_targets
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
            status: if external_targets.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: if external_targets.is_empty() {
                None
            } else {
                Some("Trace each external action to the owning form and intended service. Confirm the exact destination in provider configuration and repository history, determine which fields are submitted, and remove or correct stale and unintended targets. Do not send real credentials or personal data while testing an unknown destination.".into())
            },
            raw_data: if external_targets.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "external_targets": external_targets }))
            },
            confidence: if external_targets.is_empty() {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if external_targets.is_empty() {
                None
            } else {
                Some(
                    "External form targets are often legitimate services (newsletter, payment, or auth providers not on the known-safe list); review whether these destinations are expected."
                        .into(),
                )
            },
            why_it_matters: if external_targets.is_empty() {
                None
            } else {
                Some("A cross-site form action sends the submitted field values to that destination. If the target is unintended or untrusted, it can receive credentials, personal data, or other form content; the domain-list mismatch alone does not establish compromise.".into())
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, PageContext};
    use http::header::HeaderMap;

    fn ctx(body: &str) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("https://example.com").unwrap(),
            response_headers: HeaderMap::new(),
            status_code: 200,
            body: body.to_string(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn test_insecure_form_https_action_pass() {
        let html =
            r#"<form action="https://example.com/submit" method="post"><input type="text"></form>"#;
        let check = InsecureFormCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn test_insecure_form_http_action_fail() {
        let html =
            r#"<form action="http://example.com/submit" method="post"><input type="text"></form>"#;
        let check = InsecureFormCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Fail);
    }

    #[test]
    fn insecure_form_unquoted_http_action_fails() {
        let html = r#"<form action=http://example.com/submit method=post></form>"#;
        let result = &InsecureFormCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Fail);
    }

    #[test]
    fn data_action_is_not_mistaken_for_a_form_action() {
        let html = r#"<form data-action="http://example.com/preview" action="/submit"></form>"#;
        let result = &InsecureFormCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn test_insecure_form_relative_action_pass() {
        let html = r#"<form action="/submit" method="post"><input type="text"></form>"#;
        let check = InsecureFormCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn test_insecure_form_localhost_http_skips_preview_server() {
        let mut local_ctx =
            ctx(r#"<form action="/submit" method="post"><input type="text"></form>"#);
        local_ctx.url = url::Url::parse("http://127.0.0.1:4324").unwrap();
        local_ctx.is_localhost = true;
        local_ctx.is_strict_localhost = true;

        let check = InsecureFormCheck;
        let results = check.run(&local_ctx);
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[test]
    fn test_form_hijack_same_domain_pass() {
        let html = r#"<form action="https://example.com/api" method="post"></form>"#;
        let check = FormActionHijackCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn test_form_hijack_external_unknown_domain_warn() {
        let html = r#"<form action="https://evil.phish.net/steal" method="post"></form>"#;
        let check = FormActionHijackCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn test_form_hijack_safe_domain_pass() {
        let html = r#"<form action="https://checkout.stripe.com/pay" method="post"></form>"#;
        let check = FormActionHijackCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn insecure_form_non_https_without_forms_is_skipped() {
        let mut http_ctx = ctx("<html><body><p>No forms here</p></body></html>");
        http_ctx.url = url::Url::parse("http://example.com").unwrap();
        let results = InsecureFormCheck.run(&http_ctx);
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("HTTPS enforcement"));
    }

    #[test]
    fn insecure_form_non_https_with_forms_reports_page_integrity_risk() {
        let mut http_ctx =
            ctx(r#"<form action="/login" method="post"><input type="password"></form>"#);
        http_ctx.url = url::Url::parse("http://example.com").unwrap();
        let results = InsecureFormCheck.run(&http_ctx);
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, Severity::High);
        assert!(results[0].description.contains("form markup"));
        assert!(!results[0]
            .description
            .contains("every submission travels unencrypted"));
    }

    #[test]
    fn clean_form_result_uses_a_neutral_title_and_scoped_claim() {
        let result = &InsecureFormCheck.run(&ctx("<form action=/submit></form>"))[0];
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(result.title, "Form submission transport");
        assert!(result.description.contains("observed form"));
    }

    #[test]
    fn fakepaypal_lookalike_domain_is_not_whitelisted() {
        let html = r#"<form action="https://fakepaypal.com/checkout" method="post"></form>"#;
        let results = FormActionHijackCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn evilexample_suffix_host_is_not_same_site() {
        // Same bug on the same-host compare: evilexample.com ends with
        // example.com but is a different registrable domain.
        let html = r#"<form action="https://evilexample.com/collect" method="post"></form>"#;
        let results = FormActionHijackCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn sibling_subdomains_are_same_registrable_site() {
        let mut page = ctx(r#"<form action="https://api.example.com/submit"></form>"#);
        page.url = url::Url::parse("https://www.example.com").unwrap();
        let results = FormActionHijackCheck.run(&page);
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn unquoted_external_action_is_reviewed() {
        let html = r#"<form action=https://collector.example.net/submit method=post></form>"#;
        let result = &FormActionHijackCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Warn);
    }

    #[test]
    fn form_target_evidence_keeps_paths_without_persisting_query_secrets() {
        let insecure = InsecureFormCheck.run(&ctx(
            r#"<form action="http://forms.example.com/account/reset/short-token?token=secret"></form>"#,
        ));
        let insecure_json = serde_json::to_string(&insecure[0]).unwrap();
        assert!(
            insecure_json.contains("http://forms.example.com/account/reset/[redacted]"),
            "{insecure_json}"
        );
        assert!(!insecure_json.contains("short-token"), "{insecure_json}");
        assert!(!insecure_json.contains("token=secret"), "{insecure_json}");

        let external = FormActionHijackCheck.run(&ctx(
            r#"<form action="https://collector.example.net/forms/contact?api_key=secret"></form>"#,
        ));
        let external_json = serde_json::to_string(&external[0]).unwrap();
        assert!(
            external_json.contains("https://collector.example.net/forms/contact"),
            "{external_json}"
        );
        assert!(!external_json.contains("api_key"), "{external_json}");
        assert!(!external_json.contains("secret"), "{external_json}");
    }

    #[test]
    fn real_subdomain_of_page_host_is_same_site() {
        let html = r#"<form action="https://api.example.com/submit" method="post"></form>"#;
        let results = FormActionHijackCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn cognito_hosted_ui_domain_is_trusted() {
        let html = r#"<form action="https://myapp.auth.us-east-1.amazoncognito.com/login" method="post"></form>"#;
        let results = FormActionHijackCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn hijack_warn_is_needs_review_with_reason() {
        // The check's own fix text says legitimate external targets are
        // fine, so High confidence was dishonest.
        let html = r#"<form action="https://evil.phish.net/steal" method="post"></form>"#;
        let results = FormActionHijackCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].confidence_reason.is_some());
    }

    #[test]
    fn dot_anchored_domain_matching() {
        assert!(host_matches_domain("paypal.com", "paypal.com"));
        assert!(host_matches_domain("www.paypal.com", "paypal.com"));
        assert!(!host_matches_domain("fakepaypal.com", "paypal.com"));
        assert!(host_matches_domain("tenant.auth0.com", ".auth0.com"));
        assert!(!host_matches_domain("notauth0.com", ".auth0.com"));
        assert!(same_site("example.com", "example.com"));
        assert!(same_site("api.example.com", "example.com"));
        assert!(!same_site("evilexample.com", "example.com"));
    }
}
