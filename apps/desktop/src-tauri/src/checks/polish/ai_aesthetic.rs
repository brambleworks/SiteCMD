//! Heuristic visual-pattern signals for Polish Scan.

use super::{PolishContext, PolishResult, SignalCategory, SignalWeight};
use regex::Regex;
use std::sync::LazyLock;

const CATEGORY: SignalCategory = SignalCategory::AiAesthetic;

/// Matches CSS gradient declarations (linear-gradient, radial-gradient)
static GRADIENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(linear|radial|conic)-gradient\s*\([^)]+\)").expect("gradient regex")
});

/// Matches hex colors in the AI-favorite purple/blue/pink range
static AI_GRADIENT_COLOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Purple: 7c3aed, 8b5cf6, a855f7, 6366f1
    // Blue: 3b82f6, 2563eb, 6366f1
    // Pink: ec4899, f472b6, d946ef
    Regex::new(r"(?i)#(?:7c3aed|8b5cf6|a855f7|6366f1|3b82f6|2563eb|ec4899|f472b6|d946ef|818cf8|c084fc|e879f9|a78bfa|7dd3fc|f0abfc|c4b5fd)")
        .expect("ai gradient color regex")
});

/// Matches backdrop-filter with blur
static BACKDROP_BLUR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)backdrop-filter\s*:\s*[^;]*blur\s*\(").expect("backdrop blur regex")
});

/// Matches Tailwind backdrop-blur classes
static TW_BACKDROP_BLUR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)backdrop-blur(?:-[a-z]+)?(?-u:\b)").expect("tw backdrop blur regex")
});

/// Matches AOS (Animate on Scroll) data attributes
static AOS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\bdata-aos\s*=\s*["']"#).expect("aos regex"));

/// Matches Framer Motion scroll-triggered animation attributes
static FRAMER_SCROLL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(whileInView|useInView|useScroll)").expect("framer scroll regex")
});

/// Matches GSAP ScrollTrigger
static GSAP_SCROLL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)ScrollTrigger").expect("gsap scroll regex"));

/// Matches `<section` tags (for section count)
static SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<(section|div)(?-u:\b)[^>]*>").expect("section regex"));

/// Matches large border-radius Tailwind classes
static LARGE_RADIUS_TW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u:\b)rounded-(2xl|3xl|full)(?-u:\b)").expect("large radius tw regex")
});

/// Matches large border-radius in CSS (>= 16px, or >= 1rem)
static LARGE_RADIUS_CSS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)border-radius\s*:\s*(\d+)px").expect("large radius css regex")
});

/// Matches colored box-shadow (non-gray colors)
static BOX_SHADOW_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)box-shadow\s*:\s*([^;]+)").expect("box shadow regex"));

/// Matches Tailwind shadow-[color] classes (colored shadows)
static TW_COLORED_SHADOW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u:\b)shadow-(?:purple|blue|pink|indigo|violet|fuchsia|rose|sky|cyan|teal|emerald|green|red|orange|amber|yellow)-\d+")
        .expect("tw colored shadow regex")
});

/// Matches blob-suggesting words inside a class attribute VALUE. Running
/// this over the whole HTML matched "glow"/"orb" in body prose
///, so callers must feed it class values only.
static BLOB_CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(blob|orb|glow|gradient-bg|bg-blur)(?-u:\b)")
        .expect("blob class regex")
});

/// A class attribute and its value (quoted or unquoted).
static CLASS_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)[\s"']class\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+))"#)
        .expect("class attr regex")
});

/// Matches absolute positioning + rounded-full combined (potential blob)
static ABSOLUTE_ROUND_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)class\s*=\s*["'][^"']*\babsolute\b[^"']*\brounded-full\b[^"']*["']"#)
        .expect("absolute round regex")
});

/// Matches absolute + rounded-full in reverse order
static ROUND_ABSOLUTE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)class\s*=\s*["'][^"']*\brounded-full\b[^"']*\babsolute\b[^"']*["']"#)
        .expect("round absolute regex")
});
/// Detect unusually dense gradient usage as a soft aesthetic signal.
pub fn gradient_backgrounds(ctx: &PolishContext) -> PolishResult {
    let combined = format!("{}\n{}", ctx.html, ctx.css);
    let css_gradient_count = GRADIENT_RE.find_iter(&combined).count();

    let tw_gradient_count = combined.matches("bg-gradient").count();
    let gradient_count = css_gradient_count + tw_gradient_count;

    if gradient_count < 5 {
        return PolishResult::clear(
            "gradient-backgrounds",
            "Gradient Backgrounds",
            SignalWeight::Medium,
            CATEGORY,
        );
    }

    // Check if gradients use AI-favorite color families
    let ai_color_count = AI_GRADIENT_COLOR_RE.find_iter(&combined).count();
    // Also check for Tailwind gradient classes with purple/blue/pink
    let tw_ai_gradients = combined.matches("from-purple").count()
        + combined.matches("from-blue").count()
        + combined.matches("from-indigo").count()
        + combined.matches("from-pink").count()
        + combined.matches("from-violet").count()
        + combined.matches("from-fuchsia").count()
        + combined.matches("to-purple").count()
        + combined.matches("to-blue").count()
        + combined.matches("to-pink").count()
        + combined.matches("to-indigo").count()
        + combined.matches("to-violet").count()
        + combined.matches("to-fuchsia").count()
        + combined.matches("via-purple").count()
        + combined.matches("via-blue").count()
        + combined.matches("via-pink").count()
        + combined.matches("via-indigo").count()
        + combined.matches("via-violet").count()
        + combined.matches("via-fuchsia").count();

    let ai_indicators = ai_color_count + tw_ai_gradients;

    if gradient_count >= 5 && ai_indicators > 0 {
        // Report page-wide color references separately from gradient declarations.
        PolishResult::fired(
            "gradient-backgrounds",
            "Gradient Backgrounds",
            SignalWeight::Medium,
            CATEGORY,
            format!(
                "{} gradient declaration{}; {} purple/blue/pink color reference{} on the page",
                gradient_count,
                if gradient_count != 1 { "s" } else { "" },
                ai_indicators,
                if ai_indicators != 1 { "s" } else { "" }
            ),
            serde_json::json!({
                "gradient_count": gradient_count,
                "ai_color_matches": ai_indicators,
            }),
        )
    } else if gradient_count >= 8 {
        // Many gradients even without specific AI colors is suspicious
        PolishResult::fired(
            "gradient-backgrounds",
            "Gradient Backgrounds",
            SignalWeight::Medium,
            CATEGORY,
            format!("{} gradient declarations found", gradient_count),
            serde_json::json!({
                "gradient_count": gradient_count,
                "ai_color_matches": ai_indicators,
            }),
        )
    } else {
        PolishResult::clear(
            "gradient-backgrounds",
            "Gradient Backgrounds",
            SignalWeight::Medium,
            CATEGORY,
        )
    }
}
/// Detect repeated backdrop blur; one use is common and not meaningful.
pub fn glassmorphism(ctx: &PolishContext) -> PolishResult {
    let combined = format!("{}\n{}", ctx.html, ctx.css);

    let css_blur = BACKDROP_BLUR_RE.find_iter(&combined).count();
    let tw_blur = TW_BACKDROP_BLUR_RE.find_iter(&ctx.html).count();
    let total = css_blur + tw_blur;

    if total >= 3 {
        PolishResult::fired(
            "glassmorphism",
            "Backdrop Blur Heavy Usage",
            SignalWeight::LowMedium,
            CATEGORY,
            format!(
                "{} backdrop-blur usage{} detected",
                total,
                if total != 1 { "s" } else { "" }
            ),
            serde_json::json!({
                "css_backdrop_blur": css_blur,
                "tailwind_backdrop_blur": tw_blur,
            }),
        )
    } else {
        PolishResult::clear(
            "glassmorphism",
            "Backdrop Blur Heavy Usage",
            SignalWeight::LowMedium,
            CATEGORY,
        )
    }
}

/// Flag scroll-triggered animation on more than half of sections (Medium, 8).
pub fn scroll_animations(ctx: &PolishContext) -> PolishResult {
    let aos_count = AOS_RE.find_iter(&ctx.html).count();
    let framer_count = FRAMER_SCROLL_RE.find_iter(&ctx.html).count();
    let gsap_count = GSAP_SCROLL_RE.find_iter(&ctx.html).count();
    let total_scroll_anims = aos_count + framer_count + gsap_count;

    if total_scroll_anims == 0 {
        return PolishResult::clear(
            "scroll-animations",
            "Scroll Animations",
            SignalWeight::Medium,
            CATEGORY,
        );
    }

    // Both container counts and library markers are heuristic signals.
    let container_count = SECTION_RE.find_iter(&ctx.html).count().max(1);
    let ratio = total_scroll_anims as f64 / container_count as f64;

    if ratio > 0.5 || total_scroll_anims >= 5 {
        PolishResult::fired(
            "scroll-animations",
            "Scroll Animations",
            SignalWeight::Medium,
            CATEGORY,
            format!(
                "{} scroll-animation marker{} (attributes or library references) across ~{} section/div containers",
                total_scroll_anims,
                if total_scroll_anims != 1 { "s" } else { "" },
                container_count
            ),
            serde_json::json!({
                "aos_count": aos_count,
                "framer_count": framer_count,
                "gsap_count": gsap_count,
                "container_count": container_count,
                "ratio": (ratio * 100.0).round() / 100.0,
            }),
        )
    } else {
        PolishResult::clear(
            "scroll-animations",
            "Scroll Animations",
            SignalWeight::Medium,
            CATEGORY,
        )
    }
}

/// Flag more than 20 large border-radius declarations (Low, 3).
pub fn excessive_border_radius(ctx: &PolishContext) -> PolishResult {
    let tw_count = LARGE_RADIUS_TW_RE.find_iter(&ctx.html).count();

    let mut css_count = 0usize;
    for cap in LARGE_RADIUS_CSS_RE.captures_iter(&format!("{}\n{}", ctx.html, ctx.css)) {
        if let Ok(px) = cap[1].parse::<u32>() {
            if px >= 16 {
                css_count += 1;
            }
        }
    }

    let total = tw_count + css_count;

    if total > 20 {
        PolishResult::fired(
            "excessive-border-radius",
            "Large Border-Radius Heavy Usage",
            SignalWeight::Low,
            CATEGORY,
            // Counts class occurrences and CSS declarations, not rendered
            // elements (one CSS rule can style many elements or none).
            format!(
                "{} large border-radius usages (rounded-2xl/3xl/full classes or CSS declarations of 16px+)",
                total
            ),
            serde_json::json!({
                "tailwind_large_radius": tw_count,
                "css_large_radius": css_count,
            }),
        )
    } else {
        PolishResult::clear(
            "excessive-border-radius",
            "Large Border-Radius Heavy Usage",
            SignalWeight::Low,
            CATEGORY,
        )
    }
}

/// Flag three or more colored shadow declarations (Low, 3).
pub fn glow_shadows(ctx: &PolishContext) -> PolishResult {
    let combined = format!("{}\n{}", ctx.html, ctx.css);

    // Count Tailwind colored shadow utilities
    let tw_colored = TW_COLORED_SHADOW_RE.find_iter(&ctx.html).count();

    // Exclude zero-blur focus rings from colored glow detection.
    let mut css_colored = 0usize;
    for cap in BOX_SHADOW_RE.captures_iter(&combined) {
        let shadow_val = &cap[1];
        if has_colored_shadow(shadow_val) && shadow_has_blur(shadow_val) {
            css_colored += 1;
        }
    }

    let total = tw_colored + css_colored;

    if total >= 3 {
        PolishResult::fired(
            "glow-shadows",
            "Colored Box-Shadow Heavy Usage",
            SignalWeight::Low,
            CATEGORY,
            format!(
                "{} colored box-shadow{} (glow effects)",
                total,
                if total != 1 { "s" } else { "" }
            ),
            serde_json::json!({
                "tailwind_colored_shadows": tw_colored,
                "css_colored_shadows": css_colored,
            }),
        )
    } else {
        PolishResult::clear(
            "glow-shadows",
            "Colored Box-Shadow Heavy Usage",
            SignalWeight::Low,
            CATEGORY,
        )
    }
}

/// Check if a box-shadow value contains a non-gray color.
fn has_colored_shadow(shadow: &str) -> bool {
    // Match hex colors in the shadow value
    static HEX_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})(?-u:\b)").expect("hex regex")
    });
    static RGB_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"rgba?\s*\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)").expect("rgb regex")
    });

    // Check hex colors
    for cap in HEX_RE.captures_iter(shadow) {
        let hex = &cap[1];
        if !is_gray_hex(hex) {
            return true;
        }
    }

    // Check rgb/rgba colors
    for cap in RGB_RE.captures_iter(shadow) {
        let r: u8 = cap[1].parse().unwrap_or(0);
        let g: u8 = cap[2].parse().unwrap_or(0);
        let b: u8 = cap[3].parse().unwrap_or(0);
        if !is_gray_rgb(r, g, b) {
            return true;
        }
    }

    false
}

/// Check if a hex color is gray-ish (R, G, B channels within 30 of each other).
fn is_gray_hex(hex: &str) -> bool {
    let bytes = if hex.len() == 3 {
        let chars: Vec<u8> = hex
            .chars()
            .filter_map(|c| u8::from_str_radix(&c.to_string(), 16).ok())
            .map(|v| v * 17) // expand #abc to #aabbcc
            .collect();
        if chars.len() != 3 {
            return true;
        }
        chars
    } else if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        vec![r, g, b]
    } else {
        return true;
    };

    let max = *bytes.iter().max().unwrap_or(&0);
    let min = *bytes.iter().min().unwrap_or(&0);
    (max - min) < 30
}

/// Check if an RGB color is gray-ish.
fn is_gray_rgb(r: u8, g: u8, b: u8) -> bool {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    (max - min) < 30
}

/// True when any shadow segment has a blur radius above zero (the third
/// length). Color functions are blanked first so space-separated
/// `rgb(13 110 253 /.25)` arguments cannot be read as lengths.
fn shadow_has_blur(shadow: &str) -> bool {
    static COLOR_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(?:rgba?|hsla?|oklch|color)\([^)]*\)|#[0-9a-f]{3,8}")
            .expect("color token regex")
    });
    static LENGTH_RUN_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)(?:^|,)\s*(?:inset\s+)?-?[\d.]+(?:px|rem|em)?\s+-?[\d.]+(?:px|rem|em)?\s+([\d.]+)(?:px|rem|em)?",
        )
        .expect("shadow length regex")
    });
    let cleaned = COLOR_TOKEN_RE.replace_all(shadow, "C");
    LENGTH_RUN_RE
        .captures_iter(&cleaned)
        .any(|cap| cap[1].parse::<f64>().map(|v| v > 0.0).unwrap_or(false))
}

/// Flag two or more decorative blurred-orb patterns (Medium, 8).
pub fn floating_blobs(ctx: &PolishContext) -> PolishResult {
    // Check for class names that suggest blobs - inside class attribute
    // values only, so "glow"/"orb" in body copy do not count.
    let mut blob_classes = 0usize;
    for cap in CLASS_ATTR_RE.captures_iter(&ctx.html) {
        let value = cap
            .get(1)
            .or_else(|| cap.get(2))
            .or_else(|| cap.get(3))
            .map(|m| m.as_str())
            .unwrap_or("");
        blob_classes += BLOB_CLASS_RE.find_iter(value).count();
    }

    // Check for absolute + rounded-full pattern (either order in class string)
    let abs_round = ABSOLUTE_ROUND_RE.find_iter(&ctx.html).count()
        + ROUND_ABSOLUTE_RE.find_iter(&ctx.html).count();

    // Check for CSS patterns: position:absolute + border-radius:50% + filter:blur
    let combined = format!("{}\n{}", ctx.html, ctx.css);
    let has_blob_css = combined.contains("border-radius: 50%")
        && combined.contains("filter: blur")
        && combined.contains("position: absolute");

    let total = blob_classes + abs_round + if has_blob_css { 1 } else { 0 };

    if total >= 2 {
        PolishResult::fired(
            "floating-blobs",
            "Decorative Background Blobs",
            SignalWeight::Medium,
            CATEGORY,
            format!(
                "{} blob-like decorative element{} detected",
                total,
                if total != 1 { "s" } else { "" }
            ),
            serde_json::json!({
                "blob_class_matches": blob_classes,
                "absolute_rounded_full": abs_round,
                "css_blob_pattern": has_blob_css,
            }),
        )
    } else {
        PolishResult::clear(
            "floating-blobs",
            "Decorative Background Blobs",
            SignalWeight::Medium,
            CATEGORY,
        )
    }
}

#[cfg(test)]
mod tests;
