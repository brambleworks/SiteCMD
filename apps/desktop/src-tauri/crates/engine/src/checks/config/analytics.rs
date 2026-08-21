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
                && lower.contains(signature.domain)
                && !found.contains(&signature.name)
            {
                found.push(signature.name);
            }
        }
        if found.is_empty() && lower.contains("analytics.js") {
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
            status: if found.is_empty() {
                CheckStatus::Warn
            } else {
                CheckStatus::Pass
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if found.is_empty() {
                Some(
                    "Inventory existing server, CDN, tag-manager, and consent-gated measurement first. If important product questions remain unanswered, choose the smallest analytics approach that supplies those metrics and meets the product's consent, privacy, retention, access, residency, and cost requirements."
                        .into(),
                )
            } else {
                None
            },
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
            why_it_matters: if found.is_empty() {
                Some("When a product relies on acquisition or behavior data, defined and privacy-appropriate measurement helps distinguish evidence from assumptions; products that do not need those signals can leave this closed as not applicable.".into())
            } else {
                None
            },
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
                r#"<script src="https://{}/script.js"></script>"#,
                signature.domain
            );
            let results = AnalyticsCheck.run(&ctx(&body));
            assert_eq!(
                results[0].status,
                CheckStatus::Pass,
                "{} detected by compliance.trackers must count as analytics",
                signature.name
            );
        }
    }

    #[test]
    fn page_without_analytics_still_warns() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = AnalyticsCheck.run(&ctx(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }
}
