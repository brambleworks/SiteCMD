//! Detects blocking scripts and stylesheets in `<head>`.

use crate::checks::html_attrs::{attr_value, has_attr, tag_slices, url_attr_value};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

/// Whether a `<script src>` tag in `<head>` blocks the parser. `nomodule`
/// joins async, defer, and `type=module`: a module-supporting browser (every
/// current one) neither fetches nor executes a `nomodule` script, so counting
/// it as render-blocking describes no browser in use.
fn blocks_the_parser(tag: &str) -> bool {
    let is_module =
        attr_value(tag, "type").is_some_and(|value| value.trim().eq_ignore_ascii_case("module"));
    !has_attr(tag, "async") && !has_attr(tag, "defer") && !has_attr(tag, "nomodule") && !is_module
}

/// Whether a `<link>` carries the given rel keyword. `rel` is a space-separated
/// token list, so a substring match on the whole attribute is not enough.
fn has_rel(tag: &str, keyword: &str) -> bool {
    attr_value(tag, "rel").is_some_and(|rel| {
        rel.split_whitespace()
            .any(|token| token.eq_ignore_ascii_case(keyword))
    })
}

pub struct RenderBlockingCheck;

impl Check for RenderBlockingCheck {
    fn id(&self) -> &str {
        "performance.render_blocking"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Extract <head> content
        let head_content = extract_head(&ctx.body);
        let head = match head_content {
            Some(h) => h,
            None => return vec![pass_result()],
        };

        let mut blocking_scripts: Vec<String> = Vec::new();
        let mut blocking_styles: Vec<String> = Vec::new();
        let head_lower = head.to_ascii_lowercase();

        // Scripts in head with a real src and none of async, defer, nomodule,
        // or type=module.
        for tag in tag_slices(head, &head_lower, "script") {
            let Some(src) = url_attr_value(tag, "src") else {
                continue;
            };
            if blocks_the_parser(tag) {
                blocking_scripts.push(truncate(&src, 80));
            }
        }

        // Stylesheets without media (other than all/screen) or preload.
        for tag in tag_slices(head, &head_lower, "link") {
            if !has_rel(tag, "stylesheet") || has_rel(tag, "preload") {
                continue;
            }
            // Only absent, `all`, or `screen` media blocks rendering
            // unconditionally; conditional media should not be counted.
            if let Some(media) = attr_value(tag, "media") {
                let media = media.trim();
                if !(media.is_empty()
                    || media.eq_ignore_ascii_case("all")
                    || media.eq_ignore_ascii_case("screen"))
                {
                    continue;
                }
            }
            // A `data-href` placeholder is not a request: the theme
            // stylesheets GitHub swaps in at runtime were counted as 29
            // blocking sheets against 15 real ones.
            let Some(href) = url_attr_value(tag, "href") else {
                continue;
            };
            blocking_styles.push(truncate(&href, 80));
        }

        let script_count = blocking_scripts.len();
        let style_count = blocking_styles.len();
        let total = script_count + style_count;

        // Sync head scripts are the actionable signal. Head stylesheets
        // are technically render-blocking but universal - every styled
        // Head stylesheets matter only in bulk because presence alone is normal.
        let (status, severity) = match (script_count, style_count) {
            (0, 0..=6) => (CheckStatus::Pass, Severity::Low),
            (0, _) => (CheckStatus::Warn, Severity::Low),
            (1..=2, _) => (CheckStatus::Warn, Severity::Low),
            (3..=5, _) => (CheckStatus::Fail, Severity::Medium),
            _ => (CheckStatus::Fail, Severity::High),
        };
        let is_issue = status != CheckStatus::Pass;

        vec![CheckResult {
            check_id: "performance.render_blocking".into(),
            category: ScanCategory::Performance,
            title: if !is_issue {
                "Render-blocking resources".into()
            } else {
                "Render-blocking resources in page head".into()
            },
            description: if total == 0 {
                "No render-blocking resources detected in <head>. Scripts use async/defer and stylesheets use appropriate media attributes.".into()
            } else if !is_issue {
                format!(
                    "No render-blocking scripts in <head>. {} stylesheet{} load{} before first paint, which is normal for styled pages.",
                    style_count,
                    if style_count == 1 { "" } else { "s" },
                    if style_count == 1 { "s" } else { "" },
                )
            } else {
                format!(
                    "{} render-blocking resource{} in <head> ({} script{}, {} stylesheet{}). These delay the first paint of your page.",
                    total, if total == 1 { "" } else { "s" },
                    script_count, if script_count == 1 { "" } else { "s" },
                    style_count, if style_count == 1 { "" } else { "s" },
                )
            },
            status,
            severity,
            fix_prompt: None,
            manual_fix: if is_issue {
                Some(
                    // Avoid the brittle media-print onload swap.
                    "Move each blocking resource into one of these buckets:\n\
                     • Scripts that don't need to run before paint: add `defer` (preserves order) or `async` (order doesn't matter)\n\
                     • Scripts that aren't needed on this page at all: remove the `<script>` or load it from a user gesture\n\
                     • CSS that's needed for the above-the-fold render: inline it in `<head>` and ship the rest as a normal `<link rel=\"stylesheet\">`\n\
                     • CSS that's not needed for the initial route: code-split it into a per-route bundle your framework loads on demand\n\
                     Skip the `media=\"print\" + onload swap` trick - it's brittle and Lighthouse no longer recommends it."
                        .into(),
                )
            } else {
                None
            },
            raw_data: if total > 0 {
                Some(serde_json::json!({
                    "blocking_scripts": blocking_scripts,
                    "blocking_stylesheets": blocking_styles,
                    "total": total,
                }))
            } else {
                None
            },
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if is_issue {
                Some("Render-blocking resources delay first paint, increasing bounce rate.".into())
            } else {
                None
            },
        }]
    }
}

fn extract_head(html: &str) -> Option<&str> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<head")?;
    let end = lower.find("</head>")?;
    if end > start {
        Some(&html[start..end])
    } else {
        None
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        let end = crate::checks::floor_char_boundary(s, max.min(s.len()));
        format!("{}…", &s[..end])
    } else {
        s.to_string()
    }
}

fn pass_result() -> CheckResult {
    CheckResult {
        check_id: "performance.render_blocking".into(),
        category: ScanCategory::Performance,
        title: "Render-blocking resources".into(),
        description: "Could not detect <head> section - skipped.".into(),
        status: CheckStatus::Pass,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, PageContext};

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

    #[test]
    fn a_few_head_stylesheets_are_not_a_fail() {
        let html = r#"<html><head>
            <link rel="stylesheet" href="/base.css">
            <link rel="stylesheet" href="/theme.css">
            <link rel="stylesheet" href="/components.css">
        </head><body></body></html>"#;
        let results = RenderBlockingCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn async_in_src_url_does_not_make_a_script_non_blocking() {
        let html =
            r#"<html><head><script src="/js/async-utils.js"></script></head><body></body></html>"#;
        let results = RenderBlockingCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("1 script"));
    }

    #[test]
    fn conditional_media_stylesheets_are_not_render_blocking() {
        let stylesheets: String = (0..8)
            .map(|i| {
                format!(r#"<link rel="stylesheet" href="/m{i}.css" media="(max-width: 600px)">"#)
            })
            .collect();
        let html = format!(
            r#"<html><head>{stylesheets}<link rel="stylesheet" href="/p.css" media="print"></head><body></body></html>"#
        );
        let results = RenderBlockingCheck.run(&ctx(&html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn screen_and_all_media_stylesheets_still_count_as_blocking() {
        let stylesheets: String = (0..8)
            .map(|i| format!(r#"<link rel="stylesheet" href="/s{i}.css" media="screen">"#))
            .collect();
        let html = format!("<html><head>{stylesheets}</head><body></body></html>");
        let results = RenderBlockingCheck.run(&ctx(&html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn deferred_and_module_scripts_pass() {
        let html = r#"<html><head>
            <script src="/app.js" defer></script>
            <script async src="/analytics.js"></script>
            <script type="module" src="/main.mjs"></script>
        </head><body></body></html>"#;
        let results = RenderBlockingCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn a_data_href_placeholder_is_not_a_render_blocking_stylesheet() {
        // github.com ships 14 runtime theme stylesheets as `data-href`
        // placeholders alongside its real `href` sheets.
        let placeholders: String = (0..14)
            .map(|i| {
                format!(
                    r#"<link data-color-theme="t{i}" media="all" rel="stylesheet" data-href="/assets/theme-{i}.css" />"#
                )
            })
            .collect();
        let html = format!(
            r#"<html><head><link rel="stylesheet" href="/real.css">{placeholders}</head><body></body></html>"#
        );
        let results = RenderBlockingCheck.run(&ctx(&html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
        assert!(
            results[0].description.contains("1 stylesheet"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn a_nomodule_script_is_not_render_blocking() {
        let html = r#"<html><head><script src="/_next/static/chunks/legacy.js" noModule=""></script></head><body></body></html>"#;
        let results = RenderBlockingCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn a_stylesheet_rel_with_several_tokens_still_counts() {
        let stylesheets: String = (0..8)
            .map(|i| format!(r#"<link rel="stylesheet alternate-ignored" href="/s{i}.css">"#))
            .collect();
        let html = format!("<html><head>{stylesheets}</head><body></body></html>");
        let results = RenderBlockingCheck.run(&ctx(&html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn a_link_inside_an_inline_script_string_is_not_a_stylesheet() {
        let html = r#"<html><head>
            <script>document.write('<link rel="stylesheet" href="/from-script.css">');</script>
            <link rel="stylesheet" href="/real.css">
        </head><body></body></html>"#;
        let results = RenderBlockingCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
        assert!(
            results[0].description.contains("1 stylesheet"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn an_entity_encoded_script_src_is_reported_as_the_browser_requests_it() {
        let html = r#"<html><head><script src="/js/app.js?v=1&amp;b=2"></script></head><body></body></html>"#;
        let results = RenderBlockingCheck.run(&ctx(html));
        let raw = results[0].raw_data.as_ref().expect("raw data");
        assert_eq!(raw["blocking_scripts"][0], "/js/app.js?v=1&b=2");
    }

    #[test]
    fn many_sync_scripts_still_fail_high() {
        let scripts: String = (0..6)
            .map(|i| format!(r#"<script src="/v{i}.js"></script>"#))
            .collect();
        let html = format!("<html><head>{scripts}</head><body></body></html>");
        let results = RenderBlockingCheck.run(&ctx(&html));
        assert_eq!(results[0].status, CheckStatus::Fail);
        assert_eq!(results[0].severity, crate::checks::Severity::High);
    }
}
