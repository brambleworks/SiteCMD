//! Static image checks for lazy-loading, intrinsic dimensions, and legacy URLs.
//! Runtime layout, selected sources, and inserted images remain out of scope.

use crate::checks::html_attrs::{attr_value, tag_slices};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

static FONT_FACE_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)@font-face\s*\{([^}]*)\}").expect("valid font-face block regex")
});
static FONT_DISPLAY_POLICY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)font-display\s*:\s*(?:auto|block|swap|fallback|optional)\b")
        .expect("valid font-display policy regex")
});
static GOOGLE_DISPLAY_POLICY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:[?&]|&amp;)display=(?:auto|block|swap|fallback|optional)(?:[&#"'\s>]|$)"#)
        .expect("valid Google Fonts display policy regex")
});

/// Facts shared by image sub-checks, including source URLs for remediation.
struct ImgTag {
    src: Option<String>,
    has_usable_width: bool,
    has_usable_height: bool,
    has_lazy: bool,
}

fn is_positive_integer_attr(tag: &str, name: &str) -> bool {
    attr_value(tag, name)
        .is_some_and(|value| value.trim().parse::<u32>().is_ok_and(|number| number > 0))
}

fn collect_img_tags(body: &str, lower: &str) -> Vec<ImgTag> {
    tag_slices(body, lower, "img")
        .into_iter()
        .map(|tag| ImgTag {
            src: attr_value(tag, "src").filter(|src| !src.trim().is_empty()),
            has_usable_width: is_positive_integer_attr(tag, "width"),
            has_usable_height: is_positive_integer_attr(tag, "height"),
            has_lazy: attr_value(tag, "loading")
                .is_some_and(|value| value.eq_ignore_ascii_case("lazy")),
        })
        .collect()
}

/// Whether an image URL goes through a format-negotiating optimizer or
/// image CDN (Cloudinary f_auto, imgix auto=format, Cloudflare
/// format=auto / cdn-cgi/image / Images delivery, Next.js optimizer).
fn uses_format_negotiation(src_lower: &str) -> bool {
    src_lower.contains("f_auto")
        || src_lower.contains("auto=format")
        || src_lower.contains("format=auto")
        || src_lower.contains("/_next/image")
        || src_lower.contains("/cdn-cgi/image/")
        || src_lower.contains("imagedelivery.net")
}

fn evidence_src(src: &str) -> String {
    let trimmed = src.trim();
    if trimmed.to_ascii_lowercase().starts_with("data:") {
        return "[data-uri]".to_string();
    }
    crate::log_sanitizer::evidence_safe_url_reference(trimmed)
}

pub struct ImageOptimizationCheck;

impl Check for ImageOptimizationCheck {
    fn id(&self) -> &str {
        "performance.images"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let mut results = Vec::new();

        // Retain bounded image locations so fix prompts can identify offenders.
        let images = collect_img_tags(&ctx.body, lower);
        let img_count = images.len();
        if img_count == 0 {
            return vec![CheckResult {
                check_id: "performance.images".into(),
                category: ScanCategory::Performance,
                title: "Image optimization".into(),
                description: "No `<img>` element was found in the fetched HTML. CSS backgrounds, `<source>` elements, runtime-inserted images, and other image delivery paths are outside this markup check.".into(),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        // Lazy loading: skip the first image (above-the-fold convention).
        let unlazy: Vec<&ImgTag> = images.iter().skip(1).filter(|img| !img.has_lazy).collect();
        let missing_lazy = unlazy.len();
        let lazy_count = images.iter().filter(|img| img.has_lazy).count();

        if missing_lazy > 0 {
            let surfaced: Vec<String> = unlazy
                .iter()
                .take(5)
                .filter_map(|img| img.src.as_deref().map(evidence_src))
                .collect();
            results.push(CheckResult {
                check_id: "performance.images.lazy".into(), category: ScanCategory::Performance,
                title: "Images may be candidates for lazy loading".into(),
                description: format!(
                    "{} `<img>` element{} after the first image in source order {} no `loading=\"lazy\"` attribute. The check treats later markup as candidates but does not measure viewport position, LCP, browser fetch priority, or runtime layout.{}",
                    missing_lazy,
                    if missing_lazy == 1 { "" } else { "s" },
                    if missing_lazy == 1 { "has" } else { "have" },
                    if surfaced.is_empty() { String::new() } else { format!(" Examples: {}", surfaced.join(", ")) }
                ),
                status: CheckStatus::Warn, severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: Some("Inspect each candidate at representative breakpoints. Keep the measured LCP image and images near the initial viewport eager; add native `loading=\"lazy\"` only to images that begin sufficiently off-screen, then verify LCP, fast scrolling, no-JavaScript behavior, and back/forward navigation.".into()),
                raw_data: Some(serde_json::json!({
                    "total": img_count,
                    "lazy": lazy_count,
                    "missing_lazy": missing_lazy,
                    "missing_lazy_examples": surfaced,
                })),
                // Fold position is inferred from source order, not layout;
                // pages with several above-the-fold images are legitimate.
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Fold position is inferred from markup order: every image after the first is assumed below the fold, which layout can contradict.".into()),
                why_it_matters: Some("If these images begin well off-screen, eager fetching can consume bandwidth or compete with more important resources; source order alone does not establish that impact.".into()),
            });
        }

        // Dimensions
        let no_dims: Vec<&ImgTag> = images
            .iter()
            .filter(|img| !(img.has_usable_width && img.has_usable_height))
            .collect();
        let missing_dimensions = no_dims.len();

        if missing_dimensions > 0 {
            let surfaced: Vec<String> = no_dims
                .iter()
                .take(5)
                .filter_map(|img| img.src.as_deref().map(evidence_src))
                .collect();
            results.push(CheckResult {
                check_id: "performance.images.dimensions".into(), category: ScanCategory::Performance,
                title: "Images missing usable width/height attributes".into(),
                description: format!(
                    "{} image{} missing a positive-integer width and/or height content attribute. This can contribute to Cumulative Layout Shift unless CSS or another stable layout constraint reserves the correct aspect ratio.{}",
                    missing_dimensions,
                    if missing_dimensions == 1 { " is" } else { "s are" },
                    if surfaced.is_empty() { String::new() } else { format!(" Examples: {}", surfaced.join(", ")) }
                ),
                status: CheckStatus::Warn, severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: Some("Inspect the rendered layout first. Supply accurate positive numeric width and height attributes when intrinsic dimensions are known, or reserve the correct ratio with a stable CSS/container strategy when they are not. Verify with the image cache disabled and a layout-shift trace rather than assuming a framework component reserves the right box.".into()),
                raw_data: Some(serde_json::json!({
                    "missing_or_invalid_dimensions": missing_dimensions,
                    "missing_dimensions_examples": surfaced,
                })),
                confidence: crate::checks::IssueConfidence::Confirmed,
                confidence_reason: Some("The missing or invalid content attributes are directly observed, but CSS, aspect-ratio, containers, and runtime layout were not inspected, so the scan cannot establish that space is unreserved or that CLS occurs.".into()),
                why_it_matters: Some("If no other layout constraint reserves the correct ratio, the image can shift surrounding content when its intrinsic dimensions become available.".into()),
            });
        }

        // Source extensions are only a candidate signal. The response can be
        // negotiated to another format, and picture/srcset selection can make
        // the fallback src irrelevant for a given browser or viewport.
        let legacy_candidates: Vec<&ImgTag> = images
            .iter()
            .filter(|img| {
                img.src.as_ref().is_some_and(|src| {
                    let lower_src = src.to_ascii_lowercase();
                    let path = lower_src
                        .split(['?', '#'])
                        .next()
                        .unwrap_or(lower_src.as_str());
                    matches!(path.rsplit('.').next(), Some("jpg" | "jpeg" | "png"))
                        && !uses_format_negotiation(&lower_src)
                })
            })
            .collect();
        let legacy_looking_srcs: Vec<String> = legacy_candidates
            .iter()
            .take(5)
            .filter_map(|img| img.src.as_deref().map(evidence_src))
            .collect();

        if !legacy_candidates.is_empty() {
            results.push(CheckResult {
                check_id: "performance.images.format".into(), category: ScanCategory::Performance,
                title: "Legacy-looking image source URLs need review".into(),
                description: format!(
                    "The URL heuristic found {} `<img src>` value{} ending in .jpg, .jpeg, or .png without a recognized format-negotiation pattern. This source check did not fetch those responses, inspect Content-Type, compare bytes or visual quality, or determine which `<picture>`/`srcset` candidate a browser selects.{}",
                    legacy_candidates.len(),
                    if legacy_candidates.len() == 1 { "" } else { "s" },
                    if legacy_looking_srcs.is_empty() { String::new() } else { format!(" Examples: {}", legacy_looking_srcs.join(", ")) }
                ),
                status: CheckStatus::Warn, severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: Some("Inspect the selected production response at representative viewports and compare its Content-Type, transferred bytes, decode behavior, and visual quality with well-encoded alternatives. Keep JPEG or PNG when it is the best measured choice; otherwise add supported AVIF/WebP variants through a maintained image pipeline with correct fallbacks, cache variation, and responsive sizing.".into()),
                raw_data: Some(serde_json::json!({
                    "legacy_looking_count": legacy_candidates.len(),
                    "legacy_looking_srcs": legacy_looking_srcs,
                    "response_content_types_inspected": false,
                    "responsive_candidate_selection_measured": false,
                })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The source URL suffixes are directly observed, but URL shape does not establish the served Content-Type, selected responsive candidate, compression efficiency, visual requirements, or actual transfer cost.".into()),
                why_it_matters: Some("If the selected response is materially larger than an acceptable supported alternative, it can increase transfer or decode cost; this URL-only heuristic does not establish those savings.".into()),
            });
        }

        if results.is_empty() {
            results.push(CheckResult {
                check_id: "performance.images".into(),
                category: ScanCategory::Performance,
                title: "Image markup heuristics".into(),
                description: format!("{} `<img>` element{} inspected. None matched this check's three source heuristics: later markup without `loading=\"lazy\"`, missing usable numeric width/height attributes, or a legacy-looking src URL without a recognized negotiation pattern. This does not prove image delivery is optimized.", img_count, if img_count == 1 { " was" } else { "s were" }),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({ "img_elements_inspected": img_count })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            });
        }

        results
    }
}

pub struct FontLoadingCheck;

impl Check for FontLoadingCheck {
    fn id(&self) -> &str {
        "performance.fonts"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();

        let font_face_blocks = FONT_FACE_BLOCK_RE
            .captures_iter(&ctx.body)
            .filter_map(|capture| capture.get(1).map(|block| block.as_str()))
            .collect::<Vec<_>>();
        let font_face_count = font_face_blocks.len();
        let font_faces_without_policy = font_face_blocks
            .iter()
            .filter(|block| !FONT_DISPLAY_POLICY_RE.is_match(block))
            .count();

        // A fonts.gstatic.com preconnect does not prove a font stylesheet is
        // loaded. Count only actual Google Fonts stylesheet links.
        let google_stylesheets = tag_slices(&ctx.body, lower, "link")
            .into_iter()
            .filter(|tag| {
                attr_value(tag, "rel").is_some_and(|rel| {
                    rel.split_whitespace()
                        .any(|token| token.eq_ignore_ascii_case("stylesheet"))
                }) && attr_value(tag, "href")
                    .is_some_and(|href| href.to_ascii_lowercase().contains("fonts.googleapis.com"))
            })
            .filter_map(|tag| attr_value(tag, "href"))
            .collect::<Vec<_>>();
        let google_stylesheet_count = google_stylesheets.len();
        let google_without_policy = google_stylesheets
            .iter()
            .filter(|href| !GOOGLE_DISPLAY_POLICY_RE.is_match(href))
            .count();
        let font_count = font_face_count + google_stylesheet_count;
        let missing_policy_count = font_faces_without_policy + google_without_policy;
        let has_font_display = missing_policy_count == 0;

        if font_count == 0 {
            return vec![CheckResult {
                check_id: "performance.fonts".into(),
                category: ScanCategory::Performance,
                title: "Font loading".into(),
                description: "No inline @font-face block or Google Fonts stylesheet link was detected in the fetched HTML. Fonts loaded only through external stylesheets are outside this source check.".into(),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        let mut issues = Vec::new();
        if font_count > 3 {
            let mut parts = Vec::new();
            if font_face_count > 0 {
                parts.push(format!(
                    "{} @font-face declaration{}",
                    font_face_count,
                    if font_face_count == 1 { "" } else { "s" }
                ));
            }
            if google_stylesheet_count > 0 {
                parts.push(format!(
                    "{} Google Fonts stylesheet{}",
                    google_stylesheet_count,
                    if google_stylesheet_count == 1 {
                        ""
                    } else {
                        "s"
                    }
                ));
            }
            issues.push(format!(
                "{} detected (each @font-face weight or style counts separately) - review whether every family and weight is used",
                parts.join(" plus ")
            ));
        }
        if font_faces_without_policy > 0 {
            issues.push(format!(
                "{} @font-face declaration{} {} no explicit font-display policy",
                font_faces_without_policy,
                if font_faces_without_policy == 1 {
                    ""
                } else {
                    "s"
                },
                if font_faces_without_policy == 1 {
                    "has"
                } else {
                    "have"
                }
            ));
        }
        if google_without_policy > 0 {
            issues.push(format!(
                "{} Google Fonts stylesheet URL{} {} no explicit font-display policy",
                google_without_policy,
                if google_without_policy == 1 { "" } else { "s" },
                if google_without_policy == 1 {
                    "has"
                } else {
                    "have"
                }
            ));
        }

        vec![CheckResult {
            check_id: "performance.fonts".into(),
            category: ScanCategory::Performance,
            title: if issues.is_empty() {
                "Font loading".into()
            } else if !has_font_display {
                "Custom fonts missing font-display".into()
            } else {
                "More than 3 font-face declarations loaded".into()
            },
            description: if issues.is_empty() {
                format!(
                    "{} font source declaration{} with an explicit font-display policy.",
                    font_count,
                    if font_count == 1 { "" } else { "s" }
                )
            } else {
                format!("Font loading issues: {}", issues.join("; "))
            },
            status: if issues.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if issues.is_empty() {
                None
            } else if google_without_policy > 0 {
                // Google Fonts URLs need a display query parameter, not an owned @font-face rule.
                Some("Add a supported display policy to each affected Google Fonts stylesheet URL, for example `https://fonts.googleapis.com/css2?family=Roboto&display=swap` (use `&amp;display=swap` in literal HTML) or choose `display=optional` when immediate fallback better fits the design. Add `font-display: swap`, `fallback`, or `optional` to each affected self-hosted @font-face rule as appropriate, then verify text rendering under throttled and cached conditions.".into())
            } else {
                Some("Set an explicit `font-display` value on each affected @font-face rule. Choose `swap`, `fallback`, or `optional` based on the acceptable fallback and late-swap behavior, remove unused faces or weights, and verify first-load and repeat-load rendering under network throttling.".into())
            },
            raw_data: Some(serde_json::json!({
                "font_count": font_count,
                "font_face_declarations": font_face_count,
                "google_fonts_stylesheets": google_stylesheet_count,
                "missing_font_display": missing_policy_count,
                "has_font_display": has_font_display,
            })),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if issues.is_empty() {
                None
            } else {
                Some("Without an explicit font-display policy, the browser's default `auto` behavior can hide fallback text or replace it later depending on browser and load timing. Extra font files can also add transfer and rendering work; measure the actual page before removing intentional brand typography.".into())
            },
        }]
    }
}

#[cfg(test)]
#[path = "images_tests.rs"]
mod tests;
