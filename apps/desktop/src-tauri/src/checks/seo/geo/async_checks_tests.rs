use super::*;
use crate::checks::{CheckStatus, RobotsTxtFetch, Severity, SitemapFetch, SitemapProbe};

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

fn ctx_with_robots(fetch: RobotsTxtFetch) -> CheckContext {
    let ctx = ctx();
    assert!(ctx.probe_cache.robots_txt.set(fetch).is_ok());
    ctx
}

fn ctx_with_sitemap(xml: &str) -> CheckContext {
    let ctx = ctx();
    let sitecmd_engine::checks::seo::sitemap::SitemapParse::WellFormed(document) =
        sitecmd_engine::checks::seo::sitemap::parse_sitemap_document(xml)
    else {
        panic!("test fixture must be a valid sitemap document");
    };
    assert!(ctx
        .probe_cache
        .sitemap
        .set(SitemapProbe::Found(SitemapFetch::new(
            "https://example.com/sitemap.xml",
            xml,
            &document,
        )))
        .is_ok());
    ctx
}

#[tokio::test]
async fn crawler_shell_grades_the_seeded_robots_body_through_the_engine() {
    // The blocked-token taxonomy itself is pinned by the engine tests; this
    // proves the shell hands the shared fetch's body over.
    let robots =
        "User-agent: GPTBot\nUser-agent: ClaudeBot\nDisallow: /\n\nUser-agent: *\nAllow: /\n";
    let results = AiCrawlerBlockingCheck
        .run(&ctx_with_robots(RobotsTxtFetch::Found {
            body: robots.into(),
        }))
        .await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0]
        .description
        .contains("OpenAI model-training crawler"));
}

#[tokio::test]
async fn missing_robots_txt_yields_no_crawler_result() {
    let results = AiCrawlerBlockingCheck
        .run(&ctx_with_robots(RobotsTxtFetch::Status(404)))
        .await;
    assert!(results.is_empty());
}

#[tokio::test]
async fn freshness_shell_grades_the_seeded_sitemap_through_the_engine() {
    let xml =
        "<urlset><url><loc>https://example.com/</loc><lastmod>yesterday</lastmod></url></urlset>";
    let results = SitemapFreshnessCheck.run(&ctx_with_sitemap(xml)).await;
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0].title.contains("invalid lastmod"));
}

#[tokio::test]
async fn missing_sitemap_yields_no_freshness_result() {
    let ctx = ctx();
    assert!(ctx
        .probe_cache
        .sitemap
        .set(SitemapProbe::Missing {
            observations: vec![],
        })
        .is_ok());
    assert!(SitemapFreshnessCheck.run(&ctx).await.is_empty());
}

#[tokio::test]
async fn unreachable_llms_txt_is_inconclusive_not_missing() {
    // Closed loopback port: the probe fails immediately, exercising the
    // failure path end-to-end through the seam without a test server.
    let mut ctx = ctx();
    ctx.page.url = url::Url::parse("http://127.0.0.1:1").unwrap();
    let results = LlmsTxtCheck.run(&ctx).await;
    assert_eq!(results[0].status, CheckStatus::Skipped);
    assert_eq!(results[0].severity, Severity::Low);
    assert_eq!(
        results[0].confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(results[0].title.contains("not evaluated"));
}
