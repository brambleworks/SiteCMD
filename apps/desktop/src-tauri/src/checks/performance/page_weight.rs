//! Adapts fetched HTML byte counts to the portable page-weight verdict.

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::performance::page_weight;

pub struct PageWeightCheck;

#[async_trait::async_trait]
impl AsyncCheck for PageWeightCheck {
    fn id(&self) -> &str {
        "performance.page_weight"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        vec![page_weight::html_size_result(ctx.body.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    #[tokio::test]
    async fn grades_the_fetched_body_under_its_registered_id() {
        let ctx = crate::checks::CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse("https://example.com/").expect("static test url"),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: "<html><body>small</body></html>".into(),
                is_localhost: false,
                is_strict_localhost: false,
                http_version: Some("HTTP/2.0".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(false).clone(),
            probe_cache: Default::default(),
        };
        let results = PageWeightCheck.run(&ctx).await;
        assert_eq!(results[0].check_id, PageWeightCheck.id());
        assert_eq!(results[0].status, CheckStatus::Pass);
    }
}
