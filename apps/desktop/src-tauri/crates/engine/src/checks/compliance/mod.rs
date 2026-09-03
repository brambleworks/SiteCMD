//! Portable compliance checks (sync page analysis).

use crate::checks::html_attrs::{attr_value, tag_slices};

pub mod consent_mode;
pub mod cookie_consent;
pub mod gdpr;
pub mod legal_documents;
pub mod statements;
pub mod trackers;

/// Strip comments, scripts, and styles before matching a keyword signal: a
/// disclosure a visitor cannot read is not a disclosure.
///
/// `NON_CONTENT_BLOCK_RE` needs a closing tag, so a `<script>` truncated by the
/// body cap is not stripped and its text still reaches these predicates.
pub fn content_text_lower(lower: &str) -> std::borrow::Cow<'_, str> {
    crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(lower, " ")
}

/// Comments only, for signals that legitimately live in script text (loader
/// hosts, instrumentation attributes, the GPC browser API). Nothing executes
/// or displays a commented-out snippet, so it must not count as live tracking.
pub fn executable_text_lower(lower: &str) -> std::borrow::Cow<'_, str> {
    HTML_COMMENT_RE.replace_all(lower, " ")
}

static HTML_COMMENT_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?s)<!--.*?-->").expect("static comment regex")
});

/// The rooted path of an href, without its query or fragment. A bare relative
/// token (`href="privacy"`) returns nothing: the substring form this replaced
/// only ever credited rooted paths, and widening detection is not part of
/// tightening it.
fn rooted_href_path(href: &str) -> &str {
    let path = href.split(['?', '#']).next().unwrap_or(href);
    if let Some((_, rest)) = path.split_once("://") {
        return rest.find('/').map(|index| &rest[index..]).unwrap_or("");
    }
    if let Some(rest) = path.strip_prefix("//") {
        return rest.find('/').map(|index| &rest[index..]).unwrap_or("");
    }
    if path.starts_with('/') {
        path
    } else {
        ""
    }
}

/// True when some `<a href>` in the (lowercased) markup has a path segment the
/// predicate accepts. Segment matching is what keeps `/menu/tostadas` from
/// reading as a `/tos` link.
pub fn anchor_href_segment_matches(lower: &str, matches: impl Fn(&str) -> bool) -> bool {
    for tag in tag_slices(lower, lower, "a") {
        let Some(href) = attr_value(tag, "href") else {
            continue;
        };
        if rooted_href_path(&href)
            .split('/')
            .filter(|part| !part.is_empty())
            .any(&matches)
        {
            return true;
        }
    }
    false
}

/// The first `-`, `_`, or `.`-delimited word of a path segment, so
/// `terms-of-use` reads as "terms" while `termsheet` does not.
pub fn path_segment_head(segment: &str) -> &str {
    segment.split(['-', '_', '.']).next().unwrap_or(segment)
}

/// Multilingual privacy-policy link text tokens shared by compliance checks.
/// Keep `PRIVACY_LINK_LANGUAGES` in the confidence copy synchronized. Path
/// forms are evaluated against anchor hrefs instead, in
/// [`has_privacy_policy_link`].
pub const PRIVACY_LINK_TOKENS: &[&str] = &[
    // English (text and slug forms)
    "privacy policy",
    "privacy-policy",
    // German
    "datenschutz",
    // French: "politique de confidentialité" text and unaccented href slugs
    "confidentialité",
    "confidentialite",
    // Spanish: "política de privacidad", "aviso de privacidad"
    "privacidad",
    // Italian footer label
    "informativa sulla privacy",
    "informativa privacy",
    // Portuguese: "política de privacidade"
    "privacidade",
    // Dutch
    "privacybeleid",
    "privacyverklaring",
    // Swedish
    "integritetspolicy",
];

/// True when the (lowercased) page contains a recognizable privacy-policy
/// link signal in any covered language, as link text or as an anchor whose
/// href has a `privacy` path segment.
pub fn has_privacy_policy_link(lower: &str) -> bool {
    let content = content_text_lower(lower);
    PRIVACY_LINK_TOKENS
        .iter()
        .any(|token| content.contains(token))
        || anchor_href_segment_matches(&content, |segment| {
            path_segment_head(segment) == "privacy"
                || matches!(segment, "privacypolicy" | "privacynotice")
        })
}
