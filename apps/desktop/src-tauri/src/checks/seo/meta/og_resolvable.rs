//! Desktop transport for the engine's bounded Open Graph image probe.

use crate::checks::{probe, AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::seo::og_image::{
    evaluate_og_image, plan_og_image, OgImageProbeSkip, OgImageStep,
};
use sitecmd_engine::probe::{BodyPolicy, ProbeRequest};

pub struct OgImageResolvableCheck;

#[async_trait::async_trait]
impl AsyncCheck for OgImageResolvableCheck {
    fn id(&self) -> &str {
        "seo.og_image_status"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let (value, image_url) = match plan_og_image(&ctx.body) {
            OgImageStep::Done(results) => return results,
            OgImageStep::Probe { value, url } => (value, url),
        };
        if crate::network_policy::validate_page_subresource_target(
            &image_url,
            ctx.subordinate_policy(),
        )
        .is_err()
        {
            return evaluate_og_image(&value, Err(OgImageProbeSkip::Disallowed));
        }
        let request = ProbeRequest::get(image_url.as_str()).body(BodyPolicy::None);
        evaluate_og_image(&value, Ok(probe(&ctx.client, request).await))
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
    async fn no_og_image_is_skipped_without_a_probe() {
        let results = OgImageResolvableCheck
            .run(&ctx("<html><head></head></html>"))
            .await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[tokio::test]
    async fn relative_og_image_defers_to_the_relative_check() {
        // A relative value is seo.og_image_relative's finding; this check must
        // not probe it (probing against the page URL would mask that issue).
        let html = r#"<meta property="og:image" content="/social/card.png">"#;
        let results = OgImageResolvableCheck.run(&ctx(html)).await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("absolute"));
    }

    #[tokio::test]
    async fn policy_refused_target_is_skipped_without_a_request() {
        // A cloud-metadata target must be graded as refused by policy, not
        // probed. The policy gate stays in the shell; the copy is engine-pinned.
        let html = r#"<meta property="og:image" content="http://169.254.169.254/card.png">"#;
        let results = OgImageResolvableCheck.run(&ctx(html)).await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("network policy"));
    }
}
