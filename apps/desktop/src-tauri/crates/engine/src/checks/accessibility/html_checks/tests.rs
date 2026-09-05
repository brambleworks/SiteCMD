#![cfg(test)]

use super::*;
use crate::checks::accessibility::form_labels::FormLabelsCheck;
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
fn test_lang_present_pass() {
    let html = r#"<html lang="en"><head></head><body></body></html>"#;
    let check = LangAttributeCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn test_lang_missing_fail() {
    let html = "<html><head></head><body></body></html>";
    let check = LangAttributeCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
}

#[test]
fn test_lang_unquoted_value_passes() {
    let html = "<html lang=en><head></head><body></body></html>";
    let results = LangAttributeCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn test_hreflang_link_does_not_satisfy_lang() {
    let html =
        r#"<html><head><link rel="alternate" hreflang="en" href="/en"></head><body></body></html>"#;
    let results = LangAttributeCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
}

#[test]
fn aria_labeled_icon_link_is_not_counted_empty() {
    let html = r#"<a href="/home" aria-label="Home"><svg viewBox="0 0 1 1"></svg></a>"#;
    let results = LinkTextCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn image_link_with_alt_is_not_counted_empty() {
    let html = r#"<a href="/"><img src="/logo.png" alt="Acme"></a>"#;
    let results = LinkTextCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn truly_empty_anchor_is_flagged() {
    let html = r#"<a href="/mystery"></a>"#;
    let results = LinkTextCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
    // Plural agreement: one weak link must not read "1 links".
    assert!(
        results[0].description.contains("1 link with weak text"),
        "{}",
        results[0].description
    );
}

#[test]
fn link_text_ignores_script_strings_and_comments() {
    let html = r#"<html><body>
        <a href="/pricing">See plans and pricing</a>
        <script>el.innerHTML = '<a href="/x">click here</a><a href="/y"></a>';</script>
        <!-- <a href="/old">here</a> -->
    </body></html>"#;
    let results = LinkTextCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn test_image_alt_present_pass() {
    let html = r#"<html><body><img src="photo.jpg" alt="A photo"><img src="icon.svg" alt=""></body></html>"#;
    let check = ImageAltAccessibilityCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn test_image_alt_missing_fail() {
    let html = r#"<html><body><img src="photo.jpg"><img src="other.jpg" alt="ok"></body></html>"#;
    let check = ImageAltAccessibilityCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert!(results[0].description.contains("1 of 2"));
}

#[test]
fn test_image_alt_no_images_pass() {
    let html = "<html><body><p>No images</p></body></html>";
    let check = ImageAltAccessibilityCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn image_alt_needs_attribute_boundary_and_real_img_tag() {
    let html = r#"<html><body>
        <img src="a.jpg" data-alt="nope">
        <img src="https://cdn.example.com/f.png?alt=media">
        <image href="vector.svg"></image>
        <img-lazy src="c.jpg"></img-lazy>
        <img src="d.jpg" alt = "A photo">
    </body></html>"#;
    let results = ImageAltAccessibilityCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
    let raw = results[0].raw_data.as_ref().unwrap();
    assert_eq!(raw["total"], 3, "only real <img> tags count");
    assert_eq!(raw["missing"], 2, "data-alt and ?alt=media are not alt");
}

#[test]
fn image_alt_skips_hidden_images_and_non_content_blocks() {
    // role=presentation/aria-hidden images are out of the accessibility
    // tree; images inside comments and scripts are not rendered.
    let html = r#"<html><body>
        <img src="deco.svg" role="presentation">
        <img src="spacer.gif" aria-hidden="true">
        <!-- <img src="old.jpg"> -->
        <script>el.innerHTML = '<img src="tpl.jpg">';</script>
        <img src="real.jpg" alt="The product">
    </body></html>"#;
    let results = ImageAltAccessibilityCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
    let raw = results[0].raw_data.as_ref().unwrap();
    assert_eq!(raw["total"], 1);
}

#[test]
fn image_alt_accepts_valueless_attribute_as_an_empty_value() {
    let results = ImageAltAccessibilityCheck.run(&ctx(r#"<img src="spacer.gif" alt>"#));
    assert_eq!(results[0].status, CheckStatus::Pass);
    let raw = results[0].raw_data.as_ref().unwrap();
    assert_eq!(raw["missing_alt_attribute"], 0);
    assert_eq!(raw["empty_alt_value"], 1);
}

#[test]
fn image_alt_copy_does_not_claim_alt_quality_or_decorative_intent() {
    let results = ImageAltAccessibilityCheck.run(&ctx(
        r#"<img src="hero.jpg" alt="image"><img src="spacer.gif" alt="">"#,
    ));
    let result = &results[0];
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result.description.contains("initial HTML"));
    assert!(result.description.contains("does not establish"));
    assert!(!result.description.contains("intentionally"));
    let raw = result.raw_data.as_ref().unwrap();
    assert_eq!(raw["alt_quality_assessed"], false);
}

#[test]
fn heading_order_ignores_script_templates_and_comments() {
    let html = r#"<html><body><h1>Title</h1><h2>Section</h2>
        <script type="text/x-template"><h5>Deep template</h5></script>
        <!-- <h6>Old</h6> --></body></html>"#;
    let results = HeadingOrderCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn heading_guidance_is_contextual_and_has_no_literal_braces_or_em_dashes() {
    let html = "<html><body><h1>A</h1><h4>Deep</h4></body></html>";
    let results = HeadingOrderCheck.run(&ctx(html));
    let fix = results[0].manual_fix.as_deref().unwrap();
    assert!(!fix.contains('{') && !fix.contains('}'), "{fix}");
    assert!(fix.contains("content structure"), "{fix}");
    assert!(!fix.contains("Never skip"), "{fix}");
    assert!(!fix.contains("exactly one <h1>"), "{fix}");
    assert_eq!(results[0].severity, Severity::Low);
    assert_eq!(
        results[0].confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(results[0]
        .why_it_matters
        .as_deref()
        .is_some_and(|text| !text.contains("break")));
    for text in [fix, &results[0].description, &results[0].title] {
        assert!(
            !text.contains('\u{2014}'),
            "em-dash in emitted copy: {text}"
        );
    }
}

#[test]
fn heading_order_issues_are_best_practice_not_wcag() {
    let html = "<html><body><h1>A</h1><h1>B</h1><h4>Deep</h4></body></html>";
    let results = HeadingOrderCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(
        !results[0].description.contains("WCAG"),
        "heading structure advice must not claim a WCAG failure: {}",
        results[0].description
    );
}

#[test]
fn heading_order_leaves_h1_count_to_the_seo_check() {
    let html = "<html><body><h1>One</h1><h1>Two</h1><h2>Sub</h2></body></html>";
    let results = HeadingOrderCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "H1 count belongs to seo.headings.h1: {}",
        results[0].description
    );
    let raw = results[0].raw_data.as_ref().unwrap();
    assert!(
        raw.get("h1_count").is_none(),
        "H1 count authority is seo.headings.h1: {raw}"
    );
}

#[test]
fn heading_order_missing_h1_is_not_an_order_issue() {
    let html = "<html><body><h2>Sub</h2><h3>Detail</h3></body></html>";
    let results = HeadingOrderCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn test_landmarks_all_present_pass() {
    let html =
        "<html><body><header></header><nav></nav><main></main><footer></footer></body></html>";
    let check = AriaLandmarksCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn test_landmarks_missing_warn() {
    let html = "<html><body><div>No landmarks</div></body></html>";
    let check = AriaLandmarksCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0].description.contains("main"));
}

#[test]
fn landmarks_main_alone_passes() {
    let html = "<html><body><main><p>Content</p></main></body></html>";
    let results = AriaLandmarksCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn landmarks_custom_element_is_not_main() {
    let html = "<html><body><maintenance-banner>Down at 5</maintenance-banner></body></html>";
    let results = AriaLandmarksCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
}

#[test]
fn landmarks_unquoted_role_is_recognized() {
    // Only role="main" (double quotes) was recognized before.
    let html = "<html><body><div role=main>Content</div></body></html>";
    let results = AriaLandmarksCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn skip_nav_css_selector_does_not_count() {
    let html = r#"<html><head><style>#main { color: red; }</style></head>
        <body><nav>links</nav><div id="main">Content</div></body></html>"#;
    let results = SkipNavCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
}

#[test]
fn skip_nav_href_fragment_counts() {
    let html = r##"<html><body><a href="#main" class="sr-only">Jump</a><main id="main"></main></body></html>"##;
    let results = SkipNavCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn skip_nav_prose_that_says_skip_to_does_not_satisfy_the_check() {
    // The accessibility fixture page has no skip link and carries the sentence
    // below; the old substring test passed it while the page bypassed nothing.
    let html = r#"<html><body><nav>links</nav>
        <p>You can skip to any chapter using the list below.</p>
        <div id="content">Content</div></body></html>"#;
    let results = SkipNavCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
}

#[test]
fn skip_nav_accepts_a_fragment_anchor_that_reads_as_a_skip_link() {
    let with_text = r##"<html><body><a href="#top-of-article">Skip to content</a>
        <p>Body</p></body></html>"##;
    assert_eq!(
        SkipNavCheck.run(&ctx(with_text))[0].status,
        CheckStatus::Pass
    );

    let with_label = r##"<html><body><a href="#story" aria-label="Skip to the story"></a>
        <p>Body</p></body></html>"##;
    assert_eq!(
        SkipNavCheck.run(&ctx(with_label))[0].status,
        CheckStatus::Pass
    );

    // The same wording outside an anchor still bypasses nothing.
    let off_page = r##"<html><body><a href="/help">Skip to content</a>
        <p>Body</p></body></html>"##;
    assert_eq!(
        SkipNavCheck.run(&ctx(off_page))[0].status,
        CheckStatus::Warn
    );
}

#[test]
fn skip_nav_accepts_a_fragment_written_after_a_path_or_query() {
    // Server-rendered sites often emit the current path before the fragment.
    // It is the same document, so the link works.
    for href in [
        "/article#main",
        "/article?page=2#main",
        "article#main",
        "#main",
    ] {
        let html =
            format!(r#"<html><body><a href="{href}">Skip to content</a><p>Body</p></body></html>"#);
        assert_eq!(
            SkipNavCheck.run(&ctx(&html))[0].status,
            CheckStatus::Pass,
            "{href} is an in-page fragment"
        );
    }

    // A fragment on another document navigates away and bypasses nothing.
    for href in [
        "https://elsewhere.example/article#main",
        "//elsewhere.example/article#main",
        "/article",
        "/article#",
    ] {
        let html = format!(
            r#"<html><body><a href="{href}">Skip to the story</a><p>Body</p></body></html>"#
        );
        assert_eq!(
            SkipNavCheck.run(&ctx(&html))[0].status,
            CheckStatus::Warn,
            "{href} is not an in-page fragment"
        );
    }
}

#[test]
fn image_alt_does_not_count_a_template_bound_alt_as_missing() {
    // Alpine.js template element found on visityourteam.com: no literal src or
    // alt, both bound at runtime.
    let html = r#"<html><body>
        <img x-show="product.image" :src="product.image" :alt="product.title">
        <img src="/hero.png" alt="A team on the pitch">
    </body></html>"#;
    let results = ImageAltAccessibilityCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
    let evidence = results[0].raw_data.as_ref().expect("alt evidence");
    assert_eq!(evidence["eligible_images"], 1);
    assert_eq!(evidence["missing_alt_attribute"], 0);
    assert_eq!(evidence["template_bound_alt"], 1);
    assert!(
        results[0].description.contains("client-side template"),
        "{}",
        results[0].description
    );
}

#[test]
fn image_alt_recognizes_every_framework_binding_spelling() {
    for binding in [":alt", "v-bind:alt", "x-bind:alt", "[alt]", "@alt"] {
        let html = format!(r#"<html><body><img src="/a.png" {binding}="title"></body></html>"#);
        let results = ImageAltAccessibilityCheck.run(&ctx(&html));
        let evidence = results[0].raw_data.as_ref().expect("alt evidence");
        assert_eq!(
            evidence["template_bound_alt"], 1,
            "{binding} must read as a template binding"
        );
        assert_eq!(results[0].status, CheckStatus::Pass, "{binding}");
    }

    // A literal alt still wins, and an unrelated attribute is not a binding.
    let literal = r#"<html><body><img src="/a.png" :data-alt="x"></body></html>"#;
    let results = ImageAltAccessibilityCheck.run(&ctx(literal));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert_eq!(
        results[0].raw_data.as_ref().expect("alt evidence")["missing_alt_attribute"],
        1
    );
}

#[test]
fn test_form_labels_all_labeled_pass() {
    let html = r#"<html><body>
        <label for="name">Name</label><input id="name" type="text">
        <label>Email <input type="email"></label>
        <input type="search" aria-label="Search the site">
    </body></html>"#;
    let check = FormLabelsCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn form_labels_remain_responsive_on_large_forms() {
    for (markup, expected) in [
        (
            "<label>Name</label><input aria-label=Name>",
            CheckStatus::Pass,
        ),
        ("<label>Name</label><input>", CheckStatus::Fail),
    ] {
        let context = ctx(&markup.repeat(80_000));
        let started = std::time::Instant::now();
        let results = FormLabelsCheck.run(&context);
        let elapsed = started.elapsed();
        assert_eq!(results[0].status, expected);
        assert_eq!(results[0].raw_data.as_ref().unwrap()["inputs"], 80_000);
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "form labels took {elapsed:?}"
        );
    }
}

#[test]
fn test_form_labels_missing_fail() {
    let html = r#"<html><body><input type="text"><input type="email"><input type="password"></body></html>"#;
    let check = FormLabelsCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
}

#[test]
fn form_labels_cite_name_role_value_not_info_and_relationships() {
    let html = r#"<html><body><input type="text"></body></html>"#;
    let results = FormLabelsCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert!(results[0].description.contains("4.1.2"));
    assert!(results[0].description.contains("3.3.2"));
    assert!(results[0].description.contains("If these controls render"));
    assert_eq!(
        results[0].confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(
        !results[0].description.contains("1.3.1"),
        "must not cite SC 1.3.1 for missing labels: {}",
        results[0].description
    );
}

#[test]
fn form_control_title_is_recognized_as_an_accessible_name_fallback() {
    let results = FormLabelsCheck.run(&ctx(r#"<input type="text" title="Search query">"#));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn empty_aria_label_does_not_count_as_a_name() {
    let results = FormLabelsCheck.run(&ctx(r#"<input type="text" aria-label="   ">"#));
    assert_eq!(results[0].status, CheckStatus::Fail);
}

#[test]
fn empty_label_element_does_not_count_as_a_name() {
    let results = FormLabelsCheck.run(&ctx(
        r#"<label for="query"></label><input id="query" type="text">"#,
    ));
    assert_eq!(results[0].status, CheckStatus::Fail);
}

#[test]
fn aria_labelledby_must_reference_an_observed_id() {
    let results = FormLabelsCheck.run(&ctx(
        r#"<input type="text" aria-labelledby="missing-label">"#,
    ));
    assert_eq!(results[0].status, CheckStatus::Fail);
}

#[test]
fn form_labels_ignore_inputs_in_script_strings() {
    let html = r#"<html><body>
        <script>form.innerHTML = '<input type="text"><input type="email">';</script>
        <p>No real form here.</p>
    </body></html>"#;
    let results = FormLabelsCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
    assert!(
        results[0].description.contains("No form inputs found"),
        "{}",
        results[0].description
    );
}

#[test]
fn form_labels_does_not_credit_unrelated_sibling_labels() {
    let html = r#"<html><body>
        <label>Name</label><input type="text">
        <label>Email</label><input type="email">
    </body></html>"#;
    let check = FormLabelsCheck;
    let results = check.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Fail,
        "sibling <label> with no for= must not count as labeling the next <input>"
    );
}

#[test]
fn form_labels_pass_does_not_consume_unrelated_labels_elsewhere() {
    let footer_labels = "<label>X</label>".repeat(20);
    let html = format!(
        r#"<html><body>
            <form>
                <input type="text" name="a">
                <input type="email" name="b">
                <input type="password" name="c">
            </form>
            <footer>{footer_labels}</footer>
        </body></html>"#
    );
    let check = FormLabelsCheck;
    let results = check.run(&ctx(&html));
    assert_eq!(
        results[0].status,
        CheckStatus::Fail,
        "20 unrelated <label>s must not mask 3 unlabeled <input>s"
    );
}

#[test]
fn form_labels_wrapping_label_with_inner_elements_passes() {
    let html = r#"<html><body>
        <label><span>Email</span> <input type="email"></label>
        <label><strong>Country</strong><select><option>US</option></select></label>
    </body></html>"#;
    let results = FormLabelsCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn form_labels_input_after_label_close_is_not_wrapped() {
    let html = r#"<html><body><label><span>Name</span></label><div><input type="text"></div></body></html>"#;
    let results = FormLabelsCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
}

#[test]
fn form_labels_minified_unquoted_attributes_pass() {
    let html = "<html><body><label for=name>Name</label><input id=name type=text><input type=search aria-label=Search><input type=hidden name=csrf value=tok></body></html>";
    let results = FormLabelsCheck.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "{}",
        results[0].description
    );
}

#[test]
fn form_labels_raw_data_counts_are_real() {
    let html = r#"<html><body>
        <label for="a">A</label><input id="a" type="text">
        <input type="search" aria-label="Search">
        <label>Wrapped <input type="email"></label>
        <input type="text" name="orphan">
    </body></html>"#;
    let results = FormLabelsCheck.run(&ctx(html));
    let raw = results[0].raw_data.as_ref().unwrap();
    assert_eq!(raw["inputs"], 4);
    assert_eq!(raw["labeled_via_for"], 1);
    assert_eq!(raw["labeled_via_aria"], 1);
    assert_eq!(raw["labeled_via_wrapping"], 1);
    assert_eq!(raw["unlabeled"], 1);
}

#[test]
fn test_autoplay_none_pass() {
    let html = r#"<html><body><video src="vid.mp4"></video></body></html>"#;
    let check = AutoplayCheck;
    let results = check.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn unmuted_autoplay_is_a_contextual_warning_not_a_proven_wcag_failure() {
    let html = r#"<html><body><video src="vid.mp4" autoplay></video></body></html>"#;
    let check = AutoplayCheck;
    let results = check.run(&ctx(html));
    let result = &results[0];
    assert_eq!(result.status, CheckStatus::Warn);
    assert_eq!(result.severity, Severity::Medium);
    assert_eq!(
        result.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(result.title.contains("declared"));
    assert!(result.description.contains("browser playback policy"));
    assert!(result.description.contains("more than 3 seconds"));
    assert!(!result.description.contains("May fail"));
}

#[test]
fn muted_autoplay_still_warns_about_long_running_motion() {
    let html = r#"<html><body><video src="vid.mp4" autoplay muted></video></body></html>"#;
    let check = AutoplayCheck;
    let results = check.run(&ctx(html));
    let result = &results[0];
    assert_eq!(result.status, CheckStatus::Warn);
    assert_eq!(result.severity, Severity::Low);
    assert_eq!(
        result.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(result.description.contains("more than 5 seconds"));
    assert!(result.description.contains("parallel with other content"));
    assert!(!result.description.contains("acceptable"));
}

#[test]
fn youtube_embed_allow_autoplay_is_not_autoplaying_media() {
    let html = r#"<html><body>
        <iframe src="https://www.youtube.com/embed/abc?autoplay=0"
            allow="accelerometer; autoplay; clipboard-write; encrypted-media"></iframe>
    </body></html>"#;
    let check = AutoplayCheck;
    let results = check.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Pass,
        "an iframe allow=autoplay grant is not autoplaying media"
    );
}

#[test]
fn text_muted_class_does_not_count_as_muted_media() {
    let html = r#"<html><body>
        <p class="text-muted">Some caption</p>
        <video src="vid.mp4" autoplay></video>
    </body></html>"#;
    let check = AutoplayCheck;
    let results = check.run(&ctx(html));
    assert_eq!(
        results[0].status,
        CheckStatus::Warn,
        "text-muted class must not mask an unmuted autoplaying video"
    );
}
