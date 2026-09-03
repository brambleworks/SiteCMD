use crate::checks::compliance::{anchor_href_segment_matches, content_text_lower};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

/// Check for CCPA "Do Not Sell" notice
pub struct CcpaNoticeCheck;

impl Check for CcpaNoticeCheck {
    fn id(&self) -> &str {
        "compliance.ccpa_notice"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Only delivered content counts: a notice inside a comment, script, or
        // stylesheet is not one a visitor can act on.
        let content = content_text_lower(ctx.body_lower());

        let has_ccpa = content.contains("do not sell")
            || content.contains("do not share my personal")
            || content.contains("ccpa")
            || content.contains("california consumer privacy")
            || content.contains("california privacy")
            || content.contains("opt-out of sale");

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if has_ccpa {
                "CCPA Notice".into()
            } else {
                "No California privacy opt-out notice found".into()
            },
            description: if has_ccpa {
                "CCPA / California privacy notice detected (\"Do Not Sell\" or related language)."
                    .into()
            } else {
                "No California privacy opt-out notice found. If your business is subject to CCPA/CPRA and sells or shares personal information, provide a clear opt-out path such as \"Do Not Sell or Share My Personal Information\".".into()
            },
            status: if has_ccpa {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: if has_ccpa {
                None
            } else {
                Some("Confirm whether the business is covered by California privacy law and whether any processing qualifies as a sale or sharing. If an opt-out right applies, provide the required method, honor applicable preference signals such as GPC, and make the path discoverable in the notice and interface.".into())
            },
            manual_fix: if has_ccpa {
                None
            } else {
                Some("Confirm whether your business meets CCPA/CPRA applicability thresholds and whether you sell or share personal information. If so, add a clear opt-out link or privacy control path in the footer and privacy policy.".into())
            },
            raw_data: Some(serde_json::json!({
                "california_opt_out_marker_detected": has_ccpa,
                "business_applicability_verified": false,
                "sale_or_sharing_verified": false,
            })),
            confidence: if has_ccpa {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if has_ccpa {
                None
            } else {
                Some("The scanned page lacks a recognized marker, but SiteCMD cannot determine whether the business is covered, whether it sells or shares personal information, or whether the control exists at another URL or in a runtime privacy widget.".into())
            },
            why_it_matters: if has_ccpa {
                None
            } else {
                Some("If the business and processing are covered, an absent or ineffective opt-out path can prevent people from exercising a required right. Applicability must be established before treating this as a violation.".into())
            },
        }]
    }
}

/// Check for an accessibility statement
pub struct AccessibilityStatementCheck;

impl Check for AccessibilityStatementCheck {
    fn id(&self) -> &str {
        "compliance.accessibility_statement"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let content = content_text_lower(ctx.body_lower());

        // The statement itself, named in visible text or linked by path. A
        // WCAG mention alone is a reference to a standard, not a statement.
        let has_statement = content.contains("accessibility statement")
            || content.contains("accessibility policy")
            || content.contains("accessibility commitment")
            || anchor_href_segment_matches(&content, |segment| {
                matches!(
                    segment,
                    "accessibility" | "accessibility-statement" | "accessibility-policy"
                )
            });
        let wcag_reference_only =
            !has_statement && content.contains("wcag") && content.contains("accessibility");

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if has_statement {
                "Accessibility Statement".into()
            } else {
                "No accessibility statement found".into()
            },
            description: if has_statement {
                "Accessibility statement or link detected.".into()
            } else if wcag_reference_only {
                "The page content references WCAG, but no accessibility statement, policy, or link to one was found: a standards reference is not a statement. A statement can document actual conformance, known barriers, and a support path. Some public-sector, procurement, and covered-service regimes require accessibility information, but the required document and content depend on the organization and jurisdiction.".into()
            } else {
                "No accessibility statement marker was found in the scanned page content. A statement can document actual conformance, known barriers, and a support path. Some public-sector, procurement, and covered-service regimes require accessibility information, but the required document and content depend on the organization and jurisdiction.".into()
            },
            status: if has_statement {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: if has_statement {
                None
            } else {
                Some("Confirm the accessibility-information duties that apply to the organization and service. If a statement or equivalent information is appropriate, describe the actual conformance assessment, known limitations, review date, and an accessible barrier-reporting contact; do not claim a WCAG level that has not been evaluated.".into())
            },
            manual_fix: if has_statement {
                None
            } else {
                Some("Check the applicable regime and choose a statement page, terms section, or equivalent accessible document. Describe the service, evaluated standard and scope, known limitations, review date, and a contact method for reporting barriers. State a conformance level only when an assessment supports it.".into())
            },
            raw_data: Some(serde_json::json!({
                "accessibility_statement_marker_detected": has_statement,
                "wcag_reference_only": wcag_reference_only,
                "separate_statement_url_probed": false,
                "applicability_verified": false,
            })),
            confidence: if has_statement {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if has_statement {
                None
            } else {
                Some("This static page check does not probe a separate statement URL or determine the organization, service scope, contracts, or jurisdiction-specific information duty.".into())
            },
            why_it_matters: if has_statement {
                None
            } else {
                Some("Users need a reliable way to report barriers, and some procurement or regulatory contexts expect a published accessibility commitment.".into())
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
    fn do_not_sell_language_passes_ccpa_notice() {
        let body = r#"<html><body><footer><a href="/ccpa-opt-out">Do Not Sell or Share My Personal Information</a></footer></body></html>"#;
        let results = CcpaNoticeCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn page_without_california_optout_warns_low() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = CcpaNoticeCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        assert!(results[0].manual_fix.is_some());
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
    }

    #[test]
    fn accessibility_statement_link_passes() {
        let body = r#"<html><body><footer><a href="/accessibility">Accessibility statement</a></footer></body></html>"#;
        let results = AccessibilityStatementCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn a_wcag_mention_is_a_standards_reference_not_a_statement() {
        let body = "<html><body><p>Our WCAG checklist covers accessibility basics for the kitchen display.</p></body></html>";
        let results = AccessibilityStatementCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(
            results[0]
                .description
                .contains("a standards reference is not a statement"),
            "{}",
            results[0].description
        );
        assert_eq!(
            results[0].raw_data.as_ref().expect("evidence")["wcag_reference_only"],
            true
        );
    }

    #[test]
    fn a_footer_statement_link_passes_on_its_href_segment() {
        let body = r#"<html><body><footer><a href="/legal/accessibility">Our commitment</a></footer></body></html>"#;
        let results = AccessibilityStatementCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].confidence, crate::checks::IssueConfidence::High);
    }

    #[test]
    fn a_commented_out_statement_link_is_not_a_statement() {
        let body = r#"<html><body><!-- <a href="/accessibility">Accessibility statement</a> --></body></html>"#;
        let results = AccessibilityStatementCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn a_commented_out_do_not_sell_link_is_not_a_california_notice() {
        let body = r#"<html><body><!-- <a href="/ccpa">Do Not Sell My Personal Information</a> --></body></html>"#;
        let results = CcpaNoticeCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn page_without_accessibility_statement_warns() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = AccessibilityStatementCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        assert!(results[0].title.contains("No accessibility statement"));
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(!results[0]
            .description
            .contains("requires a published statement"));
    }
}
