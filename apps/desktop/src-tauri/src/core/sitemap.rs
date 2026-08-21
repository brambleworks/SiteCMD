//! Sitemap discovery transport over the engine's shared candidates and parser.
//! Discovery may salvage page URLs from documents the check grades invalid.

use crate::constants::API_TIMEOUT_SHORT;
use reqwest::Client;
use serde::Serialize;
use sitecmd_engine::checks::seo::sitemap::{
    parse_sitemap_document, sitemap_candidate_urls, sitemap_urls_from_robots, SitemapParse,
};
use ts_rs::TS;
const MAX_CHILD_SITEMAPS: usize = 50;
const MAX_URLS: usize = 5000;
/// Concurrent child-sitemap fetches per batch when walking a sitemap index.
const SITEMAP_CHILD_CONCURRENCY: usize = 5;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct SitemapResult {
    pub status: SitemapStatus,
    pub urls: Vec<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum SitemapStatus {
    Found,
    NotFound,
    Error,
}

/// Apply the original target's network policy to sitemap-derived URLs.
async fn validate_sitemap_target(url: &str, allow_local_dev: bool) -> Result<(), String> {
    crate::network_policy::validate_url(
        url,
        crate::network_policy::UrlPolicy::Redirect { allow_local_dev },
    )
    .await
}

/// Discover sitemaps through common paths and robots.txt.
/// Derived URLs inherit the originating scan's network policy.
#[tracing::instrument(skip(client, base_url), fields(allow_local_dev))]
pub async fn discover_sitemap(
    client: &Client,
    base_url: &str,
    allow_local_dev: bool,
) -> SitemapResult {
    let base = base_url.trim_end_matches('/');

    // Candidate paths inherit the validated base; discovered URLs are revalidated.
    for url in &sitemap_candidate_urls(base) {
        if let Some(result) = try_fetch_sitemap(client, url, allow_local_dev).await {
            return result;
        }
    }

    // Parse robots.txt sitemap directives.
    let robots_url = format!("{}/robots.txt", base);
    if let Ok(resp) = client
        .get(&robots_url)
        .timeout(API_TIMEOUT_SHORT)
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(body) = crate::http_client::read_text_limited(
                resp,
                crate::constants::MAX_SITEMAP_SIZE,
                API_TIMEOUT_SHORT,
            )
            .await
            {
                for sitemap_url in sitemap_urls_from_robots(&body) {
                    // Reject sitemap URLs that point at private / metadata /
                    // loopback addresses unless the original scan target was
                    // already strict-local.
                    if validate_sitemap_target(&sitemap_url, allow_local_dev)
                        .await
                        .is_err()
                    {
                        tracing::warn!(
                            "Skipping unsafe Sitemap: directive in robots.txt at {}",
                            crate::log_sanitizer::log_safe_url_target(&sitemap_url),
                        );
                        continue;
                    }
                    if let Some(result) =
                        try_fetch_sitemap(client, &sitemap_url, allow_local_dev).await
                    {
                        return result;
                    }
                }
            }
        }
    }

    SitemapResult {
        status: SitemapStatus::NotFound,
        urls: vec![],
        source_url: None,
    }
}

/// Fetch a user-provided sitemap URL.
///
/// `allow_local_dev` mirrors the scan-target's strict-localhost flag so child
/// sitemaps inside a sitemap index get validated with the same network policy.
#[tracing::instrument(skip(client, sitemap_url), fields(allow_local_dev))]
pub async fn fetch_sitemap_url(
    client: &Client,
    sitemap_url: &str,
    allow_local_dev: bool,
) -> SitemapResult {
    match try_fetch_sitemap(client, sitemap_url, allow_local_dev).await {
        Some(result) => result,
        None => SitemapResult {
            status: SitemapStatus::Error,
            urls: vec![],
            source_url: Some(sitemap_url.to_string()),
        },
    }
}

// Sitemap targets use the shared network policy, including DNS-resolved private
// ranges and local-development allowances inherited from the scan target.

async fn try_fetch_sitemap(
    client: &Client,
    url: &str,
    allow_local_dev: bool,
) -> Option<SitemapResult> {
    let resp = client
        .get(url)
        .timeout(API_TIMEOUT_SHORT)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = crate::http_client::read_text_limited(
        resp,
        crate::constants::MAX_SITEMAP_SIZE,
        API_TIMEOUT_SHORT,
    )
    .await
    .ok()?;
    // One parser, shared with the seo.sitemap check. A document the strict
    // grammar rejects still yields locations here: discovery's job is finding
    // pages, and a malformed sitemap's URLs are real.
    let parse = parse_sitemap_document(&body);
    let locs = parse.locs();
    if locs.is_empty() {
        return None;
    }
    if let SitemapParse::Salvaged { reason, .. } = &parse {
        tracing::info!(
            "Reading pages from a malformed sitemap at {}: {}",
            crate::log_sanitizer::log_safe_url_target(url),
            reason
        );
    }

    // A sitemap index lists child sitemaps, not pages - true even when the
    // document is malformed, so this must not be read off a successful parse
    // alone.
    if !parse.lists_child_sitemaps() {
        let mut urls: Vec<String> = locs.to_vec();
        urls.truncate(MAX_URLS);
        return Some(SitemapResult {
            status: SitemapStatus::Found,
            urls,
            source_url: Some(url.to_string()),
        });
    }

    // Fetch child sitemaps in stable batches and stop at MAX_URLS.
    if locs.len() > MAX_CHILD_SITEMAPS {
        tracing::warn!(
            "Sitemap index has {}+ child sitemaps, capping at {}",
            locs.len(),
            MAX_CHILD_SITEMAPS
        );
    }
    let mut all_urls = Vec::new();
    for batch in locs
        .iter()
        .take(MAX_CHILD_SITEMAPS)
        .collect::<Vec<_>>()
        .chunks(SITEMAP_CHILD_CONCURRENCY)
    {
        let fetches = batch.iter().map(|child_url| async move {
            if validate_sitemap_target(child_url, allow_local_dev)
                .await
                .is_err()
            {
                tracing::warn!(
                    "Skipping unsafe sitemap child URL: {}",
                    crate::log_sanitizer::log_safe_url_target(child_url)
                );
                return Vec::new();
            }
            let Ok(resp) = client
                .get(child_url.as_str())
                .timeout(API_TIMEOUT_SHORT)
                .send()
                .await
            else {
                return Vec::new();
            };
            if !resp.status().is_success() {
                return Vec::new();
            }
            match crate::http_client::read_text_limited(
                resp,
                crate::constants::MAX_SITEMAP_SIZE,
                API_TIMEOUT_SHORT,
            )
            .await
            {
                Ok(child_body) => parse_sitemap_document(&child_body).locs().to_vec(),
                Err(_) => Vec::new(),
            }
        });
        for mut page_urls in futures_util::future::join_all(fetches).await {
            all_urls.append(&mut page_urls);
        }
        if all_urls.len() >= MAX_URLS {
            all_urls.truncate(MAX_URLS);
            break;
        }
    }

    if all_urls.is_empty() {
        return None;
    }
    Some(SitemapResult {
        status: SitemapStatus::Found,
        urls: all_urls,
        source_url: Some(url.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reject private and metadata addresses supplied through robots.txt.
    #[tokio::test]
    async fn validate_sitemap_target_rejects_metadata_and_private_ips() {
        // Cloud metadata IP (the canonical SSRF target).
        assert!(
            validate_sitemap_target("http://169.254.169.254/", false)
                .await
                .is_err(),
            "169.254.169.254 must be refused as a sitemap URL when scan target is not loopback"
        );

        // RFC1918 ranges.
        assert!(
            validate_sitemap_target("http://10.0.0.1/sitemap.xml", false)
                .await
                .is_err()
        );
        assert!(
            validate_sitemap_target("http://192.168.1.10/sitemap.xml", false)
                .await
                .is_err()
        );

        // Loopback rejected when not allowed (i.e. the original target was public).
        assert!(
            validate_sitemap_target("http://127.0.0.1/sitemap.xml", false)
                .await
                .is_err()
        );

        // The metadata-host literal (must hit the network_policy domain check).
        assert!(
            validate_sitemap_target("http://metadata.google.internal/", false)
                .await
                .is_err()
        );
    }

    /// When the scan target was localhost (allow_local_dev=true), sitemaps
    /// pointing at loopback ARE allowed - that's the legitimate "scanning
    /// my own dev server" case.
    #[tokio::test]
    async fn validate_sitemap_target_allows_loopback_when_target_is_strict_local() {
        assert!(
            validate_sitemap_target("http://127.0.0.1:5173/sitemap.xml", true)
                .await
                .is_ok()
        );
        // But still rejects metadata and RFC1918 even under local-dev policy.
        assert!(validate_sitemap_target("http://169.254.169.254/", true)
            .await
            .is_err());
        assert!(validate_sitemap_target("http://10.0.0.1/", true)
            .await
            .is_err());
    }

    /// Discovery reads locations through the shared parser, so these pin the
    /// behavior discovery depends on rather than re-testing the grammar
    /// (which sitemap_document_tests.rs owns).
    fn discovered(body: &str) -> Vec<String> {
        parse_sitemap_document(body).locs().to_vec()
    }

    #[test]
    fn discovery_reads_urlset_index_and_text_documents() {
        assert_eq!(
            discovered(
                r#"<urlset><url><loc>https://example.com/</loc></url><url><loc>https://example.com/about</loc></url></urlset>"#
            ),
            vec!["https://example.com/", "https://example.com/about"]
        );
        assert_eq!(
            discovered(
                r#"<sitemapindex><sitemap><loc>https://example.com/posts.xml</loc></sitemap></sitemapindex>"#
            ),
            vec!["https://example.com/posts.xml"]
        );
        assert_eq!(
            discovered("https://example.com/\nhttps://example.com/about\n"),
            vec!["https://example.com/", "https://example.com/about"]
        );
    }

    #[test]
    fn discovery_still_reads_a_malformed_sitemap() {
        // The check calls this document invalid; discovery must still find the
        // pages, because they exist.
        let salvaged =
            discovered(r#"<urlset><url><loc>https://example.com/page1</loc></url><url>"#);
        assert_eq!(salvaged, vec!["https://example.com/page1"]);
    }

    #[test]
    fn discovery_finds_nothing_in_an_empty_or_non_sitemap_response() {
        assert!(discovered(r#"<?xml version="1.0"?><urlset></urlset>"#).is_empty());
        assert!(discovered("<html><body>404</body></html>").is_empty());
    }
}
