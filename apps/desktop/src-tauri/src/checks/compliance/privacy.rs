//! Runs bounded privacy-policy and terms path probes for engine verdicts.

use crate::checks::{probe, AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::compliance::legal_documents::{
    evaluate_privacy_policy, evaluate_terms, has_terms_link, legal_path_request,
    page_links_privacy_policy, LegalPathSweep, LegalPathWalk, PRIVACY_PATHS, TERMS_PATHS,
};

/// Probe candidate paths in order and record every response before the first hit.
async fn sweep_candidate_paths(
    ctx: &CheckContext,
    paths: &'static [&'static str],
) -> LegalPathSweep {
    let origin = crate::checks::origin_with_port(&ctx.url);
    let mut walk = LegalPathWalk::default();
    for path in paths.iter().copied() {
        let outcome = probe(&ctx.client, legal_path_request(&origin, path)).await;
        if walk.observe(path, &outcome) {
            break;
        }
    }
    walk.finish()
}

pub struct PrivacyPolicyCheck;

#[async_trait::async_trait]
impl AsyncCheck for PrivacyPolicyCheck {
    fn id(&self) -> &str {
        "compliance.privacy_policy"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        // Check if page links to a privacy policy (any covered language).
        if page_links_privacy_policy(ctx.body_lower()) {
            return evaluate_privacy_policy(true, &LegalPathSweep::Unanswered);
        }
        evaluate_privacy_policy(false, &sweep_candidate_paths(ctx, PRIVACY_PATHS).await)
    }
}

pub struct TermsOfServiceCheck;

#[async_trait::async_trait]
impl AsyncCheck for TermsOfServiceCheck {
    fn id(&self) -> &str {
        "compliance.terms"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Compliance
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        if has_terms_link(ctx.body_lower()) {
            return evaluate_terms(true, &LegalPathSweep::Unanswered);
        }
        evaluate_terms(false, &sweep_candidate_paths(ctx, TERMS_PATHS).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{CheckStatus, Severity};

    /// Context whose origin points at a closed loopback port, so the
    /// common-path probes fail immediately (connection refused) instead of
    /// reaching the network. That exercises the not-found fallthrough
    /// deterministically without a test server.
    fn ctx(url: &str, body: &str) -> CheckContext {
        CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse(url).unwrap(),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: body.to_string(),
                is_localhost: false,
                is_strict_localhost: false,
                http_version: Some("HTTP/2.0".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(false).clone(),
            probe_cache: Default::default(),
        }
    }

    #[tokio::test]
    async fn privacy_policy_link_on_page_passes_without_probing() {
        let body =
            r#"<html><body><footer><a href="/legal">Privacy Policy</a></footer></body></html>"#;
        let results = PrivacyPolicyCheck
            .run(&ctx("https://example.com", body))
            .await;
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("link found on the page"));
    }

    /// Start a loopback server that answers every request with `status_line`.
    fn answering_origin(status_line: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let origin = format!("http://{}", listener.local_addr().expect("listener addr"));
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            while let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let response =
                    format!("{status_line}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = stream.write_all(response.as_bytes());
            }
        });
        origin
    }

    #[tokio::test]
    async fn every_candidate_path_answering_404_warns_at_medium() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let origin = answering_origin("HTTP/1.1 404 Not Found");
        let results = PrivacyPolicyCheck.run(&ctx(&origin, body)).await;
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert!(results[0].title.contains("No privacy policy link found"));
        // Three real 404s, so the finding may name all three paths.
        assert!(results[0].description.contains("/legal/privacy"));
    }

    #[tokio::test]
    async fn an_unreachable_origin_declines_to_grade_the_privacy_policy() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = PrivacyPolicyCheck
            .run(&ctx("http://127.0.0.1:1", body))
            .await;
        assert_ne!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].title.contains("did not complete"));
    }

    #[tokio::test]
    async fn a_served_common_path_is_reported_as_the_evidence() {
        // Every probe succeeds, so the FIRST candidate path is the one
        // named in the result - the shell must stop at the first hit.
        let origin = answering_origin("HTTP/1.1 200 OK");

        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = PrivacyPolicyCheck.run(&ctx(&origin, body)).await;
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("/privacy)"));
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
    }

    #[tokio::test]
    async fn terms_link_on_page_passes_without_probing() {
        let body =
            r#"<html><body><footer><a href="/legal">Terms of Service</a></footer></body></html>"#;
        let results = TermsOfServiceCheck
            .run(&ctx("https://example.com", body))
            .await;
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[tokio::test]
    async fn every_candidate_terms_path_answering_404_warns_at_low() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let origin = answering_origin("HTTP/1.1 404 Not Found");
        let results = TermsOfServiceCheck.run(&ctx(&origin, body)).await;
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        assert!(results[0].title.contains("No terms of service"));
    }

    #[tokio::test]
    async fn an_unreachable_origin_declines_to_grade_the_terms() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = TermsOfServiceCheck
            .run(&ctx("http://127.0.0.1:1", body))
            .await;
        assert_ne!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].title.contains("did not complete"));
    }
}
