//! Detects HTTP subresources in HTTPS page markup and inline CSS.

use crate::checks::html_attrs::{
    all_tag_slices, attr_value, decode_url_character_references, raw_text_element_contents,
};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

pub struct MixedContentCheck;

#[derive(Debug)]
struct MixedResource {
    dedupe_key: String,
    url: String,
    kind: String,
    active: bool,
    local_endpoint: bool,
}

static CSS_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)url\(\s*["']?(http://[^\s"'()]+)"#).expect("static CSS URL regex")
});

/// CSS block comments, including an unclosed one running to end of input (a
/// truncated <style> body can end mid-comment). A url/@import inside a
/// comment is never fetched by the browser.
static CSS_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)/\*.*?(?:\*/|\z)").expect("static CSS comment regex"));

static CSS_IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)@import\s+(?:url\(\s*)?["']?(http://[^\s"'();]+)"#)
        .expect("static CSS import regex")
});

fn source_tag_name(tag: &str) -> String {
    tag.trim_start_matches('<')
        .split(|character: char| character.is_ascii_whitespace() || matches!(character, '/' | '>'))
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn is_local_endpoint(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(host)) => {
            host.eq_ignore_ascii_case("localhost")
                || host.to_ascii_lowercase().ends_with(".localhost")
        }
        Some(url::Host::Ipv4(address)) => address.is_loopback() || address.is_unspecified(),
        Some(url::Host::Ipv6(address)) => address.is_loopback() || address.is_unspecified(),
        None => false,
    }
}

fn add_http_resource(resources: &mut Vec<MixedResource>, raw_url: &str, kind: &str, active: bool) {
    let decoded = decode_url_character_references(raw_url);
    let Ok(parsed) = url::Url::parse(decoded.trim()) else {
        return;
    };
    if parsed.scheme() != "http" {
        return;
    }

    let dedupe_key = parsed.as_str().to_string();
    let local_endpoint = is_local_endpoint(&parsed);
    if let Some(existing) = resources
        .iter_mut()
        .find(|resource| resource.dedupe_key == dedupe_key)
    {
        existing.active |= active;
        existing.local_endpoint |= local_endpoint;
        if !existing.kind.split(", ").any(|value| value == kind) {
            existing.kind.push_str(", ");
            existing.kind.push_str(kind);
        }
        return;
    }

    resources.push(MixedResource {
        dedupe_key,
        url: crate::log_sanitizer::evidence_safe_page_url(parsed.as_str()),
        kind: kind.to_string(),
        active,
        local_endpoint,
    });
}

fn add_css_resources(resources: &mut Vec<MixedResource>, css: &str, kind: &str) {
    // A url/@import inside a CSS comment is not fetched by the browser, so it
    // must not raise a mixed-content finding.
    let css = CSS_COMMENT_RE.replace_all(css, " ");
    for capture in CSS_IMPORT_RE.captures_iter(&css) {
        add_http_resource(resources, &capture[1], "CSS @import", true);
    }
    for capture in CSS_URL_RE.captures_iter(&css) {
        add_http_resource(resources, &capture[1], kind, false);
    }
}

fn srcset_urls(value: &str) -> Vec<&str> {
    // HTTP srcset URLs end at whitespace; data URLs may contain ignored commas.
    value
        .split(',')
        .filter_map(|candidate| candidate.split_whitespace().next())
        .filter(|candidate| !candidate.is_empty())
        .collect()
}

fn collect_http_subresources(body: &str, lower: &str) -> Vec<MixedResource> {
    let mut resources = Vec::new();

    for tag in all_tag_slices(body, lower) {
        let name = source_tag_name(tag);
        let source_kind = match name.as_str() {
            "script" => Some(("script src", true)),
            "iframe" | "frame" => Some(("frame src", true)),
            "embed" => Some(("embedded content", true)),
            "img" => Some(("image src", false)),
            "audio" | "video" | "source" | "track" => Some(("media src", false)),
            "input"
                if attr_value(tag, "type")
                    .is_some_and(|value| value.eq_ignore_ascii_case("image")) =>
            {
                Some(("image input src", false))
            }
            _ => None,
        };
        if let Some((kind, active)) = source_kind {
            if let Some(src) = attr_value(tag, "src") {
                add_http_resource(&mut resources, &src, kind, active);
            }
        }

        if matches!(name.as_str(), "img" | "source") {
            if let Some(srcset) = attr_value(tag, "srcset") {
                for candidate in srcset_urls(&srcset) {
                    add_http_resource(&mut resources, candidate, "responsive image", false);
                }
            }
        }
        if name == "video" {
            if let Some(poster) = attr_value(tag, "poster") {
                add_http_resource(&mut resources, &poster, "video poster", false);
            }
        }
        if name == "object" {
            if let Some(data) = attr_value(tag, "data") {
                add_http_resource(&mut resources, &data, "object data", true);
            }
        }
        if matches!(name.as_str(), "image" | "use") {
            if let Some(href) = attr_value(tag, "href").or_else(|| attr_value(tag, "xlink:href")) {
                add_http_resource(&mut resources, &href, "SVG resource", false);
            }
        }
        if name == "link" {
            let rel = attr_value(tag, "rel").unwrap_or_default();
            let rel_tokens: Vec<String> = rel
                .split_ascii_whitespace()
                .map(str::to_ascii_lowercase)
                .collect();
            let resource_rel = rel_tokens.iter().any(|token| {
                matches!(
                    token.as_str(),
                    "stylesheet"
                        | "preload"
                        | "modulepreload"
                        | "prefetch"
                        | "icon"
                        | "manifest"
                        | "apple-touch-icon"
                )
            });
            if resource_rel {
                let as_value = attr_value(tag, "as")
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let active = rel_tokens
                    .iter()
                    .any(|token| matches!(token.as_str(), "stylesheet" | "modulepreload"))
                    || (rel_tokens.iter().any(|token| token == "preload")
                        && matches!(as_value.as_str(), "script" | "style" | "worker"));
                if let Some(href) = attr_value(tag, "href") {
                    add_http_resource(&mut resources, &href, "link resource", active);
                }
            }
        }
        if let Some(style) = attr_value(tag, "style") {
            add_css_resources(&mut resources, &style, "inline style URL");
        }
    }

    for css in raw_text_element_contents(body, lower, "style") {
        add_css_resources(&mut resources, css, "style block URL");
    }

    resources.sort_unstable_by(|left, right| left.url.cmp(&right.url));
    resources
}

impl Check for MixedContentCheck {
    fn id(&self) -> &str {
        "security.mixed_content"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let is_https = ctx.url.scheme() == "https";

        // Only relevant for HTTPS pages
        if !is_https && !ctx.is_localhost {
            return vec![CheckResult {
                check_id: "security.mixed_content".into(),
                category: ScanCategory::Security,
                title: "Mixed content".into(),
                description:
                    "Site is not served over HTTPS. Mixed content check is not applicable.".into(),
                status: CheckStatus::Skipped,
                severity: Severity::High,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        // Local scans still need resource-reference visibility, but a local
        // source observation cannot establish what the production build or
        // deployed HTTPS page contains.
        let resources = collect_http_subresources(&ctx.body, ctx.body_lower());
        let count = resources.len();
        let active_count = resources.iter().filter(|resource| resource.active).count();
        let local_endpoint_count = resources
            .iter()
            .filter(|resource| resource.local_endpoint)
            .count();
        let local_endpoint_note = if local_endpoint_count == 0 {
            String::new()
        } else {
            format!(
                "; {} target{} a localhost, loopback, or unspecified local endpoint. A visitor's local endpoint is not the deployment server",
                local_endpoint_count,
                if local_endpoint_count == 1 { "s" } else { "" }
            )
        };
        let selected = resources
            .iter()
            .take(5)
            .map(|resource| format!("{} ({})", resource.url, resource.kind))
            .collect::<Vec<_>>()
            .join(", ");

        vec![CheckResult {
            check_id: "security.mixed_content".into(),
            category: ScanCategory::Security,
            title: if count == 0 {
                "Mixed content".into()
            } else if is_https {
                "HTTP subresource references on HTTPS page".into()
            } else {
                "HTTP subresource references in local preview".into()
            },
            description: if count == 0 {
                if is_https {
                    "No plain-HTTP URL was found in the initial HTML's recognized resource-bearing attributes or inline CSS. This source check did not inspect the rendered DOM, fetched stylesheets, service-worker rewrites, or runtime-created requests.".into()
                } else {
                    "No plain-HTTP URL was found in the local preview's recognized resource-bearing attributes or inline CSS. The production build and rendered DOM were not inspected.".into()
                }
            } else if is_https {
                format!(
                    "Found {} distinct plain-HTTP subresource reference{} in the initial HTML of this HTTPS page; {} {} script, stylesheet, frame, object, or equivalent active-content reference{}{}. Browsers block active mixed content and may block or upgrade other resource types depending on type and policy. The source check did not prove which responsive/media candidates are selected at runtime. References: {}",
                    count,
                    if count == 1 { "" } else { "s" },
                    active_count,
                    if active_count == 1 { "is a" } else { "are" },
                    if active_count == 1 { "" } else { "s" },
                    local_endpoint_note,
                    selected,
                )
            } else {
                format!(
                    "Found {} distinct plain-HTTP subresource reference{} in the local preview: {}. This does not establish a deployed mixed-content defect because the production build, deployed scheme, and rendered resource selection were not inspected.",
                    count,
                    if count == 1 { "" } else { "s" },
                    selected,
                )
            },
            status: if count == 0 {
                CheckStatus::Pass
            } else if is_https {
                CheckStatus::Fail
            } else {
                CheckStatus::Warn
            },
            severity: if is_https && active_count > 0 {
                Severity::High
            } else {
                Severity::Medium
            },
            fix_prompt: None,
            manual_fix: if count == 0 {
                None
            } else {
                Some("Inspect each surfaced declaration in the deployed rendered page and browser network panel. For a resource that is actually selected, use a working HTTPS endpoint or a same-origin relative URL; replace localhost, loopback, or unspecified-host references with the intended deployed endpoint. Migrate or self-host only when licensing, update ownership, caching, and integrity controls remain correct. Do not change the scheme blindly if the destination does not support HTTPS, and re-test responsive/media variants plus fetched CSS.".into())
            },
            raw_data: if count > 0 {
                Some(serde_json::json!({
                    "resource_count": count,
                    "active_content_count": active_count,
                    "initial_html_only": true,
                    "resources": resources.iter().map(|resource| serde_json::json!({
                        "url": resource.url,
                        "kind": resource.kind,
                        "active_content": resource.active,
                        "local_endpoint": resource.local_endpoint,
                    })).collect::<Vec<_>>(),
                }))
            } else {
                None
            },
            confidence: if count > 0 && (!is_https || active_count == 0) {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: (count > 0 && (!is_https || active_count == 0)).then(|| {
                if is_https {
                    "The plain-HTTP references are present in initial HTML, but only responsive, media, image, icon, or CSS-resource candidates were observed; browser selection, upgrades, blocking, and rendered impact were not reproduced.".into()
                } else {
                    "The plain-HTTP references are present in a local preview, but production build substitution, deployed scheme, runtime selection, and rendered impact were not evaluated.".into()
                }
            }),
            why_it_matters: if count == 0 {
                None
            } else {
                Some("On an HTTPS page, active resources fetched over HTTP are blocked and other mixed resources may be blocked or upgraded, which can break behavior or present content from an unauthenticated network response. The observed source reference does not establish that every candidate is fetched.".into())
            },
        }]
    }
}

#[cfg(test)]
mod tests {
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
    fn test_mixed_content_clean_page_pass() {
        let check = MixedContentCheck;
        let html = r#"<html><body><img src="https://cdn.example.com/img.png"><script src="/app.js"></script></body></html>"#;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn test_mixed_content_http_img_fail() {
        let check = MixedContentCheck;
        let html = r#"<html><body><img src="http://evil.com/img.png"></body></html>"#;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert!(results[0].description.contains("http://evil.com/img.png"));
    }

    #[test]
    fn anchor_link_to_http_is_not_mixed_content() {
        let check = MixedContentCheck;
        let html = r#"<html><body><p>See <a href="http://www.ietf.org/rfc/rfc7208.txt">RFC 7208</a>.</p></body></html>"#;
        let results = check.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "anchor href over http is a navigation, not mixed content"
        );
    }

    #[test]
    fn stylesheet_link_over_http_is_mixed_content() {
        // A `<link>` href DOES load a subresource, so http there is real mixed
        // content and must still fail.
        let check = MixedContentCheck;
        let html = r#"<html><head><link rel="stylesheet" href="http://cdn.example.com/app.css"></head></html>"#;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert!(results[0]
            .description
            .contains("http://cdn.example.com/app.css"));
    }

    #[test]
    fn inert_markup_examples_and_navigation_links_are_not_mixed_content() {
        let html = r#"<!-- <img src="http://comment.example/x.png"> -->
            <script>const example = '<img src="http://docs.example/x.png">';</script>
            <a href="http://destination.example/article">Read it</a>
            <link rel="canonical" href="http://example.com/page">"#;
        let result = &MixedContentCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Pass, "{}", result.description);
    }

    #[test]
    fn unquoted_and_srcset_http_resources_are_detected() {
        let html = r#"<script src=http://cdn.example/app.js></script>
            <img srcset="https://cdn.example/a.png 1x, http://cdn.example/a@2x.png 2x">"#;
        let result = &MixedContentCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Fail, "{}", result.description);
        let raw = result.raw_data.as_ref().unwrap().to_string();
        assert!(raw.contains("app.js"), "{raw}");
        assert!(raw.contains("a@2x.png"), "{raw}");
    }

    #[test]
    fn mixed_content_evidence_keeps_path_and_removes_query_secrets() {
        let html =
            r#"<script src="http://cdn.example/assets/app.js?token=secret#fragment"></script>"#;
        let result = &MixedContentCheck.run(&ctx(html))[0];
        let serialized = serde_json::to_string(result).unwrap();
        assert!(
            serialized.contains("http://cdn.example/assets/app.js"),
            "{serialized}"
        );
        assert!(!serialized.contains("token=secret"), "{serialized}");
        assert!(!serialized.contains("fragment"), "{serialized}");
    }

    #[test]
    fn https_page_loopback_subresource_is_not_silently_discarded() {
        let html = r#"<script src="http://localhost:5173/private/app.js?token=secret"></script>"#;
        let result = &MixedContentCheck.run(&ctx(html))[0];

        assert_eq!(result.status, CheckStatus::Fail, "{}", result.description);
        assert_eq!(result.severity, Severity::High);
        let evidence = result
            .raw_data
            .as_ref()
            .expect("loopback evidence")
            .to_string();
        assert!(
            evidence.contains("http://localhost:5173/private/app.js"),
            "{evidence}"
        );
        assert!(!evidence.contains("token=secret"), "{evidence}");
    }

    #[test]
    fn inline_style_content_is_inspected_without_scanning_script_text_as_css() {
        let html = r#"<script>const css = 'url(http://docs.example/fake.png)'</script>
            <style>.hero { background: url(http://cdn.example/hero.png) }</style>"#;
        let result = &MixedContentCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Fail);
        let raw = result.raw_data.as_ref().unwrap().to_string();
        assert!(raw.contains("cdn.example/hero.png"), "{raw}");
        assert!(!raw.contains("docs.example"), "{raw}");
    }

    #[test]
    fn commented_out_css_url_is_not_mixed_content() {
        let html = r#"<style>
            /*.old { background: url(http://legacy.example/bg.png) } */
            /* @import url(http://legacy.example/old.css); */
            .hero { background: url(https://cdn.example/hero.png) }
        </style>"#;
        let result = &MixedContentCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Pass, "{}", result.description);
        assert!(!result.description.contains("legacy.example"));
    }

    #[test]
    fn localhost_preview_uses_preview_wording_not_https_claims() {
        let check = MixedContentCheck;
        let ctx = PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("http://localhost:3000").unwrap(),
            response_headers: HeaderMap::new(),
            status_code: 200,
            body: r#"<img src="http://cdn.example.com/img.png">"#.to_string(),
            is_localhost: true,
            is_strict_localhost: true,
            http_version: Some("HTTP/1.1".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        };
        let results = check.run(&ctx);
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(
            !results[0].description.contains("HTTPS page"),
            "{}",
            results[0].description
        );
        assert!(
            results[0].description.contains("local preview"),
            "{}",
            results[0].description
        );
        assert!(results[0].title.contains("local preview"));
    }

    #[test]
    fn test_mixed_content_http_page_skipped() {
        let check = MixedContentCheck;
        let ctx = PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("http://example.com").unwrap(),
            response_headers: HeaderMap::new(),
            status_code: 200,
            body: r#"<img src="http://other.com/x.png">"#.to_string(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        };
        let results = check.run(&ctx);
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }
}
