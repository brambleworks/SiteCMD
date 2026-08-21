//! Favicon plan and verdict tests.

#![cfg(test)]

use super::{favicon_href, favicon_probe_verdict, FaviconProbeVerdict};

#[test]
fn extracts_icon_href_in_both_rel_forms() {
    assert_eq!(
        favicon_href(r#"<link rel="icon" href="/favicon.svg">"#).as_deref(),
        Some("/favicon.svg")
    );
    assert_eq!(
        favicon_href(r#"<link rel='shortcut icon' href='/favicon.ico'>"#).as_deref(),
        Some("/favicon.ico")
    );
}

#[test]
fn extracts_href_when_attribute_order_reversed() {
    assert_eq!(
        favicon_href(r#"<link href="/icon.png" rel="icon" type="image/png">"#).as_deref(),
        Some("/icon.png")
    );
}

#[test]
fn manifest_link_does_not_match() {
    assert!(favicon_href(r#"<link rel="manifest" href="/site.webmanifest">"#).is_none());
}

#[test]
fn extracts_unquoted_icon_href_from_minified_markup() {
    assert_eq!(
        favicon_href("<link rel=icon href=/favicon.ico>").as_deref(),
        Some("/favicon.ico")
    );
}

#[test]
fn apple_touch_icon_is_a_different_token_and_does_not_match() {
    assert!(favicon_href(r#"<link rel="apple-touch-icon" href="/apple.png">"#).is_none());
}

#[test]
fn favicon_probe_requires_direct_missing_or_image_response_evidence() {
    assert_eq!(
        favicon_probe_verdict(200, Some("image/svg+xml")),
        FaviconProbeVerdict::UsableResponse
    );
    assert_eq!(
        favicon_probe_verdict(204, Some("image/png")),
        FaviconProbeVerdict::Review
    );
    assert_eq!(
        favicon_probe_verdict(200, Some("text/html")),
        FaviconProbeVerdict::Review
    );
    assert_eq!(
        favicon_probe_verdict(404, Some("text/html")),
        FaviconProbeVerdict::Missing
    );
    assert_eq!(
        favicon_probe_verdict(403, None),
        FaviconProbeVerdict::Review
    );
}
