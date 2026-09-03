//! Static review for visible data-controller or privacy contact details.
//! Applicability and external policy content remain out of scope.

use crate::checks::compliance::trackers::{
    consent_relevant_trackers, cookieless_trackers, name_list,
};
use crate::checks::compliance::{content_text_lower, executable_text_lower};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::sync::LazyLock;

static EMAIL_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap());

/// A link whose href points at a privacy policy (any quoting style).
static PRIVACY_LINK_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r#"href\s*=\s*["']?[^"'>\s]*(?:privacy|datenschutz|privacidad|confidentialite)[^"'>\s]*"#,
    )
    .unwrap()
});

/// `gpc` as a standalone word, so minified identifiers that merely contain
/// the letters (e.g. `wgpca`) don't count as a GPC disclosure.
static GPC_WORD_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\bgpc\b").unwrap());

/// Check for GDPR data controller contact info
pub struct DataControllerContactCheck;

impl Check for DataControllerContactCheck {
    fn id(&self) -> &str {
        "compliance.data_controller_contact"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Contact details a visitor cannot read are not disclosure, so the
        // markers are matched against delivered content only.
        let content = content_text_lower(ctx.body_lower());

        let has_controller = content.contains("data controller")
            || content.contains("data protection officer")
            || content.contains("dpo@")
            || content.contains("privacy@")
            || content.contains("gdpr@")
            || content.contains("data-controller")
            || content.contains("data protection");

        // Check for contact email near privacy terms
        let has_privacy_email = if let Some(pos) = content.find("privacy") {
            let window_start = pos.saturating_sub(500);
            let window_end = (pos + 500).min(content.len());
            let window_start = crate::checks::floor_char_boundary(&content, window_start);
            let window_end = crate::checks::ceil_char_boundary(&content, window_end);
            let window = &content[window_start..window_end];
            EMAIL_RE.is_match(window)
        } else {
            false
        };

        // A privacy-policy link is sufficient because controller details may
        // live on that dedicated page.
        let links_to_privacy_policy =
            PRIVACY_LINK_RE.is_match(&content) || super::has_privacy_policy_link(&content);

        let (status, title, description) = if has_controller || has_privacy_email {
            (
                CheckStatus::Pass,
                "Data Controller Contact",
                "Data controller contact information found.".to_string(),
            )
        } else if links_to_privacy_policy {
            (
                CheckStatus::Pass,
                "Data Controller Contact",
                "No controller contact details on this page, but it links to a privacy policy - the page where regulators expect those details to live. Scan the privacy page (or run a multi-page scan) to verify it names the controller and a contact.".to_string(),
            )
        } else {
            (
                CheckStatus::Warn,
                "No data controller contact information found",
                "No data-controller contact information or privacy-policy link was detected on this page. If the site is subject to rules that require a collection notice, verify that the controller identity and required contact details are discoverable elsewhere.".to_string(),
            )
        };
        let pass = status == CheckStatus::Pass;

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: title.into(),
            description,
            status,
            severity: Severity::Low,
            fix_prompt: if pass {
                None
            } else {
                Some("First confirm which privacy laws and notice duties apply. If controller identification is required, name the responsible organization and provide the required contact channel in the privacy notice, then link that notice from relevant collection points and site navigation.".into())
            },
            manual_fix: if pass {
                None
            } else {
                Some("Confirm applicability, then add the controller identity and legally required contact details to the privacy notice. Include DPO or representative details only when that role applies, and make the notice discoverable from relevant pages.".into())
            },
            raw_data: Some(serde_json::json!({
                "controller_marker_detected": has_controller,
                "privacy_email_detected": has_privacy_email,
                "privacy_policy_link_detected": links_to_privacy_policy,
                "applicability_verified": false,
            })),
            confidence: if has_controller || has_privacy_email {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if has_controller || has_privacy_email {
                None
            } else {
                Some("This page-level static check cannot determine whether the site is in scope, whether a notice exists at an unrecognized URL, or whether the linked policy contains the required controller details.".into())
            },
            why_it_matters: if pass {
                None
            } else {
                Some("Where a privacy notice is required, people need to know which organization is responsible and how to exercise their rights. Applicability and exact contact fields depend on the governing law and organization.".into())
            },
        }]
    }
}

/// Check cookie expiration - flags cookies with excessively long lifetimes
pub struct CookieExpirationCheck;

impl Check for CookieExpirationCheck {
    fn id(&self) -> &str {
        "compliance.cookie_expiration"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut long_lived: Vec<String> = Vec::new();
        let one_year_secs: i64 = 365 * 24 * 3600;

        for val in ctx.response_headers.get_all("set-cookie") {
            if let Ok(cookie_str) = val.to_str() {
                let parts: Vec<&str> = cookie_str.split(';').collect();
                let name = parts[0].split('=').next().unwrap_or("unknown").trim();

                for part in &parts[1..] {
                    let trimmed = part.trim().to_lowercase();
                    if let Some(max_age) = trimmed.strip_prefix("max-age=") {
                        if let Ok(secs) = max_age.parse::<i64>() {
                            if secs > one_year_secs {
                                let years = secs / one_year_secs;
                                long_lived.push(format!("{} (~{}y)", name, years));
                            }
                        }
                    }
                }
            }
        }

        let pass = long_lived.is_empty();

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if pass {
                "Cookie Expiration Audit".into()
            } else {
                "Cookie lifetime exceeds SiteCMD review threshold".into()
            },
            description: if pass {
                "No cookies with Max-Age longer than 1 year were detected.".into()
            } else {
                format!(
                    "{} cookie{} use{} Max-Age longer than SiteCMD's one-year review threshold: {}. The threshold is not a universal legal cap; an appropriate lifetime depends on purpose, necessity, jurisdiction, consent or opt-out state, and the published retention policy.",
                    long_lived.len(),
                    if long_lived.len() == 1 { "" } else { "s" },
                    if long_lived.len() == 1 { "s" } else { "" },
                    long_lived.join(", ")
                )
            },
            status: if pass {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            // Retention length alone is a Low-severity hygiene signal.
            severity: Severity::Low,
            fix_prompt: if pass {
                None
            } else {
                Some("For each surfaced cookie, identify its owner, purpose, audience, and actual retention requirement. Shorten it to the minimum justified period, align consent or preference behavior and the privacy notice, then verify the resulting Max-Age in the deployed response.".into())
            },
            manual_fix: if pass {
                None
            } else {
                Some("Review the purpose of each long-lived cookie, shorten retention where possible, and make sure the lifetime matches your privacy/cookie policy and consent settings.".into())
            },
            raw_data: if pass {
                None
            } else {
                Some(serde_json::json!({
                    "long_lived_cookies": long_lived,
                    "review_threshold_seconds": one_year_secs,
                    "threshold_is_universal_legal_cap": false,
                }))
            },
            confidence: if pass {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if pass {
                None
            } else {
                Some("SiteCMD measured Max-Age on the scanned response, but it cannot infer the cookie's purpose, owner, consent state, applicable law, or a justified retention period.".into())
            },
            why_it_matters: if pass {
                None
            } else {
                Some("Longer-lived identifiers extend the period in which a browser can be recognized. Whether that is excessive depends on the cookie's purpose, necessity, user choice, and applicable retention rules.".into())
            },
        }]
    }
}

/// Check if the site respects Do Not Track
pub struct DntRespectCheck;

impl Check for DntRespectCheck {
    fn id(&self) -> &str {
        "compliance.dnt_respect"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        // Disclosure prose must be readable content; the signal APIs below are
        // legitimately script-hosted, so those keep script text (minus
        // comments, which nothing executes or displays).
        let content = content_text_lower(lower);
        let executable = executable_text_lower(lower);

        let has_dnt_mention = content.contains("do not track") || content.contains("do-not-track");

        // One shared signature list, minus the cookieless providers: this check
        // must not report "no tracking" where compliance.trackers names a
        // provider, and must not demand a privacy-signal notice for a service
        // that stores no visitor identifier.
        let tracking_providers = consent_relevant_trackers(&executable);
        let cookieless_providers = cookieless_trackers(&executable);
        let has_tracking = !tracking_providers.is_empty();

        // This can observe disclosure text or a signal-reading API, not
        // whether the site honors DNT or GPC. Require specific GPC terms
        // rather than a substring.
        let has_gpc_mention = content.contains("global privacy control")
            || GPC_WORD_RE.is_match(&content)
            || executable.contains("globalprivacycontrol")
            || executable.contains("sec-gpc");

        let (status, title, desc) = if !has_tracking {
            (
                CheckStatus::Pass,
                "Privacy signal disclosure (DNT / GPC)",
                if cookieless_providers.is_empty() {
                    "No third-party tracking scripts detected.".to_string()
                } else {
                    let (marker, subject, object) = if cookieless_providers.len() == 1 {
                        ("marker is", "it is", "it")
                    } else {
                        ("markers are", "they are", "them")
                    };
                    format!(
                        "The only detected measurement {marker} {}, which SiteCMD classifies as cookieless: {subject} documented to store no visitor identifier, so no Do Not Track or Global Privacy Control disclosure is expected for {object}.",
                        name_list(&cookieless_providers)
                    )
                },
            )
        } else if has_dnt_mention || has_gpc_mention {
            (
                CheckStatus::Pass,
                "Privacy signal disclosure (DNT / GPC)",
                format!(
                    "Site mentions {} alongside tracking scripts.",
                    match (has_dnt_mention, has_gpc_mention) {
                        (true, true) => "Do Not Track and Global Privacy Control",
                        (true, false) => "Do Not Track",
                        _ => "Global Privacy Control",
                    },
                ),
            )
        } else {
            (CheckStatus::Warn, "No DNT/GPC handling or disclosure marker found", "Recognized tracking markers appear in the scanned page source, but no Do Not Track or Global Privacy Control marker was found there. This check does not inspect a separate privacy policy, geographic logic, or network behavior. DNT is generally voluntary; obligations to honor GPC depend on whether the business and processing are covered by applicable law.".to_string())
        };

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: title.into(),
            description: desc,
            status,
            severity: Severity::Low,
            fix_prompt: if status == CheckStatus::Warn {
                Some("Determine whether privacy signals apply to the business and processing. If GPC or another opt-out signal must be honored, process it at every relevant collection and sharing boundary, persist the choice where appropriate, and explain the behavior in the privacy notice. Treat DNT separately and state accurately whether it is honored.".into())
            } else {
                None
            },
            manual_fix: if status == CheckStatus::Warn {
                Some("Add a short privacy-signal section to your privacy page. State whether you honor Do Not Track or Global Privacy Control, and if you do, conditionally block non-essential tracking when those signals are present.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "tracking_marker_detected": has_tracking,
                "consent_relevant_trackers": tracking_providers,
                "cookieless_trackers": cookieless_providers,
                "dnt_marker_detected": has_dnt_mention,
                "gpc_marker_detected": has_gpc_mention,
                "separate_privacy_policy_inspected": false,
                "runtime_behavior_verified": false,
            })),
            confidence: if status == CheckStatus::Warn {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: if status == CheckStatus::Warn {
                Some("The scanned page source lacks a recognized marker, but SiteCMD does not inspect every privacy-policy URL or verify server-side and runtime handling. Legal applicability also depends on the business and data practice.".into())
            } else {
                None
            },
            why_it_matters: if status == CheckStatus::Warn {
                Some("Where a covered opt-out signal applies, failing to honor it can conflict with user choice and regulatory obligations. Even when DNT is voluntary, an accurate disclosure avoids promising behavior the site does not implement.".into())
            } else {
                None
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, PageContext};

    fn ctx_with_body(body: &str) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("https://example.com").unwrap(),
            response_headers: http::header::HeaderMap::new(),
            status_code: 200,
            body: body.to_string(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn homepage_linking_to_privacy_policy_passes() {
        let body = r#"<html><body><h1>Acme</h1><footer><a href="/privacy-policy">Privacy</a></footer></body></html>"#;
        let results = DataControllerContactCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("links to a privacy policy"));
    }

    #[test]
    fn page_without_contact_or_privacy_link_requires_applicability_review() {
        let body = "<html><body><h1>Acme</h1><p>We sell widgets.</p></body></html>";
        let results = DataControllerContactCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].fix_prompt.is_some());
    }

    #[test]
    fn anchor_text_privacy_link_is_credited_like_the_sibling_check() {
        let body =
            r#"<html><body><footer><a href="/legal">Privacy Policy</a></footer></body></html>"#;
        let results = DataControllerContactCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("links to a privacy policy"));
    }

    #[test]
    fn cookie_expiration_is_labeled_as_a_review_threshold_not_a_legal_cap() {
        let mut headers = http::header::HeaderMap::new();
        headers.append(
            "set-cookie",
            "tracker=abc; Max-Age=63072000; Path=/".parse().unwrap(),
        );
        let mut ctx = ctx_with_body("<html><body></body></html>");
        ctx.response_headers = headers;
        let results = CookieExpirationCheck.run(&ctx);
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].title.contains("SiteCMD review threshold"));
        assert!(results[0].description.contains("not a universal legal cap"));
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].fix_prompt.is_some());
    }

    #[test]
    fn controller_details_on_page_pass() {
        let body = "<html><body><p>The data controller is Acme GmbH, reachable at privacy@acme.example.</p></body></html>";
        let results = DataControllerContactCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn gpc_letters_inside_minified_identifier_are_not_disclosure() {
        let body = r#"<html><body><script src="https://www.googletagmanager.com/gtag.js"></script><script>var awgpcab=1;</script></body></html>"#;
        let results = DntRespectCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].description.contains("scanned page source"));
    }

    #[test]
    fn tracking_detection_agrees_with_the_shared_tracker_list() {
        // A comScore beacon is tracking; claiming "no third-party tracking
        // scripts detected" here would contradict compliance.trackers.
        let body = r#"<html><body><img alt="" height="1" width="1" src="https://sb.scorecardresearch.com/p?c1=2"></body></html>"#;
        let results = DntRespectCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].raw_data.as_ref().expect("evidence")["tracking_marker_detected"],
            true
        );
    }

    #[test]
    fn a_cookieless_analytics_service_needs_no_privacy_signal_disclosure() {
        let body = r#"<html><head><script src="https://plausible.io/js/script.js" defer></script></head><body></body></html>"#;
        let results = DntRespectCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(
            results[0].description.contains("Plausible Analytics")
                && results[0].description.contains("cookieless"),
            "the pass must name what it found: {}",
            results[0].description
        );
        assert_eq!(
            results[0].raw_data.as_ref().expect("evidence")["tracking_marker_detected"],
            false
        );
    }

    #[test]
    fn a_cookieless_provider_beside_a_cookie_setting_one_still_needs_a_disclosure() {
        let body = r#"<html><head>
            <script src="https://cdn.usefathom.com/script.js" data-site="X" defer></script>
            <script src="https://www.googletagmanager.com/gtag/js?id=G-XYZ"></script>
        </head><body><h1>Widgets</h1></body></html>"#;
        let results = DntRespectCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        let evidence = results[0].raw_data.as_ref().expect("evidence");
        assert_eq!(evidence["tracking_marker_detected"], true);
        assert_eq!(
            evidence["cookieless_trackers"],
            serde_json::json!(["Fathom Analytics"])
        );
    }

    #[test]
    fn two_cookieless_providers_read_as_plural() {
        let body = r#"<html><head>
            <script src="https://plausible.io/js/script.js" defer></script>
            <script src="https://cdn.usefathom.com/script.js" defer></script>
        </head><body></body></html>"#;
        let results = DntRespectCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(
            results[0].description.contains(
                "The only detected measurement markers are Plausible Analytics and Fathom Analytics"
            ) && results[0].description.contains("they are documented")
                && results[0].description.contains("expected for them."),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn a_commented_out_tag_is_not_live_tracking() {
        let body = r#"<html><body><!-- <script src="https://www.googletagmanager.com/gtag.js"></script> --><p>Nothing here.</p></body></html>"#;
        let results = DntRespectCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(
            results[0].raw_data.as_ref().expect("evidence")["tracking_marker_detected"],
            false
        );
    }

    #[test]
    fn a_gpc_mention_inside_an_html_comment_is_not_a_disclosure() {
        let body = r#"<html><body><script src="https://www.googletagmanager.com/gtag.js"></script><!-- GPC review pending. --></body></html>"#;
        let results = DntRespectCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].raw_data.as_ref().expect("evidence")["gpc_marker_detected"],
            false
        );
    }

    #[test]
    fn a_privacy_link_inside_an_html_comment_is_not_controller_disclosure() {
        let body =
            r#"<html><body><!-- <a href="/privacy-policy">Privacy Policy</a> --></body></html>"#;
        let results = DataControllerContactCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn gpc_word_or_api_counts_as_disclosure() {
        let prose = r#"<html><body><script src="https://www.googletagmanager.com/gtag.js"></script><p>We honor GPC signals.</p></body></html>"#;
        let results = DntRespectCheck.run(&ctx_with_body(prose));
        assert_eq!(results[0].status, CheckStatus::Pass);

        let api = r#"<html><body><script src="https://www.googletagmanager.com/gtag.js"></script><script>if(navigator.globalPrivacyControl){optOut()}</script></body></html>"#;
        let results = DntRespectCheck.run(&ctx_with_body(api));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }
}
