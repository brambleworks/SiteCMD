//! Source-level `fetchpriority` hint requiring browser confirmation.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
pub struct PageSpeedHintsCheck;

impl Check for PageSpeedHintsCheck {
    fn id(&self) -> &str {
        "seo.page_speed_hints"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let image_tags = crate::checks::html_attrs::tag_slices(&ctx.body, ctx.body_lower(), "img");
        let image_count = image_tags.len();
        let first_eager = image_tags.into_iter().find(|tag| {
            !crate::checks::html_attrs::attr_value(tag, "loading")
                .is_some_and(|value| value.eq_ignore_ascii_case("lazy"))
        });
        let candidate_src = first_eager
            .and_then(|tag| crate::checks::html_attrs::attr_value(tag, "src"))
            .filter(|value| !value.trim().is_empty());
        let priority = first_eager
            .and_then(|tag| crate::checks::html_attrs::attr_value(tag, "fetchpriority"))
            .filter(|value| !value.trim().is_empty());
        let high_priority = priority
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("high"));
        let has_candidate = first_eager.is_some();
        let needs_review = has_candidate && !high_priority;
        let safe_candidate = candidate_src
            .as_deref()
            .map(crate::log_sanitizer::evidence_safe_url_reference);

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if high_priority {
                "Image fetch priority hint".into()
            } else if needs_review {
                "Potential LCP image priority needs review".into()
            } else {
                "Image fetch priority hint".into()
            },
            description: if high_priority {
                "The first non-lazy <img> in source order has fetchpriority=high. That confirms the markup hint only; it does not prove this image is the rendered LCP element or that the hint improves the measured load.".into()
            } else if needs_review {
                format!(
                    "The first non-lazy <img> in source order{} does not have fetchpriority=high. Source order is only a candidate heuristic: the actual LCP element may be text, a background image, a later responsive candidate, or already prioritized adequately by the browser. Confirm in a production browser trace before changing markup.",
                    safe_candidate.as_deref().map(|src| format!(" ({})", src)).unwrap_or_default()
                )
            } else {
                format!(
                    "No non-lazy <img> candidate was found among {} image {}. This source check therefore has no image-priority recommendation; it does not assess text or CSS-background LCP elements.",
                    image_count,
                    if image_count == 1 { "tag" } else { "tags" }
                )
            },
            status: if needs_review {
                CheckStatus::Warn
            } else {
                CheckStatus::Pass
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if needs_review {
                Some("Record a production-like browser performance trace and identify the actual LCP element and selected responsive image candidate. If an above-the-fold image is the LCP resource and discovery/priority is late, add `fetchpriority=\"high\"` to that image (and avoid lazy-loading it); consider a matching preload only when the trace shows discovery is the bottleneck. Re-measure because unnecessary high-priority hints can compete with more important resources.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "image_tag_count": image_count,
                "first_eager_image_src": safe_candidate,
                "first_eager_fetchpriority": priority,
                "rendered_lcp_verified": false,
                "selected_responsive_candidate_verified": false,
            })),
            confidence: if needs_review {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: if needs_review {
                Some("The missing attribute is directly observed on a source-order candidate, but the rendered LCP element, browser scheduling, selected srcset candidate, and performance impact were not measured.".into())
            } else {
                None
            },
            why_it_matters: if needs_review {
                Some("If browser measurement confirms that this image is the LCP resource and is discovered or prioritized late, an accurate priority hint can improve its scheduling. This source heuristic does not establish that condition.".into())
            } else {
                None
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn eager_hero_image_without_hints_warns() {
        let body = r#"<html><body><img src="/hero.jpg" alt="Hero"><p>Welcome</p></body></html>"#;
        let results = PageSpeedHintsCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("fetchpriority"));
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["first_eager_image_src"],
            "/hero.jpg"
        );
    }

    #[test]
    fn fetchpriority_high_on_the_hero_passes() {
        let body =
            r#"<html><body><img src="/hero.jpg" fetchpriority="high" alt="Hero"></body></html>"#;
        let results = PageSpeedHintsCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("fetchpriority"));
    }

    #[test]
    fn unrelated_preload_does_not_clear_the_image_review_hint() {
        let body = r#"<html><head><link rel="preload" href="/hero.jpg" as="image"></head>
            <body><img src="/hero.jpg" alt="Hero"></body></html>"#;
        let results = PageSpeedHintsCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].description.contains("production browser trace"));
    }

    #[test]
    fn page_without_images_passes_with_nothing_to_hint() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = PageSpeedHintsCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0]
            .description
            .contains("no image-priority recommendation"));
    }

    #[test]
    fn lazy_images_are_skipped_when_finding_the_hero() {
        // The first eager image is the LCP candidate; a lazy image above it
        // must not satisfy (or dodge) the hero-hint requirement.
        let body = r#"<html><body>
            <img src="/footer-logo.png" loading="lazy" alt="">
            <img src="/hero.jpg" alt="Hero">
        </body></html>"#;
        let results = PageSpeedHintsCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("first non-lazy <img>"));
    }

    #[test]
    fn lcp_claims_are_hedged_not_asserted() {
        let body = r#"<html><body><img src="/hero.jpg" alt="Hero"></body></html>"#;
        let results = PageSpeedHintsCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(
            results[0].description.contains("candidate heuristic"),
            "{}",
            results[0].description
        );
        let why = results[0].why_it_matters.as_deref().unwrap_or("");
        assert!(
            why.contains("If browser measurement confirms")
                && !why.contains("hurting Core Web Vitals"),
            "why_it_matters must hedge: {why}"
        );
    }

    #[test]
    fn fetchpriority_text_in_another_attribute_does_not_count() {
        let body = r#"<img src="/hero.jpg" alt="Add fetchpriority=high later">"#;
        let result = &PageSpeedHintsCheck.run(&ctx_with_body(body))[0];
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(
            result.raw_data.as_ref().unwrap()["first_eager_fetchpriority"],
            serde_json::Value::Null
        );
    }
}
