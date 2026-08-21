//! Desktop transport for portable HTTPS-enforcement probes.

use crate::checks::{probe, AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::security::https_enforcement::{
    evaluate_https_enforcement, http_origin_request, plan_https_enforcement, HttpsEnforcementStep,
};

pub struct HttpsEnforcementCheck;

#[async_trait::async_trait]
impl AsyncCheck for HttpsEnforcementCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "security.https_enforcement"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let http_url = match plan_https_enforcement(&ctx.url) {
            HttpsEnforcementStep::Done(results) => return results,
            HttpsEnforcementStep::Probe { url } => url,
        };
        let outcome = probe(&ctx.client, http_origin_request(&http_url)).await;
        evaluate_https_enforcement(http_url.as_str(), outcome)
    }

    fn skip_in_predeploy(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    fn ctx_for(url: &str) -> CheckContext {
        CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse(url).expect("static test url"),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: String::new(),
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
    async fn an_http_scan_target_is_skipped_without_any_request() {
        let results = HttpsEnforcementCheck
            .run(&ctx_for("http://example.com/page"))
            .await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("HTTPS was not tested"));
    }

    #[tokio::test]
    async fn an_unreachable_http_origin_is_skipped_not_failed() {
        let results = HttpsEnforcementCheck
            .run(&ctx_for("https://sitecmd-unreachable.invalid/page"))
            .await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("could not obtain"));
    }
}
