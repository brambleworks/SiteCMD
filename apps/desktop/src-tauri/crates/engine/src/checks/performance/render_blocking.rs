//! Detects blocking scripts and stylesheets in `<head>`.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

static RB_SCRIPT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)<script\s[^>]*src\s*=\s*["']([^"']+)["'][^>]*>"#).unwrap());
/// async/defer as attributes (whitespace-preceded). Matched against the
/// tag with the src value blanked out: `\b` alone matched "async" inside
/// URLs like /js/async-utils.js, hiding genuinely blocking scripts
///.
static ASYNC_DEFER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\s(async|defer)[\s/>=]|type\s*=\s*["']?module\b"#).unwrap());
static RB_LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)<link\s[^>]*>"#).unwrap());
static RB_STYLESHEET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)rel\s*=\s*["']stylesheet["']"#).unwrap());
static RB_HREF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)href\s*=\s*["']([^"']+)["']"#).unwrap());
static MEDIA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)media\s*=\s*["']([^"']+)["']"#).unwrap());
static PRELOAD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)rel\s*=\s*["']preload["']"#).unwrap());

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

        // Find scripts in head without async/defer/type=module
        for cap in RB_SCRIPT_RE.captures_iter(head) {
            let full_tag = cap.get(0).map(|m: regex::Match| m.as_str()).unwrap_or("");
            let src = &cap[1];

            // Blank the src value so URL text can't satisfy the
            // attribute match.
            let attrs_only = full_tag.replace(src, " ");
            if !ASYNC_DEFER_RE.is_match(&attrs_only) {
                blocking_scripts.push(truncate(src, 80));
            }
        }

        // Find stylesheets without media (other than all/screen) or preload
        for tag in RB_LINK_RE.find_iter(head) {
            let tag_str: &str = tag.as_str();
            if RB_STYLESHEET_RE.is_match(tag_str) && !PRELOAD_RE.is_match(tag_str) {
                // Only absent, `all`, or `screen` media blocks rendering
                // unconditionally; conditional media should not be counted.
                if let Some(media_cap) = MEDIA_RE.captures(tag_str) {
                    let media = media_cap[1].trim();
                    if !(media.is_empty()
                        || media.eq_ignore_ascii_case("all")
                        || media.eq_ignore_ascii_case("screen"))
                    {
                        continue;
                    }
                }
                if let Some(href_cap) = RB_HREF_RE.captures(tag_str) {
                    blocking_styles.push(truncate(&href_cap[1], 80));
                }
            }
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
