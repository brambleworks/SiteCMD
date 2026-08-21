//! Ephemeral per-page signals consumed by cross-page session analysis.
//! They are skipped during serialization and never reach storage or the frontend.

use std::sync::LazyLock;

/// Per-page internal-link cap for bounded session memory.
pub(crate) const MAX_INTERNAL_LINKS_PER_PAGE: usize = 200;

#[derive(Debug, Clone)]
pub struct PageSignals {
    pub url: String,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub h1: Option<String>,
    pub canonical: Option<String>,
    pub noindex: bool,
    /// (hreflang value, resolved absolute href)
    pub hreflang: Vec<(String, String)>,
    /// Same-origin outgoing links, resolved absolute, normalized, deduped.
    pub internal_links: Vec<String>,
    /// True when more unique same-origin links existed after the bounded cap.
    pub internal_links_truncated: bool,
}

static TITLE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    let re = regex::Regex::new(r"(?is)<title(?:\s[^>]*)?>(.*?)</title\s*>");
    re.expect("static regex compiles") // allow-expect: compile-time literal regex
});
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

pub fn extract_page_signals(page_url: &url::Url, body: &str) -> PageSignals {
    extract_page_signals_with_headers(page_url, body, &reqwest::header::HeaderMap::new())
}

pub fn extract_page_signals_with_headers(
    page_url: &url::Url,
    body: &str,
    response_headers: &reqwest::header::HeaderMap,
) -> PageSignals {
    // Markup examples in scripts/comments/styles are not document metadata or
    // navigation. Use the same exclusion boundary as the page-level checks.
    let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(body, " ");
    let lower = scannable.to_ascii_lowercase();

    let title = TITLE_RE
        .captures(&scannable)
        .map(|c| collapse_whitespace(&TAG_STRIP_RE.replace_all(&c[1], " ")))
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

    let origin = crate::checks::origin_with_port(page_url);
    let mut internal_links = Vec::new();
    let mut internal_links_truncated = false;
    for tag in crate::checks::html_attrs::tag_slices(&scannable, &lower, "a") {
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
        if !matches!(resolved.scheme(), "http" | "https") {
            continue;
        }
        if crate::checks::origin_with_port(&resolved) != origin {
            continue;
        }
        let normalized = normalize_page_url(resolved.as_str());
        if !internal_links.contains(&normalized) {
            if internal_links.len() >= MAX_INTERNAL_LINKS_PER_PAGE {
                internal_links_truncated = true;
                break;
            }
            internal_links.push(normalized);
        }
    }

    PageSignals {
        url: normalize_page_url(page_url.as_str()),
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
        let from_header = extract_page_signals_with_headers(
            &url::Url::parse("https://example.com/").unwrap(),
            "<html></html>",
            &headers,
        );
        assert!(from_header.noindex);

        let mut news_headers = reqwest::header::HeaderMap::new();
        news_headers.insert(
            reqwest::header::HeaderName::from_static("x-robots-tag"),
            reqwest::header::HeaderValue::from_static("googlebot-news: noindex"),
        );
        let news_only = extract_page_signals_with_headers(
            &url::Url::parse("https://example.com/").unwrap(),
            "<html></html>",
            &news_headers,
        );
        assert!(!news_only.noindex);
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
