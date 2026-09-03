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
    /// Why this URL set is known to be shorter than what the site publishes,
    /// when it is. Scan-internal evidence for checks that must not report a
    /// clean verdict over a partial set, so it stays off the IPC contract.
    #[serde(skip)]
    #[ts(skip)]
    pub partial_because: Option<String>,
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
    // A sitemap answering at a conventional path does not mean it is the only
    // one the site publishes, so robots.txt is still read for the declared set.
    for url in &sitemap_candidate_urls(base) {
        if let Some(mut result) = try_fetch_sitemap(client, url, allow_local_dev).await {
            let declared = robots_sitemap_urls(client, base).await;
            note_partial(&mut result, unread_sitemaps_reason(&declared, Some(url)));
            return result;
        }
    }

    // Parse robots.txt sitemap directives.
    let declared = robots_sitemap_urls(client, base).await;
    for sitemap_url in &declared {
        // Reject sitemap URLs that point at private / metadata / loopback
        // addresses unless the original scan target was already strict-local.
        if validate_sitemap_target(sitemap_url, allow_local_dev)
            .await
            .is_err()
        {
            tracing::warn!(
                "Skipping unsafe Sitemap: directive in robots.txt at {}",
                crate::log_sanitizer::log_safe_url_target(sitemap_url),
            );
            continue;
        }
        if let Some(mut result) = try_fetch_sitemap(client, sitemap_url, allow_local_dev).await {
            note_partial(
                &mut result,
                unread_sitemaps_reason(&declared, Some(sitemap_url)),
            );
            return result;
        }
    }

    SitemapResult {
        status: SitemapStatus::NotFound,
        urls: vec![],
        source_url: None,
        partial_because: None,
    }
}

/// The sitemap URLs a site declares in robots.txt, empty when robots.txt is
/// missing, unreadable, or declares none.
async fn robots_sitemap_urls(client: &Client, base: &str) -> Vec<String> {
    let robots_url = format!("{}/robots.txt", base);
    let Ok(response) = client
        .get(&robots_url)
        .timeout(API_TIMEOUT_SHORT)
        .send()
        .await
    else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    match crate::http_client::read_text_limited(
        response,
        crate::constants::MAX_SITEMAP_SIZE,
        API_TIMEOUT_SHORT,
    )
    .await
    {
        Ok(body) => sitemap_urls_from_robots(&body),
        Err(_) => Vec::new(),
    }
}

/// Which bound cut a sitemap read short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SitemapBound {
    /// URLs listed by a single sitemap document.
    Urls,
    /// Child sitemaps listed by a sitemap index.
    ChildSitemaps,
    /// URLs collected while walking a sitemap index. The walk stops as soon as
    /// it holds `cap` URLs, so reaching the cap exactly still means it stopped
    /// with children left unread.
    IndexUrls,
}

/// Why a URL set is known to be shorter than what the document offered, or
/// `None` when the whole document was read. `observed` is what the document
/// offered (for `IndexUrls`, what the walk had collected when it stopped).
fn partial_reason(bound: SitemapBound, observed: usize, cap: usize) -> Option<String> {
    let truncated = match bound {
        SitemapBound::IndexUrls => observed >= cap,
        SitemapBound::Urls | SitemapBound::ChildSitemaps => observed > cap,
    };
    if !truncated {
        return None;
    }
    Some(match bound {
        SitemapBound::Urls => {
            format!("the sitemap lists {observed} URLs and SiteCMD read the first {cap}")
        }
        SitemapBound::ChildSitemaps => format!(
            "the sitemap index lists {observed} child sitemaps and SiteCMD read the first {cap}"
        ),
        SitemapBound::IndexUrls => format!(
            "the sitemap index yielded more than {cap} URLs and SiteCMD read the first {cap}"
        ),
    })
}

/// A sitemap URL reduced to the identity two references to the same document
/// share. robots.txt directives are raw text written by the site: a site
/// scanned at its www host can declare the sitemap on its apex host, a site on
/// https can declare it as http, and a robots.txt can list the same URL twice.
/// None of those is a further sitemap, so none may mark a set partial.
fn sitemap_identity(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url.trim()) else {
        return url.trim().trim_end_matches('/').to_ascii_lowercase();
    };
    parsed.set_fragment(None);
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let port = parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
    let path = parsed.path().trim_end_matches('/');
    let query = parsed.query().map(|q| format!("?{q}")).unwrap_or_default();
    format!("{host}{port}{path}{query}")
}

/// Why the set is partial because the site declares sitemaps that were not the
/// one read, or `None` when everything declared was covered by the one read.
fn unread_sitemaps_reason(declared: &[String], read: Option<&str>) -> Option<String> {
    let read = read.map(sitemap_identity);
    let unread: std::collections::HashSet<String> = declared
        .iter()
        .map(|url| sitemap_identity(url))
        .filter(|identity| Some(identity) != read.as_ref())
        .collect();
    if unread.is_empty() {
        return None;
    }
    let unread = unread.len();
    Some(format!(
        "robots.txt declares {unread} further sitemap{} that {} not read",
        if unread == 1 { "" } else { "s" },
        if unread == 1 { "was" } else { "were" },
    ))
}

/// Record one more reason a set is partial, keeping any already recorded: a set
/// can be both truncated at a cap and missing a sitemap the site declares.
fn note_partial(result: &mut SitemapResult, reason: Option<String>) {
    let Some(reason) = reason else {
        return;
    };
    result.partial_because = Some(match result.partial_because.take() {
        Some(existing) => format!("{existing}; {reason}"),
        None => reason,
    });
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
            partial_because: None,
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
        let listed = locs.len();
        let mut urls: Vec<String> = locs.to_vec();
        urls.truncate(MAX_URLS);
        return Some(SitemapResult {
            status: SitemapStatus::Found,
            urls,
            source_url: Some(url.to_string()),
            partial_because: partial_reason(SitemapBound::Urls, listed, MAX_URLS),
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
    let child_cap = partial_reason(SitemapBound::ChildSitemaps, locs.len(), MAX_CHILD_SITEMAPS);
    let mut url_cap = None;
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
            url_cap = partial_reason(SitemapBound::IndexUrls, all_urls.len(), MAX_URLS);
            all_urls.truncate(MAX_URLS);
            break;
        }
    }

    if all_urls.is_empty() {
        return None;
    }
    let mut result = SitemapResult {
        status: SitemapStatus::Found,
        urls: all_urls,
        source_url: Some(url.to_string()),
        partial_because: child_cap,
    };
    note_partial(&mut result, url_cap);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The truncation decision, which no network test can reach: the caps are
    /// 50 child sitemaps and 5000 URLs, so a fixture large enough to hit them
    /// would be a five-thousand-entry document. Deciding it in a pure function
    /// is what makes "SiteCMD read only part of this sitemap" testable at all.
    #[test]
    fn a_sitemap_read_within_its_caps_is_not_partial() {
        assert_eq!(partial_reason(SitemapBound::Urls, MAX_URLS, MAX_URLS), None);
        assert_eq!(
            partial_reason(
                SitemapBound::ChildSitemaps,
                MAX_CHILD_SITEMAPS,
                MAX_CHILD_SITEMAPS
            ),
            None
        );
        assert_eq!(partial_reason(SitemapBound::Urls, 0, MAX_URLS), None);
    }

    #[test]
    fn a_truncated_sitemap_read_names_the_bound_that_cut_it() {
        assert_eq!(
            partial_reason(SitemapBound::Urls, 9000, MAX_URLS).as_deref(),
            Some("the sitemap lists 9000 URLs and SiteCMD read the first 5000")
        );
        assert_eq!(
            partial_reason(SitemapBound::ChildSitemaps, 120, MAX_CHILD_SITEMAPS).as_deref(),
            Some("the sitemap index lists 120 child sitemaps and SiteCMD read the first 50")
        );
    }

    /// The index walk stops as soon as it holds the cap, so hitting it exactly
    /// still leaves children unread. The other two bounds truncate a document
    /// already in hand, so reaching the cap exactly reads all of it.
    #[test]
    fn an_index_walk_that_stopped_on_the_url_cap_is_partial_at_the_cap_itself() {
        assert_eq!(
            partial_reason(SitemapBound::IndexUrls, MAX_URLS, MAX_URLS).as_deref(),
            Some("the sitemap index yielded more than 5000 URLs and SiteCMD read the first 5000")
        );
        assert_eq!(
            partial_reason(SitemapBound::IndexUrls, MAX_URLS - 1, MAX_URLS),
            None
        );
    }

    #[test]
    fn a_site_declaring_only_the_sitemap_that_was_read_is_not_partial() {
        let declared = vec!["https://example.com/sitemap.xml".to_string()];
        assert_eq!(
            unread_sitemaps_reason(&declared, Some("https://example.com/sitemap.xml")),
            None
        );
        assert_eq!(
            unread_sitemaps_reason(&[], Some("https://example.com/sitemap.xml")),
            None
        );
    }

    /// robots.txt directives are raw text. A site scanned at its www host
    /// declares its sitemap on the apex, a site on https writes http, a
    /// generator appends a trailing slash, and a hand-edited robots.txt lists
    /// the same URL twice. Comparing those as strings marks a whole sitemap
    /// partial and turns a real Pass into a Skipped on ordinary sites.
    #[test]
    fn the_same_sitemap_declared_another_way_is_not_a_further_sitemap() {
        let read = Some("https://www.example.com/sitemap.xml");
        for declared in [
            vec!["https://example.com/sitemap.xml".to_string()],
            vec!["http://www.example.com/sitemap.xml".to_string()],
            vec!["https://www.example.com/sitemap.xml/".to_string()],
            vec!["https://WWW.Example.com/sitemap.xml".to_string()],
            vec![
                "https://www.example.com/sitemap.xml".to_string(),
                "https://example.com/sitemap.xml".to_string(),
            ],
        ] {
            assert_eq!(
                unread_sitemaps_reason(&declared, read),
                None,
                "{declared:?} is the sitemap that was read, written another way"
            );
        }
    }

    /// Two references to one unread sitemap are one unread sitemap.
    #[test]
    fn a_sitemap_declared_twice_counts_once() {
        let declared = vec![
            "https://example.com/sitemap.xml".to_string(),
            "https://example.com/sitemap-news.xml".to_string(),
            "https://www.example.com/sitemap-news.xml".to_string(),
        ];
        assert_eq!(
            unread_sitemaps_reason(&declared, Some("https://example.com/sitemap.xml")).as_deref(),
            Some("robots.txt declares 1 further sitemap that was not read")
        );
    }

    #[test]
    fn a_site_declaring_sitemaps_beyond_the_one_read_is_partial() {
        let declared = vec![
            "https://example.com/sitemap.xml".to_string(),
            "https://example.com/sitemap-news.xml".to_string(),
            "https://example.com/sitemap-images.xml".to_string(),
        ];
        assert_eq!(
            unread_sitemaps_reason(&declared, Some("https://example.com/sitemap.xml")).as_deref(),
            Some("robots.txt declares 2 further sitemaps that were not read")
        );
        let one_more = vec![
            "https://example.com/sitemap.xml".to_string(),
            "https://example.com/sitemap-news.xml".to_string(),
        ];
        assert_eq!(
            unread_sitemaps_reason(&one_more, Some("https://example.com/sitemap.xml")).as_deref(),
            Some("robots.txt declares 1 further sitemap that was not read")
        );
    }

    /// A set can be short for more than one reason at once, and a caller that
    /// overwrote instead of appending would report only the last one found.
    #[test]
    fn every_reason_a_set_is_partial_is_kept() {
        let mut result = SitemapResult {
            status: SitemapStatus::Found,
            urls: vec!["https://example.com/a".to_string()],
            source_url: Some("https://example.com/sitemap.xml".to_string()),
            partial_because: None,
        };
        note_partial(&mut result, None);
        assert_eq!(result.partial_because, None);

        note_partial(
            &mut result,
            partial_reason(SitemapBound::Urls, 9000, MAX_URLS),
        );
        note_partial(
            &mut result,
            unread_sitemaps_reason(
                &["https://example.com/sitemap-news.xml".to_string()],
                Some("https://example.com/sitemap.xml"),
            ),
        );
        assert_eq!(
            result.partial_because.as_deref(),
            Some(
                "the sitemap lists 9000 URLs and SiteCMD read the first 5000; robots.txt declares \
                 1 further sitemap that was not read"
            )
        );
    }

    /// The pure helpers above prove the decision; this proves discovery still
    /// asks them. Nothing observable changes when a `partial_reason` call is
    /// dropped from a fetch path, so no behavioral test would notice a set
    /// silently going back to reporting a clean Pass over a truncated read.
    #[test]
    fn every_truncation_path_records_why_the_set_is_partial() {
        const SOURCE: &str = include_str!("sitemap.rs");
        let code = SOURCE
            .split_once("#[cfg(test)]")
            .expect("sitemap.rs must have a test module")
            .0;

        for bound in [
            "SitemapBound::Urls",
            "SitemapBound::ChildSitemaps",
            "SitemapBound::IndexUrls",
        ] {
            assert!(
                code.contains(&format!("partial_reason({bound}")),
                "the {bound} truncation path must record why the set is partial"
            );
        }
        assert!(
            code.contains("note_partial(&mut result, url_cap)"),
            "the sitemap index walk must hand its URL-cap reason to the result it returns; \
             computing the reason and dropping it leaves a truncated read looking whole"
        );
        assert_eq!(
            code.matches("unread_sitemaps_reason(&declared").count(),
            2,
            "both discovery paths, the conventional candidate and the robots.txt directive, must \
             compare what was read against every sitemap the site declares"
        );
    }

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
