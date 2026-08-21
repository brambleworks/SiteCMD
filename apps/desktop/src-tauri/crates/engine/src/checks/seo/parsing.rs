//! Shared metadata-parsing helpers for the SEO checks: attribute reads over
//! the HTML tokenizer, meta extraction from scannable markup, and document
//! title extraction.

use regex::Regex;
use std::sync::LazyLock;

/// An inline SVG block. Its `<title>` is an accessible label for the graphic,
/// not a document title, so it is stripped before counting `<title>` tags.
pub static SVG_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<svg\b.*?</svg>").expect("valid svg block regex"));
// allow-expect: compile-time literal regex

/// Reads an HTML attribute using the shared HTML tokenizer. This supports
/// quoted, unquoted, boolean, and whitespace-around-equals syntax without
/// accepting a prefixed attribute such as `data-name`.
pub fn extract_attr_value(tag: &str, attr: &str) -> Option<String> {
    crate::checks::html_attrs::attr_value(tag, attr)
}

/// Extracts the content of the first matching meta name/property in scannable
/// initial markup. Meta-looking examples inside comments, scripts, and styles
/// are deliberately excluded.
pub fn extract_meta(body: &str, name: &str) -> Option<String> {
    let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(body, " ");
    let lower = scannable.to_ascii_lowercase();
    for tag in crate::checks::html_attrs::tag_slices(&scannable, &lower, "meta") {
        let matches_name = extract_attr_value(tag, "name")
            .or_else(|| extract_attr_value(tag, "property"))
            .is_some_and(|value| value.eq_ignore_ascii_case(name));

        if matches_name {
            return extract_attr_value(tag, "content");
        }
    }
    None
}

/// Extract the first document-title candidate outside comments, scripts,
/// styles, and inline SVG. An SVG `<title>` labels that graphic and must not
/// satisfy the page-title check.
pub fn extract_document_title(body: &str) -> Option<String> {
    let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(body, " ");
    let without_svg = SVG_BLOCK_RE.replace_all(&scannable, " ");
    let lower = without_svg.to_ascii_lowercase();
    let opening = crate::checks::html_attrs::tag_slices(&without_svg, &lower, "title")
        .into_iter()
        .next()?;
    let opening_start = without_svg.find(opening)?;
    let content_start = opening_start + opening.len();
    let mut cursor = content_start;
    let close_start = loop {
        let relative = lower[cursor..].find("</title")?;
        let candidate = cursor + relative;
        let after_name = candidate + "</title".len();
        if matches!(
            lower[after_name..].chars().next(),
            Some(' ' | '\t' | '\n' | '\r' | '>')
        ) {
            break candidate;
        }
        cursor = after_name;
    };
    let content = without_svg[content_start..close_start].trim();
    (!content.is_empty()).then(|| content.to_string())
}
