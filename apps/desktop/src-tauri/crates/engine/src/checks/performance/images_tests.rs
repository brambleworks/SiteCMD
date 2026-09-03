use super::*;
use crate::checks::{Check, CheckStatus, PageContext};
use http::header::HeaderMap;

fn ctx(body: &str) -> PageContext {
    PageContext {
        evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        url: url::Url::parse("https://example.com").unwrap(),
        response_headers: HeaderMap::new(),
        status_code: 200,
        body: body.to_string(),
        is_localhost: false,
        is_strict_localhost: false,
        http_version: Some("HTTP/2.0".to_string()),
        body_lower_cache: std::sync::OnceLock::new(),
    }
}

#[test]
fn test_images_no_images_pass() {
    let html = "<html><body><p>No images here</p></body></html>";
    let check = ImageOptimizationCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0].description.contains("<img>"));
    assert!(results[0].description.contains("fetched HTML"));
}

#[test]
fn test_images_missing_dimensions_warn() {
    let html = r#"<html><body><img src="photo.jpg" alt="test"><img src="photo2.jpg" alt="test2"></body></html>"#;
    let check = ImageOptimizationCheck;
    let results = check.run(&ctx(html));
    let dims = results
        .iter()
        .find(|r| r.check_id == "performance.images.dimensions");
    assert!(dims.is_some(), "should produce a dimensions warning");
    let dims = dims.unwrap();
    assert_eq!(dims.status, CheckStatus::Warn);
    assert_eq!(dims.confidence, crate::checks::IssueConfidence::Confirmed);
    assert!(dims
        .confidence_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("CSS")));
}

#[test]
fn test_images_with_dimensions_and_lazy_pass() {
    let html = r#"<html><body>
            <img src="hero.webp" width="800" height="600" alt="hero">
            <img src="thumb.webp" width="200" height="200" loading="lazy" alt="thumb">
        </body></html>"#;
    let check = ImageOptimizationCheck;
    let results = check.run(&ctx(html));
    // Should pass overall (has dimensions, has webp, has lazy on 2nd)
    let all_pass = results.iter().all(|r| r.status == CheckStatus::Pass);
    assert!(
        all_pass,
        "well-optimized images should pass: {:?}",
        results
            .iter()
            .map(|r| (&r.check_id, &r.status))
            .collect::<Vec<_>>()
    );
    assert!(results[0].description.contains("three source heuristics"));
    assert!(!results[0].description.contains("All optimized"));
}

#[test]
fn test_images_no_modern_format_warn() {
    let html =
        r#"<html><body><img src="photo.jpg" width="800" height="600" alt="test"></body></html>"#;
    let check = ImageOptimizationCheck;
    let results = check.run(&ctx(html));
    let format_result = results
        .iter()
        .find(|r| r.check_id == "performance.images.format");
    assert!(
        format_result.is_some(),
        "should warn about missing next-gen formats"
    );
    let format_result = format_result.unwrap();
    assert_eq!(format_result.status, CheckStatus::Warn);
    assert_eq!(
        format_result.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(format_result.description.contains("URL heuristic"));
    assert!(format_result.description.contains("did not fetch"));
    assert!(!format_result
        .description
        .contains("universal modern-browser"));
    assert!(!format_result.description.contains("~50%"));
}

#[test]
fn raw_data_includes_src_urls_for_each_subcheck() {
    let html = r#"<html><body>
            <img src="hero.jpg" alt="hero">
            <img src="card-1.png" alt="one">
            <img src="card-2.png" alt="two">
            <img src="card-3.png" alt="three">
        </body></html>"#;
    let check = ImageOptimizationCheck;
    let results = check.run(&ctx(html));

    // Lazy check: expect "card-1.png" etc in missing_lazy_examples
    let lazy = results
        .iter()
        .find(|r| r.check_id == "performance.images.lazy")
        .expect("lazy subcheck must fire");
    let lazy_examples = lazy
        .raw_data
        .as_ref()
        .and_then(|v| v.get("missing_lazy_examples"))
        .and_then(|v| v.as_array())
        .expect("missing_lazy_examples must be populated");
    assert!(
        lazy_examples
            .iter()
            .any(|v| v.as_str() == Some("card-1.png")),
        "raw_data.missing_lazy_examples must contain card-1.png: {:?}",
        lazy_examples
    );

    // Dimensions check: expect hero.jpg etc in missing_dimensions_examples
    let dims = results
        .iter()
        .find(|r| r.check_id == "performance.images.dimensions")
        .expect("dimensions subcheck must fire");
    let dim_examples = dims
        .raw_data
        .as_ref()
        .and_then(|v| v.get("missing_dimensions_examples"))
        .and_then(|v| v.as_array())
        .expect("missing_dimensions_examples must be populated");
    assert!(
        dim_examples.iter().any(|v| v.as_str() == Some("hero.jpg")),
        "raw_data.missing_dimensions_examples must contain hero.jpg: {:?}",
        dim_examples
    );

    // Format check: expect hero.jpg and card-*.png in legacy_looking_srcs
    let format = results
        .iter()
        .find(|r| r.check_id == "performance.images.format")
        .expect("format subcheck must fire");
    let format_examples = format
        .raw_data
        .as_ref()
        .and_then(|v| v.get("legacy_looking_srcs"))
        .and_then(|v| v.as_array())
        .expect("legacy_looking_srcs must be populated");
    assert!(
        format_examples
            .iter()
            .any(|v| v.as_str() == Some("hero.jpg")),
        "raw_data.legacy_looking_srcs must contain hero.jpg: {:?}",
        format_examples
    );
}

#[test]
fn image_source_evidence_preserves_case() {
    let html = r#"<IMG SRC="/Photos/Hero.JPG" ALT="hero">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    let format = results
        .iter()
        .find(|result| result.check_id == "performance.images.format")
        .expect("legacy-looking source should be surfaced");
    assert_eq!(
        format.raw_data.as_ref().unwrap()["legacy_looking_srcs"][0],
        "/Photos/Hero.JPG"
    );
    assert!(format.description.contains("/Photos/Hero.JPG"));
}

#[test]
fn unquoted_lazy_and_dimensions_are_recognized() {
    let html = r#"<img src=hero.webp width=800 height=600>
            <img src=thumb.webp width=200 height=200 loading=lazy>"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    assert!(
        results.iter().all(|r| r.status == CheckStatus::Pass),
        "{:?}",
        results
            .iter()
            .map(|r| (&r.check_id, &r.title))
            .collect::<Vec<_>>()
    );
}

#[test]
fn style_width_and_data_width_do_not_count_as_dimensions() {
    let html = r#"<img src="a.webp" style="width:100%" data-width="800" data-height="600" loading="lazy">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    let dims = results
        .iter()
        .find(|r| r.check_id == "performance.images.dimensions");
    assert!(dims.is_some(), "dimensions warning must fire");
}

#[test]
fn format_negotiating_cdns_are_not_flagged_as_legacy() {
    let html = r#"<img src="/_next/image?url=%2Fhero.jpg&w=828&q=75" width="800" height="600">
            <img src="https://res.cloudinary.com/demo/image/upload/f_auto/sample.jpg" width="1" height="1" loading="lazy">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    assert!(
        !results
            .iter()
            .any(|r| r.check_id == "performance.images.format"),
        "negotiated URLs must not trigger the format warning"
    );
}

#[test]
fn single_missing_lazy_image_uses_singular_grammar() {
    let html = r#"<img src="hero.webp" width="800" height="600"><img src="second.webp" width="800" height="600">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    let lazy = results
        .iter()
        .find(|r| r.check_id == "performance.images.lazy")
        .expect("lazy subcheck must fire");
    assert!(
        lazy.description.contains("1 `<img>` element after") && lazy.description.contains("has no"),
        "singular grammar expected: {}",
        lazy.description
    );
    // The fold inference is a markup-order heuristic; confidence must
    // say so.
    assert_eq!(lazy.confidence, crate::checks::IssueConfidence::NeedsReview);
    assert!(lazy.confidence_reason.is_some());
    assert!(lazy
        .why_it_matters
        .as_deref()
        .is_some_and(|why| why.starts_with("If")));
}

#[test]
fn single_image_missing_dimensions_uses_singular_and_hedged_cls_claim() {
    let html = r#"<img src="a.webp" loading="lazy">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    let dims = results
        .iter()
        .find(|r| r.check_id == "performance.images.dimensions")
        .expect("dimensions subcheck must fire");
    assert!(
        dims.description.contains("1 image is missing")
            && dims.description.contains("can contribute"),
        "expected singular + hedged copy: {}",
        dims.description
    );
}

#[test]
fn empty_or_non_numeric_dimensions_do_not_reserve_an_intrinsic_ratio() {
    let html = r#"<img src="a.webp" width="" height="auto"><img src="b.webp" width="eight" height="10" loading="lazy">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    let dims = results
        .iter()
        .find(|result| result.check_id == "performance.images.dimensions")
        .expect("invalid dimensions should be surfaced");
    assert!(dims.title.contains("usable width/height"));
    assert_eq!(
        dims.raw_data.as_ref().unwrap()["missing_or_invalid_dimensions"],
        2
    );
}

#[test]
fn test_fonts_no_custom_fonts_pass() {
    let html = "<html><body><p>System fonts only</p></body></html>";
    let check = FontLoadingCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(results[0].description.contains("fetched HTML"));
    assert!(!results[0].description.contains("Using system fonts"));
}

#[test]
fn test_fonts_google_fonts_without_display_warn() {
    let html = r#"<html><head><link href="https://fonts.googleapis.com/css2?family=Roboto" rel="stylesheet"></head></html>"#;
    let check = FontLoadingCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0]
        .description
        .contains("explicit font-display policy"));
    assert!(results[0]
        .why_it_matters
        .as_deref()
        .is_some_and(|why| why.contains("can")));
}

#[test]
fn google_fonts_fix_names_the_display_swap_url_parameter() {
    let html = r#"<link href="https://fonts.googleapis.com/css2?family=Roboto" rel="stylesheet">"#;
    let results = FontLoadingCheck.run(&ctx(html));
    let fix = results[0].manual_fix.as_deref().unwrap_or("");
    assert!(
        fix.contains("&display=swap") && fix.contains("fonts.googleapis.com"),
        "Google Fonts trigger must get the URL-parameter fix: {fix}"
    );
}

#[test]
fn font_face_rules_are_reported_as_declarations_not_fonts() {
    let css = "@font-face{font-family:A;font-weight:400;font-display:swap}".repeat(4);
    let html = format!("<style>{css}</style>");
    let results = FontLoadingCheck.run(&ctx(&html));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(
        results[0].description.contains("font-face declarations")
            && !results[0].description.contains("custom fonts detected"),
        "copy must describe what was counted: {}",
        results[0].description
    );
    assert!(results[0].title.contains("font-face declarations"));
}

#[test]
fn test_fonts_html_escaped_display_swap_passes() {
    let html = r#"<link href="https://fonts.googleapis.com/css2?family=Roboto&amp;display=swap" rel="stylesheet">"#;
    let results = FontLoadingCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn google_fonts_optional_display_is_an_explicit_policy() {
    let html = r#"<link href="https://fonts.googleapis.com/css2?family=Roboto&amp;display=optional" rel="stylesheet">"#;
    let results = FontLoadingCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn one_font_display_declaration_does_not_hide_an_unconfigured_face() {
    let html = r#"<style>
            @font-face { font-family: A; src: url(a.woff2); font-display: swap; }
            @font-face { font-family: B; src: url(b.woff2); }
        </style>"#;
    let results = FontLoadingCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0].description.contains("1 @font-face declaration"));
}

/// The BBC article-card shape: a `<picture>` whose `<source type="image/webp">`
/// carries the responsive candidates and whose `<img>` keeps a .jpg fallback.
/// A supporting browser downloads the WebP, so grading the fallback URL called
/// 92 of 93 images legacy on bbc.co.uk.
#[test]
fn a_jpeg_fallback_inside_a_webp_picture_is_not_a_legacy_source() {
    let html = r#"<picture><source srcSet="https://cdn.example.net/a.jpg.webp 480w, https://cdn.example.net/a-800.jpg.webp 800w" type="image/webp" sizes="480px"/><img alt="story" src="https://cdn.example.net/a.jpg" srcSet="https://cdn.example.net/a.jpg 480w" width="240" height="135" loading="lazy"/></picture>"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    assert!(
        !results
            .iter()
            .any(|result| result.check_id == "performance.images.format"),
        "a WebP source in the enclosing picture answers the format question: {:?}",
        results
            .iter()
            .map(|result| result.description.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn an_avif_srcset_candidate_answers_the_format_question_for_its_own_element() {
    let html = r#"<img src="/hero.png" srcset="/hero.avif 1x, /hero-2x.avif 2x" width="800" height="600" loading="lazy">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    assert!(!results
        .iter()
        .any(|result| result.check_id == "performance.images.format"));
}

#[test]
fn a_picture_without_a_modern_source_still_grades_its_fallback() {
    let html = r#"<picture><source srcset="/wide.png" media="(min-width: 800px)"><img src="/hero.png" width="800" height="600" loading="lazy"></picture>"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    let format = results
        .iter()
        .find(|result| result.check_id == "performance.images.format")
        .expect("a picture offering only PNG keeps the legacy-source finding");
    assert_eq!(format.raw_data.as_ref().unwrap()["legacy_looking_count"], 1);
}

#[test]
fn a_modern_picture_does_not_exempt_an_unrelated_image_after_it() {
    let html = r#"<picture><source srcset="/a.webp" type="image/webp"><img src="/a.jpg" width="8" height="6" loading="lazy"></picture>
        <img src="/standalone.png" width="8" height="6" loading="lazy">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    let format = results
        .iter()
        .find(|result| result.check_id == "performance.images.format")
        .expect("the standalone PNG is still a candidate");
    assert_eq!(format.raw_data.as_ref().unwrap()["legacy_looking_count"], 1);
    assert!(
        format.description.contains("standalone.png"),
        "{}",
        format.description
    );
}

#[test]
fn one_repeated_source_url_is_listed_once() {
    // github.com listed particles-170bd1fd231f4669.png three times as "3 values".
    let repeated: String = (0..3)
        .map(|_| {
            r#"<img src="/assets/particles.png" width="8" height="6" loading="lazy">"#.to_string()
        })
        .collect();
    let results = ImageOptimizationCheck.run(&ctx(&repeated));
    let format = results
        .iter()
        .find(|result| result.check_id == "performance.images.format")
        .expect("format subcheck must fire");
    let examples = format.raw_data.as_ref().unwrap()["legacy_looking_srcs"]
        .as_array()
        .expect("examples array")
        .clone();
    assert_eq!(examples.len(), 1, "{examples:?}");
    assert_eq!(format.raw_data.as_ref().unwrap()["legacy_looking_count"], 3);
}

#[test]
fn pixel_spacers_and_tracking_beacons_are_not_page_images() {
    // hn: `<img src="s.gif" height="1" width="14">` and `height="10" width="0"`.
    // bbc: a comScore beacon with no dimensions at all.
    let html = r#"<img src="/hero.webp" width="800" height="600">
        <img src="s.gif" height="1" width="14">
        <img src="s.gif" height="10" width="0">
        <img src="https://sb.scorecardresearch.com/p?c1=2&amp;c2=17986528" alt="">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    assert!(
        results.len() == 1 && results[0].check_id == "performance.images",
        "only the hero image is gradeable: {:?}",
        results
            .iter()
            .map(|result| result.check_id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(
        results[0]
            .description
            .contains("3 `<img>` elements were not graded (3 tracking beacons or spacers)"),
        "{}",
        results[0].description
    );
    let raw = results[0].raw_data.as_ref().expect("raw data");
    assert_eq!(raw["img_elements_found"], 4);
    assert_eq!(raw["beacon_or_spacer_elements"], 3);
}

#[test]
fn framework_bound_sources_are_reported_as_ungraded_not_as_defects() {
    // smarthomeu.com: an Alpine template element with no literal src at all was
    // graded as an image missing width, height, and lazy loading.
    let html = r#"<img src="/hero.webp" width="800" height="600">
        <img x-show="product.image" :src="product.image" :alt="product.title">
        <img v-bind:src="item.url" alt="vue">
        <img [src]="item.url" alt="angular">
        <img src="/real.webp" @loading="mode" width="8" height="6">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    assert!(
        results.len() == 1 && results[0].check_id == "performance.images",
        "bound elements carry no measurable image: {:?}",
        results
            .iter()
            .map(|result| format!("{}: {}", result.check_id, result.description))
            .collect::<Vec<_>>()
    );
    let raw = results[0].raw_data.as_ref().expect("raw data");
    // Three elements bind `src` itself and are not images this check can see.
    // The fourth has a literal src and only binds `loading`, so it is a real
    // image whose lazy-loading state alone is unreadable.
    assert_eq!(raw["template_binding_elements"], 3);
    assert!(
        results[0]
            .description
            .contains("3 client-side template bindings"),
        "{}",
        results[0].description
    );
}

#[test]
fn a_runtime_bound_attribute_only_withdraws_the_sub_check_that_reads_it() {
    // A literal src with `:width`/`:height` is still a gradeable image URL:
    // dropping it from performance.images.format would hide a real finding.
    let html = r#"<img src="/first.webp" width="8" height="6" loading="lazy">
        <img src="/hero.png" :width="w" :height="h" :loading="mode">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));

    let format = results
        .iter()
        .find(|result| result.check_id == "performance.images.format")
        .expect("the legacy-looking src is still graded");
    assert_eq!(format.raw_data.as_ref().unwrap()["legacy_looking_count"], 1);

    assert!(
        !results
            .iter()
            .any(|result| result.check_id == "performance.images.dimensions"),
        "bound width/height is unreadable, not missing"
    );
    assert!(
        !results
            .iter()
            .any(|result| result.check_id == "performance.images.lazy"),
        "bound loading is unreadable, not missing"
    );
}

#[test]
fn a_bound_attribute_count_is_reported_next_to_the_finding_it_limits() {
    let html = r#"<img src="/first.webp" width="8" height="6">
        <img src="/second.webp" :loading="mode" width="8" height="6">
        <img src="/third.webp" width="8" height="6">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    let lazy = results
        .iter()
        .find(|result| result.check_id == "performance.images.lazy")
        .expect("the third image is a lazy candidate");

    assert_eq!(
        lazy.raw_data.as_ref().unwrap()["runtime_bound_loading_elements"],
        1
    );
    assert!(
        lazy.description
            .contains("1 further element binds `loading` at runtime"),
        "{}",
        lazy.description
    );
}

#[test]
fn one_ungraded_element_is_described_in_the_singular() {
    let html =
        r#"<img src="/hero.webp" width="8" height="6"><img src="s.gif" width="1" height="1">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    assert!(
        results[0]
            .description
            .contains("1 `<img>` element was not graded (1 tracking beacon or spacer)"),
        "{}",
        results[0].description
    );
}

#[test]
fn a_literal_source_with_a_bound_alt_is_still_a_real_image() {
    // Only the bound attribute is unmeasurable; the element itself is real.
    let html = r#"<img src="/first.webp" width="8" height="6">
        <img src="/hero.png" :alt="product.title">"#;
    let results = ImageOptimizationCheck.run(&ctx(html));
    assert!(results
        .iter()
        .any(|result| result.check_id == "performance.images.format"));
    assert!(results
        .iter()
        .any(|result| result.check_id == "performance.images.dimensions"));
}
