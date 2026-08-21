//! Google Consent Mode review signals. Static evidence never proves runtime consent.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

pub struct ConsentModeCheck;

/// Inline signals that Consent Mode is configured on the page itself.
const CONSENT_MODE_SIGNALS: &[&str] = &[
    "gtag('consent'",
    "gtag(\"consent\"",
    "'consent', 'default'",
    "\"consent\", \"default\"",
    "'consent','default'",
    "\"consent\",\"default\"",
    "consent_mode",
    "consentmode",
];

/// Consent platforms that implement Google Consent Mode on the site's behalf.
/// When one of these is present we cannot claim consent mode is missing.
const CONSENT_PLATFORMS: &[&str] = &[
    "cookiebot",
    "onetrust",
    "cookieyes",
    "cookie-law-info",
    "complianz",
    "iubenda",
    "termly",
    "usercentrics",
    "didomi",
    "quantcast",
    "civic-cookie-control",
    "cookieconsent",
];

impl Check for ConsentModeCheck {
    fn id(&self) -> &str {
        "compliance.consent_mode"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();

        let has_google_tags = lower.contains("googletagmanager.com")
            || lower.contains("google-analytics.com")
            || lower.contains("gtag/js");

        if !has_google_tags {
            return vec![];
        }

        let has_consent_signal = CONSENT_MODE_SIGNALS
            .iter()
            .any(|signal| lower.contains(signal));
        let detected_platform = CONSENT_PLATFORMS
            .iter()
            .find(|platform| lower.contains(*platform));

        if has_consent_signal || detected_platform.is_some() {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Google Consent Mode".into(),
                description: if has_consent_signal {
                    "Google-tag and Consent Mode markers are both present in static source. This does not verify ordering, default values, regional scope, updates after a choice, or the resulting network behavior.".into()
                } else {
                    format!(
                        "Google-tag and consent-platform markers are both present ({}). Verify Consent Mode v2 and the required Google-product consent signals in that platform; static source cannot see its remote configuration.",
                        detected_platform.unwrap_or(&"consent platform")
                    )
                },
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "inline_consent_signal": has_consent_signal,
                    "consent_platform": detected_platform,
                })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Static marker co-occurrence does not establish initialization order, consent-state values, geographic scope, later updates, or tag behavior on the network.".into()),
                why_it_matters: None,
            }];
        }

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: "Google tags with no visible Consent Mode signal".into(),
            description: "The static source contains a Google tag URL but no recognizable Consent Mode or consent-platform marker. This does not establish whether the tag executes or whether consent is configured inside Tag Manager. For Google Ads-linked measurement, personalization, or remarketing involving EEA end-user data, Google requires applicable consent to be collected and consent signals shared for those use cases.".into(),
            status: CheckStatus::Warn,
            severity: Severity::Medium,
            fix_prompt: Some("Identify which Google products receive data, whether that data feeds advertising features, and which visitors require a consent choice under the organization's policy and applicable rules. If Consent Mode applies, set appropriate regional defaults before measurement commands, send v2 consent signals, update them when the choice changes, and verify the network result with Tag Assistant. For Tag Manager, use its consent APIs or a reviewed CMP template rather than relying on source-string order.".into()),
            manual_fix: Some("Review the Google-product and regional consent requirements, then configure Consent Mode v2 in gtag, Tag Manager, or the CMP. Defaults must match the actual policy rather than blindly denying or granting every storage type; verify initial and updated states with Tag Assistant.".into()),
            raw_data: Some(serde_json::json!({
                "inline_consent_signal": false,
                "consent_platform": serde_json::Value::Null,
                "static_source_only": true,
            })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some(
                "Consent Mode can be configured inside a Google Tag Manager container or a consent platform's remote settings, which a static page scan cannot see.".into(),
            ),
            why_it_matters: Some(
                "For applicable Google advertising and linked-measurement use cases, missing consent signals can limit product functionality and make tag behavior diverge from visitor choices. This finding is a Google-product configuration review, not a legal-compliance verdict.".into(),
            ),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, IssueConfidence, PageContext};
    use http::header::HeaderMap;

    fn ctx_with_body(body: &str) -> PageContext {
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
    fn no_google_tags_emits_nothing() {
        let results = ConsentModeCheck.run(&ctx_with_body("<script src=\"/app.js\"></script>"));
        assert!(results.is_empty());
    }

    #[test]
    fn google_tags_without_consent_warns_at_needs_review() {
        let body =
            r#"<script async src="https://www.googletagmanager.com/gtag/js?id=G-XYZ"></script>"#;
        let results = ConsentModeCheck.run(&ctx_with_body(body));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(results[0].fix_prompt.is_some());
        assert!(results[0].description.contains("static source"));
    }

    #[test]
    fn inline_consent_default_passes() {
        let body = r#"
            <script>gtag('consent', 'default', { ad_storage: 'denied' });</script>
            <script async src="https://www.googletagmanager.com/gtag/js?id=G-XYZ"></script>
        "#;
        let results = ConsentModeCheck.run(&ctx_with_body(body));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(results[0].description.contains("does not verify ordering"));
    }

    #[test]
    fn consent_platform_presence_passes_with_verify_note() {
        let body = r#"
            <script src="https://consent.cookiebot.com/uc.js"></script>
            <script async src="https://www.googletagmanager.com/gtag/js?id=G-XYZ"></script>
        "#;
        let results = ConsentModeCheck.run(&ctx_with_body(body));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("Verify Consent Mode v2"));
    }
}
