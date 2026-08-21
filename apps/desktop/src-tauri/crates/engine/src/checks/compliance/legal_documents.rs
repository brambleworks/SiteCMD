//! Privacy and terms checks using page links plus common-path probes.
//! Successful probes are evidence to verify, not proof of document content.

use crate::checks::compliance::has_privacy_policy_link;
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::probe::{ProbeOutcome, ProbeRequest};

/// Common privacy-policy paths, probed in order until one succeeds.
pub const PRIVACY_PATHS: &[&str] = &["/privacy", "/privacy-policy", "/legal/privacy"];

/// Common terms paths, probed in order until one succeeds.
pub const TERMS_PATHS: &[&str] = &["/terms", "/terms-of-service", "/tos", "/legal/terms"];

/// The existence probe for one candidate path: a status-only HEAD, because
/// only reachability is claimed and the body is never inspected.
pub fn legal_path_request(origin: &str, path: &str) -> ProbeRequest {
    ProbeRequest::head(format!("{origin}{path}"))
}

/// Whether a candidate-path probe counts as "something is served here".
/// A failed request is not evidence either way.
pub fn legal_path_found(outcome: &ProbeOutcome) -> bool {
    matches!(outcome, ProbeOutcome::Response(response) if (200..300).contains(&response.status))
}

/// Outcome of an answered-only candidate-path sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalPathSweep {
    /// A candidate path answered successfully. That path is the evidence.
    Served(&'static str),
    /// Answered paths when none served the document.
    NoneServed { probed: Vec<&'static str> },
    /// No candidate path answered. No evidence in either direction.
    Unanswered,
}

/// Accumulate an ordered sweep where the first hit wins and only answers are evidence.
#[derive(Debug, Default)]
pub struct LegalPathWalk {
    served: Option<&'static str>,
    answered: Vec<&'static str>,
}

impl LegalPathWalk {
    /// Fold one outcome, returning whether the first successful path was found.
    pub fn observe(&mut self, path: &'static str, outcome: &ProbeOutcome) -> bool {
        if legal_path_found(outcome) {
            self.served = Some(path);
            return true;
        }
        // Any response counts as an answer: a 404 at /privacy is real
        // evidence that nothing is served there, where a timeout is evidence
        // of nothing at all.
        if matches!(outcome, ProbeOutcome::Response(_)) {
            self.answered.push(path);
        }
        false
    }

    pub fn finish(self) -> LegalPathSweep {
        match self.served {
            Some(path) => LegalPathSweep::Served(path),
            None if self.answered.is_empty() => LegalPathSweep::Unanswered,
            None => LegalPathSweep::NoneServed {
                probed: self.answered,
            },
        }
    }
}

/// True when the (lowercased) page contains a terms-of-service link signal.
pub fn has_terms_link(lower: &str) -> bool {
    lower.contains("terms of service")
        || lower.contains("terms-of-service")
        || lower.contains("terms and conditions")
        || lower.contains("/terms")
        || lower.contains("/tos")
}

/// Grade the page link first, then the answered candidate-path sweep.
pub fn evaluate_privacy_policy(link_in_page: bool, sweep: &LegalPathSweep) -> Vec<CheckResult> {
    if link_in_page {
        return vec![CheckResult {
            check_id: "compliance.privacy_policy".into(),
            category: ScanCategory::Compliance,
            title: "Privacy policy".into(),
            description: "Privacy policy link found on the page.".into(),
            status: CheckStatus::Pass,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }];
    }

    let probed = match sweep {
        LegalPathSweep::Served(path) => {
            return vec![CheckResult {
            check_id: "compliance.privacy_policy".into(),
            category: ScanCategory::Compliance,
            title: "Privacy policy".into(),
            description: format!("A common privacy-policy path ({path}) returned a successful response. Verify that it is an actual policy rather than a generic application route."),
            status: CheckStatus::Pass,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: IssueConfidence::NeedsReview,
            confidence_reason: Some("A successful HEAD response at a common path does not prove the response contains a privacy notice; single-page applications and custom error routes may return success for unknown paths.".into()),
            why_it_matters: None,
        }]
        }
        LegalPathSweep::Unanswered => return vec![unswept_result(
            "compliance.privacy_policy",
            "Privacy policy probe did not complete",
            "No privacy-policy link was detected on this page and none of the common privacy paths returned a response, so this scan could not establish whether a policy is published.",
            Severity::Medium,
        )],
        LegalPathSweep::NoneServed { probed } => probed,
    };

    vec![CheckResult {
        check_id: "compliance.privacy_policy".into(),
        category: ScanCategory::Compliance,
        title: "No privacy policy link found".into(),
        description: format!("No recognizable privacy-policy link was detected on this page, and the common privacy paths that responded ({}) did not return a successful response. If the site collects personal data, manually confirm that a notice is not published at another path or in an unsupported language before treating it as missing.", probed.join(", ")),
        status: CheckStatus::Warn,
        severity: Severity::Medium,
        fix_prompt: Some("Confirm whether the site collects personal data and which notice duties apply. If a notice is required, publish one that matches the actual data, purposes, legal bases, recipients, retention, transfers, and rights, then link it from relevant collection points and persistent navigation.".into()),
        manual_fix: Some("Publish a privacy policy, link it from the footer and any data-collection flow, and make sure it matches the analytics, forms, accounts, and third-party tools you actually use.".into()),
        raw_data: Some(serde_json::json!({
            "privacy_policy_link_detected": false,
            "successful_common_path_detected": false,
            // The paths that ANSWERED, never the candidate table: a path
            // whose probe failed was not tested and must not be listed as
            // evidence that nothing is published there.
            "probed_paths": probed,
            "data_collection_or_applicability_verified": false,
        })),
        // Link detection is a language-limited heuristic; a policy the
        // detector cannot read may still exist.
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some("Link detection covers English, German, French, Spanish, Italian, Portuguese, Dutch, and Swedish link text plus the /privacy, /privacy-policy, and /legal/privacy paths. A policy linked in another language or at an uncommon path would not be detected - check the site footer to confirm.".into()),
        why_it_matters: Some("Where a privacy notice is required, people need a clear place to understand what data is collected, why it is used, who receives it, how long it is kept, and which rights apply.".into()),
    }]
}

/// Grade `compliance.terms` from the in-page link signal and what the
/// candidate-path sweep established. `link_in_page` outranks the sweep, for
/// the reason [`evaluate_privacy_policy`] gives.
pub fn evaluate_terms(link_in_page: bool, sweep: &LegalPathSweep) -> Vec<CheckResult> {
    if link_in_page {
        return vec![CheckResult {
            check_id: "compliance.terms".into(),
            category: ScanCategory::Compliance,
            title: "Terms of service".into(),
            description: "Terms of service link found on the page.".into(),
            status: CheckStatus::Pass,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }];
    }

    let probed = match sweep {
        LegalPathSweep::Served(path) => {
            return vec![CheckResult {
            check_id: "compliance.terms".into(),
            category: ScanCategory::Compliance,
            title: "Terms of service".into(),
            description: format!("A common terms path ({path}) returned a successful response. Verify that it is an actual terms document rather than a generic application route."),
            status: CheckStatus::Pass,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: IssueConfidence::NeedsReview,
            confidence_reason: Some("A successful HEAD response at a common path does not prove the response contains terms; single-page applications and custom error routes may return success for unknown paths.".into()),
            why_it_matters: None,
        }]
        }
        LegalPathSweep::Unanswered => return vec![unswept_result(
            "compliance.terms",
            "Terms of service probe did not complete",
            "No terms of service link was detected on this page and none of the common terms paths returned a response, so this scan could not establish whether terms are published.",
            Severity::Low,
        )],
        LegalPathSweep::NoneServed { probed } => probed,
    };

    vec![CheckResult {
        check_id: "compliance.terms".into(),
        category: ScanCategory::Compliance,
        title: "No terms of service page found".into(),
        description: "No terms of service found. That is often fine for a simple brochure site, but it matters more once you take payments, host accounts, or accept user content.".into(),
        status: CheckStatus::Warn,
        severity: Severity::Low,
        fix_prompt: Some("Determine whether accounts, payments, subscriptions, user content, or another relationship needs contractual terms in the relevant jurisdictions. If so, draft terms that match the real service and obtain appropriate legal review; do not add generic boilerplate solely to clear this finding.".into()),
        manual_fix: Some("Add a terms page if this site handles accounts, payments, subscriptions, or user content, and link it from the footer.".into()),
        raw_data: Some(serde_json::json!({
            "terms_link_detected": false,
            "successful_common_path_detected": false,
            // The paths that ANSWERED, for the reason the privacy row gives.
            "probed_paths": probed,
            "business_model_or_applicability_verified": false,
        })),
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some("SiteCMD checks only English terms markers and a few common paths, and it cannot determine the site's business model, jurisdiction, or whether contractual terms are required.".into()),
        why_it_matters: Some("When a site forms an ongoing customer or user relationship, accurate terms can define service rules, payment/refund expectations, and dispute processes. A brochure site may not need the same document.".into()),
    }]
}

/// Skipped verdict when neither a page link nor a candidate path produced evidence.
fn unswept_result(
    check_id: &str,
    title: &str,
    description: &str,
    severity: Severity,
) -> CheckResult {
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Compliance,
        title: title.into(),
        description: description.into(),
        status: CheckStatus::Skipped,
        severity,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "probed_paths": Vec::<&str>::new(),
            "reason": "no_candidate_path_responded",
        })),
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some(
            "None of the candidate paths returned a response, so this result establishes neither the presence nor the absence of the document. Re-scan once the origin is reachable.".into(),
        ),
        why_it_matters: None,
    }
}

/// Convenience for callers that hold the lowercased page body: the privacy
/// link signal, shared with the GDPR data-controller check.
pub fn page_links_privacy_policy(lower: &str) -> bool {
    has_privacy_policy_link(lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{ProbeFailure, ProbeFailureClass, ProbeResponse};

    fn head_response(status: u16) -> ProbeOutcome {
        ProbeOutcome::Response(ProbeResponse {
            status,
            final_url: String::new(),
            content_type: None,
            content_length: None,
            headers: Vec::new(),
            body: None,
        })
    }

    #[test]
    fn the_path_probe_is_a_status_only_head() {
        let request = legal_path_request("https://example.com", "/privacy");
        assert_eq!(request.url, "https://example.com/privacy");
        assert_eq!(request.method, crate::probe::ProbeMethod::Head);
        assert_eq!(request.body, crate::probe::BodyPolicy::None);
    }

    #[test]
    fn only_a_successful_response_counts_as_a_served_path() {
        assert!(legal_path_found(&head_response(200)));
        assert!(!legal_path_found(&head_response(404)));
        assert!(!legal_path_found(&head_response(500)));
        assert!(!legal_path_found(&ProbeOutcome::Failure(ProbeFailure {
            class: ProbeFailureClass::Transport,
            detail: "connection refused".into(),
        })));
    }

    #[test]
    fn european_language_privacy_links_count_as_privacy_policy() {
        for footer in [
            "politique de confidentialité", // French
            "política de privacidad",       // Spanish
            "informativa sulla privacy",    // Italian
            "política de privacidade",      // Portuguese
            "privacybeleid",                // Dutch
            "integritetspolicy",            // Swedish
            "datenschutz",                  // German
        ] {
            assert!(
                page_links_privacy_policy(footer),
                "footer link '{footer}' must count as a privacy policy link"
            );
        }
    }

    #[test]
    fn terms_link_signals_cover_text_and_path_forms() {
        assert!(has_terms_link("terms of service"));
        assert!(has_terms_link("terms and conditions"));
        assert!(has_terms_link(r#"<a href="/tos">legal</a>"#));
        assert!(!has_terms_link("<a href=\"/about\">about us</a>"));
    }

    /// Walk a candidate list against one outcome per path, the way every
    /// caller does.
    fn sweep(paths: &'static [&'static str], outcomes: &[ProbeOutcome]) -> LegalPathSweep {
        let mut walk = LegalPathWalk::default();
        for (path, outcome) in paths.iter().copied().zip(outcomes) {
            if walk.observe(path, outcome) {
                break;
            }
        }
        walk.finish()
    }

    fn failed() -> ProbeOutcome {
        ProbeOutcome::Failure(ProbeFailure {
            class: ProbeFailureClass::Transport,
            detail: "connection refused".into(),
        })
    }

    #[test]
    fn the_walk_separates_nothing_served_from_nothing_answered() {
        assert_eq!(
            sweep(PRIVACY_PATHS, &[failed(), failed(), failed()]),
            LegalPathSweep::Unanswered
        );
        assert_eq!(
            sweep(
                PRIVACY_PATHS,
                &[head_response(404), head_response(404), head_response(404)]
            ),
            LegalPathSweep::NoneServed {
                probed: vec!["/privacy", "/privacy-policy", "/legal/privacy"],
            }
        );
        // A failed path is dropped from the evidence rather than counted as
        // "nothing served here".
        assert_eq!(
            sweep(
                PRIVACY_PATHS,
                &[head_response(404), failed(), head_response(404)]
            ),
            LegalPathSweep::NoneServed {
                probed: vec!["/privacy", "/legal/privacy"],
            }
        );
        // The first hit wins and stops the walk.
        assert_eq!(
            sweep(
                PRIVACY_PATHS,
                &[head_response(200), head_response(200), head_response(200)]
            ),
            LegalPathSweep::Served("/privacy")
        );
    }

    #[test]
    fn an_in_page_link_passes_at_high_confidence() {
        let results = evaluate_privacy_policy(true, &LegalPathSweep::Unanswered);
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].confidence, IssueConfidence::High);
        assert!(results[0].description.contains("link found on the page"));
    }

    #[test]
    fn a_common_path_hit_passes_but_needs_review() {
        let results = evaluate_privacy_policy(false, &LegalPathSweep::Served("/privacy"));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(results[0].description.contains("/privacy"));
        assert!(results[0].description.contains("Verify"));
    }

    #[test]
    fn missing_privacy_policy_warns_at_medium_pending_applicability_review() {
        let results = evaluate_privacy_policy(
            false,
            &sweep(
                PRIVACY_PATHS,
                &[head_response(404), head_response(404), head_response(404)],
            ),
        );
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert!(results[0].title.contains("No privacy policy link found"));
        // Detection is language-limited, so the fired result must not
        // claim certainty.
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(results[0]
            .confidence_reason
            .as_deref()
            .unwrap()
            .contains("French"));
        assert!(results[0].fix_prompt.is_some());
    }

    #[test]
    fn missing_terms_warns_low_pending_business_model_review() {
        let results = evaluate_terms(
            false,
            &sweep(
                TERMS_PATHS,
                &[
                    head_response(404),
                    head_response(404),
                    head_response(404),
                    head_response(404),
                ],
            ),
        );
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        assert_eq!(results[0].confidence, IssueConfidence::NeedsReview);
        assert!(results[0].fix_prompt.is_some());
        assert!(results[0].title.contains("No terms of service"));
    }

    #[test]
    fn terms_link_and_path_hits_pass_at_their_own_confidences() {
        assert_eq!(
            evaluate_terms(true, &LegalPathSweep::Unanswered)[0].confidence,
            IssueConfidence::High
        );
        let path_hit = evaluate_terms(false, &LegalPathSweep::Served("/tos"));
        assert_eq!(path_hit[0].status, CheckStatus::Pass);
        assert_eq!(path_hit[0].confidence, IssueConfidence::NeedsReview);
        assert!(path_hit[0].description.contains("/tos"));
    }

    #[test]
    fn a_sweep_nothing_answered_declines_to_grade_either_document() {
        for results in [
            evaluate_privacy_policy(false, &LegalPathSweep::Unanswered),
            evaluate_terms(false, &LegalPathSweep::Unanswered),
        ] {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].status, CheckStatus::Skipped);
            assert!(results[0].title.contains("did not complete"));
            assert!(results[0].fix_prompt.is_none());
        }
    }

    #[test]
    fn a_partially_answered_sweep_still_warns_and_names_what_answered() {
        let results = evaluate_privacy_policy(
            false,
            &sweep(PRIVACY_PATHS, &[head_response(404), failed(), failed()]),
        );
        assert_eq!(results[0].status, CheckStatus::Warn);
        // One real 404 is real evidence, and the copy claims that path only.
        assert!(results[0].description.contains("(/privacy)"));
        assert!(!results[0].description.contains("/legal/privacy"));
        assert_eq!(
            results[0].raw_data.as_ref().expect("raw data")["probed_paths"],
            serde_json::json!(["/privacy"])
        );
    }
}
