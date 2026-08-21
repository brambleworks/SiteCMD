//! HTML-only coverage for accessibility rules that otherwise require axe.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

/// A `<meta...>` tag and its attribute run.
static META_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<meta\b[^>]*>").expect("static meta tag regex"));
// allow-expect: compile-time literal regex

/// A `name="viewport"` attribute, quote-agnostic.
static VIEWPORT_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)["'\s]name\s*=\s*["']?viewport["'\s/>]"#).expect("static viewport name regex")
});
// allow-expect: compile-time literal regex

/// The quoted `content` attribute value of a meta tag.
static CONTENT_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)["'\s]content\s*=\s*("([^"]*)"|'([^']*)')"#)
        .expect("static content attr regex")
});
// allow-expect: compile-time literal regex

/// One heading element: opening tag attributes + inner HTML. The regex crate
/// has no backreferences, so the closing tag matches any heading level;
/// headings cannot nest, so the nearest close is the right one.
static HEADING_ELEMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<h[1-6]\b([^>]*)>(.*?)</h[1-6]\s*>").expect("static heading element regex")
});
// allow-expect: compile-time literal regex

/// An `<iframe...>` opening tag.
static IFRAME_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<iframe\b[^>]*>").expect("static iframe tag regex"));
// allow-expect: compile-time literal regex

/// A non-empty accessible-name attribute (aria-label, aria-labelledby,
/// title, alt), quote-agnostic.
static NAMED_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:aria-label|aria-labelledby|title|alt)\s*=\s*["']?[^"'\s>]"#)
        .expect("static named attr regex")
});
// allow-expect: compile-time literal regex

/// Removed from the accessibility tree: aria-hidden or presentation role.
static HIDDEN_FROM_TREE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)["'\s](?:aria-hidden\s*=\s*["']?true|role\s*=\s*["']?(?:presentation|none))(?-u:\b)"#,
    )
    .expect("static hidden-from-tree regex")
});
// allow-expect: compile-time literal regex

/// Any HTML tag, for stripping children down to text.
static ANY_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").expect("static any tag regex"));
// allow-expect: compile-time literal regex

/// An iframe the page itself hides: zero-sized or display:none /
/// visibility:hidden inline style (the Google Tag Manager noscript pattern).
/// Axe only grades visible frames, so these are out of scope.
static HIDDEN_IFRAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?:["'\s](?:width|height)\s*=\s*["']?0(?:px)?["'\s/>]|display\s*:\s*none|visibility\s*:\s*hidden)"#,
    )
    .expect("static hidden iframe regex")
});
// allow-expect: compile-time literal regex

/// `<noscript>` blocks: their iframes only render with JavaScript disabled
/// (the GTM snippet), so they are not part of the page axe would grade.
static NOSCRIPT_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<noscript\b.*?</noscript>").expect("static noscript block regex")
});
// allow-expect: compile-time literal regex

/// An `<img...>` opening tag.
static IMG_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<img\b[^>]*>").expect("static img tag regex"));
// allow-expect: compile-time literal regex

/// The quoted alt attribute value of an img tag.
static ALT_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)["'\s]alt\s*=\s*("([^"]*)"|'([^']*)')"#).expect("static alt value regex")
});
// allow-expect: compile-time literal regex

/// The quoted value out of a two-alternative quoted capture.
fn quoted_value(caps: &regex::Captures) -> String {
    caps.get(2)
        .or_else(|| caps.get(3))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

/// Zoom restrictions parsed out of a viewport meta content value.
fn viewport_zoom_restrictions(content: &str) -> Vec<String> {
    let mut restrictions = Vec::new();
    for directive in content.split([',', ';']) {
        let Some((key, value)) = directive.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_ascii_lowercase();
        if key == "user-scalable" && (value == "no" || value == "0") {
            restrictions.push("user-scalable=no".to_string());
        }
        if key == "maximum-scale" {
            if let Ok(scale) = value.parse::<f32>() {
                // WCAG 1.4.4 requires 200% text scaling; axe meta-viewport
                // flags any maximum-scale below 2.
                if scale < 2.0 {
                    restrictions.push(format!("maximum-scale={}", value));
                }
            }
        }
    }
    restrictions
}

pub struct ViewportZoomCheck;
impl Check for ViewportZoomCheck {
    fn id(&self) -> &str {
        "accessibility.viewport_zoom"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut restrictions: Vec<String> = Vec::new();
        for tag_match in META_TAG_RE.find_iter(&ctx.body) {
            let tag = tag_match.as_str();
            if !VIEWPORT_NAME_RE.is_match(tag) {
                continue;
            }
            if let Some(caps) = CONTENT_ATTR_RE.captures(tag) {
                restrictions.extend(viewport_zoom_restrictions(&quoted_value(&caps)));
            }
        }
        restrictions.dedup();
        let restricted = !restrictions.is_empty();
        // Distinguish blocked zoom from a limited maximum scale.
        let blocks_zoom = restrictions.iter().any(|r| r == "user-scalable=no");
        vec![CheckResult {
            check_id: "accessibility.viewport_zoom".into(),
            category: ScanCategory::Accessibility,
            title: if blocks_zoom {
                "Viewport meta tag blocks pinch-to-zoom".into()
            } else if restricted {
                "Viewport meta tag restricts zooming".into()
            } else {
                "Viewport zoom".into()
            },
            description: if blocks_zoom {
                format!(
                    "The viewport meta tag disables zooming ({}). Low-vision users rely on pinch-to-zoom to read; WCAG 2.2 SC 1.4.4 requires text to scale to at least 200%.",
                    restrictions.join(", ")
                )
            } else if restricted {
                format!(
                    "The viewport meta tag limits how far users can zoom ({}). Low-vision users rely on pinch-to-zoom to read; WCAG 2.2 SC 1.4.4 requires text to scale to at least 200%.",
                    restrictions.join(", ")
                )
            } else {
                "The viewport meta tag does not restrict zooming.".into()
            },
            status: if restricted {
                CheckStatus::Fail
            } else {
                CheckStatus::Pass
            },
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: if restricted {
                Some("Remove user-scalable=no and maximum-scale from the viewport meta tag: <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">.".into())
            } else {
                None
            },
            raw_data: if restricted {
                Some(serde_json::json!({ "restrictions": restrictions }))
            } else {
                None
            },
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if blocks_zoom {
                Some("When the browser honors this directive, users who rely on magnification cannot use pinch-to-zoom; browser policies that ignore it do not make the author restriction portable or accessible.".into())
            } else if restricted {
                Some("When the browser honors this directive, the configured cap prevents users from reaching the 200% text-resizing level required by WCAG 2.2 SC 1.4.4; actual behavior still varies by browser.".into())
            } else {
                None
            },
        }]
    }
}

pub struct EmptyHeadingsCheck;
impl Check for EmptyHeadingsCheck {
    fn id(&self) -> &str {
        "accessibility.empty_headings"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Comments, scripts, and styles are not page headings.
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let mut empty = 0u32;
        let mut total = 0u32;
        for caps in HEADING_ELEMENT_RE.captures_iter(&scannable) {
            let attrs = &caps[1];
            let inner = &caps[2];
            // aria-hidden / presentation headings are out of the tree; a
            // labeled heading has an accessible name without visible text.
            if HIDDEN_FROM_TREE_RE.is_match(attrs) {
                continue;
            }
            total += 1;
            if NAMED_ATTR_RE.is_match(attrs) {
                continue;
            }
            if !ANY_TAG_RE.replace_all(inner, " ").trim().is_empty() {
                continue;
            }
            // An image with alt text or a labeled child still names the
            // heading (same crediting as empty link detection).
            if NAMED_ATTR_RE.is_match(inner) || inner.to_ascii_lowercase().contains("<title") {
                continue;
            }
            empty += 1;
        }
        vec![CheckResult {
            check_id: "accessibility.empty_headings".into(),
            category: ScanCategory::Accessibility,
            title: if empty == 0 {
                "Empty headings".into()
            } else {
                "Headings with no text".into()
            },
            description: if empty == 0 {
                "No empty heading elements found.".into()
            } else {
                format!(
                    "{} of {} heading{} contain{} no text and no accessible name. Screen readers announce them as blank entries in the page outline, and users navigating by heading land on nothing.",
                    empty,
                    total,
                    if total == 1 { "" } else { "s" },
                    if empty == 1 { "s" } else { "" }
                )
            },
            status: if empty == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: if empty == 0 {
                None
            } else {
                Some("Put real text in each heading, or remove the empty heading element. If a heading is styled as a decorative spacer, use a styled <div> instead.".into())
            },
            raw_data: Some(serde_json::json!({ "empty": empty, "total": total })),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if empty == 0 {
                None
            } else {
                Some("Screen reader users navigate by headings; blank entries waste their time and hide the page structure.".into())
            },
        }]
    }
}

pub struct IframeTitleCheck;
impl Check for IframeTitleCheck {
    fn id(&self) -> &str {
        "accessibility.iframe_title"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Noscript iframes (the GTM snippet) never render with JS enabled;
        // comments/scripts are not markup.
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let scannable = NOSCRIPT_BLOCK_RE.replace_all(&scannable, " ");
        let mut untitled = 0u32;
        let mut total = 0u32;
        for tag_match in IFRAME_TAG_RE.find_iter(&scannable) {
            let tag = tag_match.as_str();
            // Hidden or zero-sized frames are not graded; neither are frames
            // removed from the accessibility tree.
            if HIDDEN_IFRAME_RE.is_match(tag) || HIDDEN_FROM_TREE_RE.is_match(tag) {
                continue;
            }
            total += 1;
            if !NAMED_ATTR_RE.is_match(tag) {
                untitled += 1;
            }
        }
        vec![CheckResult {
            check_id: "accessibility.iframe_title".into(),
            category: ScanCategory::Accessibility,
            title: if untitled == 0 {
                "Iframe titles".into()
            } else {
                "Iframes without an accessible title".into()
            },
            description: if untitled == 0 {
                if total == 0 {
                    "No visible iframes found.".into()
                } else if total == 1 {
                    "The visible iframe has an accessible title.".into()
                } else {
                    format!("All {} visible iframes have an accessible title.", total)
                }
            } else {
                format!(
                    "{} of {} visible iframe{} {} no title or aria-label. Screen readers announce them only as \"frame\", so users cannot tell what embedded content is (a video, a map, a payment form) without entering it.",
                    untitled,
                    total,
                    if total == 1 { "" } else { "s" },
                    if untitled == 1 { "has" } else { "have" }
                )
            },
            status: if untitled == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: if untitled == 0 {
                None
            } else {
                Some("Add a title attribute describing each iframe's content: <iframe title=\"Product demo video\" ...>.".into())
            },
            raw_data: Some(serde_json::json!({ "untitled": untitled, "total": total })),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if untitled == 0 {
                None
            } else {
                Some("Embedded videos, maps, and forms are anonymous boxes to screen reader users without a frame title.".into())
            },
        }]
    }
}

/// Prefixes that repeat what a screen reader already announces ("image").
const REDUNDANT_ALT_PREFIXES: &[&str] = &[
    "image of ",
    "picture of ",
    "photo of ",
    "photograph of ",
    "graphic of ",
    "screenshot of ",
    "icon of ",
];

/// Alt values that are only the media type, carrying no information.
const REDUNDANT_ALT_EXACT: &[&str] = &["image", "picture", "photo", "photograph", "graphic"];

fn is_redundant_alt(alt: &str) -> bool {
    let normalized = alt.trim().to_ascii_lowercase();
    // "A photo of..." is the same redundancy with an article in front.
    let stripped = normalized
        .strip_prefix("a ")
        .or_else(|| normalized.strip_prefix("an "))
        .unwrap_or(&normalized);
    REDUNDANT_ALT_EXACT.contains(&stripped)
        || REDUNDANT_ALT_PREFIXES
            .iter()
            .any(|prefix| stripped.starts_with(prefix))
}

pub struct RedundantAltTextCheck;
impl Check for RedundantAltTextCheck {
    fn id(&self) -> &str {
        "accessibility.redundant_alt"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let mut redundant: Vec<String> = Vec::new();
        for tag_match in IMG_TAG_RE.find_iter(&scannable) {
            let tag = tag_match.as_str();
            if HIDDEN_FROM_TREE_RE.is_match(tag) {
                continue;
            }
            if let Some(caps) = ALT_VALUE_RE.captures(tag) {
                let alt = quoted_value(&caps);
                if is_redundant_alt(&alt) {
                    redundant.push(alt.trim().to_string());
                }
            }
        }
        let listed = redundant
            .iter()
            .take(5)
            .map(|alt| format!("\"{}\"", alt))
            .collect::<Vec<_>>()
            .join(", ");
        vec![CheckResult {
            check_id: "accessibility.redundant_alt".into(),
            category: ScanCategory::Accessibility,
            title: if redundant.is_empty() {
                "Alt text phrasing".into()
            } else {
                "Alt text repeats \"image of\"".into()
            },
            description: if redundant.is_empty() {
                "No alt text starting with redundant phrases like \"image of\" was found.".into()
            } else {
                format!(
                    "{} image{} alt text that starts by naming the media type ({}). Screen readers already announce images as images, so users hear \"image, image of ...\" - the prefix is pure noise.",
                    redundant.len(),
                    if redundant.len() == 1 { " has" } else { "s have" },
                    listed
                )
            },
            status: if redundant.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if redundant.is_empty() {
                None
            } else {
                Some("Drop the \"image of\" / \"photo of\" prefix and describe the content directly: alt=\"Team at the 2026 offsite\" instead of alt=\"Photo of team at the 2026 offsite\".".into())
            },
            raw_data: if redundant.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "redundant_alts": redundant }))
            },
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if redundant.is_empty() {
                None
            } else {
                Some("For each flagged image, assistive technology can announce the element as an image and then repeat words such as \"image\" or \"photo\" from its alternative text, adding avoidable noise.".into())
            },
        }]
    }
}

#[cfg(test)]
mod tests;
