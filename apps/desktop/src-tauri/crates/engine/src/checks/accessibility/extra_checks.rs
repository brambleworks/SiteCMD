//! Static checks for contrast hints, ARIA misuse, focus styles, and tab order.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::sync::LazyLock;

// The leading boundary prevents `background-color` from matching as `color`.
static TEXT_COLOR_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(?:^|[\s;{"'])color\s*:\s*#([0-9a-f]{6})\b"#).unwrap()
});
static BG_COLOR_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)background(?:-color)?\s*:\s*#([0-9a-f]{6})\b"#).unwrap()
});
// Match all opening tags, then classify focusability; keep attribute matching
// quote-agnostic for template syntax.
static ELEMENT_TAG_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?is)<([a-z][a-z0-9-]*)\b([^>]*)>").unwrap());
static ARIA_HIDDEN_TRUE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(?:^|["'\s])aria-hidden\s*=\s*["']?true(?-u:\b)"#).unwrap()
});
static HREF_ATTR_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"(?i)(?:^|["'\s])href\s*="#).unwrap());
static HIDDEN_INPUT_TYPE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)(?:^|["'\s])type\s*=\s*["']?hidden(?-u:\b)"#).unwrap()
});
static TABINDEX_VALUE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"(?i)(?:^|["'\s])tabindex\s*=\s*["']?(-?\d+)"#).unwrap());

/// Whether an element participates in keyboard focus order.
fn is_focusable(tag_name: &str, attrs: &str) -> bool {
    if let Some(caps) = TABINDEX_VALUE_RE.captures(attrs) {
        if let Ok(value) = caps[1].parse::<i32>() {
            return value >= 0;
        }
    }
    match tag_name {
        "a" | "area" => HREF_ATTR_RE.is_match(attrs),
        "button" | "select" | "textarea" => true,
        "input" => !HIDDEN_INPUT_TYPE_RE.is_match(attrs),
        _ => false,
    }
}
static PRES_ROLE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)<(?:a|button|input)\s[^>]*role\s*=\s*["']?(?:presentation|none)["']?"#)
        .unwrap()
});

/// Whether every channel in a `#rrggbb` color is near the light end of its range.
fn is_light_hex(hex: &str) -> bool {
    let channels = [
        u8::from_str_radix(hex.get(0..2).unwrap_or(""), 16),
        u8::from_str_radix(hex.get(2..4).unwrap_or(""), 16),
        u8::from_str_radix(hex.get(4..6).unwrap_or(""), 16),
    ];
    channels
        .iter()
        .all(|channel| matches!(channel, Ok(value) if *value >= 0xC0))
}
static TABINDEX_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    // Quoted (double or single) OR unquoted numeric tabindex.
    regex::Regex::new(r#"(?i)tabindex\s*=\s*(?:["']?(\d+)["']?)"#).unwrap()
});
/// A linked external stylesheet (rel and href in either order).
static EXTERNAL_STYLESHEET_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"(?i)<link\s[^>]*rel\s*=\s*["']?stylesheet"#).unwrap());

/// Check for sufficient color contrast hints (inline styles with low-contrast patterns)
pub struct ColorContrastHintsCheck;

impl Check for ColorContrastHintsCheck {
    fn id(&self) -> &str {
        "accessibility.color_contrast_hints"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();

        // Count genuinely light foreground colors and light backgrounds
        // declared inline. Both must appear before we suspect light-on-light
        // (an unreadable pairing).
        let light_text_count = TEXT_COLOR_RE
            .captures_iter(lower)
            .filter(|caps| is_light_hex(&caps[1]))
            .count();
        let light_bg_count = BG_COLOR_RE
            .captures_iter(lower)
            .filter(|caps| is_light_hex(&caps[1]))
            .count();

        let suspicious = light_text_count > 0 && light_bg_count > 0;
        let fix = "Measure the foreground/background pair on each surfaced element with computed browser styles. WCAG AA requires at least 4.5:1 for normal text and 3:1 for large text; do not change colors based on this document-wide hint alone.";

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if suspicious {
                "Possible low-contrast inline colors".into()
            } else {
                "Color Contrast Hints".into()
            },
            description: if suspicious {
                format!(
                    "Found {} light text {} and {} light {} in inline styles - potential contrast issues.",
                    light_text_count,
                    if light_text_count == 1 { "color" } else { "colors" },
                    light_bg_count,
                    if light_bg_count == 1 { "background" } else { "backgrounds" }
                )
            } else {
                "No document-wide light-inline-color co-occurrence was detected. Computed foreground/background contrast was not computed by this static fallback.".into()
            },
            status: if suspicious {
                CheckStatus::Warn
            } else {
                CheckStatus::Pass
            },
            severity: Severity::Medium,
            fix_prompt: suspicious.then(|| fix.into()),
            manual_fix: if suspicious { Some(fix.into()) } else { None },
            raw_data: Some(serde_json::json!({
                "light_inline_text_color_count": light_text_count,
                "light_inline_background_color_count": light_bg_count,
                "measurement": "document_wide_inline_color_cooccurrence",
                "computed_contrast_measured": false,
                "foreground_background_pairs_established": false,
            })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: if suspicious {
                Some("The static fallback found light inline text and background declarations somewhere on the page, but it does not pair them on the same rendered element or measure computed contrast. axe-core supersedes this hint when browser analysis runs.".into())
            } else {
                Some("This fallback checks only a narrow inline-color pattern. It does not inspect external stylesheets, computed colors, opacity, images, gradients, states, or actual element pairs; axe-core is authoritative when browser analysis runs.".into())
            },
            why_it_matters: if suspicious {
                Some("If the detected colors are actually paired, low contrast can make text difficult to read for users with low vision or in low-quality display and lighting conditions.".into())
            } else {
                None
            },
        }]
    }
}

/// Check for focus indicators (custom focus styles or outlines)
pub struct FocusIndicatorCheck;

impl Check for FocusIndicatorCheck {
    fn id(&self) -> &str {
        "accessibility.focus_indicators"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();

        let removes_outline = lower.contains("outline: none")
            || lower.contains("outline:none")
            || lower.contains("outline: 0")
            || lower.contains("outline:0");

        let has_focus_styles = lower.contains(":focus")
            || lower.contains(":focus-visible")
            || lower.contains("focus-within");

        // External stylesheets may provide the focus indicator this check cannot inspect.
        let has_external_css = EXTERNAL_STYLESHEET_RE.is_match(lower);

        let is_issue = removes_outline && !has_focus_styles;
        let (status, desc) = if is_issue && has_external_css {
            (CheckStatus::Warn, "Inspected inline markup removes focus outlines and contains no inline :focus/:focus-visible replacement. A replacement may live in the linked external stylesheets, which this check does not read.".to_string())
        } else if is_issue {
            (CheckStatus::Warn, "Inspected page markup removes focus outlines and contains no detected :focus/:focus-visible replacement. This static check does not compute runtime or shadow-DOM styles, so confirm by keyboard before changing CSS.".to_string())
        } else if removes_outline && has_focus_styles {
            (CheckStatus::Pass, "A focus-style marker was detected alongside the outline reset. This static check does not compute whether the rendered indicator is visible or meets contrast/area requirements.".to_string())
        } else {
            (
                CheckStatus::Pass,
                "No focus-outline reset was detected in the inspected markup. Runtime and computed focus styles were not measured by this static check.".to_string(),
            )
        };

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if is_issue {
                "Focus outline reset has no detected replacement".into()
            } else {
                "Focus Indicators".into()
            },
            description: desc,
            status,
            severity: Severity::Medium,
            fix_prompt: if is_issue {
                Some("Tab through the rendered page first. If focused controls have no visible indicator, remove the outline reset or add a high-contrast :focus-visible treatment that remains visible in every component state.".into())
            } else {
                None
            },
            manual_fix: if is_issue {
                Some("Never remove outline without providing a visible :focus or :focus-visible alternative. Keyboard accessibility depends on visible focus indicators.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "outline_reset_detected": removes_outline,
                "inline_focus_rule_detected": has_focus_styles,
                "external_stylesheet_detected": has_external_css,
                "computed_focus_style_measured": false,
            })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: if is_issue {
                Some(if has_external_css {
                    "External stylesheets are linked but not inspected, so a focus replacement may be defined there. Tab through the page to confirm the focused element stays visible."
                } else {
                    "This is a static markup/CSS heuristic and does not compute runtime, injected, component-scoped, or shadow-DOM focus styles. Tab through the page to confirm the indicator is actually absent."
                }.into())
            } else if removes_outline {
                Some("A focus selector marker is present, but static source text cannot establish the rendered indicator's visibility, contrast, size, or component coverage. Confirm by keyboard and computed-style inspection.".into())
            } else {
                Some("No outline reset was found in the markup inspected by this fallback, but external, runtime-injected, component-scoped, and shadow-DOM styles are not computed.".into())
            },
            why_it_matters: if is_issue {
                Some("If no replacement is present at runtime, keyboard users cannot tell which element is active on the page.".into())
            } else {
                None
            },
        }]
    }
}

/// Check for ARIA attribute misuse
pub struct AriaUsageCheck;

impl Check for AriaUsageCheck {
    fn id(&self) -> &str {
        "accessibility.aria_usage"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let mut issues: Vec<String> = Vec::new();

        // Check for aria-hidden="true" on focusable elements. Every opening
        // tag is classified with is_focusable so non-focusable elements
        // (href-less anchors, hidden inputs, plain divs) don't count.
        let hidden_focusable = ELEMENT_TAG_RE
            .captures_iter(lower)
            .filter(|caps| {
                ARIA_HIDDEN_TRUE_RE.is_match(&caps[2]) && is_focusable(&caps[1], &caps[2])
            })
            .count();
        if hidden_focusable > 0 {
            issues.push(format!(
                "{} focusable {} with aria-hidden=\"true\"",
                hidden_focusable,
                if hidden_focusable == 1 {
                    "element"
                } else {
                    "elements"
                }
            ));
        }

        // Check for role="presentation" or role="none" on interactive elements
        let pres_count = PRES_ROLE_RE.find_iter(lower).count();
        if pres_count > 0 {
            issues.push(format!(
                "{} interactive {} with role=\"presentation/none\"",
                pres_count,
                if pres_count == 1 {
                    "element"
                } else {
                    "elements"
                }
            ));
        }

        // Check for empty aria-label
        if lower.contains("aria-label=\"\"") || lower.contains("aria-label=''") {
            issues.push("Empty aria-label attribute found".into());
        }

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if issues.is_empty() {
                "ARIA Usage".into()
            } else {
                "ARIA misuse patterns detected".into()
            },
            description: if issues.is_empty() {
                "No ARIA misuse patterns detected.".into()
            } else {
                format!("ARIA issues found: {}", issues.join("; "))
            },
            status: if issues.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: if issues.is_empty() {
                None
            } else {
                Some("Review ARIA attribute usage: don't hide focusable elements with aria-hidden, don't strip semantics from interactive elements, and avoid empty aria-label values.".into())
            },
            raw_data: if issues.is_empty() {
                None
            } else {
                Some(serde_json::json!({ "issues": issues }))
            },
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if !issues.is_empty() {
                Some("Assistive technology misreads your UI, making features unusable.".into())
            } else {
                None
            },
        }]
    }
}

/// Check for tabindex misuse (positive values or excessive negative)
pub struct TabindexCheck;

impl Check for TabindexCheck {
    fn id(&self) -> &str {
        "accessibility.tabindex"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Accessibility
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Ignore tabindex text outside page elements.
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let mut positive_values = Vec::new();

        for cap in TABINDEX_RE.captures_iter(&scannable) {
            if let Ok(val) = cap[1].parse::<i32>() {
                if val > 0 {
                    positive_values.push(val);
                }
            }
        }
        let positive_count = positive_values.len() as u32;

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if positive_count == 0 {
                "Tabindex Usage".into()
            } else {
                "Positive tabindex values override tab order".into()
            },
            description: if positive_count == 0 {
                "No positive tabindex values found - keyboard navigation follows document order."
                    .into()
            } else {
                format!(
                    "{} {} with positive tabindex values. This overrides natural tab order and confuses keyboard users.",
                    positive_count,
                    if positive_count == 1 { "element" } else { "elements" }
                )
            },
            status: if positive_count == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Medium,
            fix_prompt: if positive_count == 0 {
                None
            } else {
                Some("Remove positive tabindex values and arrange interactive elements in the intended DOM order. Use tabindex=\"0\" only to join natural order and tabindex=\"-1\" only for programmatic focus.".into())
            },
            manual_fix: if positive_count == 0 {
                None
            } else {
                Some("Remove positive tabindex values. Use tabindex=\"0\" to add elements to natural tab order, or tabindex=\"-1\" for programmatic focus only.".into())
            },
            raw_data: Some(serde_json::json!({
                "positive_tabindex_count": positive_count,
                "positive_values": positive_values.into_iter().take(20).collect::<Vec<_>>(),
            })),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if positive_count == 0 {
                None
            } else {
                Some("Keyboard users tab through elements in an unpredictable order.".into())
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(body: &str) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("https://example.com").unwrap(),
            response_headers: http::header::HeaderMap::new(),
            status_code: 200,
            body: body.to_string(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    fn status(body: &str) -> CheckStatus {
        AriaUsageCheck.run(&ctx(body))[0].status
    }

    #[test]
    fn decorative_aside_with_aria_hidden_is_not_a_focusable_element() {
        let html = r#"<aside aria-hidden="true"><p>Decorative sidebar</p></aside>"#;
        assert_eq!(status(html), CheckStatus::Pass);
    }

    #[test]
    fn single_quoted_aria_hidden_on_a_link_is_still_caught() {
        let html = r#"<a href="/next" aria-hidden='true'>Skip</a>"#;
        assert_eq!(status(html), CheckStatus::Warn);
    }

    #[test]
    fn non_focusable_elements_with_aria_hidden_are_not_flagged() {
        let html = r#"
            <a aria-hidden="true"><svg></svg></a>
            <input type="hidden" name="csrf" aria-hidden="true">
            <button tabindex="-1" aria-hidden="true">x</button>
            <div aria-hidden="true">decoration</div>
        "#;
        assert_eq!(status(html), CheckStatus::Pass);
    }

    #[test]
    fn focusable_elements_with_aria_hidden_are_flagged() {
        for html in [
            r#"<button aria-hidden="true">Hidden button</button>"#,
            r#"<input type="text" aria-hidden="true">"#,
            r#"<div tabindex="0" aria-hidden="true">fake button</div>"#,
        ] {
            assert_eq!(status(html), CheckStatus::Warn, "should flag: {html}");
        }
    }

    fn tabindex_status(body: &str) -> CheckStatus {
        TabindexCheck.run(&ctx(body))[0].status
    }

    #[test]
    fn tabindex_in_script_strings_does_not_count() {
        let html = r#"<script>el.innerHTML = '<div tabindex="5">x</div>';</script>
            <button>Real button</button>"#;
        assert_eq!(tabindex_status(html), CheckStatus::Pass);
    }

    #[test]
    fn positive_tabindex_in_markup_still_warns() {
        let html = r#"<div tabindex="3">Jumps the queue</div>"#;
        let result = &TabindexCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(
            result.raw_data.as_ref().expect("tabindex evidence")["positive_values"],
            serde_json::json!([3])
        );
    }

    fn contrast_status(body: &str) -> CheckStatus {
        ColorContrastHintsCheck.run(&ctx(body))[0].status
    }

    #[test]
    fn light_background_with_dark_text_is_not_low_contrast() {
        let html = r#"<div style="background-color:#ffffff;color:#111111">Readable</div>"#;
        let result = &ColorContrastHintsCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(
            result.confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(result.description.contains("not computed"));
    }

    #[test]
    fn dark_saturated_color_is_not_classified_as_light() {
        let html = r#"<div style="color:#d00000;background:#ffffff">Warning</div>"#;
        assert_eq!(contrast_status(html), CheckStatus::Pass);
    }

    #[test]
    fn genuine_light_on_light_is_flagged() {
        let html =
            r#"<span style="color:#eeeeee">Faint</span><div style="background:#ffffff">bg</div>"#;
        let result = &ColorContrastHintsCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(
            result.confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert_eq!(
            result.raw_data.as_ref().expect("contrast evidence")["measurement"],
            "document_wide_inline_color_cooccurrence"
        );
    }

    #[test]
    fn focus_reset_with_external_css_is_inconclusive_not_fail() {
        let html = r#"<html><head>
            <link rel="stylesheet" href="/app.css">
            <style>button { outline: none; }</style>
        </head><body></body></html>"#;
        let results = FocusIndicatorCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        let evidence = results[0].raw_data.as_ref().expect("focus evidence");
        assert_eq!(evidence["external_stylesheet_detected"], true);
        assert_eq!(evidence["outline_reset_detected"], true);
    }

    #[test]
    fn focus_reset_with_no_external_css_still_requires_runtime_review() {
        let html = r#"<html><head><style>* { outline: none; }</style></head><body></body></html>"#;
        let results = FocusIndicatorCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
    }

    #[test]
    fn focus_reset_with_inline_replacement_passes() {
        let html = r#"<style>button { outline: none; } button:focus-visible { box-shadow: 0 0 0 2px; }</style>"#;
        let results = FocusIndicatorCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].description.contains("marker"));
    }
}
