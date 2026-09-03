//! Review-level static signatures for analytics and tracking providers.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

/// One third-party tracker/analytics signature. `marker` is the lowercase
/// source string that identifies the provider: usually a host, sometimes a
/// script path or instrumentation attribute. `analytics` marks services whose
/// primary purpose is traffic or product analytics; ad and social pixels are
/// trackers but not analytics. `cookieless` marks providers documented to set
/// no cookie and store no per-visitor identifier: their presence is a
/// disclosure fact, never a reason to expect a consent control. The two flags
/// are independent, since Google Analytics is analytics and is not cookieless.
pub struct TrackerSignature {
    pub marker: &'static str,
    pub name: &'static str,
    pub analytics: bool,
    pub cookieless: bool,
}

const fn sig(marker: &'static str, name: &'static str, analytics: bool) -> TrackerSignature {
    TrackerSignature {
        marker,
        name,
        analytics,
        cookieless: false,
    }
}

/// A provider whose documented design sets no cookie and stores no visitor
/// identifier (Plausible, Umami, Fathom, Simple Analytics).
const fn cookieless_sig(marker: &'static str, name: &'static str) -> TrackerSignature {
    TrackerSignature {
        marker,
        name,
        analytics: true,
        cookieless: true,
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
    cookieless_sig("plausible.io", "Plausible Analytics"),
    cookieless_sig("umami.is", "Umami Analytics"),
    cookieless_sig("usefathom.com", "Fathom Analytics"),
    cookieless_sig("simpleanalytics.com", "Simple Analytics"),
    sig("googlesyndication.com", "Google AdSense", false),
    sig("adsbygoogle", "Google AdSense", false),
    sig("scorecardresearch.com", "comScore", true),
    sig("chartbeat.com", "Chartbeat", true),
    sig("ati-host.net", "AT Internet / Piano Analytics", true),
    sig("aticdn.net", "AT Internet / Piano Analytics", true),
    sig("rudderlabs.com", "RudderStack", true),
    sig("rudderstack.com", "RudderStack", true),
    sig("ruddersnippetversion", "RudderStack", true),
    sig("js.hs-scripts.com", "HubSpot", true),
    // GA4 markup instrumentation: govuk-style pages carry no vendor host in
    // the initial HTML, only these attributes on the elements they measure.
    sig("data-ga4-", "Google Analytics 4", true),
];

/// Provider names matched in `lower`, deduplicated, restricted to one side of
/// the cookieless axis.
fn matched_provider_names(lower: &str, cookieless: bool) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    for signature in TRACKER_SIGNATURES {
        if signature.cookieless == cookieless
            && lower.contains(signature.marker)
            && !names.contains(&signature.name)
        {
            names.push(signature.name);
        }
    }
    names
}

/// Providers whose presence implies consent-relevant storage or data sharing.
/// Shared with the consent and privacy-signal checks so they cannot disagree
/// with `compliance.trackers` about whether such tracking is present.
pub fn consent_relevant_trackers(lower: &str) -> Vec<&'static str> {
    matched_provider_names(lower, false)
}

/// Providers SiteCMD classifies as cookieless: reported as detected, but never
/// treated as a reason to expect a consent banner or a privacy-signal notice.
pub fn cookieless_trackers(lower: &str) -> Vec<&'static str> {
    matched_provider_names(lower, true)
}

/// English list for check copy: "a", "a and b", "a, b, and c".
pub fn name_list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => (*only).to_string(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

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
            if lower.contains(signature.marker) {
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
        // Triggers and mitigation must read the same text. A form inside a
        // comment or a script template is not a form a visitor can submit, and
        // a consent cue in the same place is not one they can read.
        let content = super::content_text_lower(ctx.body_lower());
        let form_count = content.matches("<form").count();
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
            content.matches("type=\"email\"").count() + content.matches("type='email'").count();
        let has_email = email_input_count > 0;
        let has_privacy_link = content.contains("privacy") || content.contains("consent");
        let has_checkbox =
            content.contains("type=\"checkbox\"") || content.contains("type='checkbox'");

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
    fn the_adsense_loader_is_a_detected_tracker() {
        // visityourteam.com loads the AdSense tag beside Plausible; the tag is
        // an ad network, so it is a tracker rather than an analytics service.
        let body = r#"<html><head>
            <script async src="https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=ca-pub-1234567890123456" crossorigin="anonymous"></script>
        </head><body><ins class="adsbygoogle" data-ad-client="ca-pub-1234567890123456"></ins>
            <script>(adsbygoogle = window.adsbygoogle || []).push({});</script>
        </body></html>"#;
        let results = ThirdPartyTrackerCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        let trackers = results[0].raw_data.as_ref().unwrap()["trackers"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(
            trackers,
            vec![serde_json::json!("Google AdSense")],
            "the host and the inline queue name one provider once: {trackers:?}"
        );
        // An ad tag is not analytics, so config.analytics must not claim it as
        // product measurement.
        let analytics = crate::checks::config::analytics::AnalyticsCheck.run(&ctx_with_body(body));
        assert!(
            !analytics[0].description.contains("AdSense"),
            "{}",
            analytics[0].description
        );
    }

    #[test]
    fn comscore_beacon_markup_is_a_detected_tracker() {
        // www.bbc.co.uk ships comScore as an image beacon, not a script tag.
        let body = r#"<html><body><img alt="" height="1" width="1" src="https://sb.scorecardresearch.com/p?c1=2&amp;c2=17986528&amp;cs_ucfr=0"></body></html>"#;
        let results = ThirdPartyTrackerCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("comScore"));
    }

    #[test]
    fn rudderstack_and_hubspot_loaders_are_detected_trackers() {
        // laravel.com loads both through inline snippets.
        let body = r#"<html><head>
            <script>window.RudderSnippetVersion="3.0.3";</script>
            <script src="https://cdn.rudderlabs.com/v3/modern/rsa.min.js"></script>
            <script src="https://js.hs-scripts.com/45240648.js"></script>
        </head></html>"#;
        let results = ThirdPartyTrackerCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        let trackers = results[0].raw_data.as_ref().unwrap()["trackers"]
            .as_array()
            .unwrap()
            .clone();
        assert!(
            trackers.contains(&serde_json::json!("RudderStack")),
            "{trackers:?}"
        );
        assert!(
            trackers.contains(&serde_json::json!("HubSpot")),
            "{trackers:?}"
        );
        assert_eq!(
            trackers
                .iter()
                .filter(|name| **name == serde_json::json!("RudderStack"))
                .count(),
            1,
            "three RudderStack markers must report one provider: {trackers:?}"
        );
    }

    #[test]
    fn ga4_data_attributes_are_a_detected_tracker() {
        // www.gov.uk instruments GA4 through markup attributes only.
        let body = r#"<html><body><a data-ga4-link='{"event_name":"navigation"}' href="/x">x</a></body></html>"#;
        let results = ThirdPartyTrackerCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("Google Analytics 4"));
    }

    #[test]
    fn a_form_and_a_cue_that_both_live_in_a_comment_produce_no_finding() {
        // Triggers and mitigation read the same stripped text, so a page whose
        // only form is commented out is a page with no form.
        let body = r#"<html><body><!-- <form action="/subscribe" method="post">
            <input type="email" name="email"><a href="/privacy">Privacy</a>
        </form> --><p>Nothing here yet.</p></body></html>"#;
        let results = FormConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("No forms detected"));
        let evidence = results[0].raw_data.as_ref().expect("form evidence");
        assert_eq!(evidence["form_count"], 0);
        assert_eq!(evidence["email_input_count"], 0);
    }

    #[test]
    fn a_commented_out_privacy_word_is_not_a_form_consent_cue() {
        let body = r#"<html><body><!-- privacy notice pending --><form action="/subscribe" method="post">
            <input type="email" name="email"><button>Join</button>
        </form></body></html>"#;
        let results = FormConsentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].raw_data.as_ref().expect("form evidence")
                ["privacy_or_consent_text_detected"],
            false
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
