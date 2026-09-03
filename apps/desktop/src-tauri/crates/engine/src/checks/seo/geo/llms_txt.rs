//! Portable `seo.llms_txt` verdict: grades a classified /llms.txt probe.
//! The optional community proposal is confirmed by endpoint/body presence
//! only; absence and inconclusive probes are never scored as defects.

use crate::checks::{
    looks_like_html_shell, CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity,
};
use crate::probe::{ProbeFailureClass, ProbeOutcome};

/// The canonical llms.txt probe URL for a scanned page.
pub fn llms_txt_url(page_url: &url::Url) -> String {
    format!("{}/llms.txt", crate::checks::origin_with_port(page_url))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmsRepresentation {
    Present,
    Empty,
    HtmlRewrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmsAbsence {
    ConfirmedMissing(u16),
    HtmlRewrite,
}

pub fn classify_llms_representation(content_type: &str, body: &str) -> LlmsRepresentation {
    if looks_like_html_shell(content_type, body) {
        LlmsRepresentation::HtmlRewrite
    } else if body.trim().is_empty() {
        LlmsRepresentation::Empty
    } else {
        LlmsRepresentation::Present
    }
}

/// Grade the `seo.llms_txt` outcome from one classified probe.
pub fn evaluate_llms_txt(outcome: ProbeOutcome) -> Vec<CheckResult> {
    const CHECK_ID: &str = "seo.llms_txt";
    match outcome {
        ProbeOutcome::Response(response) if (200..300).contains(&response.status) => {
            let final_url = crate::log_sanitizer::evidence_safe_page_url(&response.final_url);
            let content_type = response
                .content_type
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase();
            let Some(body) = response.body else {
                return vec![llms_unavailable_result(
                    CHECK_ID,
                    Some(response.status),
                    "The successful response body could not be read within the probe limits.",
                )];
            };

            let representation = classify_llms_representation(&content_type, &body.text);
            if representation == LlmsRepresentation::HtmlRewrite {
                return vec![missing_llms_result(CHECK_ID, LlmsAbsence::HtmlRewrite)];
            }
            let has_content = representation == LlmsRepresentation::Present;

            vec![CheckResult {
                check_id: CHECK_ID.into(),
                category: ScanCategory::Seo,
                title: if has_content {
                    "llms.txt file observed".into()
                } else {
                    "Empty llms.txt response".into()
                },
                description: if has_content {
                    format!(
                        "A nonempty response was found at /llms.txt ({} bytes). llms.txt is an optional community proposal; this check confirms endpoint/body presence only and does not validate the proposed Markdown shape, factual accuracy, adoption by a client, crawling, ranking, or citation behavior.",
                        body.bytes
                    )
                } else {
                    "The /llms.txt endpoint returned a successful response with an empty or whitespace-only body. Because the site chose to publish the optional endpoint, confirm whether it is intentionally empty or an incomplete deployment.".into()
                },
                status: if has_content {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Warn
                },
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: if has_content {
                    None
                } else {
                    Some("If the endpoint is intentional, populate it according to the current llms.txt proposal with concise, public, accurate information and links that remain synchronized with the site. If no supported client/use case justifies it, remove the empty route instead of adding filler. Re-fetch the deployed URL and test the specific client you target.".into())
                },
                raw_data: Some(serde_json::json!({
                    "endpoint_success": true,
                    "usable_text_file_observed": has_content,
                    "representation": if has_content { "nonempty_text" } else { "empty_text" },
                    "bytes": body.bytes,
                    "content_type": content_type,
                    "final_target": final_url,
                    "proposal_conformance_validated": false,
                    "client_support_verified": false,
                })),
                confidence: IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: if has_content {
                    None
                } else {
                    Some("An intentionally published but empty endpoint cannot provide the information its operator intended. Whether a populated file has any effect depends on support by the target client.".into())
                },
            }]
        }
        ProbeOutcome::Response(response) if matches!(response.status, 404 | 410) => vec![
            missing_llms_result(CHECK_ID, LlmsAbsence::ConfirmedMissing(response.status)),
        ],
        ProbeOutcome::Response(response) => vec![llms_unavailable_result(
            CHECK_ID,
            Some(response.status),
            "The endpoint returned a non-success response, so file presence/content was not evaluated.",
        )],
        ProbeOutcome::Failure(failure) => vec![llms_unavailable_result(
            CHECK_ID,
            None,
            match failure.class {
                ProbeFailureClass::Timeout => {
                    "The endpoint request timed out, so file presence/content was not evaluated."
                }
                // Under the success-only body policy a cap overrun means a
                // successful response arrived whose body could not be read.
                ProbeFailureClass::BodyCapExceeded => {
                    "The successful response body could not be read within the probe limits."
                }
                ProbeFailureClass::DnsUnresolved => {
                    "The endpoint's host did not resolve, so file presence/content was not evaluated."
                }
                ProbeFailureClass::Transport => {
                    "The endpoint request failed, so file presence/content was not evaluated."
                }
            },
        )],
    }
}

pub fn missing_llms_result(check_id: &str, absence: LlmsAbsence) -> CheckResult {
    let (title, description, status_code, confirmed_missing, html_rewrite) = match absence {
        LlmsAbsence::ConfirmedMissing(status_code) => (
            "No llms.txt file observed",
            format!(
                "The /llms.txt endpoint returned HTTP {status_code}, so no file was observed at that location. llms.txt is an optional community proposal; absence alone is not a product or SEO defect and no client support is assumed."
            ),
            Some(status_code),
            true,
            false,
        ),
        LlmsAbsence::HtmlRewrite => (
            "llms.txt text file not observed",
            "The /llms.txt endpoint returned a successful HTML document or application-shell rewrite rather than a usable text file. This can be an intentional catch-all route. llms.txt is an optional community proposal, so no product or SEO defect is inferred and no client support is assumed.".into(),
            None,
            false,
            true,
        ),
    };
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Seo,
        title: title.into(),
        description,
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "usable_text_file_observed": false,
            "http_status": status_code,
            "confirmed_missing": confirmed_missing,
            "html_rewrite": html_rewrite,
            "optional_convention": true,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

pub fn llms_unavailable_result(
    check_id: &str,
    status_code: Option<u16>,
    detail: &str,
) -> CheckResult {
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Seo,
        title: "llms.txt not evaluated".into(),
        description: format!(
            "{} llms.txt is optional; this inconclusive probe is not treated as evidence that the file is missing or defective.",
            detail
        ),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "status_code": status_code,
            "probe_conclusive": false,
            "usable_text_file_observed": serde_json::Value::Null,
            "optional_convention": true,
        })),
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some(
            "The endpoint did not return a readable success, so file presence and contents could not be established."
                .into(),
        ),
        why_it_matters: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{ProbeBody, ProbeFailure, ProbeResponse};

    fn success(content_type: &str, text: &str) -> ProbeOutcome {
        ProbeOutcome::Response(ProbeResponse {
            status: 200,
            final_url: "https://example.com/llms.txt".into(),
            content_type: Some(content_type.to_string()),
            content_length: None,
            headers: Vec::new(),
            body: Some(ProbeBody {
                text: text.to_string(),
                bytes: text.len(),
                utf8_valid: true,
            }),
        })
    }

    #[test]
    fn any_nonempty_plain_text_is_not_rejected_by_an_arbitrary_length_threshold() {
        assert_eq!(
            classify_llms_representation("text/plain", "# Acme"),
            LlmsRepresentation::Present
        );
    }

    #[test]
    fn html_catch_all_is_not_treated_as_llms_text() {
        assert_eq!(
            classify_llms_representation(
                "text/html; charset=utf-8",
                "<!doctype html><html><body>App</body></html>"
            ),
            LlmsRepresentation::HtmlRewrite
        );
    }

    #[test]
    fn confirmed_missing_llms_file_has_exact_status_evidence() {
        let result = missing_llms_result("seo.llms_txt", LlmsAbsence::ConfirmedMissing(404));
        let raw = result.raw_data.as_ref().unwrap();
        assert_eq!(result.status, CheckStatus::Skipped);
        assert_eq!(result.title, "No llms.txt file observed");
        assert_eq!(raw["http_status"], 404);
        assert_eq!(raw["confirmed_missing"], true);
        assert_eq!(raw["html_rewrite"], false);
    }

    #[test]
    fn html_rewrite_is_not_described_as_a_confirmed_missing_endpoint() {
        let result = missing_llms_result("seo.llms_txt", LlmsAbsence::HtmlRewrite);
        let raw = result.raw_data.as_ref().unwrap();
        assert_eq!(result.status, CheckStatus::Skipped);
        assert_eq!(result.title, "llms.txt text file not observed");
        assert_eq!(raw["confirmed_missing"], false);
        assert_eq!(raw["html_rewrite"], true);
        assert!(result.description.contains("successful HTML"));
    }

    #[test]
    fn nonempty_text_body_passes_with_bounded_claims() {
        let results = evaluate_llms_txt(success("text/plain", "# Acme\n"));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("does not validate"));
    }

    #[test]
    fn whitespace_only_success_body_is_the_empty_endpoint_warning() {
        let results = evaluate_llms_txt(success("text/plain", "   \n"));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].title.contains("Empty"));
    }

    #[test]
    fn transport_failure_is_inconclusive_not_missing() {
        let results = evaluate_llms_txt(ProbeOutcome::Failure(ProbeFailure {
            class: crate::probe::ProbeFailureClass::Transport,
            detail: "connection refused".into(),
        }));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(results[0].title.contains("not evaluated"));
    }

    #[test]
    fn llms_url_is_origin_scoped() {
        let url = url::Url::parse("https://example.com/blog/post").unwrap();
        assert_eq!(llms_txt_url(&url), "https://example.com/llms.txt");
    }
}
