//! CSS architecture signals for Polish Scan.

use super::{PolishContext, PolishResult, SignalCategory, SignalWeight};
use regex::Regex;
use std::sync::LazyLock;

const CATEGORY: SignalCategory = SignalCategory::CssArchitecture;
/// Matches any HTML element opening tag (captures tag name)
static ELEMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<([a-z][a-z0-9]*)(?-u:\b)[^>]*>").expect("element regex"));

/// Match `style` attributes without accepting names such as `data-style`.
static INLINE_STYLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<[a-z][a-z0-9]*\b[^>]*\sstyle\s*=\s*["'][^"']*["'][^>]*>"#)
        .expect("inline style regex")
});

/// Matches a `class="..."` or `class='...'` attribute, capturing the value
static CLASS_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\bclass\s*=\s*["']([^"']*)["']"#).expect("class attr regex")
});

/// Matches `<link rel="stylesheet">` - any order of attributes
static LINK_STYLESHEET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<link\b[^>]*\brel\s*=\s*["']stylesheet["'][^>]*>"#)
        .expect("link stylesheet regex")
});

/// Known Tailwind / utility class prefixes.
/// A class is "utility" if it matches any of these patterns.
const TAILWIND_PREFIXES: &[&str] = &[
    // Layout
    "flex",
    "grid",
    "block",
    "inline",
    "hidden",
    "container",
    "absolute",
    "relative",
    "fixed",
    "sticky",
    // Spacing
    "p-",
    "px-",
    "py-",
    "pt-",
    "pb-",
    "pl-",
    "pr-",
    "ps-",
    "pe-",
    "m-",
    "mx-",
    "my-",
    "mt-",
    "mb-",
    "ml-",
    "mr-",
    "ms-",
    "me-",
    "gap-",
    "space-",
    // Sizing
    "w-",
    "h-",
    "min-w-",
    "min-h-",
    "max-w-",
    "max-h-",
    "size-",
    // Typography
    "text-",
    "font-",
    "leading-",
    "tracking-",
    "line-clamp-",
    "uppercase",
    "lowercase",
    "capitalize",
    "italic",
    "not-italic",
    "truncate",
    "antialiased",
    // Colors / backgrounds
    "bg-",
    "from-",
    "to-",
    "via-",
    // Borders
    "border",
    "border-",
    "rounded",
    "rounded-",
    "ring-",
    "outline-",
    "divide-",
    // Effects
    "shadow",
    "shadow-",
    "opacity-",
    "blur-",
    "backdrop-",
    "transition",
    "transition-",
    "duration-",
    "ease-",
    "delay-",
    "animate-",
    // Flexbox/Grid specifics
    "justify-",
    "items-",
    "self-",
    "content-",
    "place-",
    "col-",
    "row-",
    "auto-cols-",
    "auto-rows-",
    "flex-",
    "grow",
    "shrink",
    "basis-",
    "order-",
    // Overflow / position
    "overflow-",
    "z-",
    "top-",
    "right-",
    "bottom-",
    "left-",
    "inset-",
    // Interactivity
    "cursor-",
    "pointer-events-",
    "select-",
    "touch-",
    // Pseudo-class prefixes (hover:, focus:, etc.)
    "hover:",
    "focus:",
    "active:",
    "disabled:",
    "group-",
    "dark:",
    "sm:",
    "md:",
    "lg:",
    "xl:",
    "2xl:",
    // Misc
    "sr-only",
    "not-sr-only",
    "aspect-",
    "object-",
    "whitespace-",
    "break-",
];

/// CSS-in-JS marker patterns in class names (styled-components, Emotion, CSS modules)
const CSS_IN_JS_MARKERS: &[&str] = &[
    "sc-",        // styled-components
    "css-",       // Emotion
    "emotion-",   // Emotion
    "jss",        // JSS (Material-UI)
    "makeStyles", // Material-UI
];

/// CSS module hashed class pattern: short alphanumeric hash suffix (e.g. `header_a1b2c3`)
static CSS_MODULE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]*_[a-zA-Z0-9]{5,8}$").expect("css module regex")
});

/// Distinguishes Tailwind utility suffixes from semantic class names using
/// numeric, bracketed, slash, and known-token forms.
fn looks_like_utility_value(value: &str) -> bool {
    // Arbitrary values in brackets: [20px], [#ff0000]
    if value.starts_with('[') {
        return true;
    }
    // Starts with a digit: "4", "2.5", "1/2"
    if value.starts_with(|c: char| c.is_ascii_digit()) {
        return true;
    }
    // Fraction/opacity notation: "1/2", "red-500/50"
    if value.contains('/') {
        return true;
    }

    // Split on dashes and check segments
    let segments: Vec<&str> = value.split('-').collect();

    // Known Tailwind value tokens (colors, sizes, keywords)
    const UTILITY_TOKENS: &[&str] = &[
        // Sizing
        "auto",
        "full",
        "screen",
        "min",
        "max",
        "fit",
        "px",
        "none",
        // Breakpoint sizes
        "xs",
        "sm",
        "md",
        "lg",
        "xl",
        "2xl",
        "3xl",
        "4xl",
        "5xl",
        // Typography
        "base",
        "thin",
        "extralight",
        "light",
        "normal",
        "medium",
        "semibold",
        "bold",
        "extrabold",
        "black",
        // Colors
        "white",
        "transparent",
        "current",
        "inherit",
        "slate",
        "gray",
        "zinc",
        "neutral",
        "stone",
        "red",
        "orange",
        "amber",
        "yellow",
        "lime",
        "green",
        "emerald",
        "teal",
        "cyan",
        "sky",
        "blue",
        "indigo",
        "violet",
        "purple",
        "fuchsia",
        "pink",
        "rose",
        // Layout
        "center",
        "start",
        "end",
        "between",
        "around",
        "evenly",
        "stretch",
        "baseline",
        "wrap",
        "nowrap",
        "reverse",
        "row",
        "col",
        "block",
        "inline",
        "hidden",
        // Misc
        "clip",
        "visible",
        "scroll",
        "contain",
        "cover",
        "inside",
        "outside",
        "top",
        "right",
        "bottom",
        "left",
        "both",
        "x",
        "y",
        "t",
        "b",
        "l",
        "r",
        "s",
        "e",
    ];

    // If first segment is a known token, it's a utility
    if UTILITY_TOKENS.contains(&segments[0]) {
        return true;
    }

    // If any segment is purely numeric, it's a utility (e.g. "gray-900")
    if segments
        .iter()
        .any(|s| s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty())
    {
        return true;
    }

    // Two-segment values where both are known tokens (e.g. "no-repeat")
    if segments.len() == 2
        && segments
            .iter()
            .all(|s| UTILITY_TOKENS.contains(s) || s.chars().all(|c| c.is_ascii_digit()))
    {
        return true;
    }

    false
}

/// Classify a single class name as a Tailwind/utility class or a custom class.
fn is_utility_class(class: &str) -> bool {
    // Exact matches
    if matches!(
        class,
        "flex"
            | "grid"
            | "block"
            | "inline"
            | "hidden"
            | "container"
            | "absolute"
            | "relative"
            | "fixed"
            | "sticky"
            | "uppercase"
            | "lowercase"
            | "capitalize"
            | "italic"
            | "not-italic"
            | "truncate"
            | "antialiased"
            | "border"
            | "rounded"
            | "shadow"
            | "transition"
            | "grow"
            | "shrink"
            | "sr-only"
            | "not-sr-only"
    ) {
        return true;
    }

    // Prefix matches - for prefixes ending in `-`, validate the suffix
    // looks like a utility value (not a semantic custom class like "my-header")
    for prefix in TAILWIND_PREFIXES {
        if let Some(rest) = class.strip_prefix(prefix) {
            if prefix.ends_with('-') {
                if rest.is_empty() {
                    continue;
                }
                if looks_like_utility_value(rest) {
                    return true;
                }
                continue;
            }
            return true;
        }
    }

    // Negative value utilities (e.g. -mt-4, -translate-x-1)
    if class.starts_with('-') && class.len() > 1 {
        let without_dash = &class[1..];
        // Recurse without the leading dash
        if is_utility_class(without_dash) {
            return true;
        }
    }

    false
}

/// Extract all class names from all elements in the HTML.
fn extract_all_classes(html: &str) -> Vec<Vec<String>> {
    CLASS_ATTR_RE
        .captures_iter(html)
        .map(|cap| {
            cap[1]
                .split_whitespace()
                .filter(|c| !c.is_empty())
                .map(|c| c.to_string())
                .collect()
        })
        .collect()
}

/// Check if the HTML or CSS has CSS-in-JS markers
fn has_css_in_js_markers(html: &str, css: &str) -> bool {
    let combined = format!("{} {}", html, css);
    CSS_IN_JS_MARKERS
        .iter()
        .any(|marker| combined.contains(marker))
}

/// Check if any class names look like CSS module hashes
fn has_css_module_classes(class_lists: &[Vec<String>]) -> bool {
    class_lists
        .iter()
        .flatten()
        .any(|c| CSS_MODULE_RE.is_match(c))
}

/// Flag pages where inline styles exceed 15% of elements (High, 15).
pub fn inline_style_density(ctx: &PolishContext) -> PolishResult {
    let total_elements = ELEMENT_RE.find_iter(&ctx.html).count();
    if total_elements == 0 {
        return PolishResult::clear(
            "inline-style-density",
            "High Inline-Style Density",
            SignalWeight::High,
            CATEGORY,
        );
    }

    let inline_count = INLINE_STYLE_RE.find_iter(&ctx.html).count();
    let ratio = inline_count as f64 / total_elements as f64;

    if ratio > 0.15 {
        PolishResult::fired(
            "inline-style-density",
            "High Inline-Style Density",
            SignalWeight::High,
            CATEGORY,
            format!(
                "{}% of elements have inline styles ({} of {})",
                (ratio * 100.0).round() as u32,
                inline_count,
                total_elements
            ),
            serde_json::json!({
                "inline_style_count": inline_count,
                "total_elements": total_elements,
                "ratio": (ratio * 1000.0).round() / 1000.0,
            }),
        )
    } else {
        PolishResult::clear(
            "inline-style-density",
            "High Inline-Style Density",
            SignalWeight::High,
            CATEGORY,
        )
    }
}

/// Detect excessive Tailwind utility density with few custom abstractions.
/// Fires above 10 utilities per element and below 5% custom class names.
pub fn tailwind_class_density(ctx: &PolishContext) -> PolishResult {
    let class_lists = extract_all_classes(&ctx.html);
    if class_lists.is_empty() {
        return PolishResult::clear(
            "tailwind-class-density",
            "High Tailwind Utility Density",
            SignalWeight::High,
            CATEGORY,
        );
    }

    let elements_with_classes = class_lists.len();
    let mut total_utility = 0usize;
    let mut total_custom = 0usize;
    let mut total_classes = 0usize;

    for classes in &class_lists {
        for class in classes {
            total_classes += 1;
            if is_utility_class(class) {
                total_utility += 1;
            } else {
                total_custom += 1;
            }
        }
    }

    if total_classes == 0 {
        return PolishResult::clear(
            "tailwind-class-density",
            "High Tailwind Utility Density",
            SignalWeight::High,
            CATEGORY,
        );
    }

    let avg_utility_per_element = total_utility as f64 / elements_with_classes as f64;
    let custom_pct = (total_custom as f64 / total_classes as f64) * 100.0;

    if avg_utility_per_element > 10.0 && custom_pct < 5.0 {
        PolishResult::fired(
            "tailwind-class-density",
            "High Tailwind Utility Density",
            SignalWeight::High,
            CATEGORY,
            format!(
                "avg {:.1} utility classes/element, {:.1}% custom",
                avg_utility_per_element, custom_pct
            ),
            serde_json::json!({
                "avg_utility_per_element": (avg_utility_per_element * 10.0).round() / 10.0,
                "custom_percentage": (custom_pct * 10.0).round() / 10.0,
                "total_utility": total_utility,
                "total_custom": total_custom,
                "elements_with_classes": elements_with_classes,
            }),
        )
    } else {
        PolishResult::clear(
            "tailwind-class-density",
            "High Tailwind Utility Density",
            SignalWeight::High,
            CATEGORY,
        )
    }
}

/// Detect pages with no stylesheet, CSS-in-JS, CSS module, or style block.
/// Weight: Medium.
pub fn no_css_architecture(ctx: &PolishContext) -> PolishResult {
    let has_stylesheet = LINK_STYLESHEET_RE.is_match(&ctx.html);
    let has_css_in_js = has_css_in_js_markers(&ctx.html, &ctx.css);
    let class_lists = extract_all_classes(&ctx.html);
    let has_modules = has_css_module_classes(&class_lists);

    let has_style_block = ctx.html.contains("<style");

    if !has_stylesheet && !has_css_in_js && !has_modules && !has_style_block {
        PolishResult::fired(
            "no-css-architecture",
            "No Custom CSS Found",
            SignalWeight::Medium,
            CATEGORY,
            "No external stylesheets, CSS-in-JS, or CSS modules detected".to_string(),
            serde_json::json!({
                "has_stylesheet": false,
                "has_css_in_js": false,
                "has_css_modules": false,
                "has_style_block": false,
            }),
        )
    } else {
        PolishResult::clear(
            "no-css-architecture",
            "No Custom CSS Found",
            SignalWeight::Medium,
            CATEGORY,
        )
    }
}
/// Retain the manifest slot without penalizing intentional utility-first CSS.
pub fn utility_to_custom_ratio(_ctx: &PolishContext) -> PolishResult {
    PolishResult::clear(
        "utility-to-custom-ratio",
        "Utility-Only CSS (No Custom Classes)",
        SignalWeight::Medium,
        CATEGORY,
    )
}

#[cfg(test)]
mod tests;
