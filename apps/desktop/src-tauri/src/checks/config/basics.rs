//! Desktop transports for site-configuration probes.
//! The engine owns plans and verdicts; these adapters enforce network policy.

use crate::checks::{probe, AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::config::{alt_host, favicon, missing_page};

// The sync analytics-detection check lives in the engine; re-export it so the
// `basics::AnalyticsCheck` registration path keeps resolving.
pub use sitecmd_engine::checks::config::analytics::AnalyticsCheck;

pub struct FaviconCheck;

/// Run one planned favicon probe through the desktop's page-subresource
/// network policy. A disallowed target is never requested.
async fn run_favicon_probe(
    ctx: &CheckContext,
    url: &str,
) -> Result<sitecmd_engine::probe::ProbeOutcome, favicon::FaviconProbeSkip> {
    let Ok(parsed) = url::Url::parse(url) else {
        return Err(favicon::FaviconProbeSkip::Failed);
    };
    if crate::network_policy::validate_page_subresource_target(&parsed, ctx.subordinate_policy())
        .is_err()
    {
        return Err(favicon::FaviconProbeSkip::Disallowed);
    }
    Ok(probe(&ctx.client, favicon::favicon_probe_request(url)).await)
}

#[async_trait::async_trait]
impl AsyncCheck for FaviconCheck {
    fn id(&self) -> &str {
        "config.favicon"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Polish
    }
    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let base = crate::checks::origin_with_port(&ctx.url);
        let step = favicon::plan_favicon(&ctx.body, &base, |href| {
            ctx.url.join(href).ok().map(|url| url.to_string())
        });
        match step {
            favicon::FaviconStep::Done(results) => results,
            favicon::FaviconStep::ProbeDeclared { url, safe_href } => {
                let outcome = run_favicon_probe(ctx, &url).await;
                favicon::evaluate_declared(&safe_href, &url, outcome)
            }
            favicon::FaviconStep::ProbeFallback { url } => {
                let outcome = run_favicon_probe(ctx, &url).await;
                favicon::evaluate_fallback(outcome)
            }
        }
    }
}

pub struct Custom404Check;

#[async_trait::async_trait]
impl AsyncCheck for Custom404Check {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "config.custom_404"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Polish
    }
    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        if ctx.is_localhost {
            return vec![missing_page::localhost_skip_result()];
        }
        let base = crate::checks::origin_with_port(&ctx.url);
        let outcome = probe(&ctx.client, missing_page::missing_page_probe_request(&base)).await;
        missing_page::evaluate_missing_page(outcome)
    }
}

pub struct WwwRedirectCheck;

#[async_trait::async_trait]
impl AsyncCheck for WwwRedirectCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "config.www_redirect"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }
    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let alternate = alt_host::alternate_host(ctx.url.host_str().unwrap_or(""));
        let request = alt_host::alt_host_probe_request(ctx.url.scheme(), &alternate);
        let outcome = probe(&ctx.client, request).await;
        alt_host::evaluate_alt_host(&alternate, outcome)
    }
    fn skip_in_predeploy(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    fn ctx(url: &str, body: &str, is_localhost: bool) -> CheckContext {
        CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse(url).unwrap(),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: body.to_string(),
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
    async fn an_inline_favicon_needs_no_probe() {
        let html = r#"<link rel="icon" href="data:image/svg+xml,<svg/>">"#;
        let results = FaviconCheck
            .run(&ctx("https://example.com", html, false))
            .await;
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("inline image data URI"));
    }

    #[tokio::test]
    async fn an_unreachable_declared_icon_is_skipped_not_failed() {
        // A closed loopback port fails the probe, exercising the shell's
        // policy gate, probe execution, and the engine's no-claim verdict.
        let html = r#"<link rel="icon" href="/favicon.svg">"#;
        let results = FaviconCheck
            .run(&ctx("http://127.0.0.1:1", html, false))
            .await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[tokio::test]
    async fn custom_404_is_skipped_on_localhost_preview() {
        let results = Custom404Check
            .run(&ctx("http://localhost:3000", "", true))
            .await;
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("localhost preview"));
    }

    #[tokio::test]
    async fn an_unresponsive_alternate_host_produces_no_verdict() {
        let results = WwwRedirectCheck
            .run(&ctx("http://127.0.0.1:1", "", false))
            .await;
        assert_ne!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("did not complete"));
    }
}
