//! Review-level static signatures for cookie-consent interfaces.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

/// Detects cookie consent banners/scripts
pub struct CookieConsentCheck;

/// Pre-compiled regex patterns for generic cookie consent HTML detection
static CONSENT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r#"(?i)id\s*=\s*["']cookie[_-]?(?:consent|banner|notice|popup|bar|modal)"#,
        r#"(?i)class\s*=\s*["'][^"']*cookie[_-]?(?:consent|banner|notice|popup|bar|modal)"#,
        r#"(?i)accept[_\s-]*(?:all[_\s-]*)?cookies"#,
    ]
    .into_iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

/// Known cookie consent platforms and their detection signatures
const CONSENT_SIGNATURES: &[(&str, &str)] = &[
    ("cookiebot", "Cookiebot"),
    ("cookieconsent", "CookieConsent (Osano)"),
    ("onetrust", "OneTrust"),
    ("trustarc", "TrustArc"),
    ("quantcast", "Quantcast Choice"),
    ("cookie-notice", "Cookie Notice"),
    ("complianz", "Complianz"),
    ("iubenda", "Iubenda"),
    ("termly", "Termly"),
    ("cookie-law-info", "CookieYes / Cookie Law Info"),
    ("gdpr-cookie-consent", "GDPR Cookie Consent"),
    ("eu-cookie-law", "EU Cookie Law"),
    ("cookie-bar", "Cookie Bar"),
    ("cookie-consent", "Cookie Consent"),
    ("civic-cookie-control", "Civic Cookie Control"),
];

impl Check for CookieConsentCheck {
    fn id(&self) -> &str {
        "compliance.cookie_consent"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        // A CMP loader legitimately lives in a script tag, but nothing runs a
        // commented-out one.
        let executable = super::executable_text_lower(lower);

        // Check for known consent platforms
        let mut detected_platforms: Vec<String> = Vec::new();
        for (signature, name) in CONSENT_SIGNATURES {
            if executable.contains(signature) {
                detected_platforms.push(name.to_string());
            }
        }

        // Check for common cookie consent HTML patterns. A banner that only
        // exists inside a comment, script, or stylesheet is not a control the
        // visitor can use, so those blocks are stripped first; the platform
        // signatures above still read the whole body because a CMP loader
        // legitimately lives in a script tag.
        let consent_markup = super::content_text_lower(lower);
        let mut has_generic_consent = false;
        for re in CONSENT_PATTERNS.iter() {
            if re.is_match(&consent_markup) {
                has_generic_consent = true;
                break;
            }
        }

        // Check if the site uses cookies (via Set-Cookie header)
        let sets_cookies = ctx.response_headers.get_all("set-cookie").iter().count() > 0;

        // Tracking that typically requires consent: the shared signature list
        // minus the cookieless providers, plus the inline gtag call a page can
        // carry without a vendor host. A cookieless analytics service stores
        // nothing to gate, so it must not manufacture a consent finding.
        let tracking_providers = super::trackers::consent_relevant_trackers(&executable);
        let cookieless_providers = super::trackers::cookieless_trackers(&executable);
        let has_tracking = !tracking_providers.is_empty() || executable.contains("gtag(");

        let has_consent = !detected_platforms.is_empty() || has_generic_consent;
        let needs_consent = sets_cookies || has_tracking;
        // A Set-Cookie header alone does not reveal whether consent is required.
        let cookies_only = sets_cookies && !has_tracking;

        let (status, severity, title, description) = if has_consent {
            (
                CheckStatus::Pass,
                Severity::Low,
                "Cookie consent",
                if !detected_platforms.is_empty() {
                    format!(
                        "Cookie-consent platform marker detected: {}. Static source does not verify that it initializes, records a valid choice, or gates the relevant storage and scripts.",
                        detected_platforms.join(", ")
                    )
                } else {
                    "Cookie-consent markup marker detected on the page. Static source does not verify that the control is visible, records a valid choice, or gates the relevant storage and scripts.".into()
                },
            )
        } else if needs_consent {
            (
                CheckStatus::Warn,
                Severity::Medium,
                "No visible cookie-consent mechanism detected",
                if cookies_only {
                    "The initial response sets cookies, but SiteCMD found no recognizable consent marker. Static headers do not reveal whether each cookie is necessary, whether regional consent rules apply, or whether another layer manages choice.".to_string()
                } else {
                    let mut reasons = Vec::new();
                    if sets_cookies {
                        reasons.push("sets cookies");
                    }
                    if has_tracking {
                        reasons.push("contains tracking markers");
                    }
                    format!(
                        "The page {} and contains no recognizable consent marker. Static source does not establish whether those scripts execute, are gated remotely, store information, or require consent for the visitor and purpose involved.",
                        reasons.join(" and ")
                    )
                },
            )
        } else {
            (
                CheckStatus::Pass,
                Severity::Low,
                "Cookie consent",
                if cookieless_providers.is_empty() {
                    "No cookie consent banner detected, but no cookies or tracking scripts found either.".to_string()
                } else {
                    let marker = if cookieless_providers.len() == 1 {
                        "marker is"
                    } else {
                        "markers are"
                    };
                    format!(
                        "No cookie consent banner detected. The initial response sets no cookies, and the detected measurement {marker} {}, which SiteCMD classifies as cookieless: nothing observed here stores a visitor identifier to gate. Runtime storage was not inspected.",
                        super::trackers::name_list(&cookieless_providers)
                    )
                },
            )
        };
        let needs_runtime_review = status == CheckStatus::Warn || has_consent;
        let remediation = "Inventory the cookies, local storage, pixels, and requests that actually run before and after a visitor choice. Determine the applicable jurisdictions and exemptions, then implement a consent or preference mechanism that gates the relevant technologies and preserves necessary security functionality. Verify behavior in the Network and Storage panels; visible banner text alone is not evidence that collection is controlled.";

        vec![CheckResult {
            check_id: "compliance.cookie_consent".into(),
            category: ScanCategory::Compliance,
            title: title.into(),
            description,
            status,
            severity,
            fix_prompt: if !has_consent && needs_consent {
                Some(remediation.into())
            } else {
                None
            },
            manual_fix: if !has_consent && needs_consent {
                Some(
                    // A banner alone does not establish GDPR compliance.
                    remediation.into(),
                )
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "consent_detected": has_consent,
                "platforms": detected_platforms,
                "sets_cookies": sets_cookies,
                "has_tracking": has_tracking,
                "consent_relevant_trackers": tracking_providers,
                "cookieless_trackers": cookieless_providers,
                "runtime_gating_verified": false,
            })),
            confidence: if needs_runtime_review {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: if needs_runtime_review {
                Some("Static HTML and the initial Set-Cookie headers cannot establish technology purpose, regional applicability, runtime execution, consent validity, or whether a remote CMP configuration gates collection.".into())
            } else {
                None
            },
            why_it_matters: if !has_consent && needs_consent {
                Some("If non-essential storage or tracking runs before a required choice, the site may collect data contrary to visitor expectations or applicable rules. The finding requires runtime and legal-context review before it becomes a compliance conclusion.".into())
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

    fn ctx(body: &str, headers: http::header::HeaderMap) -> PageContext {
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

    fn ctx_with_body(body: &str) -> PageContext {
        ctx(body, http::header::HeaderMap::new())
    }

    #[test]
    fn known_consent_platform_passes_and_is_named() {
        let body = r#"<html><head><script src="https://consent.cookiebot.com/uc.js"></script></head><body></body></html>"#;
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("Cookiebot"));
    }

    #[test]
    fn generic_banner_markup_counts_as_consent() {
        // No platform signature, just the HTML pattern a hand-rolled banner
        // uses. Must hit the generic-pattern branch, not the platform list.
        let body = r#"<html><body><div id="cookie-banner"><p>We use them.</p></div></body></html>"#;
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0]
            .description
            .contains("Cookie-consent markup marker detected"));
    }

    #[test]
    fn tracking_scripts_without_consent_warn() {
        let body = r#"<html><head><script src="https://www.googletagmanager.com/gtag.js"></script></head><body></body></html>"#;
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert!(results[0].description.contains("tracking markers"));
        assert!(results[0]
            .description
            .contains("does not establish whether those scripts execute"));
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].fix_prompt.is_some());
    }

    #[test]
    fn set_cookie_without_consent_warns_as_needs_review() {
        let mut headers = http::header::HeaderMap::new();
        headers.append("set-cookie", "sid=abc123; Path=/".parse().unwrap());
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = CookieConsentCheck.run(&ctx(body, headers));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("sets cookies"));
        assert!(results[0]
            .description
            .contains("Static headers do not reveal"));
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0]
            .confidence_reason
            .as_deref()
            .unwrap()
            .contains("cannot establish technology purpose"));
    }

    #[test]
    fn tracking_detection_agrees_with_the_shared_tracker_list() {
        // www.bbc.co.uk ships a comScore beacon and no consent markup in the
        // initial HTML; this check must not report "no tracking scripts".
        let body = r#"<html><body><img alt="" height="1" width="1" src="https://sb.scorecardresearch.com/p?c1=2&amp;c2=17986528"></body></html>"#;
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].raw_data.as_ref().expect("evidence")["has_tracking"],
            true
        );
    }

    #[test]
    fn a_cookieless_analytics_service_does_not_manufacture_a_consent_finding() {
        // astro.build ships Fathom and nothing else; recommending a consent
        // banner there would be a finding about storage that does not exist.
        let body = r#"<html><head><script src="https://cdn.usefathom.com/script.js" data-site="EZBHTSIG" defer></script></head><body></body></html>"#;
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
        assert!(
            results[0].description.contains("Fathom Analytics"),
            "the pass must name what it found rather than claim nothing: {}",
            results[0].description
        );
        assert!(!results[0]
            .description
            .contains("no cookies or tracking scripts found"));
        let evidence = results[0].raw_data.as_ref().expect("evidence");
        assert_eq!(evidence["has_tracking"], false);
        assert_eq!(
            evidence["cookieless_trackers"],
            serde_json::json!(["Fathom Analytics"])
        );
    }

    #[test]
    fn a_cookieless_provider_beside_a_cookie_setting_one_still_needs_consent() {
        // laravel.com's shape: Fathom sits beside Google Tag Manager,
        // RudderStack, and HubSpot. One cookieless provider must never excuse
        // the tags that do store an identifier.
        let body = r#"<html><head>
            <script src="https://cdn.usefathom.com/script.js" data-site="X" defer></script>
            <script src="https://www.googletagmanager.com/gtag/js?id=G-XYZ"></script>
            <script src="https://cdn.rudderlabs.com/v3/modern/rsa.min.js"></script>
            <script src="https://js.hs-scripts.com/45240648.js"></script>
        </head><body></body></html>"#;
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(
            results[0].status,
            CheckStatus::Warn,
            "{}",
            results[0].description
        );
        assert_eq!(results[0].severity, Severity::Medium);
        let evidence = results[0].raw_data.as_ref().expect("evidence");
        assert_eq!(evidence["has_tracking"], true);
        assert_eq!(
            evidence["cookieless_trackers"],
            serde_json::json!(["Fathom Analytics"]),
            "the cookieless provider is still reported, just not as a reason"
        );
    }

    #[test]
    fn two_cookieless_providers_read_as_plural() {
        let body = r#"<html><head>
            <script src="https://plausible.io/js/script.js" defer></script>
            <script src="https://cdn.usefathom.com/script.js" defer></script>
        </head><body></body></html>"#;
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(
            results[0]
                .description
                .contains("measurement markers are Plausible Analytics and Fathom Analytics"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn a_commented_out_tag_is_not_live_tracking() {
        let body = r#"<html><body><!-- <script src="https://www.googletagmanager.com/gtag/js?id=G-X"></script> --></body></html>"#;
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(
            results[0].raw_data.as_ref().expect("evidence")["has_tracking"],
            false
        );
    }

    #[test]
    fn a_commented_out_banner_is_not_a_consent_mechanism() {
        let body = r#"<html><head><script src="https://www.googletagmanager.com/gtag.js"></script></head><body><!-- <div id="cookie-banner">Accept all cookies</div> --></body></html>"#;
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].raw_data.as_ref().expect("evidence")["consent_detected"],
            false
        );
    }

    #[test]
    fn static_site_without_cookies_or_tracking_passes() {
        let body = "<html><body><h1>Widgets</h1><p>We sell widgets.</p></body></html>";
        let results = CookieConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0]
            .description
            .contains("no cookies or tracking scripts found"));
    }
}
