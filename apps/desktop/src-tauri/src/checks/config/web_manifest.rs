//! Desktop transport and network policy for web-manifest probes.

use crate::checks::{probe, AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::config::web_manifest::{
    evaluate_web_manifest, manifest_request, plan_web_manifest, WebManifestProbeSkip,
    WebManifestStep,
};

pub struct WebManifestCheck;

#[async_trait::async_trait]
impl AsyncCheck for WebManifestCheck {
    fn id(&self) -> &str {
        "config.web_manifest"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Polish
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let (safe_href, manifest_url) = match plan_web_manifest(&ctx.body, &ctx.url) {
            WebManifestStep::Done(results) => return results,
            WebManifestStep::Probe { safe_href, url } => (safe_href, url),
        };

        if crate::network_policy::validate_page_subresource_target(
            &manifest_url,
            ctx.is_strict_localhost,
        )
        .is_err()
        {
            return evaluate_web_manifest(
                &safe_href,
                Err(WebManifestProbeSkip::Disallowed {
                    safe_url: crate::log_sanitizer::evidence_safe_page_url(manifest_url.as_str()),
                }),
            );
        }

        let outcome = probe(&ctx.client, manifest_request(&manifest_url)).await;
        evaluate_web_manifest(&safe_href, Ok(outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    fn ctx(body: &str) -> CheckContext {
        CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse("https://example.com/page").expect("static test url"),
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
    async fn a_page_without_a_manifest_passes_without_any_request() {
        let results = WebManifestCheck
            .run(&ctx("<html><head></head></html>"))
            .await;
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("No web app manifest"));
    }

    #[tokio::test]
    async fn a_policy_refused_manifest_target_is_skipped_without_a_request() {
        // A cloud-metadata target must be graded as refused, not probed.
        let body = r#"<link rel="manifest" href="http://169.254.169.254/m.json">"#;
        let results = WebManifestCheck.run(&ctx(body)).await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("network policy"));
    }
}
