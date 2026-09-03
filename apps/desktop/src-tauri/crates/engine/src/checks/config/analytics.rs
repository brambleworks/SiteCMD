//! Client-side analytics detection using the shared tracker signatures.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

pub struct AnalyticsCheck;
impl Check for AnalyticsCheck {
    fn id(&self) -> &str {
        "config.analytics"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        // Share tracker signatures so analytics checks cannot disagree.
        let mut found: Vec<&str> = Vec::new();
        for signature in crate::checks::compliance::trackers::TRACKER_SIGNATURES {
            if signature.analytics
                && lower.contains(signature.marker)
                && !found.contains(&signature.name)
            {
                found.push(signature.name);
            }
        }
        // First-party loader names: the vendor is unresolved, but measurement
        // is plainly wired up, so "no analytics" would be the wrong claim.
        if found.is_empty() && (lower.contains("analytics.js") || lower.contains("load-analytics"))
        {
            found.push("Generic analytics");
        }
        vec![CheckResult {
            check_id: "config.analytics".into(),
            category: ScanCategory::Seo,
            title: if found.is_empty() {
                "No recognized client-side analytics detected".into()
            } else {
                "Analytics".into()
            },
            description: if found.is_empty() {
                "The scanned HTML does not contain a recognized client-side analytics marker. This does not establish that measurement is absent: server-side analytics, tag managers, consent-gated scripts, reverse-proxy logs, and unrecognized providers may collect equivalent product signals. Analytics is optional and should serve a defined product question.".into()
            } else {
                format!("Analytics detected: {}.", found.join(", "))
            },
            // Analytics is optional, so its absence is a fact to report, not
            // a defect to fix.
            status: CheckStatus::Pass,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({"analytics": found})),
            confidence: if found.is_empty() {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: if found.is_empty() {
                Some("Only recognized markers in the scanned HTML were evaluated; server-side, infrastructure, consent-gated, and unrecognized analytics were not resolved.".into())
            } else {
                None
            },
            why_it_matters: None,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::AnalyticsCheck;
    use crate::checks::{Check, CheckStatus, PageContext};

    fn ctx(body: &str) -> PageContext {
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
    fn mixpanel_counts_as_analytics() {
        let body = r#"<html><head><script src="https://cdn.mixpanel.com/mixpanel.js"></script></head></html>"#;
        let results = AnalyticsCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("Mixpanel"));
    }

    #[test]
    fn every_analytics_class_tracker_signature_is_detected_here() {
        for signature in crate::checks::compliance::trackers::TRACKER_SIGNATURES {
            if !signature.analytics {
                continue;
            }
            let body = format!(
                r#"<script src="https://x/{}/s.js"></script>"#,
                signature.marker
            );
            let results = AnalyticsCheck.run(&ctx(&body));
            let detected = results[0].raw_data.as_ref().expect("analytics evidence")["analytics"]
                .as_array()
                .expect("analytics list")
                .clone();
            assert!(
                !detected.is_empty(),
                "{} detected by compliance.trackers must count as analytics",
                signature.name
            );
        }
    }

    #[test]
    fn an_absent_optional_feature_is_reported_as_a_pass_not_a_warning() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = AnalyticsCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("Analytics is optional"));
        assert!(results[0].manual_fix.is_none());
        assert!(results[0].fix_prompt.is_none());
        assert_eq!(
            results[0].raw_data.as_ref().expect("analytics evidence")["analytics"],
            serde_json::json!([])
        );
    }

    #[test]
    fn ga4_data_attribute_instrumentation_counts_as_analytics() {
        // www.gov.uk carries no analytics host in the initial HTML: the GA4
        // wiring is data attributes plus a first-party loader module.
        let body = r#"<html><body><a class="govuk-link" data-ga4-link='{"event_name":"navigation"}' href="/x">x</a>
            <script type="module" src="/assets/frontend/govuk_publishing_components/load-analytics-ad855bc6.js"></script></body></html>"#;
        let results = AnalyticsCheck.run(&ctx(body));
        assert!(results[0].description.contains("Google Analytics 4"));
    }

    #[test]
    fn a_first_party_analytics_loader_is_not_reported_as_no_analytics() {
        let body = r#"<html><body><script src="/assets/load-analytics-ad855bc6.js"></script></body></html>"#;
        let results = AnalyticsCheck.run(&ctx(body));
        assert!(results[0].description.contains("Generic analytics"));
    }
}
