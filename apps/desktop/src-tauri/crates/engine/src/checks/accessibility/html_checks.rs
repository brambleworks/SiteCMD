//! Static HTML accessibility checks.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

/// The opening `<html...>` tag and its attribute run.
static HTML_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<html\b([^>]*)>").unwrap());
/// A `lang` (or `xml:lang`) attribute on the `<html>` tag whose value starts
/// with a letter. The leading boundary keeps `hreflang=` from satisfying it,
/// and the optional-quote form accepts unquoted `lang=en`.
static LANG_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)(?:^|\s)(?:xml:)?lang\s*=\s*["']?[a-z]"#).unwrap());

/// One anchor element and its (attributes, inner HTML).
static ANCHOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<a\b([^>]*)>(.*?)</a>").unwrap());
/// Any HTML tag, for stripping an anchor's children down to its text.
static ANY_TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
/// A non-empty accessible-name attribute (on the anchor or a labeled child).
static NAMED_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:aria-label|aria-labelledby|title|alt)\s*=\s*["']?[^"'\s>]"#).unwrap()
});

/// Counts anchors with no visible text, labeling attribute, or labeled child.
fn empty_link_count(body: &str) -> usize {
    ANCHOR_RE
        .captures_iter(body)
        .filter(|caps| {
            let attrs = &caps[1];
            let inner = &caps[2];
            if NAMED_ATTR_RE.is_match(attrs) {
                return false;
            }
            if !ANY_TAG_RE.replace_all(inner, " ").trim().is_empty() {
                return false;
            }
            if NAMED_ATTR_RE.is_match(inner) || inner.to_ascii_lowercase().contains("<title") {
                return false;
            }
            true
        })
        .count()
}

pub struct LangAttributeCheck;
impl Check for LangAttributeCheck {
    fn id(&self) -> &str {
        "accessibility.lang"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Read quote-agnostic `lang` only from the root html tag.
        let has_lang = HTML_TAG_RE
            .captures(&ctx.body)
            .map(|caps| LANG_ATTR_RE.is_match(&caps[1]))
            .unwrap_or(false);
        vec![CheckResult {
            check_id: "accessibility.lang".into(),
            category: ScanCategory::Accessibility,
            title: if has_lang {
                "Language attribute".into()
            } else {
                "Page language not declared".into()
            },
            description: if has_lang {
                "The page declares a document language, so assistive technology has a better chance of reading it correctly."
                    .into()
            } else {
                "The root <html> tag is missing a lang attribute, so screen readers have to guess which language and pronunciation rules to use.".into()
            },
            status: if has_lang {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: if has_lang {
                None
            } else {
                Some("Add lang to <html>: <html lang=\"en\">".into())
            },
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if has_lang {
                None
            } else {
                Some(
                    "A missing document language can make the whole page harder to understand for screen reader users."
                        .into(),
                )
            },
        }]
    }
}

pub struct ImageAltAccessibilityCheck;
impl Check for ImageAltAccessibilityCheck {
    fn id(&self) -> &str {
        "accessibility.image_alt"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // The shared tokenizer follows browser attribute semantics.
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let lower = scannable.to_ascii_lowercase();
        let mut total = 0u32;
        let mut missing = 0u32;
        let mut empty = 0u32;
        let mut excluded_from_accessibility_tree = 0u32;
        for tag in crate::checks::html_attrs::tag_slices(&scannable, &lower, "img") {
            let role = crate::checks::html_attrs::attr_value(tag, "role")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let aria_hidden = crate::checks::html_attrs::attr_value(tag, "aria-hidden")
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("true");
            if matches!(role.as_str(), "presentation" | "none") || aria_hidden {
                excluded_from_accessibility_tree += 1;
                continue;
            }
            total += 1;
            match crate::checks::html_attrs::attr_value(tag, "alt") {
                None => missing += 1,
                Some(value) if value.trim().is_empty() => empty += 1,
                Some(_) => {}
            }
        }
        if total == 0 {
            return vec![CheckResult {
                check_id: "accessibility.image_alt".into(),
                category: ScanCategory::Accessibility,
                title: "Image alternative-text attributes".into(),
                description: format!(
                    "No eligible <img> elements were found in the initial HTML. This source check does not inspect images inserted into the rendered DOM. {} image{} explicitly removed from the accessibility tree {} excluded.",
                    excluded_from_accessibility_tree,
                    if excluded_from_accessibility_tree == 1 { "" } else { "s" },
                    if excluded_from_accessibility_tree == 1 { "was" } else { "were" }
                ),
                status: CheckStatus::Pass,
                severity: Severity::High,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "eligible_images": 0,
                    "missing_alt_attribute": 0,
                    "empty_alt_value": 0,
                    "excluded_from_accessibility_tree": excluded_from_accessibility_tree,
                    "source_scope": "initial_html",
                    "rendered_dom_assessed": false,
                    "alt_quality_assessed": false
                })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }
        vec![CheckResult {
            check_id: "accessibility.image_alt".into(),
            category: ScanCategory::Accessibility,
            title: if missing == 0 {
                "Image alternative-text attributes".into()
            } else {
                "Images missing alt attributes".into()
            },
            description: if missing == 0 {
                format!(
                    "All {} eligible <img> elements in the initial HTML include an alt attribute; {} have an empty value. Attribute presence does not establish whether nonempty text accurately conveys each image's purpose or whether an empty value is appropriate.",
                    total, empty
                )
            } else {
                format!(
                    "{} of {} eligible <img> element{} in the initial HTML {} no alt attribute. This source check does not assess rendered-DOM changes or whether existing alternative text accurately conveys each image's purpose.",
                    missing,
                    total,
                    if total == 1 { "" } else { "s" },
                    if missing == 1 { "has" } else { "have" }
                )
            },
            status: if missing == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: if missing > 0 {
                Some("For each listed source element, add an alt attribute that matches the image's purpose in context. Use concise equivalent text for informative images; use alt=\"\" only when the image is truly decorative or its information is already conveyed nearby. Then inspect the rendered DOM and test representative pages with a screen reader.".into())
            } else {
                None
            },
            raw_data: Some(
                serde_json::json!({
                    "eligible_images": total,
                    "missing_alt_attribute": missing,
                    "empty_alt_value": empty,
                    "excluded_from_accessibility_tree": excluded_from_accessibility_tree,
                    "source_scope": "initial_html",
                    "rendered_dom_assessed": false,
                    "alt_quality_assessed": false,
                    // Legacy aliases retained so existing local reports and
                    // integrations do not break while the precise fields are
                    // adopted.
                    "total": total,
                    "missing": missing,
                    "decorative": empty
                }),
            ),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: Some("Attribute presence is direct evidence from the initial HTML. JavaScript-rendered changes and the contextual quality of nonempty or empty values are outside this static check; axe-core replaces this result when rendered-DOM analysis runs.".into()),
            why_it_matters: if missing > 0 {
                Some("An informative image without a text alternative can withhold content or function from people who cannot perceive the image. The correct alternative depends on that image's purpose in context.".into())
            } else {
                None
            },
        }]
    }
}

pub struct HeadingOrderCheck;
impl Check for HeadingOrderCheck {
    fn id(&self) -> &str {
        "accessibility.headings"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Scan headings in document order so repeated levels do not hide jumps.
        use std::sync::LazyLock;
        static HEADING_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
            // Real tag boundary: `\b` also matched custom elements like
            // <h2-widget> at the hyphen.
            regex::Regex::new(r"(?i)<h([1-6])[\s/>]").expect("static heading regex")
            // allow-expect: compile-time literal regex
        });

        // Comments, scripts (framework templates, JS string literals),
        // and styles are not page headings.
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(ctx.body_lower(), " ");
        let mut levels: Vec<u8> = Vec::new();
        for cap in HEADING_RE.captures_iter(&scannable) {
            if let Some(level_str) = cap.get(1) {
                if let Ok(level) = level_str.as_str().parse::<u8>() {
                    levels.push(level);
                }
            }
        }

        let mut skips = Vec::new();
        for window in levels.windows(2) {
            let (a, b) = (window[0], window[1]);
            if b > a + 1 {
                skips.push(format!("H{}→H{}", a, b));
            }
        }

        // Multiple H1s is its own structural issue (separate from skips).
        let h1_count = levels.iter().filter(|l| **l == 1).count();
        let multiple_h1s = h1_count > 1;
        let missing_h1 = h1_count == 0 && !levels.is_empty();

        let mut issues: Vec<String> = Vec::new();
        if !skips.is_empty() {
            issues.push(format!(
                "{} heading level skip{} in document order: {}",
                skips.len(),
                if skips.len() == 1 { "" } else { "s" },
                skips.join(", ")
            ));
        }
        if multiple_h1s {
            issues.push(format!(
                "{} <h1> elements found; review whether one is the clear page heading",
                h1_count
            ));
        }
        if missing_h1 {
            issues.push(
                "Static markup has headings but no <h1>; review whether the page has a clear top-level heading"
                    .into(),
            );
        }

        vec![CheckResult {
            check_id: "accessibility.headings".into(),
            category: ScanCategory::Accessibility,
            title: if issues.is_empty() {
                "Heading structure".into()
            } else if !skips.is_empty() {
                "Heading level jumps need review".into()
            } else if multiple_h1s {
                "Multiple H1 elements need review".into()
            } else {
                "No H1 found in static headings".into()
            },
            description: if issues.is_empty() {
                "No numeric heading-level jumps were detected in document order, and the static markup contains one <h1>.".into()
            } else {
                // Not a WCAG failure: sequential levels and a single H1
                // are best practice (axe ships them as best-practice
                // rules, not violations). Citing SC 1.3.1 here overstated
                // the finding.
                format!(
                    "{}. These are document-structure review signals rather than conformance determinations: the correct levels depend on the page's actual section hierarchy.",
                    issues.join(". ")
                )
            },
            status: if issues.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: if issues.is_empty() {
                None
            } else {
                Some("Review the rendered headings against the page's real content outline. Give the primary topic a clear top-level heading, nest subsection headings under their actual parent, and use CSS rather than heading levels for visual size. Do not rewrite a legitimate structure solely to make every numeric level consecutive or force exactly one H1.".into())
            },
            manual_fix: if issues.is_empty() {
                None
            } else {
                Some("Review the rendered headings against the content structure. Use a clear top-level heading for the page topic, choose subsection levels from their real parent sections, and use CSS for visual sizing. Multiple H1 elements or a numeric jump can be legitimate, so verify the outline with a screen reader before changing it.".into())
            },
            raw_data: Some(serde_json::json!({
                "levels_in_order": levels,
                "skips": skips,
                "h1_count": h1_count,
            })),
            confidence: if issues.is_empty() {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: if issues.is_empty() {
                None
            } else {
                Some("The static heading tags and document order are directly observed, but markup alone cannot determine the intended content hierarchy or whether multiple H1 elements are confusing in context.".into())
            },
            why_it_matters: if issues.is_empty() {
                None
            } else {
                Some("Many screen reader users navigate by headings. A hierarchy that does not match the visible content can make sections harder to understand and reach, while legitimate document structures should not be flattened mechanically.".into())
            },
        }]
    }
}

/// Landmark elements with a real tag boundary (`<main` alone matched
/// custom elements like `<maintenance-banner>`).
static LANDMARK_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(main|nav|header|footer)[\s/>]").unwrap());
/// Landmark roles, quote-agnostic (only `role="..."` was recognized).
static LANDMARK_ROLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"["'\s]role\s*=\s*["']?(main|navigation|banner|contentinfo)(?-u:\b)"#).unwrap()
});

pub struct AriaLandmarksCheck;
impl Check for AriaLandmarksCheck {
    fn id(&self) -> &str {
        "accessibility.landmarks"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let mut found: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for cap in LANDMARK_TAG_RE.captures_iter(lower) {
            found.insert(match &cap[1] {
                "nav" => "nav",
                "header" => "header",
                "footer" => "footer",
                _ => "main",
            });
        }
        for cap in LANDMARK_ROLE_RE.captures_iter(lower) {
            found.insert(match &cap[1] {
                "navigation" => "nav",
                "banner" => "header",
                "contentinfo" => "footer",
                _ => "main",
            });
        }
        let has_main = found.contains("main");
        let missing: Vec<&str> = ["main", "nav", "header", "footer"]
            .into_iter()
            .filter(|l| !found.contains(l))
            .collect();
        // Only `main` is universal; navigation, header, and footer are optional.
        vec![CheckResult {
            check_id: "accessibility.landmarks".into(),
            category: ScanCategory::Accessibility,
            title: if has_main {
                "ARIA landmarks".into()
            } else {
                "No main content landmark".into()
            },
            description: if missing.is_empty() {
                "All common landmark regions found (main, nav, header, footer).".into()
            } else if has_main {
                format!("Main content landmark found. Not present: {}. Only add those regions if the page actually has them.", missing.join(", "))
            } else {
                "No <main> element or role=\"main\" region found. Screen readers use the main landmark to jump straight to the page content.".into()
            },
            status: if has_main {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: if has_main {
                None
            } else {
                Some("Wrap the primary page content in a <main> element. Use <nav>, <header>, and <footer> for the regions the page actually has.".into())
            },
            raw_data: Some(serde_json::json!({
                "main": has_main,
                "nav": found.contains("nav"),
                "header": found.contains("header"),
                "footer": found.contains("footer"),
            })),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if has_main {
                None
            } else {
                Some("Screen reader users can't jump straight to the content without a main landmark.".into())
            },
        }]
    }
}

pub struct LinkTextCheck;
impl Check for LinkTextCheck {
    fn id(&self) -> &str {
        "accessibility.link_text"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Ignore anchor-like text outside rendered markup.
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let lower = scannable.to_lowercase();
        let bad_patterns = [
            "click here",
            "read more",
            "learn more",
            "here",
            "more",
            "link",
        ];
        let mut bad_count = 0;
        for pattern in &bad_patterns {
            // Look for <a...>pattern</a>
            let search = format!(">{}</a>", pattern);
            bad_count += lower.matches(&search).count();
        }
        // Count anchors with no accessible name (see empty_link_count). This
        // credits aria-label/title on the link, img alt text, and svg titles,
        // instead of flagging every icon link that ends in `></a>`.
        let empty_text_links = empty_link_count(&scannable);
        let total_issues = bad_count + empty_text_links;
        vec![CheckResult {
            check_id: "accessibility.link_text".into(),
            category: ScanCategory::Accessibility,
            title: if total_issues == 0 {
                "Link text quality".into()
            } else {
                "Links with generic or empty text".into()
            },
            description: if total_issues == 0 {
                "Link text looks descriptive. No generic 'click here' style links or empty links were detected.".into()
            } else {
                format!(
                    "Found {} link{} with weak text: {} generic label{} and {} empty link{}. A link should make sense out of context.",
                    total_issues,
                    if total_issues == 1 { "" } else { "s" },
                    bad_count,
                    if bad_count == 1 { "" } else { "s" },
                    empty_text_links,
                    if empty_text_links == 1 { "" } else { "s" }
                )
            },
            status: if total_issues == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: if total_issues > 0 {
                Some("Replace generic labels with text that describes the destination or action, and give icon-only links an accessible name.".into())
            } else {
                None
            },
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if total_issues == 0 {
                None
            } else {
                Some("People often scan links out of context. Vague labels slow everyone down and are especially rough on screen reader navigation.".into())
            },
        }]
    }
}

/// An in-page anchor to the main content region. Bare `#main`/`#content`
/// substrings matched CSS selectors and JS anywhere on the page
///, so the fragment must sit in an href attribute.
static SKIP_HREF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"["'\s]href\s*=\s*["']?#(?:main|content)(?-u:\b)"#).unwrap());

pub struct SkipNavCheck;
impl Check for SkipNavCheck {
    fn id(&self) -> &str {
        "accessibility.skip_nav"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let has_skip =
            lower.contains("skip to") || lower.contains("skip nav") || SKIP_HREF_RE.is_match(lower);
        vec![CheckResult {
            check_id: "accessibility.skip_nav".into(),
            category: ScanCategory::Accessibility,
            title: if has_skip {
                "Skip navigation".into()
            } else {
                "No skip navigation link".into()
            },
            description: if has_skip {
                "Skip navigation link found. Keyboard users can jump to main content.".into()
            } else {
                // A skip link is one way to satisfy WCAG 2.4.1 (Bypass
                // Blocks); landmarks and headings can also satisfy it, so
                // its absence is a warning, not a definite failure.
                "No skip navigation link detected. A 'skip to content' link is the most direct way to let keyboard users bypass repeated navigation; landmarks and headings can also serve that role.".into()
            },
            status: if has_skip {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: if has_skip {
                None
            } else {
                Some("Add a skip link as the first focusable element: <a href=\"#main\" class=\"sr-only\">Skip to main content</a>".into())
            },
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if has_skip {
                None
            } else {
                Some("Without an effective bypass mechanism, keyboard users may need to traverse repeated navigation before reaching the main content on each page.".into())
            },
        }]
    }
}

/// Opening media tags eligible for autoplay checks.
static MEDIA_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?is)<(?:video|audio)\b[^>]*>"#).unwrap());

/// Boolean `autoplay` attributes, excluding URL and attribute-value text.
static AUTOPLAY_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)[\s"'/]autoplay(?:[\s/>=]|$)"#).unwrap());

/// Boolean `muted` attributes, excluding class-name text.
static MUTED_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)[\s"'/]muted(?:[\s/>=]|$)"#).unwrap());

pub struct AutoplayCheck;
impl Check for AutoplayCheck {
    fn id(&self) -> &str {
        "accessibility.autoplay"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        // Autoplaying media elements, and whether any of them plays unmuted.
        let autoplay_tags: Vec<&str> = MEDIA_TAG_RE
            .find_iter(lower)
            .map(|m| m.as_str())
            .filter(|tag| AUTOPLAY_ATTR_RE.is_match(tag))
            .collect();
        let has_autoplay = !autoplay_tags.is_empty();
        if !has_autoplay {
            return vec![CheckResult {
                check_id: "accessibility.autoplay".into(),
                category: ScanCategory::Accessibility,
                title: "Auto-playing media".into(),
                description: "No auto-playing media detected.".into(),
                status: CheckStatus::Pass,
                severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }
        let unmuted_count = autoplay_tags
            .iter()
            .filter(|tag| !MUTED_ATTR_RE.is_match(tag))
            .count();
        let all_muted = unmuted_count == 0;
        let autoplay_count = autoplay_tags.len();
        vec![CheckResult {
            check_id: "accessibility.autoplay".into(),
            category: ScanCategory::Accessibility,
            title: if all_muted {
                "Muted autoplay declared; motion needs review".into()
            } else {
                "Unmuted autoplay declared; behavior needs review".into()
            },
            description: if all_muted {
                format!(
                    "{} media {} declare muted autoplay. This avoids automatic sound in the inspected markup, but the static scan cannot establish whether motion lasts more than 5 seconds, runs in parallel with other content, is essential, or has pause/stop/hide controls; review WCAG 2.2 SC 2.2.2 (Pause, Stop, Hide).",
                    autoplay_count,
                    if autoplay_count == 1 { "element" } else { "elements" }
                )
            } else {
                format!(
                    "{} of {} autoplaying media {} lack a muted attribute. Static markup cannot establish whether browser playback policy permits playback, whether audio lasts more than 3 seconds, or whether a pause/stop or independent volume control is available; review WCAG 2.2 SC 1.4.2 (Audio Control).",
                    unmuted_count,
                    autoplay_count,
                    if autoplay_count == 1 { "element" } else { "elements" }
                )
            },
            status: CheckStatus::Warn,
            severity: if all_muted {
                Severity::Low
            } else {
                Severity::Medium
            },
            fix_prompt: None,
            manual_fix: if all_muted {
                Some("Run the page and determine whether the moving content starts automatically, lasts more than five seconds, appears beside other content, and is non-essential. If so, provide a keyboard-accessible pause/stop/hide control and a non-moving reduced-motion experience; otherwise document why the criterion does not apply.".into())
            } else {
                Some("Test actual playback in supported browsers. Prefer user-initiated playback; otherwise start video muted and expose accessible media controls. If audio can continue for more than three seconds, provide pause/stop or independent volume control, and separately review long-running motion under WCAG 2.2 SC 2.2.2.".into())
            },
            raw_data: Some(serde_json::json!({
                "autoplay_media_count": autoplay_count,
                "unmuted_autoplay_count": unmuted_count,
                "runtime_playback_observed": false,
                "duration_and_controls_observed": false,
            })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("The autoplay and muted attributes are direct markup evidence, but browser playback, duration, visible motion, essentiality, and available controls require runtime review.".into()),
            why_it_matters: if all_muted {
                Some("Long-running automatic motion can distract users or make content difficult to use when no pause, stop, or hide mechanism is available.".into())
            } else {
                Some("Unexpected audio can interfere with screen-reader output and concentration; audio that actually plays for more than three seconds needs a way to pause/stop it or control its volume independently.".into())
            },
        }]
    }
}

#[cfg(test)]
mod tests;
