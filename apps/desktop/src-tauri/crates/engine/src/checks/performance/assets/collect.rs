//! Policy-gated, deduplicated asset URL collection with bounded samples.

use crate::checks::html_attrs::{attr_value, tag_slices};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// What kind of asset a URL was referenced as. Drives sampling priority and
/// the per-kind byte breakdown in raw_data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Image,
    Script,
    Style,
}

impl AssetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Script => "script",
            Self::Style => "style",
        }
    }

    /// Images first: they dominate real-world page weight, so when the cap
    /// bites we keep the sample where the bytes are.
    fn sample_priority(self) -> u8 {
        match self {
            Self::Image => 0,
            Self::Script => 1,
            Self::Style => 2,
        }
    }
}

/// A fetchable asset reference extracted from page markup.
#[derive(Debug, Clone)]
pub struct CollectedAsset {
    pub url: url::Url,
    pub kind: AssetKind,
    /// Whether the element declared responsive candidates.
    pub has_srcset: bool,
    /// Group whose responsive candidates count as one browser download.
    /// Scripts and styles use singleton groups.
    pub group: u32,
}

/// Everything the sampler learned from the markup before any network I/O.
#[derive(Debug, Default)]
pub struct AssetCollection {
    /// Deduplicated, prioritized list of assets to fetch, capped at the
    /// sample limit.
    pub sampled: Vec<CollectedAsset>,
    /// Fetchable assets discovered before the sample cap was applied.
    pub fetchable_found: usize,
    /// Every asset reference discovered, including data URIs and skipped ones.
    pub found: usize,
    /// Inline data: URIs, counted toward page weight without fetching.
    pub data_uri_count: usize,
    pub data_uri_bytes: u64,
    /// References skipped because they cannot or must not be fetched
    /// (blob:, javascript:, unparseable, or refused by the runtime's
    /// network policy).
    pub skipped_unsupported: usize,
}

/// Collect, validate, dedup, prioritize, and cap asset URLs from page markup.
/// `lower` must preserve `body` byte offsets, and `allow_target` validates every
/// untrusted resolved URL before sampling.
pub fn collect_assets(
    body: &str,
    lower: &str,
    page_url: &url::Url,
    allow_target: impl Fn(&url::Url) -> bool,
    limit: usize,
) -> AssetCollection {
    // (kind, url, has_srcset, group). Every push takes the next group id; the
    // src + srcset candidates of a single image element share one id.
    let mut refs: Vec<(AssetKind, String, bool, u32)> = Vec::new();
    let mut next_group: u32 = 0;

    for tag in tag_slices(body, lower, "img") {
        collect_image_like(tag, &mut refs, next_group);
        next_group += 1;
    }
    for tag in tag_slices(body, lower, "source") {
        collect_image_like(tag, &mut refs, next_group);
        next_group += 1;
    }
    for tag in tag_slices(body, lower, "script") {
        if let Some(src) = attr_value(tag, "src") {
            refs.push((AssetKind::Script, src, false, next_group));
            next_group += 1;
        }
    }
    for tag in tag_slices(body, lower, "link") {
        let is_stylesheet = attr_value(tag, "rel").is_some_and(|rel| {
            rel.split_whitespace()
                .any(|token| token.eq_ignore_ascii_case("stylesheet"))
        });
        if is_stylesheet {
            if let Some(href) = attr_value(tag, "href") {
                refs.push((AssetKind::Style, href, false, next_group));
                next_group += 1;
            }
        }
    }

    let mut collection = AssetCollection::default();
    let mut index_by_url: HashMap<String, usize> = HashMap::new();
    let mut fetchable: Vec<CollectedAsset> = Vec::new();

    for (kind, raw, has_srcset, group) in refs {
        let reference = raw.trim();
        if reference.is_empty() {
            continue;
        }
        collection.found += 1;

        let scheme_probe = reference.to_ascii_lowercase();
        if scheme_probe.starts_with("data:") {
            collection.data_uri_count += 1;
            collection.data_uri_bytes += data_uri_estimated_bytes(reference);
            continue;
        }
        // Never fetch page-controlled non-HTTP schemes.
        if scheme_probe.starts_with("blob:") || scheme_probe.starts_with("javascript:") {
            collection.skipped_unsupported += 1;
            continue;
        }

        let resolved = match page_url.join(reference) {
            Ok(resolved) => resolved,
            Err(_) => {
                collection.skipped_unsupported += 1;
                continue;
            }
        };
        // SSRF gate: asset URLs come from untrusted page markup, exactly like
        // external link hrefs. The runtime's policy decides; the refusal is
        // recorded here.
        if !allow_target(&resolved) {
            collection.skipped_unsupported += 1;
            continue;
        }

        match index_by_url.entry(resolved.to_string()) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                let existing = &mut fetchable[*entry.get()];
                existing.has_srcset = existing.has_srcset || has_srcset;
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(fetchable.len());
                fetchable.push(CollectedAsset {
                    url: resolved,
                    kind,
                    has_srcset,
                    group,
                });
            }
        }
    }

    collection.fetchable_found = fetchable.len();
    // Stable sort keeps document order within each kind.
    fetchable.sort_by_key(|asset| asset.kind.sample_priority());
    fetchable.truncate(limit);
    collection.sampled = fetchable;
    collection
}

/// Extract src + srcset references from an `<img>` or `<source>` tag. All
/// references from one element share `group` so the weight check counts only
/// one of them (a browser downloads a single candidate per element).
fn collect_image_like(tag: &str, refs: &mut Vec<(AssetKind, String, bool, u32)>, group: u32) {
    let candidates: Vec<String> = attr_value(tag, "srcset")
        .as_deref()
        .map(parse_srcset)
        .unwrap_or_default();
    let has_srcset = !candidates.is_empty();
    if let Some(src) = attr_value(tag, "src") {
        refs.push((AssetKind::Image, src, has_srcset, group));
    }
    for candidate in candidates {
        refs.push((AssetKind::Image, candidate, true, group));
    }
}

/// Parse srcset URLs while preserving comma-bearing data URIs as one candidate.
pub(super) fn parse_srcset(value: &str) -> Vec<String> {
    let (head, data_tail) = match value.to_ascii_lowercase().find("data:") {
        Some(index) => (&value[..index], Some(value[index..].to_string())),
        None => (value, None),
    };
    let mut urls: Vec<String> = head
        .split(',')
        .filter_map(|candidate| {
            candidate
                .split_whitespace()
                .next()
                .map(|url| url.to_string())
        })
        .filter(|url| !url.is_empty())
        .collect();
    if let Some(tail) = data_tail {
        urls.push(tail);
    }
    urls
}

/// Estimate the byte weight an inline data: URI adds to the document.
/// base64 payloads decode to roughly 3/4 of their text length; other
/// payloads are counted as-is.
fn data_uri_estimated_bytes(value: &str) -> u64 {
    let Some(comma) = value.find(',') else {
        return 0;
    };
    let meta = &value[..comma];
    let payload = &value[comma + 1..];
    if meta.to_ascii_lowercase().contains(";base64") {
        (payload.len() as u64 * 3) / 4
    } else {
        payload.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allow_all(_url: &url::Url) -> bool {
        true
    }

    fn collect(html: &str, page: &str, limit: usize) -> AssetCollection {
        let lower = html.to_ascii_lowercase();
        let page_url = url::Url::parse(page).expect("page url");
        collect_assets(html, &lower, &page_url, allow_all, limit)
    }

    fn sampled_urls(collection: &AssetCollection) -> Vec<String> {
        collection
            .sampled
            .iter()
            .map(|asset| asset.url.to_string())
            .collect()
    }

    #[test]
    fn collects_all_supported_asset_sources_and_resolves_relative_urls() {
        let html = r#"<html><head>
            <link rel="stylesheet" href="/css/App.css">
            <link rel="preload" href="/skip/preload.woff2">
            <script src="js/Main.js"></script>
            <script>inline();</script>
        </head><body>
            <img src="/img/Hero.png" alt="hero">
            <picture>
                <source srcset="/img/hero.avif" type="image/avif">
                <img src="/img/fallback.jpg" alt="fallback">
            </picture>
        </body></html>"#;
        let collection = collect(html, "https://example.com/page/", 30);
        let urls = sampled_urls(&collection);
        assert!(urls.contains(&"https://example.com/css/App.css".to_string()));
        assert!(urls.contains(&"https://example.com/page/js/Main.js".to_string()));
        assert!(urls.contains(&"https://example.com/img/Hero.png".to_string()));
        assert!(urls.contains(&"https://example.com/img/hero.avif".to_string()));
        assert!(urls.contains(&"https://example.com/img/fallback.jpg".to_string()));
        assert!(
            !urls.iter().any(|url| url.contains("preload")),
            "non-stylesheet links must not be collected"
        );
        assert_eq!(collection.sampled.len(), 5);
        assert_eq!(collection.found, 5);
    }

    #[test]
    fn preserves_original_url_casing() {
        let html = r#"<IMG SRC="/Photos/IMG_2024.JPG" alt="x">"#;
        let collection = collect(html, "https://example.com/", 30);
        assert_eq!(
            sampled_urls(&collection),
            vec!["https://example.com/Photos/IMG_2024.JPG".to_string()]
        );
    }

    #[test]
    fn dedups_repeated_urls_and_merges_srcset_flags() {
        let html = r#"
            <img src="/img/a.png" alt="one">
            <img src="/img/a.png" srcset="/img/a.png 1x, /img/a-2x.png 2x" alt="two">
        "#;
        let collection = collect(html, "https://example.com/", 30);
        assert_eq!(collection.sampled.len(), 2, "a.png must appear once");
        let a = collection
            .sampled
            .iter()
            .find(|asset| asset.url.path() == "/img/a.png")
            .expect("a.png collected");
        assert!(
            a.has_srcset,
            "srcset presence must survive dedup against the plain reference"
        );
    }

    #[test]
    fn prioritizes_images_over_scripts_over_styles_when_capped() {
        let html = r#"
            <link rel="stylesheet" href="/one.css">
            <script src="/one.js"></script>
            <img src="/one.png" alt="a">
            <img src="/two.png" alt="b">
            <script src="/two.js"></script>
        "#;
        let collection = collect(html, "https://example.com/", 3);
        let urls = sampled_urls(&collection);
        assert_eq!(
            urls,
            vec![
                "https://example.com/one.png".to_string(),
                "https://example.com/two.png".to_string(),
                "https://example.com/one.js".to_string(),
            ]
        );
        assert_eq!(collection.fetchable_found, 5);
        assert_eq!(collection.found, 5);
    }

    #[test]
    fn counts_data_uris_without_fetching() {
        // 8 base64 chars decode to 6 bytes.
        let html = r#"<img src="data:image/png;base64,AAAAAAAA" alt="dot">"#;
        let collection = collect(html, "https://example.com/", 30);
        assert!(collection.sampled.is_empty());
        assert_eq!(collection.data_uri_count, 1);
        assert_eq!(collection.data_uri_bytes, 6);
        assert_eq!(collection.found, 1);
    }

    #[test]
    fn skips_blob_and_javascript_schemes() {
        let html = r#"
            <img src="blob:https://example.com/uuid" alt="a">
            <script src="javascript:alert(1)"></script>
            <img src="/real.png" alt="b">
        "#;
        let collection = collect(html, "https://example.com/", 30);
        assert_eq!(
            sampled_urls(&collection),
            vec!["https://example.com/real.png".to_string()]
        );
        assert_eq!(collection.skipped_unsupported, 2);
        assert_eq!(collection.found, 3);
    }

    #[test]
    fn refused_targets_count_as_skipped_unsupported() {
        let html = r#"
            <img src="http://169.254.169.254/latest/meta-data/x.png" alt="a">
            <img src="https://cdn.example.com/ok.png" alt="c">
        "#;
        let lower = html.to_ascii_lowercase();
        let page_url = url::Url::parse("https://example.com/").expect("page url");
        let collection = collect_assets(
            html,
            &lower,
            &page_url,
            |url| url.host_str() != Some("169.254.169.254"),
            30,
        );
        assert_eq!(
            sampled_urls(&collection),
            vec!["https://cdn.example.com/ok.png".to_string()]
        );
        assert_eq!(collection.skipped_unsupported, 1);
    }

    #[test]
    fn parse_srcset_extracts_urls_and_drops_descriptors() {
        assert_eq!(
            parse_srcset("/img/a-320.jpg 320w, /img/a-640.jpg 640w"),
            vec!["/img/a-320.jpg".to_string(), "/img/a-640.jpg".to_string()]
        );
        assert_eq!(
            parse_srcset(" hero.png 1x ,hero-2x.png 2x "),
            vec!["hero.png".to_string(), "hero-2x.png".to_string()]
        );
        assert!(parse_srcset("").is_empty());
        assert!(parse_srcset("  ,  ").is_empty());
    }

    #[test]
    fn parse_srcset_keeps_data_uris_opaque() {
        // The comma inside a data URI must not be split into bogus candidates.
        let candidates = parse_srcset("/small.png 1x, data:image/png;base64,AAAA 2x");
        assert_eq!(candidates[0], "/small.png");
        assert_eq!(candidates.len(), 2);
        assert!(candidates[1].starts_with("data:"));
    }

    #[test]
    fn attr_value_ignores_data_src_and_preserves_case() {
        let tag = r#"<img data-src="/lazy/Wrong.png" src="/right/Photo.PNG">"#;
        assert_eq!(attr_value(tag, "src"), Some("/right/Photo.PNG".to_string()));
        let tag = r#"<img data-src="/lazy/only.png">"#;
        assert_eq!(attr_value(tag, "src"), None);
    }

    #[test]
    fn unquoted_attributes_on_minified_html_are_collected() {
        let collection = collect(
            "<img src=/hero.png><script src=/app.js></script><link rel=stylesheet href=/style.css>",
            "https://example.com/",
            30,
        );
        let urls = sampled_urls(&collection);
        assert!(urls.contains(&"https://example.com/hero.png".to_string()));
        assert!(urls.contains(&"https://example.com/app.js".to_string()));
        assert!(urls.contains(&"https://example.com/style.css".to_string()));
    }

    #[test]
    fn data_uri_estimated_bytes_handles_base64_and_plain_payloads() {
        assert_eq!(
            data_uri_estimated_bytes("data:image/png;base64,AAAAAAAA"),
            6
        );
        assert_eq!(data_uri_estimated_bytes("data:text/plain,hello"), 5);
        assert_eq!(data_uri_estimated_bytes("data:image/png"), 0);
    }
}
