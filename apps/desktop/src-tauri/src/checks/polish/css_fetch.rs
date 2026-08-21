//! Bounded stylesheet fetcher with explicit discovery and fetch coverage.

use crate::constants::CHECK_PROBE_TIMEOUT;
use regex::Regex;
use std::sync::LazyLock;

/// Maximum number of external stylesheets to fetch. The fetches run
/// concurrently with per-fetch timeouts, so raising this widens coverage
/// without extending the worst-case wall time.
const MAX_CSS_FILES: usize = 8;

/// Regex to match `<link rel="stylesheet" href="...">` tags.
/// Handles both `rel` before `href` and `href` before `rel`.
static LINK_STYLESHEET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)<link\s[^>]*?(?:rel\s*=\s*["']stylesheet["'][^>]*?href\s*=\s*["']([^"']+)["']|href\s*=\s*["']([^"']+)["'][^>]*?rel\s*=\s*["']stylesheet["'])"#
    ).expect("Failed to compile link stylesheet regex")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StylesheetFetchResult {
    pub css: String,
    pub stylesheets_discovered: usize,
    pub stylesheets_fetched: usize,
}

impl StylesheetFetchResult {
    pub fn coverage_complete(&self) -> bool {
        self.stylesheets_fetched == self.stylesheets_discovered
    }
}

/// Extract every stylesheet URL from HTML, resolving relative paths against
/// the base URL. The fetch cap is applied by `fetch_stylesheets`, not here:
/// the full list is what makes `stylesheets_discovered` an honest count.
pub fn extract_stylesheet_urls(html: &str, base_url: &url::Url) -> Vec<String> {
    LINK_STYLESHEET_RE
        .captures_iter(html)
        .filter_map(|cap| {
            // Group 1 or Group 2 depending on attribute order
            let href = decode_href_entities(cap.get(1).or_else(|| cap.get(2))?.as_str());
            // Resolve relative URLs
            match base_url.join(&href) {
                Ok(abs) => Some(abs.to_string()),
                Err(_) => Some(href),
            }
        })
        .collect()
}

fn decode_href_entities(href: &str) -> String {
    href.replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&#x26;", "&")
        .replace("&#X26;", "&")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
}

/// Fetch bounded linked CSS with explicit discovery and success counts.
/// `allow_local_dev` must match the scan client so public pages cannot steer
/// subresource requests to private targets.
pub async fn fetch_stylesheets(
    html: &str,
    base_url: &url::Url,
    client: &reqwest::Client,
    allow_local_dev: bool,
) -> StylesheetFetchResult {
    let urls = extract_stylesheet_urls(html, base_url);
    let stylesheets_discovered = urls.len();

    // The stylesheets are independent; fetch them concurrently so the worst
    // case is one probe timeout, not the sum of all of them. join_all
    // preserves input order, keeping the concatenated output deterministic.
    let fetches = urls.iter().take(MAX_CSS_FILES).map(|url| async move {
        let safe_url = crate::log_sanitizer::log_safe_url_target(url.as_str());
        // The scanned page controls this href; refuse targets that would let it
        // pivot our machine onto internal/loopback endpoints via SSRF.
        match url::Url::parse(url) {
            Ok(parsed) => {
                if let Err(reason) = crate::network_policy::validate_page_subresource_target(
                    &parsed,
                    allow_local_dev,
                ) {
                    tracing::warn!("Skipping disallowed CSS target {}: {}", safe_url, reason);
                    return None;
                }
            }
            Err(e) => {
                tracing::warn!("Skipping unparseable CSS target {}: {}", safe_url, e);
                return None;
            }
        }
        match tokio::time::timeout(CHECK_PROBE_TIMEOUT, client.get(url).send()).await {
            Ok(Ok(resp)) if resp.status().is_success() => {
                match crate::http_client::read_text_limited(
                    resp,
                    crate::constants::MAX_STYLESHEET_BODY_SIZE,
                    CHECK_PROBE_TIMEOUT,
                )
                .await
                {
                    Ok(text) => {
                        tracing::debug!("Fetched CSS ({} bytes): {}", text.len(), safe_url);
                        Some(text)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read CSS body from {}: {}", safe_url, e);
                        None
                    }
                }
            }
            Ok(Ok(resp)) => {
                tracing::warn!("CSS fetch returned {} for {}", resp.status(), safe_url);
                None
            }
            Ok(Err(e)) => {
                tracing::warn!("CSS fetch failed for {}: {}", safe_url, e);
                None
            }
            Err(_) => {
                tracing::warn!("CSS fetch timed out for {}", safe_url);
                None
            }
        }
    });
    let css_parts = futures_util::future::join_all(fetches).await;
    let stylesheets_fetched = css_parts.iter().filter(|part| part.is_some()).count();
    let css = css_parts
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");

    StylesheetFetchResult {
        css,
        stylesheets_discovered,
        stylesheets_fetched,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> url::Url {
        url::Url::parse("https://example.com/page").unwrap()
    }

    #[test]
    fn extracts_absolute_stylesheet_urls() {
        let html = r#"<link rel="stylesheet" href="https://cdn.example.com/style.css">"#;
        let urls = extract_stylesheet_urls(html, &base());
        assert_eq!(urls, vec!["https://cdn.example.com/style.css"]);
    }

    #[test]
    fn extracts_relative_stylesheet_urls() {
        let html = r#"<link rel="stylesheet" href="/css/main.css">"#;
        let urls = extract_stylesheet_urls(html, &base());
        assert_eq!(urls, vec!["https://example.com/css/main.css"]);
    }

    #[test]
    fn handles_href_before_rel() {
        let html = r#"<link href="/style.css" rel="stylesheet">"#;
        let urls = extract_stylesheet_urls(html, &base());
        assert_eq!(urls, vec!["https://example.com/style.css"]);
    }

    #[test]
    fn decodes_html_entities_in_stylesheet_urls() {
        let html =
            r#"<link rel="stylesheet" href="/css/main.css?delta=1&amp;theme=site&amp;include=a">"#;
        let urls = extract_stylesheet_urls(html, &base());
        assert_eq!(
            urls,
            vec!["https://example.com/css/main.css?delta=1&theme=site&include=a"]
        );
    }

    #[test]
    fn extraction_returns_every_stylesheet_not_a_capped_prefix() {
        let html: String = (0..MAX_CSS_FILES + 2)
            .map(|i| format!(r#"<link rel="stylesheet" href="/s{i}.css">"#))
            .collect();
        let urls = extract_stylesheet_urls(&html, &base());
        assert_eq!(urls.len(), MAX_CSS_FILES + 2);
    }

    #[tokio::test]
    async fn pages_linking_more_stylesheets_than_the_cap_report_incomplete_coverage() {
        // Loopback rejection keeps this discovered-count test deterministic.
        let html: String = (0..MAX_CSS_FILES + 1)
            .map(|i| format!(r#"<link rel="stylesheet" href="http://127.0.0.1/s{i}.css">"#))
            .collect();
        let result = fetch_stylesheets(&html, &base(), crate::http_client::client(), false).await;
        assert_eq!(result.stylesheets_discovered, MAX_CSS_FILES + 1);
        assert!(
            !result.coverage_complete(),
            "stylesheets past the fetch cap were not inspected; coverage must read incomplete"
        );
    }

    #[test]
    fn ignores_non_stylesheet_links() {
        let html = r#"
            <link rel="icon" href="/favicon.ico">
            <link rel="preload" href="/font.woff2" as="font">
            <link rel="stylesheet" href="/style.css">
        "#;
        let urls = extract_stylesheet_urls(html, &base());
        assert_eq!(urls, vec!["https://example.com/style.css"]);
    }

    #[tokio::test]
    async fn skips_ssrf_stylesheet_targets_without_fetching() {
        let html = r#"<link rel="stylesheet" href="http://169.254.169.254/latest/meta-data/">
            <link rel="stylesheet" href="http://127.0.0.1:11434/style.css">"#;
        let css = fetch_stylesheets(html, &base(), crate::http_client::client(), false).await;
        assert_eq!(
            css.css, "",
            "internal SSRF targets must be skipped, not fetched"
        );
        assert_eq!(css.stylesheets_discovered, 2);
        assert_eq!(css.stylesheets_fetched, 0);
        assert!(!css.coverage_complete());
    }
}
