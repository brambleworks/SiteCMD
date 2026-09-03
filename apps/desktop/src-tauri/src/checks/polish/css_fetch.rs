//! Bounded stylesheet fetcher with explicit discovery and fetch coverage.

use crate::constants::CHECK_PROBE_TIMEOUT;
use regex::Regex;
use std::sync::LazyLock;

use super::stylesheet_cache::StylesheetCache;

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

/// What one stylesheet request produced. The distinction matters because the
/// execution-wide cache may only memoize answers the origin actually gave: a
/// repeat request would return the same thing. A request that never got an
/// answer must be retried by the next page, or one blip would report every
/// later page as having incomplete CSS coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StylesheetOutcome {
    /// A usable stylesheet body.
    Body(String),
    /// The origin answered and the answer is settled: a non-success status, or
    /// a body that could not be read within the size and time limits.
    Refused,
    /// No answer arrived: a probe timeout or a transport failure. Never cached.
    Unavailable,
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
/// subresource requests to private targets. `cache` is the current scan
/// execution's stylesheet store; pass `None` for a single-page scan, which has
/// no other page to share stylesheets with. Only settled answers are reused:
/// see `StylesheetOutcome`.
pub async fn fetch_stylesheets(
    html: &str,
    base_url: &url::Url,
    client: &reqwest::Client,
    allow_local_dev: bool,
    cache: Option<&StylesheetCache>,
) -> StylesheetFetchResult {
    fetch_stylesheets_with(html, base_url, allow_local_dev, cache, |url| async move {
        fetch_one_stylesheet(&url, client).await
    })
    .await
}

/// Discovery, subresource policy, and caching around an injected fetcher.
/// Tests substitute the fetcher to observe exactly which URLs left the cache.
async fn fetch_stylesheets_with<F, Fut>(
    html: &str,
    base_url: &url::Url,
    allow_local_dev: bool,
    cache: Option<&StylesheetCache>,
    fetch_one: F,
) -> StylesheetFetchResult
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = StylesheetOutcome>,
{
    let urls = extract_stylesheet_urls(html, base_url);
    let stylesheets_discovered = urls.len();
    let fetch_one = &fetch_one;

    // The stylesheets are independent; fetch them concurrently so the worst
    // case is one probe timeout, not the sum of all of them. join_all
    // preserves input order, keeping the concatenated output deterministic.
    let fetches = urls.iter().take(MAX_CSS_FILES).map(|url| async move {
        let safe_url = crate::log_sanitizer::log_safe_url_target(url.as_str());
        // The scanned page controls this href; refuse targets that would let it
        // pivot our machine onto internal/loopback endpoints via SSRF. The
        // policy runs ahead of the cache so a body recorded while one page
        // allowed local targets can never be served to a page that does not.
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
        if let Some(cached) = cache.and_then(|cache| cache.get(url)) {
            tracing::debug!("Reusing stylesheet already read this scan: {}", safe_url);
            return cached;
        }
        // Only settled answers are memoized. A stylesheet that never answered
        // is retried by the next page, so one blip cannot report every later
        // page as having incomplete CSS coverage.
        match fetch_one(url.clone()).await {
            StylesheetOutcome::Body(text) => {
                if let Some(cache) = cache {
                    cache.insert(url, Some(text.clone()));
                }
                Some(text)
            }
            StylesheetOutcome::Refused => {
                if let Some(cache) = cache {
                    cache.insert(url, None);
                }
                None
            }
            StylesheetOutcome::Unavailable => None,
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

/// Classify a body that could not be read. Only the size cap is settled: the
/// origin answered, and re-reading the same stylesheet would hit the same cap.
/// A stalled body or a connection reset mid-body taught this scan nothing
/// about the stylesheet, so the next page must be free to ask again rather
/// than inherit one blip as five degraded polish signals for the rest of the
/// session.
fn outcome_from_body_error(error: &crate::http_client::BodyReadError) -> StylesheetOutcome {
    use crate::http_client::BodyReadError;
    match error {
        BodyReadError::TooLarge { .. } => StylesheetOutcome::Refused,
        BodyReadError::TimedOut { .. } | BodyReadError::Transport(_) => {
            StylesheetOutcome::Unavailable
        }
    }
}

/// Fetch one stylesheet body, classifying every outcome that leaves the
/// stylesheet uninspected as either a settled refusal or a missing answer.
async fn fetch_one_stylesheet(url: &str, client: &reqwest::Client) -> StylesheetOutcome {
    let safe_url = crate::log_sanitizer::log_safe_url_target(url);
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
                    StylesheetOutcome::Body(text)
                }
                Err(e) => {
                    tracing::warn!("Failed to read CSS body from {}: {}", safe_url, e);
                    outcome_from_body_error(&e)
                }
            }
        }
        Ok(Ok(resp)) => {
            tracing::warn!("CSS fetch returned {} for {}", resp.status(), safe_url);
            StylesheetOutcome::Refused
        }
        Ok(Err(e)) => {
            tracing::warn!("CSS fetch failed for {}: {}", safe_url, e);
            StylesheetOutcome::Unavailable
        }
        Err(_) => {
            tracing::warn!("CSS fetch timed out for {}", safe_url);
            StylesheetOutcome::Unavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::BodyReadError;

    fn base() -> url::Url {
        url::Url::parse("https://example.com/page").unwrap()
    }

    const SITE_CSS: &str = "https://cdn.example.com/site.css";
    const THEME_CSS: &str = "https://cdn.example.com/theme.css";
    const REFUSED_CSS: &str = "https://cdn.example.com/refused.css";
    const FLAKY_CSS: &str = "https://cdn.example.com/flaky.css";

    fn page_html(hrefs: &[&str]) -> String {
        hrefs
            .iter()
            .map(|href| format!(r#"<link rel="stylesheet" href="{href}">"#))
            .collect()
    }

    /// Every page of the session links the same three stylesheets.
    fn shared_page_html() -> String {
        page_html(&[SITE_CSS, THEME_CSS, REFUSED_CSS])
    }

    type FetchLog = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

    /// Stand in for the network: record every URL that reaches the fetcher and
    /// answer from a fixed table. `REFUSED_CSS` is absent, so it stands for a
    /// stylesheet the origin answered but would not serve.
    fn recording_fetcher(
        log: FetchLog,
    ) -> impl Fn(String) -> std::future::Ready<StylesheetOutcome> {
        move |url| {
            log.lock().expect("fetch log").push(url.clone());
            let outcome = match url.as_str() {
                SITE_CSS => StylesheetOutcome::Body("body{color:#111}".to_string()),
                THEME_CSS => StylesheetOutcome::Body(".theme{}".to_string()),
                _ => StylesheetOutcome::Refused,
            };
            std::future::ready(outcome)
        }
    }

    async fn scan_page(cache: Option<&StylesheetCache>, log: &FetchLog) -> StylesheetFetchResult {
        fetch_stylesheets_with(
            &shared_page_html(),
            &base(),
            false,
            cache,
            recording_fetcher(log.clone()),
        )
        .await
    }

    const A_CSS: &str = "https://cdn.example.com/a.css";
    const B_CSS: &str = "https://cdn.example.com/b.css";
    const C_CSS: &str = "https://cdn.example.com/c.css";

    /// Fetcher for the budget tests: every stylesheet answers with the same
    /// ten-byte body, so the byte budget is the only thing that varies.
    fn sized_fetcher(log: FetchLog) -> impl Fn(String) -> std::future::Ready<StylesheetOutcome> {
        move |url| {
            log.lock().expect("fetch log").push(url);
            std::future::ready(StylesheetOutcome::Body("x".repeat(10)))
        }
    }

    /// `FLAKY_CSS` stalls mid-body on its first attempt and serves a body on
    /// the second; everything else answers with a body over the size cap. Both
    /// failures run through the same classifier the live fetcher uses, so the
    /// cache policy is exercised against the real mapping rather than against
    /// hand-picked outcomes.
    fn flaky_fetcher(
        log: FetchLog,
        attempts: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> impl Fn(String) -> std::future::Ready<StylesheetOutcome> {
        move |url| {
            log.lock().expect("fetch log").push(url.clone());
            let outcome = if url == FLAKY_CSS {
                if attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    outcome_from_body_error(&BodyReadError::TimedOut {
                        timeout: CHECK_PROBE_TIMEOUT,
                    })
                } else {
                    StylesheetOutcome::Body(".flaky{}".to_string())
                }
            } else {
                outcome_from_body_error(&BodyReadError::TooLarge {
                    max_bytes: 1,
                    received_bytes: 2,
                })
            };
            std::future::ready(outcome)
        }
    }

    fn fetch_count(log: &FetchLog, url: &str) -> usize {
        log.lock()
            .expect("fetch log")
            .iter()
            .filter(|fetched| *fetched == url)
            .count()
    }

    #[tokio::test]
    async fn one_execution_fetches_each_shared_stylesheet_once_across_its_pages() {
        let cache = StylesheetCache::new();
        let log: FetchLog = Default::default();

        let first = scan_page(Some(&cache), &log).await;
        let second = scan_page(Some(&cache), &log).await;
        let third = scan_page(Some(&cache), &log).await;

        assert_eq!(fetch_count(&log, SITE_CSS), 1, "site stylesheet refetched");
        assert_eq!(
            fetch_count(&log, THEME_CSS),
            1,
            "theme stylesheet refetched"
        );
        assert_eq!(
            fetch_count(&log, REFUSED_CSS),
            1,
            "a settled refusal must not be re-requested on every page"
        );
        assert_eq!(
            first, second,
            "a cache hit must produce the same result as the miss that filled it"
        );
        assert_eq!(second, third);
        assert_eq!(first.stylesheets_discovered, 3);
        assert_eq!(first.stylesheets_fetched, 2);
        assert!(!first.coverage_complete());
    }

    #[tokio::test]
    async fn a_stylesheet_that_never_answered_is_retried_but_a_refusal_is_not() {
        // A body read fails for settled and transient reasons alike, and only
        // the settled one may be memoized for the rest of the execution.
        assert_eq!(
            outcome_from_body_error(&BodyReadError::TooLarge {
                max_bytes: 1,
                received_bytes: 2,
            }),
            StylesheetOutcome::Refused,
            "a body over the size cap is settled: re-reading it ends the same way"
        );
        assert_eq!(
            outcome_from_body_error(&BodyReadError::TimedOut {
                timeout: CHECK_PROBE_TIMEOUT,
            }),
            StylesheetOutcome::Unavailable,
            "a body that stalled after the headers taught this scan nothing, so it must be retried"
        );

        let cache = StylesheetCache::new();
        let log: FetchLog = Default::default();
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let html = page_html(&[FLAKY_CSS, REFUSED_CSS]);

        let first = fetch_stylesheets_with(
            &html,
            &base(),
            false,
            Some(&cache),
            flaky_fetcher(log.clone(), attempts.clone()),
        )
        .await;
        assert_eq!(first.stylesheets_fetched, 0);
        assert!(!first.coverage_complete());

        let second = fetch_stylesheets_with(
            &html,
            &base(),
            false,
            Some(&cache),
            flaky_fetcher(log.clone(), attempts.clone()),
        )
        .await;

        assert_eq!(
            fetch_count(&log, FLAKY_CSS),
            2,
            "a stylesheet that never answered must be re-attempted by the next page"
        );
        assert_eq!(
            fetch_count(&log, REFUSED_CSS),
            1,
            "an answered refusal is settled and must not be re-requested"
        );
        // The blip on page one must not be frozen into every later page:
        // mark_incomplete_polish_css_results downgrades five polish signals
        // whenever fetched < discovered, so a stale miss would silently
        // degrade the rest of the session.
        assert_eq!(second.stylesheets_fetched, 1);
        assert_eq!(second.css, ".flaky{}");
    }

    #[tokio::test]
    async fn without_a_cache_every_page_refetches_the_same_stylesheets() {
        let log: FetchLog = Default::default();

        let first = scan_page(None, &log).await;
        let second = scan_page(None, &log).await;

        assert_eq!(fetch_count(&log, SITE_CSS), 2);
        assert_eq!(fetch_count(&log, THEME_CSS), 2);
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn a_second_execution_never_reads_the_first_executions_stylesheets() {
        let log: FetchLog = Default::default();

        let first_execution = StylesheetCache::new();
        let first = scan_page(Some(&first_execution), &log).await;
        scan_page(Some(&first_execution), &log).await;
        assert_eq!(fetch_count(&log, SITE_CSS), 1);

        drop(first_execution);
        let second_execution = StylesheetCache::new();
        let refetched = scan_page(Some(&second_execution), &log).await;

        assert_eq!(
            fetch_count(&log, SITE_CSS),
            2,
            "a new scan execution must re-read the site, not serve last run's CSS"
        );
        assert_eq!(first, refetched);
    }

    #[tokio::test]
    async fn the_byte_budget_evicts_and_only_the_evicted_stylesheet_is_read_again() {
        // Room for two of the three ten-byte bodies.
        let cache = StylesheetCache::with_limits(64, 20);
        let log: FetchLog = Default::default();
        let page = |hrefs: &[&str]| -> String {
            hrefs
                .iter()
                .map(|href| format!(r#"<link rel="stylesheet" href="{href}">"#))
                .collect()
        };

        let first = fetch_stylesheets_with(
            &page(&[A_CSS, B_CSS, C_CSS]),
            &base(),
            false,
            Some(&cache),
            sized_fetcher(log.clone()),
        )
        .await;
        assert_eq!(first.stylesheets_fetched, 3);

        // The third body pushed the first one out; the other two are in budget.
        let second = fetch_stylesheets_with(
            &page(&[C_CSS, B_CSS]),
            &base(),
            false,
            Some(&cache),
            sized_fetcher(log.clone()),
        )
        .await;
        assert_eq!(
            fetch_count(&log, B_CSS),
            1,
            "a stylesheet still inside the budget must come from the cache"
        );
        assert_eq!(fetch_count(&log, C_CSS), 1);
        assert_eq!(
            second.css,
            format!("{}\n{}", "x".repeat(10), "x".repeat(10)),
            "a cached body must be served verbatim"
        );

        // The evicted one costs a second read rather than reading as absent.
        fetch_stylesheets_with(
            &page(&[A_CSS]),
            &base(),
            false,
            Some(&cache),
            sized_fetcher(log.clone()),
        )
        .await;
        assert_eq!(
            fetch_count(&log, A_CSS),
            2,
            "the byte budget evicted this stylesheet; the later page must re-read it"
        );
        assert!(cache.bytes() <= 20, "the byte budget must hold");
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
        let result =
            fetch_stylesheets(&html, &base(), crate::http_client::client(), false, None).await;
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
        let css = fetch_stylesheets(html, &base(), crate::http_client::client(), false, None).await;
        assert_eq!(
            css.css, "",
            "internal SSRF targets must be skipped, not fetched"
        );
        assert_eq!(css.stylesheets_discovered, 2);
        assert_eq!(css.stylesheets_fetched, 0);
        assert!(!css.coverage_complete());
    }
}
