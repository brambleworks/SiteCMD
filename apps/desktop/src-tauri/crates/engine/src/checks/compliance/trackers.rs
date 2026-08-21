//! Review-level static signatures for analytics and tracking providers.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

/// One third-party tracker/analytics signature. `analytics` marks services
/// whose primary purpose is traffic or product analytics; ad and social
/// pixels are trackers but not analytics.
pub struct TrackerSignature {
    pub domain: &'static str,
    pub name: &'static str,
    pub analytics: bool,
}

const fn sig(domain: &'static str, name: &'static str, analytics: bool) -> TrackerSignature {
    TrackerSignature {
        domain,
        name,
        analytics,
    }
}

/// Shared tracker signatures for compliance and analytics checks.
/// Multiple domains for the same provider are deduplicated by display name.
pub const TRACKER_SIGNATURES: &[TrackerSignature] = &[
    sig("google-analytics.com", "Google Analytics", true),
    sig("googletagmanager.com", "Google Tag Manager", true),
    sig("gtag/js", "Google Analytics (gtag)", true),
    sig("doubleclick.net", "Google Ads (DoubleClick)", false),
    sig("facebook.net", "Meta (Facebook) Pixel", false),
    sig("connect.facebook", "Meta (Facebook) Pixel", false),
    sig("snap.licdn.com", "LinkedIn Insight Tag", false),
    sig("analytics.tiktok.com", "TikTok Pixel", false),
    sig("ct.pinterest.com", "Pinterest Tag", false),
    sig("static.ads-twitter.com", "X / Twitter Pixel", false),
    sig("redditstatic.com/ads", "Reddit Pixel", false),
    sig("bat.bing.com", "Microsoft Ads UET", false),
    sig("hotjar.com", "Hotjar", true),
    sig("clarity.ms", "Microsoft Clarity", true),
    sig("fullstory.com", "FullStory", true),
    sig("logrocket.com", "LogRocket", true),
    sig("posthog.com", "PostHog", true),
    sig("segment.com", "Segment", true),
    sig("mixpanel.com", "Mixpanel", true),
    sig("amplitude.com", "Amplitude", true),
    sig("plausible.io", "Plausible Analytics", true),
    sig("umami.is", "Umami Analytics", true),
    sig("usefathom.com", "Fathom Analytics", true),
    sig("simpleanalytics.com", "Simple Analytics", true),
];

pub struct ThirdPartyTrackerCheck;
impl Check for ThirdPartyTrackerCheck {
    fn id(&self) -> &str {
        "compliance.trackers"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let mut found: Vec<String> = Vec::new();
        for signature in TRACKER_SIGNATURES {
            if lower.contains(signature.domain) {
                let s = signature.name.to_string();
                if !found.contains(&s) {
                    found.push(s);
                }
            }
        }

        vec![CheckResult {
            check_id: "compliance.trackers".into(),
            category: ScanCategory::Compliance,
            title: if found.is_empty() {
                "Third-party trackers".into()
            } else {
                // The list includes analytics services that may be cookieless.
                "Third-party analytics or tracking scripts detected".into()
            },
            description: if found.is_empty() {
                "No third-party tracking scripts were detected on this page.".into()
            } else {
                format!("Detected third-party tracking or analytics script markers: {}. Static signature matches do not establish whether they execute, collect identifying data, or are already controlled by a regional privacy mechanism.", found.join(", "))
            },
            status: if found.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: if found.is_empty() {
                None
            } else {
                Some("Inventory what each detected script actually sends, when it executes, and which visitors receive it. Remove unnecessary vendors, document required disclosures, and gate or configure data collection according to the applicable purpose, jurisdiction, and user choice.".into())
            },
            manual_fix: if found.is_empty() {
                None
            } else {
                Some("Review which detected scripts are needed, what data they send, and when they execute. Apply the disclosure, consent, opt-out, or configuration required for the actual purpose and jurisdictions rather than assuming every analytics script has the same rule.".into())
            },
            raw_data: Some(serde_json::json!({
                "trackers": found,
                "measurement": "static_script_signature_match",
                "execution_or_data_collection_verified": false,
            })),
            confidence: if found.is_empty() {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if found.is_empty() {
                None
            } else {
                Some("SiteCMD found provider strings in static page source. That does not establish whether they execute, collect identifying data, are gated by a consent manager, or are subject to a particular regional rule.".into())
            },
            why_it_matters: if found.is_empty() {
                None
            } else {
                Some("Depending on purpose and jurisdiction, third-party collection may require disclosure, consent, or an opt-out. Even when no specific consent rule applies, unnecessary data sharing increases privacy and vendor risk.".into())
            },
        }]
    }
}

pub struct FormConsentCheck;
impl Check for FormConsentCheck {
    fn id(&self) -> &str {
        "compliance.form_consent"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let form_count = lower.matches("<form").count();
        let has_form = form_count > 0;
        if !has_form {
            return vec![CheckResult {
                check_id: "compliance.form_consent".into(),
                category: ScanCategory::Compliance,
                title: "Form consent".into(),
                description: "No forms detected on this page.".into(),
                status: CheckStatus::Pass,
                severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "form_count": 0,
                    "email_input_count": 0,
                    "privacy_or_consent_text_detected": false,
                    "checkbox_detected": false,
                    "proximity_or_visibility_verified": false,
                })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        let email_input_count =
            lower.matches("type=\"email\"").count() + lower.matches("type='email'").count();
        let has_email = email_input_count > 0;
        let has_privacy_link = lower.contains("privacy") || lower.contains("consent");
        let has_checkbox = lower.contains("type=\"checkbox\"") || lower.contains("type='checkbox'");

        if has_email && !has_privacy_link && !has_checkbox {
            vec![CheckResult {
                check_id: "compliance.form_consent".into(),
                category: ScanCategory::Compliance,
                title: "No privacy or consent cue detected for email collection".into(),
                description: "The inspected static markup includes an email input but no privacy/consent text or checkbox marker anywhere on the page. SiteCMD does not know the form's purpose or whether consent is the applicable legal basis.".into(),
                status: CheckStatus::Warn, severity: Severity::Low,
                fix_prompt: Some("Identify the email collection purpose, audience, jurisdictions, and applicable lawful basis first. Provide the required collection notice at the form or a clear link to it. If consent is the applicable basis or separate marketing consent is required, use specific affirmative language and an unchecked opt-in; do not add a mandatory consent checkbox when the processing instead depends on a contract or another basis.".into()),
                manual_fix: Some("Review the form's purpose and applicable privacy rules, then add an accurate collection notice. Add a separate unchecked opt-in only when consent is the basis or is otherwise required.".into()),
                raw_data: Some(serde_json::json!({
                    "form_count": form_count,
                    "email_input_count": email_input_count,
                    "privacy_or_consent_text_detected": has_privacy_link,
                    "checkbox_detected": has_checkbox,
                    "proximity_or_visibility_verified": false,
                })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Static markup shows no obvious privacy, consent, or checkbox marker anywhere on the page, but it cannot determine the form's purpose, audience, regional legal requirements, runtime-rendered disclosure, or whether separate consent is required.".into()),
                why_it_matters: Some("People should be able to understand why their email is collected and how it will be used. The required notice, lawful basis, and any separate consent depend on the purpose and jurisdictions involved.".into()),
            }]
        } else {
            // Report marker detection only; visibility and proximity are not measured.
            vec![CheckResult {
                check_id: "compliance.form_consent".into(),
                category: ScanCategory::Compliance,
                title: "Form consent".into(),
                description: if !has_email {
                    "No email-collecting form was detected on this page.".into()
                } else {
                    "The page collects an email address and consent-related markup (privacy/consent text or a checkbox) is present on the page. The scan does not verify that the cue is visible or placed near the form.".into()
                },
                status: CheckStatus::Pass,
                severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "form_count": form_count,
                    "email_input_count": email_input_count,
                    "privacy_or_consent_text_detected": has_privacy_link,
                    "checkbox_detected": has_checkbox,
                    "proximity_or_visibility_verified": false,
                })),
                confidence: if has_email {
                    crate::checks::IssueConfidence::NeedsReview
                } else {
                    crate::checks::IssueConfidence::High
                },
                confidence_reason: if has_email {
                    Some("A page-wide privacy/consent text marker or checkbox was found, but this static check does not establish its purpose, visibility, proximity to the email form, default state, or legal sufficiency.".into())
                } else {
                    None
                },
                why_it_matters: None,
            }]
        }
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
    fn page_without_trackers_passes() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = ThirdPartyTrackerCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("No third-party tracking"));
    }

    #[test]
    fn detected_trackers_warn_and_are_named() {
        let body = r#"<html><head>
            <script src="https://www.google-analytics.com/analytics.js"></script>
            <script src="https://static.hotjar.com/c/hotjar.com.js"></script>
        </head><body></body></html>"#;
        let results = ThirdPartyTrackerCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].description.contains("Google Analytics"));
        assert!(results[0].description.contains("Hotjar"));
        assert!(results[0]
            .description
            .contains("do not establish whether they execute"));
        assert!(results[0].fix_prompt.is_some());
    }

    #[test]
    fn both_meta_pixel_signatures_report_one_provider() {
        // facebook.net and connect.facebook are two signatures of the same
        // Meta SDK; the dedup-by-name loop must report the provider once.
        let body = r#"<html><head>
            <script src="https://connect.facebook.net/en_US/fbevents.js"></script>
        </head><body></body></html>"#;
        let results = ThirdPartyTrackerCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        let trackers = results[0].raw_data.as_ref().unwrap()["trackers"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(
            trackers.len(),
            1,
            "Meta pixel matched by two signatures must be listed once: {trackers:?}"
        );
        assert_eq!(trackers[0], "Meta (Facebook) Pixel");
    }

    #[test]
    fn page_without_forms_passes_form_consent() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = FormConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("No forms detected"));
    }

    #[test]
    fn email_form_without_consent_cue_warns() {
        let body = r#"<html><body><form action="/subscribe" method="post">
            <input type="email" name="email"><button>Join</button>
        </form></body></html>"#;
        let results = FormConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        assert!(results[0]
            .title
            .contains("No privacy or consent cue detected"));
        assert!(results[0].description.contains("inspected static markup"));
        assert!(!results[0]
            .why_it_matters
            .as_deref()
            .unwrap_or_default()
            .contains("create compliance issues"));
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].fix_prompt.is_some());
        let evidence = results[0].raw_data.as_ref().expect("form evidence");
        assert_eq!(evidence["form_count"], 1);
        assert_eq!(evidence["email_input_count"], 1);
        assert_eq!(evidence["privacy_or_consent_text_detected"], false);
        assert_eq!(evidence["checkbox_detected"], false);
    }

    #[test]
    fn email_form_with_privacy_link_passes_without_claiming_visibility() {
        let body = r#"<html><body><form action="/subscribe" method="post">
            <input type="email" name="email">
            <a href="/privacy">Privacy policy</a>
        </form></body></html>"#;
        let results = FormConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(
            !results[0]
                .description
                .contains("visible privacy or consent cue nearby"),
            "{}",
            results[0].description
        );
        assert!(results[0].description.contains("does not verify"));
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0]
            .confidence_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("proximity")));
    }

    #[test]
    fn form_without_email_input_passes_with_accurate_copy() {
        let body = r#"<html><body><form action="/search"><input type="text" name="q"></form></body></html>"#;
        let results = FormConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(
            results[0]
                .description
                .contains("No email-collecting form was detected"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn tracker_title_says_analytics_or_tracking() {
        // Cookieless analytics (Plausible etc.) are on the list; the title
        // must not flatly call everything "tracking".
        let body = r#"<html><head><script defer data-domain="x.com" src="https://plausible.io/js/script.js"></script></head></html>"#;
        let results = ThirdPartyTrackerCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].title,
            "Third-party analytics or tracking scripts detected"
        );
    }
}
