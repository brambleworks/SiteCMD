//! Desktop transport for the reflected-origin CORS probe.

use crate::checks::{probe, AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::cors::{
    evaluate_reflection, reflection_localhost_skip_result, reflection_probe_request,
};

// The sync header-configuration verdict lives in the engine; re-export it so
// `cors::CorsCheck` keeps resolving at the desktop registration site.
pub use sitecmd_engine::checks::security::cors::CorsCheck;

/// Probe whether the server reflects a foreign origin, including the
/// higher-risk credentials-enabled variant.
pub struct CorsReflectionProbeCheck;

#[async_trait::async_trait]
impl AsyncCheck for CorsReflectionProbeCheck {
    fn id(&self) -> &str {
        "security.cors_reflection"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        if ctx.is_localhost {
            return reflection_localhost_skip_result();
        }
        let outcome = probe(&ctx.client, reflection_probe_request(ctx.url.as_str())).await;
        evaluate_reflection(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    fn ctx(url: &str, is_localhost: bool) -> CheckContext {
        CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse(url).unwrap(),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: String::new(),
                is_localhost,
                is_strict_localhost: is_localhost,
                http_version: Some("HTTP/2.0".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(is_localhost).clone(),
            probe_cache: Default::default(),
        }
    }

    #[tokio::test]
    async fn localhost_preview_is_skipped_before_any_request() {
        let results = CorsReflectionProbeCheck
            .run(&ctx("http://localhost:3000", true))
            .await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[tokio::test]
    async fn an_unreachable_origin_makes_no_reflection_claim() {
        let results = CorsReflectionProbeCheck
            .run(&ctx("http://127.0.0.1:1", false))
            .await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }
}
