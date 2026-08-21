//! Desktop transport shell for the engine's sitemap verdict.

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::seo::sitemap::evaluate_sitemap;

pub struct SitemapCheck;

#[async_trait::async_trait]
impl AsyncCheck for SitemapCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "seo.sitemap"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let robots = ctx.robots_txt().await;
        let probe = ctx.sitemap().await;
        evaluate_sitemap(&ctx.page, robots, probe)
    }
}

#[cfg(test)]
mod tests {
    use crate::checks::{
        AsyncCheck, CheckContext, CheckStatus, RobotsTxtFetch, SitemapProbe,
        SitemapProbeObservation,
    };

    fn ctx_with(sitemap: SitemapProbe, robots: RobotsTxtFetch) -> CheckContext {
        let ctx = CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse("https://example.com").unwrap(),
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
        };
        assert!(ctx.probe_cache.sitemap.set(sitemap).is_ok());
        assert!(ctx.probe_cache.robots_txt.set(robots).is_ok());
        ctx
    }

    #[tokio::test]
    async fn shell_grades_the_seeded_probes_through_the_engine() {
        // The verdict itself is pinned by the engine tests and the golden
        // corpus; this proves the shell hands both memoized fetches over.
        let probe = SitemapProbe::Missing {
            observations: vec![SitemapProbeObservation {
                url: "https://example.com/sitemap.xml".into(),
                outcome: "HTTP 404".into(),
            }],
        };
        let robots = RobotsTxtFetch::Found {
            body: "User-agent: *\nDisallow:\nSitemap: https://cdn.example-assets.net/sitemap.xml\n"
                .into(),
        };
        let results = super::SitemapCheck.run(&ctx_with(probe, robots)).await;
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].title,
            "Cross-origin sitemap declaration not verified"
        );
    }
}
