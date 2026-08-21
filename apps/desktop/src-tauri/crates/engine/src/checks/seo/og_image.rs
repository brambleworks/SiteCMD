//! Plan and grade bounded probes for absolute Open Graph image URLs.
//! This check records status and content type, not platform rendering behavior.

use crate::checks::seo::parsing::extract_meta;
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::probe::ProbeOutcome;

/// What the runtime should do after reading the page's og:image state:
/// either the verdict is already complete (nothing probeable), or the one
/// absolute image URL needs a bounded status probe.
pub enum OgImageStep {
    Done(Vec<CheckResult>),
    Probe { value: String, url: url::Url },
}

/// Why the runtime never executed the planned probe. The runtime keeps its
/// network policy; the refusal itself is graded here so both runtimes agree
/// on what an unprobed target means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OgImageProbeSkip {
    Disallowed,
}

fn skipped(description: &str) -> Vec<CheckResult> {
    vec![CheckResult {
        check_id: "seo.og_image_status".into(),
        category: ScanCategory::Seo,
        title: "Open Graph image".into(),
        description: description.into(),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }]
}

/// Decide whether the page has an absolute og:image worth probing.
pub fn plan_og_image(body: &str) -> OgImageStep {
    let Some(raw) = extract_meta(body, "og:image") else {
        // No og:image is seo.open_graph's finding, not a broken image.
        return OgImageStep::Done(skipped("No og:image tag to verify."));
    };
    let value = raw.trim().to_string();
    let lower = value.to_ascii_lowercase();
    // A non-absolute value is seo.og_image_relative's finding; probing a
    // relative path against the page URL would mask that distinct issue.
    if value.is_empty() || !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return OgImageStep::Done(skipped("og:image is not an absolute URL; the resolvable check only probes absolute URLs (seo.og_image_relative covers the relative case)."));
    }
    let Ok(url) = url::Url::parse(&value) else {
        return OgImageStep::Done(skipped("og:image value did not parse as a URL."));
    };
    OgImageStep::Probe { value, url }
}

/// Grade the planned probe's outcome (or the runtime's refusal to run it).
pub fn evaluate_og_image(
    value: &str,
    outcome: Result<ProbeOutcome, OgImageProbeSkip>,
) -> Vec<CheckResult> {
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(OgImageProbeSkip::Disallowed) => {
            return skipped("The og:image target is outside the scanner's permitted page-subresource network policy, so it was not requested.");
        }
    };
    match outcome {
        ProbeOutcome::Response(response) => vec![graded_result(
            value,
            response.status,
            response.content_type.as_deref(),
        )],
        ProbeOutcome::Failure(_) => skipped("Could not reach the og:image URL to verify it."),
    }
}

/// Fail direct 404/410 responses; ambiguous statuses remain review states.
fn graded_result(value: &str, status: u16, content_type: Option<&str>) -> CheckResult {
    let safe_value = crate::log_sanitizer::evidence_safe_page_url(value);
    let missing = matches!(status, 404 | 410);
    let image_type = content_type.is_some_and(|header| {
        header
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase()
            .starts_with("image/")
    });
    let usable_response = (200..300).contains(&status) && status != 204 && image_type;
    let needs_review = !missing && !usable_response;
    CheckResult {
        check_id: "seo.og_image_status".into(),
        category: ScanCategory::Seo,
        title: if missing {
            format!("Open Graph image returned HTTP {}", status)
        } else if needs_review {
            "Open Graph image response needs review".into()
        } else {
            "Open Graph image response".into()
        },
        description: if missing {
            format!(
                "The og:image target ({}) returned HTTP {} to this scanner. A social crawler receiving the same response cannot fetch an image, but responses can vary by time, user agent, region, and access policy.",
                safe_value, status
            )
        } else if needs_review {
            format!(
                "The og:image target ({}) returned HTTP {} with Content-Type '{}'. That does not confirm a usable image response, and a social crawler's response may differ. Verify the exact platform path before changing bot rules, authentication, or the asset.",
                safe_value,
                status,
                content_type.unwrap_or("not provided")
            )
        } else {
            format!(
                "The og:image target ({}) returned HTTP {} with an image Content-Type. This probe does not decode the bytes, validate dimensions/content, or confirm that any social platform will render a preview.",
                safe_value, status
            )
        },
        status: if missing {
            CheckStatus::Fail
        } else if needs_review {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        },
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: if missing {
            Some("Confirm the exact deployed URL and response from the intended public environment, correct a stale path or missing asset when present, and then use each supported platform's current preview debugger or rescrape tool. Account for CDN cache and user-agent/region-specific behavior.".into())
        } else if needs_review {
            Some("Test the page with the current preview/debugger for each supported social platform and inspect the image response it receives. If it fails there too, correct the status, image Content-Type/body, redirects, access policy, or metadata URL as the evidence requires; do not broadly allowlist crawlers solely from this probe.".into())
        } else {
            None
        },
        raw_data: Some(serde_json::json!({
            "og_image": safe_value,
            "status_code": status,
            "content_type": content_type,
            "image_body_decoded": false,
            "platform_preview_verified": false,
        })),
        confidence: if needs_review {
            IssueConfidence::NeedsReview
        } else {
            IssueConfidence::High
        },
        confidence_reason: if needs_review {
            Some("The status and Content-Type are directly observed, but crawler-specific responses, transient failures, missing/sniffed media types, and image decoding were not resolved by this probe.".into())
        } else {
            None
        },
        why_it_matters: if missing {
            Some("If supported social crawlers receive the same missing response, they cannot use this image in the page's shared-link preview.".into())
        } else if needs_review {
            Some("If the response is also unusable for supported social crawlers, the intended preview image may be omitted. This scanner response alone does not establish that outcome.".into())
        } else {
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{ProbeFailure, ProbeFailureClass, ProbeResponse};

    #[test]
    fn no_og_image_completes_without_a_probe() {
        let OgImageStep::Done(results) = plan_og_image("<html><head></head></html>") else {
            panic!("no og:image must not plan a probe");
        };
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[test]
    fn relative_og_image_defers_to_the_relative_check() {
        // A relative value is seo.og_image_relative's finding; this check must
        // not probe it (probing against the page URL would mask that issue).
        let html = r#"<meta property="og:image" content="/social/card.png">"#;
        let OgImageStep::Done(results) = plan_og_image(html) else {
            panic!("relative og:image must not plan a probe");
        };
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("absolute"));
    }

    #[test]
    fn absolute_og_image_plans_a_probe() {
        let html = r#"<meta property="og:image" content="https://example.com/card.png">"#;
        let OgImageStep::Probe { value, url } = plan_og_image(html) else {
            panic!("absolute og:image must plan a probe");
        };
        assert_eq!(value, "https://example.com/card.png");
        assert_eq!(url.as_str(), "https://example.com/card.png");
    }

    #[test]
    fn hard_404_is_broken_with_high_confidence() {
        let result = graded_result(
            "https://example.com/card.png?token=secret",
            404,
            Some("text/html"),
        );
        assert_eq!(result.status, CheckStatus::Fail);
        assert_eq!(result.confidence, IssueConfidence::High);
        assert!(result.title.contains("returned HTTP 404"));
        let raw = result.raw_data.unwrap().to_string();
        assert!(raw.contains("/card.png"), "{raw}");
        assert!(!raw.contains("token=secret"), "{raw}");
    }

    #[test]
    fn bot_gated_403_and_429_are_hedged_needs_review() {
        for status in [403u16, 429] {
            let result = graded_result("https://example.com/card.png", status, Some("text/html"));
            assert_eq!(result.status, CheckStatus::Warn, "status {status}");
            assert_eq!(result.confidence, IssueConfidence::NeedsReview);
            assert!(
                !result.title.contains("does not load")
                    && result.description.contains("may differ"),
                "copy must hedge for {status}: {}",
                result.description
            );
        }
    }

    #[test]
    fn server_errors_and_non_image_successes_need_review() {
        for (status, content_type) in [
            (500, Some("text/html")),
            (200, Some("text/html")),
            (204, None),
        ] {
            let result = graded_result("https://example.com/card.png", status, content_type);
            assert_eq!(result.status, CheckStatus::Warn, "status {status}");
            assert_eq!(result.confidence, IssueConfidence::NeedsReview);
        }
    }

    #[test]
    fn image_typed_success_passes_with_bounded_claim() {
        let result = graded_result("https://example.com/card.png", 200, Some("image/png"));
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.description.contains("does not decode"));
        assert!(!result.description.contains("social previews can load"));
    }

    #[test]
    fn transport_failure_is_a_skip_not_a_broken_image() {
        let results = evaluate_og_image(
            "https://example.com/card.png",
            Ok(ProbeOutcome::Failure(ProbeFailure {
                class: ProbeFailureClass::Transport,
                detail: "connection refused".into(),
            })),
        );
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("Could not reach"));
    }

    #[test]
    fn disallowed_target_is_never_graded_as_available() {
        let results = evaluate_og_image(
            "https://internal.example/card.png",
            Err(OgImageProbeSkip::Disallowed),
        );
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("network policy"));
    }

    #[test]
    fn probed_response_grades_from_status_and_content_type() {
        let results = evaluate_og_image(
            "https://example.com/card.png",
            Ok(ProbeOutcome::Response(ProbeResponse {
                status: 200,
                final_url: "https://example.com/card.png".into(),
                content_type: Some("image/webp".into()),
                content_length: None,
                headers: Vec::new(),
                body: None,
            })),
        );
        assert_eq!(results[0].status, CheckStatus::Pass);
    }
}
