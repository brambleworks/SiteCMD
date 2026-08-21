//! Desktop transport for the robots.txt sitemap-directive check.

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::seo::robots_directives::evaluate_sitemap_in_robots;

// The sync page-source hint checks live in the engine; re-export them so the
// `extras::` registration paths keep resolving in the desktop config module.
pub use sitecmd_engine::checks::config::extras::{
    PrintStylesheetCheck, ResponsiveDesignCheck, TrailingSlashCheck,
};

pub struct SitemapInRobotsCheck;

#[async_trait::async_trait]
impl AsyncCheck for SitemapInRobotsCheck {
    fn origin_scoped(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "config.sitemap_in_robots"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }
    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        evaluate_sitemap_in_robots(ctx.robots_txt().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    fn ctx() -> CheckContext {
        CheckContext {
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
        }
    }

    #[tokio::test]
    async fn the_shell_feeds_the_cached_robots_state_to_the_engine_verdict() {
        // Exercises the full shell wiring: probe-cache read, outcome
        // mapping, and the engine's confirmed-missing verdict.
        let missing = ctx();
        assert!(missing
            .probe_cache
            .robots_txt
            .set(crate::checks::RobotsTxtFetch::Status(404))
            .is_ok());
        let result = SitemapInRobotsCheck.run(&missing).await.remove(0);
        assert_eq!(result.status, CheckStatus::Skipped);
        assert_eq!(result.title, "No robots.txt file to inspect");
    }
}
