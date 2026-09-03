//! Ephemeral per-page signals consumed by cross-page session analysis.
//! They are skipped during serialization and never reach storage or the frontend.

use std::sync::LazyLock;

/// Per-page internal-link cap for bounded session memory.
pub(crate) const MAX_INTERNAL_LINKS_PER_PAGE: usize = 200;

#[derive(Debug, Clone)]
pub struct PageSignals {
    pub url: String,
    /// The normalized URL this scan asked for, before any redirect. A page
    /// linked only through its pre-redirect URL is still a linked page.
    pub requested_url: String,
    /// Status of the final response the signals were read from. Session
    /// analysis compares pages, and an error page is not the page that was
    /// asked for, so error responses are excluded from every comparison.
    pub status_code: u16,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Option<String>,
    pub canonical: Option<String>,
    pub noindex: bool,
    /// (hreflang value, resolved absolute href)
    pub hreflang: Vec<(String, String)>,
    /// Same-site outgoing links, resolved absolute, folded onto the scanned
    /// origin, normalized, deduped.
    pub internal_links: Vec<String>,
    /// True when more unique same-site links existed after the bounded cap.
    pub internal_links_truncated: bool,
}

static H1_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    let re = regex::Regex::new(r"(?is)<h1(?:\s[^>]*)?>(.*?)</h1\s*>");
    re.expect("static regex compiles") // allow-expect: compile-time literal regex
});
static TAG_STRIP_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<[^>]*>").expect("static regex compiles")); // allow-expect: compile-time literal regex
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rel_has(rel: &str, expected: &str) -> bool {
    rel.split_ascii_whitespace()
        .any(|token| token.eq_ignore_ascii_case(expected))
}

fn has_noindex_directive(value: &str) -> bool {
    value
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .map(|token| token.trim_matches(':'))
        .any(|token| token.eq_ignore_ascii_case("noindex") || token.eq_ignore_ascii_case("none"))
}

fn x_robots_blocks_general_index(value: &str) -> bool {
    let mut applies_to_general_index = None;
    value.split(',').any(|clause| {
        let clause = clause.trim();
        if let Some((agent, directives)) = clause.split_once(':') {
            let applies = matches!(
                agent.trim().to_ascii_lowercase().as_str(),
                "googlebot" | "robots" | "*"
            );
            applies_to_general_index = Some(applies);
            applies && has_noindex_directive(directives)
        } else {
            applies_to_general_index.unwrap_or(true) && has_noindex_directive(clause)
        }
    })
}

/// A link to the same site as `page_url`, rewritten onto the scanned origin, or
/// `None` when the link leaves the site.
///
/// A site reached at `https://example.com` still links to itself as
/// `http://example.com/...` and `https://www.example.com/...`. Those are the
/// same pages, so an anchor written that way is a real inbound link; treating
/// the twins as foreign origins made the pages they point at look unlinked.
/// Only the scheme and a leading `www.` may differ: an explicit port and the
/// rest of the host must match, so a genuinely different host stays foreign.
fn same_site_link(resolved: &url::Url, page_url: &url::Url) -> Option<url::Url> {
    if !matches!(resolved.scheme(), "http" | "https") {
        return None;
    }
    if resolved.port() != page_url.port() {
        return None;
    }
    fn bare_host(url: &url::Url) -> Option<String> {
        let host = url.host_str()?.to_ascii_lowercase();
        Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
    }
    if bare_host(resolved)? != bare_host(page_url)? {
        return None;
    }
    let mut on_scanned_origin = resolved.clone();
    on_scanned_origin.set_scheme(page_url.scheme()).ok()?;
    on_scanned_origin.set_host(page_url.host_str()).ok()?;
    Some(on_scanned_origin)
}

/// Normalize a URL for cross-page identity comparison: drop the fragment,
/// lowercase host, trim a trailing slash on non-root paths.
pub fn normalize_page_url(input: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(input) else {
        return input.trim_end_matches('/').to_string();
    };
    parsed.set_fragment(None);
    let mut out = parsed.to_string();
    if parsed.path() != "/" && out.ends_with('/') {
        out.pop();
    }
    out
}

/// Signals for a page fetched directly at `page_url` with a 200 response.
pub fn extract_page_signals(page_url: &url::Url, body: &str) -> PageSignals {
    extract_page_signals_with_headers(
        page_url,
        page_url,
        200,
        body,
        &reqwest::header::HeaderMap::new(),
    )
}

/// Signals for a fetched page. `page_url` is the effective (post-redirect) URL
/// the body came from, `requested_url` is the URL the scan asked for, and
/// `status_code` is the final response status.
pub fn extract_page_signals_with_headers(
    page_url: &url::Url,
    requested_url: &url::Url,
    status_code: u16,
    body: &str,
    response_headers: &reqwest::header::HeaderMap,
) -> PageSignals {
    // Markup examples in scripts/comments/styles are not document metadata or
    // navigation. Use the same exclusion boundary as the page-level checks.
    let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(body, " ");
    let lower = scannable.to_ascii_lowercase();

    // One title authority with the page-level title check: an inline SVG
    // <title> labels a graphic, not the document, and must not become the
    // title every page on the site appears to share.
    let title = sitecmd_engine::checks::seo::parsing::extract_document_title(body)
        .map(|t| collapse_whitespace(&TAG_STRIP_RE.replace_all(&t, " ")))
        .filter(|t| !t.is_empty());

    let h1 = H1_RE
        .captures(&scannable)
        .map(|c| collapse_whitespace(&TAG_STRIP_RE.replace_all(&c[1], " ")))
        .filter(|t| !t.is_empty());

    let mut meta_description = None;
    let mut noindex = response_headers
        .get_all("x-robots-tag")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(x_robots_blocks_general_index);
    for tag in crate::checks::html_attrs::tag_slices(&scannable, &lower, "meta") {
        let name = crate::checks::html_attrs::attr_value(tag, "name")
            .map(|name| name.to_ascii_lowercase());
        match name.as_deref() {
            Some("description") if meta_description.is_none() => {
                meta_description = crate::checks::html_attrs::attr_value(tag, "content")
                    .map(|c| collapse_whitespace(&c))
                    .filter(|c| !c.is_empty());
            }
            Some("robots") | Some("googlebot")
                if crate::checks::html_attrs::attr_value(tag, "content")
                    .is_some_and(|content| has_noindex_directive(&content)) =>
            {
                noindex = true;
            }
            _ => {}
        }
    }

    let document_base = crate::checks::html_attrs::tag_slices(&scannable, &lower, "base")
        .into_iter()
        .find_map(|tag| crate::checks::html_attrs::attr_value(tag, "href"))
        .and_then(|href| page_url.join(href.trim()).ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .unwrap_or_else(|| page_url.clone());

    let mut canonical = None;
    let mut hreflang = Vec::new();
    for tag in crate::checks::html_attrs::tag_slices(&scannable, &lower, "link") {
        let rel = crate::checks::html_attrs::attr_value(tag, "rel").unwrap_or_default();
        if canonical.is_none() && rel_has(&rel, "canonical") {
            canonical = crate::checks::html_attrs::attr_value(tag, "href")
                .and_then(|href| document_base.join(href.trim()).ok())
                .map(|u| normalize_page_url(u.as_str()));
        }
        if rel_has(&rel, "alternate") {
            if let (Some(lang), Some(href)) = (
                crate::checks::html_attrs::attr_value(tag, "hreflang"),
                crate::checks::html_attrs::attr_value(tag, "href"),
            ) {
                if let Ok(resolved) = document_base.join(href.trim()) {
                    hreflang.push((
                        lang.trim().to_ascii_lowercase(),
                        normalize_page_url(resolved.as_str()),
                    ));
                }
            }
        }
    }

    // Inbound-link evidence is deliberately generous: a rel="nofollow" anchor
    // and an anchor inside <template> or <noscript> still show that the site
    // links to the page, which is the question orphan analysis asks. Counting
    // them can only remove an orphan claim, never invent one. Anchors written
    // inside scripts, comments, and styles are excluded above because they are
    // markup examples rather than navigation. Same-site scheme and www twins are
    // folded onto the scanned origin by `same_site_link` so a page linked only
    // as its http:// or www form is not reported as unlinked.
    // Links already written on the scanned origin are collected first and
    // folded twins only with the budget left over. Sharing one pass would let a
    // twin take the last slot under the cap and push a real link out of the
    // set, which is the one direction this fold must never go.
    let anchors = crate::checks::html_attrs::tag_slices(&scannable, &lower, "a");
    let mut internal_links: Vec<String> = Vec::new();
    let mut internal_links_truncated = false;
    for collecting_twins in [false, true] {
        for tag in anchors.iter().copied() {
            let Some(href) = crate::checks::html_attrs::attr_value(tag, "href") else {
                continue;
            };
            let href = href.trim();
            if href.is_empty() || href.starts_with('#') {
                continue;
            }
            let Ok(resolved) = document_base.join(href) else {
                continue;
            };
            let Some(same_site) = same_site_link(&resolved, page_url) else {
                continue;
            };
            if (same_site != resolved) != collecting_twins {
                continue;
            }
            let normalized = normalize_page_url(same_site.as_str());
            if internal_links.contains(&normalized) {
                continue;
            }
            if internal_links.len() >= MAX_INTERNAL_LINKS_PER_PAGE {
                internal_links_truncated = true;
                break;
            }
            internal_links.push(normalized);
        }
    }

    PageSignals {
        url: normalize_page_url(page_url.as_str()),
        requested_url: normalize_page_url(requested_url.as_str()),
        status_code,
        title,
        meta_description,
        h1,
        canonical,
        noindex,
        hreflang,
        internal_links,
        internal_links_truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals(url: &str, body: &str) -> PageSignals {
        extract_page_signals(&url::Url::parse(url).unwrap(), body)
    }

    fn signals_with_headers(
        url: &str,
        body: &str,
        headers: &reqwest::header::HeaderMap,
    ) -> PageSignals {
        let parsed = url::Url::parse(url).unwrap();
        extract_page_signals_with_headers(&parsed, &parsed, 200, body, headers)
    }

    #[test]
    fn extracts_title_description_h1() {
        let s = signals(
            "https://example.com/page",
            r#"<title>  My   Page </title>
               <meta name="description" content="A description.">
               <h1>Main <em>Heading</em></h1>"#,
        );
        assert_eq!(s.title.as_deref(), Some("My Page"));
        assert_eq!(s.meta_description.as_deref(), Some("A description."));
        assert_eq!(s.h1.as_deref(), Some("Main Heading"));
    }

    #[test]
    fn detects_noindex_and_canonical() {
        let s = signals(
            "https://example.com/a/",
            r#"<meta name="robots" content="noindex, nofollow">
               <link rel="canonical" href="/b/">"#,
        );
        assert!(s.noindex);
        assert_eq!(s.canonical.as_deref(), Some("https://example.com/b"));
    }

    #[test]
    fn collects_same_origin_links_only_normalized() {
        let s = signals(
            "https://example.com/",
            r#"<a href="/about/">About</a>
               <a href="/about/#team">Team</a>
               <a href="https://other.com/x">External</a>
               <a href="mailto:hi@example.com">Mail</a>"#,
        );
        assert_eq!(s.internal_links, vec!["https://example.com/about"]);
    }

    #[test]
    fn same_site_scheme_and_www_twins_are_links_to_this_site() {
        let s = signals(
            "https://example.com/",
            r#"<a href="http://example.com/http-twin">h</a>
               <a href="https://www.example.com/www-twin">w</a>
               <a href="https://example.com/plain">p</a>
               <a href="https://other.com/x">o</a>
               <a href="https://example.com.evil.test/lookalike">e</a>
               <a href="https://notexample.com/x">n</a>"#,
        );
        assert_eq!(
            s.internal_links,
            vec![
                "https://example.com/plain",
                "https://example.com/http-twin",
                "https://example.com/www-twin",
            ]
        );
    }

    #[test]
    fn a_site_scanned_on_its_www_host_folds_the_bare_host_the_same_way() {
        let s = signals(
            "https://www.example.com/",
            r#"<a href="https://example.com/bare">b</a>"#,
        );
        assert_eq!(s.internal_links, vec!["https://www.example.com/bare"]);
    }

    #[test]
    fn an_explicit_port_still_separates_sites() {
        let s = signals(
            "https://example.com:8443/",
            r#"<a href="https://example.com:9443/other-port">o</a>
               <a href="http://example.com:8443/same-port">s</a>"#,
        );
        assert_eq!(s.internal_links, vec!["https://example.com:8443/same-port"]);
    }

    /// The per-page cap bounds session memory, and a folded twin must not spend
    /// it: a twin taking the last slot would drop a link written on the scanned
    /// origin and could invent the orphan claim the fold exists to remove.
    #[test]
    fn a_folded_twin_never_takes_the_slot_of_a_link_on_the_scanned_origin() {
        let mut body = String::from(r#"<a href="http://example.com/twin-first">t</a>"#);
        for i in 0..MAX_INTERNAL_LINKS_PER_PAGE {
            body.push_str(&format!(r#"<a href="/page-{i}">p</a>"#));
        }
        let s = signals("https://example.com/", &body);

        assert_eq!(s.internal_links.len(), MAX_INTERNAL_LINKS_PER_PAGE);
        assert!(
            !s.internal_links
                .contains(&"https://example.com/twin-first".to_string()),
            "the twin displaced a link written on the scanned origin"
        );
        assert!(s.internal_links.contains(&format!(
            "https://example.com/page-{}",
            MAX_INTERNAL_LINKS_PER_PAGE - 1
        )));
        assert!(s.internal_links_truncated);
    }

    #[test]
    fn extracts_hreflang_pairs() {
        let s = signals(
            "https://example.com/",
            r#"<link rel="alternate" hreflang="de" href="/de/">
               <link rel="alternate" hreflang="en" href="https://example.com/">"#,
        );
        assert_eq!(s.hreflang.len(), 2);
        assert_eq!(s.hreflang[0].0, "de");
        assert_eq!(s.hreflang[0].1, "https://example.com/de");
    }

    #[test]
    fn supports_unquoted_attributes_and_ignores_markup_examples() {
        let s = signals(
            "https://example.com/",
            r#"<script>const sample = '<a href=/fake>fake</a>';</script>
               <!-- <link rel=canonical href=/wrong> -->
               <link rel=canonical href=/real>
               <a href=/about>About</a>"#,
        );
        assert_eq!(s.canonical.as_deref(), Some("https://example.com/real"));
        assert_eq!(s.internal_links, vec!["https://example.com/about"]);
    }

    #[test]
    fn applies_the_document_base_to_resolved_page_relationships() {
        let s = signals(
            "https://example.com/docs/page",
            r#"<base href="https://example.com/locales/">
               <link rel=canonical href=en>
               <link rel=alternate hreflang=de href=de>
               <a href=help>Help</a>"#,
        );
        assert_eq!(
            s.canonical.as_deref(),
            Some("https://example.com/locales/en")
        );
        assert_eq!(s.hreflang[0].1, "https://example.com/locales/de");
        assert_eq!(s.internal_links, vec!["https://example.com/locales/help"]);
    }

    #[test]
    fn noindex_matching_uses_directive_tokens_and_response_headers() {
        let false_positive = signals(
            "https://example.com/",
            r#"<meta name=robots content=noindexif>"#,
        );
        assert!(!false_positive.noindex);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.append(
            reqwest::header::HeaderName::from_static("x-robots-tag"),
            reqwest::header::HeaderValue::from_static("googlebot: noindex, nofollow"),
        );
        let from_header = signals_with_headers("https://example.com/", "<html></html>", &headers);
        assert!(from_header.noindex);

        let mut news_headers = reqwest::header::HeaderMap::new();
        news_headers.insert(
            reqwest::header::HeaderName::from_static("x-robots-tag"),
            reqwest::header::HeaderValue::from_static("googlebot-news: noindex"),
        );
        let news_only =
            signals_with_headers("https://example.com/", "<html></html>", &news_headers);
        assert!(!news_only.noindex);
    }

    #[test]
    fn an_inline_svg_title_is_not_the_document_title() {
        let icon_only = signals(
            "https://example.com/x",
            r#"<html><head></head><body>
               <svg viewBox="0 0 24 24"><title>Menu icon</title></svg>
               <h1>Page</h1></body></html>"#,
        );
        assert_eq!(icon_only.title, None);

        let real_title = signals(
            "https://example.com/y",
            r#"<html><head><title>Real Title</title></head><body>
               <svg><title>Menu icon</title></svg></body></html>"#,
        );
        assert_eq!(real_title.title.as_deref(), Some("Real Title"));
    }

    #[test]
    fn a_response_carries_its_status_and_the_url_that_was_requested() {
        let signals = extract_page_signals_with_headers(
            &url::Url::parse("https://example.com/new").unwrap(),
            &url::Url::parse("https://example.com/old/").unwrap(),
            404,
            "<html><head><title>Not found</title></head></html>",
            &reqwest::header::HeaderMap::new(),
        );

        assert_eq!(signals.url, "https://example.com/new");
        assert_eq!(signals.requested_url, "https://example.com/old");
        assert_eq!(signals.status_code, 404);
    }

    #[test]
    fn a_page_fetched_without_a_redirect_reports_itself_as_the_requested_url() {
        let signals = signals("https://example.com/a/", "<html></html>");

        assert_eq!(signals.requested_url, signals.url);
        assert_eq!(signals.status_code, 200);
    }

    #[test]
    fn normalize_treats_trailing_slash_and_fragment_as_same_page() {
        assert_eq!(
            normalize_page_url("https://example.com/a/"),
            normalize_page_url("https://example.com/a#x")
        );
        assert_eq!(
            normalize_page_url("https://example.com/"),
            "https://example.com/"
        );
    }
}
