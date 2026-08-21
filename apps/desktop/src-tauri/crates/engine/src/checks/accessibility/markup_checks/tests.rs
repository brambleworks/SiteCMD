use super::*;
use crate::checks::{Check, CheckStatus};

fn ctx(body: &str) -> PageContext {
    PageContext {
        evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        url: url::Url::parse("https://example.com").expect("static test url"),
        response_headers: http::header::HeaderMap::new(),
        status_code: 200,
        body: body.to_string(),
        is_localhost: false,
        is_strict_localhost: false,
        http_version: Some("HTTP/2.0".to_string()),
        body_lower_cache: std::sync::OnceLock::new(),
    }
}

#[test]
fn user_scalable_no_and_low_maximum_scale_fail() {
    let html =
        r#"<meta name="viewport" content="width=device-width, initial-scale=1, user-scalable=no">"#;
    let results = ViewportZoomCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert!(results[0].description.contains("user-scalable=no"));

    let html = r#"<meta content="width=device-width, maximum-scale=1.0" name='viewport'>"#;
    let results = ViewportZoomCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert!(results[0].description.contains("maximum-scale=1.0"));
}

#[test]
fn user_scalable_no_titles_as_blocked_but_maximum_scale_as_restricted() {
    let html =
        r#"<meta name="viewport" content="width=device-width, initial-scale=1, user-scalable=no">"#;
    let results = ViewportZoomCheck.run(&ctx(html));
    assert!(results[0].title.contains("blocks pinch-to-zoom"));
    assert!(results[0].description.contains("disables zooming"));

    let html = r#"<meta name="viewport" content="width=device-width, maximum-scale=1.5">"#;
    let results = ViewportZoomCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert!(
        results[0].title.contains("restricts zooming"),
        "maximum-scale alone must not title as blocked: {}",
        results[0].title
    );
    assert!(results[0]
        .description
        .contains("limits how far users can zoom"));
}

#[test]
fn permissive_or_missing_viewport_passes() {
    // The standard viewport tag does not restrict zoom.
    let html = r#"<meta name="viewport" content="width=device-width, initial-scale=1">"#;
    assert_eq!(
        ViewportZoomCheck.run(&ctx(html))[0].status,
        CheckStatus::Pass
    );
    // maximum-scale=5 satisfies the 200% requirement.
    let html = r#"<meta name="viewport" content="width=device-width, maximum-scale=5">"#;
    assert_eq!(
        ViewportZoomCheck.run(&ctx(html))[0].status,
        CheckStatus::Pass
    );
    // Missing viewport is seo.viewport's finding, not a zoom restriction.
    assert_eq!(ViewportZoomCheck.run(&ctx(""))[0].status, CheckStatus::Pass);
    // user-scalable=yes must not substring-match the =no test.
    let html = r#"<meta name="viewport" content="width=device-width, user-scalable=yes">"#;
    assert_eq!(
        ViewportZoomCheck.run(&ctx(html))[0].status,
        CheckStatus::Pass
    );
}

#[test]
fn heading_with_no_text_and_no_name_warns() {
    let html = r#"<h1>Real title</h1><h2></h2><h3>   </h3>"#;
    let results = EmptyHeadingsCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0].description.contains("2 of 3"));
}

#[test]
fn labeled_hidden_or_image_headings_are_not_empty() {
    // aria-hidden headings are out of the tree; an aria-label or an
    // alt-texted image names the heading; script content is not a heading.
    let html = r#"
        <h1>Title</h1>
        <h2 aria-hidden="true"></h2>
        <h2 aria-label="Quarterly results"></h2>
        <h3><img src="/wordmark.svg" alt="Acme Corp"></h3>
        <script>const h = "<h4></h4>";</script>
    "#;
    let results = EmptyHeadingsCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn visible_untitled_iframe_fails() {
    let html =
        r#"<iframe src="https://www.youtube.com/embed/xyz" width="560" height="315"></iframe>"#;
    let results = IframeTitleCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Fail);
    assert!(results[0].description.contains("1 of 1"));
}

#[test]
fn titled_hidden_and_noscript_iframes_pass() {
    // A titled frame, the hidden GTM noscript frame, a zero-sized pixel,
    // and an aria-hidden frame are all fine.
    let html = r#"
        <iframe title="Product demo video" src="/demo"></iframe>
        <noscript><iframe src="https://www.googletagmanager.com/ns.html?id=GTM-X"
            height="0" width="0" style="display:none;visibility:hidden"></iframe></noscript>
        <iframe src="/pixel" width="0" height="0"></iframe>
        <iframe src="/decoration" aria-hidden="true"></iframe>
    "#;
    let results = IframeTitleCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}

#[test]
fn media_type_prefixes_and_bare_type_words_warn() {
    let html = r#"
        <img src="/a.jpg" alt="Image of the founding team">
        <img src="/b.jpg" alt="a photo of our office">
        <img src="/c.jpg" alt="picture">
    "#;
    let results = RedundantAltTextCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(results[0].description.contains("3 images"));
}

#[test]
fn single_redundant_alt_reads_singular() {
    let html = r#"<img src="/a.jpg" alt="Image of the founding team">"#;
    let results = RedundantAltTextCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Warn);
    assert!(
        results[0].description.contains("1 image has"),
        "singular count must read '1 image has': {}",
        results[0].description
    );
}

#[test]
fn single_empty_heading_and_iframe_read_singular() {
    let results = EmptyHeadingsCheck.run(&ctx("<h1>Title</h1><h2></h2>"));
    assert!(
        results[0].description.contains("1 of 2 headings contains"),
        "{}",
        results[0].description
    );

    let results = IframeTitleCheck.run(&ctx(r#"<iframe src="/embed"></iframe>"#));
    assert!(
        results[0].description.contains("1 of 1 visible iframe has"),
        "{}",
        results[0].description
    );
}

#[test]
fn descriptive_alt_text_is_not_flagged() {
    // "photo booth" starts with a type word but not the redundant phrase;
    // real descriptions and decorative empties are fine.
    let html = r#"
        <img src="/a.jpg" alt="Founding team at the 2026 offsite">
        <img src="/b.jpg" alt="Photo booth rental pricing table">
        <img src="/c.jpg" alt="">
        <img src="/d.jpg" alt="Imagery dashboard">
    "#;
    let results = RedundantAltTextCheck.run(&ctx(html));
    assert_eq!(results[0].status, CheckStatus::Pass);
}
