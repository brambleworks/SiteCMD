//! Page-source hints for print styles, responsive markup, and trailing slashes.
//! Sitemap directives remain an async transport check.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::sync::LazyLock;

static STYLE_BLOCK_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?is)<style(?:\s[^<>]*?)?>(.*?)</style\s*>")
        .expect("valid inline-style block regex")
});

static SCRIPT_OR_HTML_COMMENT_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?is)<!--.*?-->|<script(?:\s[^<>]*?)?>.*?</script\s*>")
        .expect("valid script/comment regex")
});

static CSS_COMMENT_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)/\*.*?\*/").expect("valid CSS-comment regex"));

static MEDIA_RULE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)@media\s+([^{}]+)\{").expect("valid media-rule regex")
});

static WIDTH_MEDIA_FEATURE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(?:min-|max-)?(?:device-)?(?:width|inline-size)\s*[:<>=]")
        .expect("valid width media-feature regex")
});

static FLEXIBLE_LAYOUT_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)\bdisplay\s*:\s*(?:flex|grid)\b").expect("valid flexible-layout regex")
});

fn markup_without_scripts_or_html_comments(body_lower: &str) -> String {
    SCRIPT_OR_HTML_COMMENT_RE
        .replace_all(body_lower, " ")
        .into_owned()
}

fn sanitized_inline_css(scannable_markup: &str) -> String {
    let mut css = String::new();
    for captures in STYLE_BLOCK_RE.captures_iter(scannable_markup) {
        css.push_str(&captures[1]);
        css.push('\n');
    }
    CSS_COMMENT_RE.replace_all(&css, " ").into_owned()
}

fn media_targets_print(value: &str) -> bool {
    value.split(',').any(|query| {
        let query = query.trim().to_ascii_lowercase();
        query == "print"
            || query.starts_with("print ")
            || query.starts_with("print(")
            || query == "only print"
            || query.starts_with("only print ")
    })
}

/// Check for print stylesheet
pub struct PrintStylesheetCheck;

impl Check for PrintStylesheetCheck {
    fn id(&self) -> &str {
        "config.print_stylesheet"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Polish
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let scannable = markup_without_scripts_or_html_comments(lower);
        let stylesheet_links: Vec<_> =
            crate::checks::html_attrs::tag_slices(&scannable, &scannable, "link")
                .into_iter()
                .filter(|tag| {
                    crate::checks::html_attrs::attr_value(tag, "rel").is_some_and(|rel| {
                        rel.split_ascii_whitespace()
                            .any(|token| token.eq_ignore_ascii_case("stylesheet"))
                    })
                })
                .collect();
        let has_print_link = stylesheet_links.iter().any(|tag| {
            crate::checks::html_attrs::attr_value(tag, "media")
                .is_some_and(|media| media_targets_print(&media))
        });
        let style_tags = crate::checks::html_attrs::tag_slices(&scannable, &scannable, "style");
        let has_print_style_media = style_tags.iter().any(|tag| {
            crate::checks::html_attrs::attr_value(tag, "media")
                .is_some_and(|media| media_targets_print(&media))
        });
        let inline_css = sanitized_inline_css(&scannable);
        let has_inline_print_query = MEDIA_RULE_RE
            .captures_iter(&inline_css)
            .any(|captures| media_targets_print(&captures[1]));
        let has_print = has_print_link || has_print_style_media || has_inline_print_query;
        let has_external_stylesheet = !stylesheet_links.is_empty();

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if has_print {
                "Print-specific source hint observed".into()
            } else {
                "Print behavior not evaluated".into()
            },
            description: if has_print {
                "A print-targeted stylesheet/media attribute or inline @media print rule was observed. This source check does not evaluate print preview, inherited screen styles, content visibility, readability, pagination, paper sizes, or PDF output.".into()
            } else if has_external_stylesheet {
                "No print-specific marker was observed in the initial HTML or inline style blocks. Linked external stylesheets are not fetched, and a page may print acceptably without dedicated rules, so no print defect is inferred.".into()
            } else {
                "No print-specific marker was observed in the initial HTML or inline style blocks. Dedicated print CSS is optional, the product's print/PDF use case is unknown, and ordinary styles may already print acceptably; no defect is inferred.".into()
            },
            status: if has_print {
                CheckStatus::Pass
            } else {
                CheckStatus::Skipped
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "print_stylesheet_link": has_print_link,
                "print_style_media_attribute": has_print_style_media,
                "inline_print_media_query": has_inline_print_query,
                "external_stylesheet_present": has_external_stylesheet,
                "print_output_verified": false,
            })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("Only page HTML and inline style blocks were inspected. External CSS, browser print rendering, PDF output, and whether users need printing were not evaluated.".into()),
            why_it_matters: None,
        }]
    }
}

/// Report source-level responsive hints without claiming rendered behavior.
pub struct ResponsiveDesignCheck;

impl Check for ResponsiveDesignCheck {
    fn id(&self) -> &str {
        "config.responsive_design"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let scannable = markup_without_scripts_or_html_comments(lower);
        let viewport_content =
            crate::checks::html_attrs::tag_slices(&scannable, &scannable, "meta")
                .into_iter()
                .find_map(|tag| {
                    crate::checks::html_attrs::attr_value(tag, "name")
                        .filter(|value| value.eq_ignore_ascii_case("viewport"))
                        .map(|_| {
                            crate::checks::html_attrs::attr_value(tag, "content")
                                .unwrap_or_default()
                        })
                });
        let has_device_width = viewport_content.as_deref().is_some_and(|content| {
            content
                .split(',')
                .filter_map(|directive| directive.trim().split_once('='))
                .any(|(name, value)| {
                    name.trim().eq_ignore_ascii_case("width")
                        && value.trim().eq_ignore_ascii_case("device-width")
                })
        });

        let inline_css = sanitized_inline_css(&scannable);
        let has_width_media_query = MEDIA_RULE_RE
            .captures_iter(&inline_css)
            .any(|captures| WIDTH_MEDIA_FEATURE_RE.is_match(&captures[1]));
        let has_container_query = inline_css.contains("@container");
        let has_flexible_layout_rule = FLEXIBLE_LAYOUT_RE.is_match(&inline_css);

        let has_responsive_image_markup =
            !crate::checks::html_attrs::tag_slices(&scannable, &scannable, "picture").is_empty()
                || crate::checks::html_attrs::tag_slices(&scannable, &scannable, "img")
                    .into_iter()
                    .any(|tag| crate::checks::html_attrs::has_attr(tag, "srcset"));
        let has_external_stylesheet =
            crate::checks::html_attrs::tag_slices(&scannable, &scannable, "link")
                .into_iter()
                .any(|tag| {
                    crate::checks::html_attrs::attr_value(tag, "rel").is_some_and(|rel| {
                        rel.split_ascii_whitespace()
                            .any(|token| token.eq_ignore_ascii_case("stylesheet"))
                    })
                });

        let hint_count = [
            has_width_media_query,
            has_container_query,
            has_flexible_layout_rule,
            has_responsive_image_markup,
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        let has_bounded_source_evidence = has_device_width && hint_count > 0;

        let description = if has_bounded_source_evidence {
            format!(
                "The initial HTML contains a width=device-width viewport plus {} responsive source hint{}. This does not verify the rendered layout, breakpoint behavior, text reflow, touch targets, zoom behavior, or CSS loaded at runtime.",
                hint_count,
                if hint_count == 1 { "" } else { "s" }
            )
        } else if has_external_stylesheet {
            "The initial HTML does not contain enough inline evidence to assess responsive layout, and external stylesheets are not fetched by this check. No conclusion about the rendered layout was made.".into()
        } else {
            "The initial HTML does not contain enough source evidence to assess responsive layout, and this check does not render or resize the page. This is not evidence that the layout is nonresponsive.".into()
        };

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if has_bounded_source_evidence {
                "Responsive source hints".into()
            } else {
                "Responsive layout not evaluated".into()
            },
            description,
            status: if has_bounded_source_evidence {
                CheckStatus::Pass
            } else {
                CheckStatus::Skipped
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "viewport_content": viewport_content,
                "has_device_width": has_device_width,
                "inline_width_media_query": has_width_media_query,
                "inline_container_query": has_container_query,
                "inline_flexible_layout_rule": has_flexible_layout_rule,
                "responsive_image_markup": has_responsive_image_markup,
                "external_stylesheet_present": has_external_stylesheet,
                "rendered_layout_verified": false,
            })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("This check inspects only initial HTML and inline style blocks. It does not fetch external CSS, execute runtime styling, resize a browser, or evaluate the rendered result.".into()),
            why_it_matters: None,
        }]
    }
}

/// Check trailing slash consistency in internal links
pub struct TrailingSlashCheck;

impl Check for TrailingSlashCheck {
    fn id(&self) -> &str {
        "config.trailing_slash"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }
    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let scannable_lower = scannable.to_ascii_lowercase();
        let page_host = ctx.url.host_str().unwrap_or("");
        let document_base =
            crate::checks::html_attrs::tag_slices(&scannable, &scannable_lower, "base")
                .into_iter()
                .find_map(|tag| crate::checks::html_attrs::attr_value(tag, "href"))
                .and_then(|href| ctx.url.join(&href).ok());
        let resolution_base = document_base.as_ref().unwrap_or(&ctx.url);
        let mut variants: std::collections::BTreeMap<String, (bool, bool)> =
            std::collections::BTreeMap::new();

        for tag in crate::checks::html_attrs::tag_slices(&scannable, &scannable_lower, "a") {
            let Some(href) = crate::checks::html_attrs::attr_value(tag, "href") else {
                continue;
            };
            let Ok(url) = resolution_base.join(&href) else {
                continue;
            };
            if !matches!(url.scheme(), "http" | "https")
                || !url
                    .host_str()
                    .is_some_and(|host| host.eq_ignore_ascii_case(page_host))
            {
                continue;
            }
            let path = url.path();
            if path == "/" || path.is_empty() {
                continue;
            }
            let last_segment = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
            if last_segment.contains('.') {
                // File-like targets do not participate in directory-style
                // slash conventions; server behavior is unknown either way.
                continue;
            }
            let with_slash = path.ends_with('/');
            let base = path.trim_end_matches('/').to_string();
            let entry = variants.entry(base).or_insert((false, false));
            if with_slash {
                entry.0 = true;
            } else {
                entry.1 = true;
            }
        }

        let with_slash = variants.values().filter(|(with, _)| *with).count();
        let without_slash = variants.values().filter(|(_, without)| *without).count();
        let conflict_count = variants
            .values()
            .filter(|(with, without)| *with && *without)
            .count();
        let has_conflict = conflict_count > 0;

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if has_conflict {
                "Same internal paths use both slash variants".into()
            } else {
                "No conflicting trailing-slash link variants observed".into()
            },
            description: if has_conflict {
                format!(
                    "SiteCMD observed {} route-like internal base path{} linked both with and without a trailing slash. This is direct link evidence, but it does not show whether the server treats the variants as distinct, redirects one, or emits a consistent canonical URL.",
                    conflict_count,
                    if conflict_count == 1 { "" } else { "s" }
                )
            } else if variants.is_empty() {
                "No route-like same-host links were available for this source check after excluding the root and file-like targets. No slash-convention conclusion was made.".into()
            } else {
                format!(
                    "Observed {} route-like internal base path{}. Some targets use a trailing slash and others may not, but no identical base path was linked in both forms; different route types can intentionally use different conventions.",
                    variants.len(),
                    if variants.len() == 1 { "" } else { "s" }
                )
            },
            status: if has_conflict {
                CheckStatus::Warn
            } else if variants.is_empty() {
                CheckStatus::Skipped
            } else {
                CheckStatus::Pass
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if has_conflict {
                Some("Request representative `/path` and `/path/` variants and inspect their status, redirect destination, final URL, and canonical tag. If the server exposes unintended duplicate URLs, choose the route convention the framework supports, update internal links, and add a fixed canonical redirect for the other form. If both forms are intentional or already normalize to one URL, do not rewrite links solely to satisfy this style signal.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "distinct_route_bases": variants.len(),
                "bases_linked_with_slash": with_slash,
                "bases_linked_without_slash": without_slash,
                "conflicting_base_paths": conflict_count,
                "document_base_applied": document_base.is_some(),
                "server_behavior_verified": false,
            })),
            confidence: if has_conflict || variants.is_empty() {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: if has_conflict {
                Some("Internal-link style is directly counted, but server routing and canonicalization were not tested for each variant.".into())
            } else if variants.is_empty() {
                Some("The fetched HTML did not provide enough eligible link evidence; runtime navigation and other pages were not inspected.".into())
            } else {
                None
            },
            why_it_matters: if has_conflict {
                Some("On servers that treat slash variants as distinct, mixed internal links can create competing URL signals or extra crawling. When the server already redirects or resolves both forms consistently, the style difference alone may have no search impact.".into())
            } else {
                None
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_print_stylesheet_present_pass() {
        let html = r#"<html><head><style>@media print { .no-print { display: none; } }</style></head></html>"#;
        let check = PrintStylesheetCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn missing_print_stylesheet_is_not_a_defect_without_a_print_use_case() {
        let html = "<html><head></head><body>No print styles</body></html>";
        let check = PrintStylesheetCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].manual_fix.is_none());
    }

    #[test]
    fn print_stylesheet_with_external_css_is_inconclusive() {
        let html =
            r#"<html><head><link rel="stylesheet" href="/app.css"></head><body></body></html>"#;
        let results = PrintStylesheetCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(
            results[0]
                .description
                .contains("external stylesheets are not fetched"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn print_words_inside_script_do_not_count_as_css() {
        let html = r#"<html><body><script>const sample = '@media print { body {} }';</script></body></html>"#;
        let results = PrintStylesheetCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[test]
    fn style_markup_inside_script_does_not_count_as_print_css() {
        let html = r#"<script>const sample = '<style media=print>@media print { body {} }</style><link rel=stylesheet media=print>';</script>"#;
        let results = PrintStylesheetCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[test]
    fn commented_print_rule_does_not_count_but_media_list_does() {
        let commented = r#"<style>/* @media print { body {} } */</style>"#;
        assert_eq!(
            PrintStylesheetCheck.run(&ctx(commented))[0].status,
            CheckStatus::Skipped
        );

        let media_list = r#"<style>@media screen, print { body {} }</style>"#;
        assert_eq!(
            PrintStylesheetCheck.run(&ctx(media_list))[0].status,
            CheckStatus::Pass
        );
    }

    #[test]
    fn unquoted_print_media_attribute_is_detected() {
        let html = r#"<link rel=stylesheet media=print href=/print.css>"#;
        let results = PrintStylesheetCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn test_responsive_design_with_viewport_and_media_pass() {
        let html = r#"<html><head><meta name="viewport" content="width=device-width"><style>@media (max-width: 768px) { .col { width: 100%; } }</style></head></html>"#;
        let check = ResponsiveDesignCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0]
            .description
            .contains("does not verify the rendered layout"));
    }

    #[test]
    fn responsive_design_without_source_hints_is_inconclusive_not_failure() {
        let html = "<html><head></head><body><table width=\"800\"></table></body></html>";
        let check = ResponsiveDesignCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert_eq!(results[0].severity, Severity::Low);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].manual_fix.is_none());
        assert!(!results[0].description.contains("No responsive design"));
    }

    #[test]
    fn framework_words_in_prose_or_scripts_are_not_responsive_evidence() {
        let html = r#"<html><body><p>Compare Bootstrap, Tailwind, Foundation and Bulma.</p><script>const css = 'display: grid; @media (max-width: 10px); <meta name=viewport content=width=device-width><picture></picture>';</script></body></html>"#;
        let results = ResponsiveDesignCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[test]
    fn non_media_max_width_and_script_style_examples_are_not_responsive_hints() {
        let html = r#"<meta name=viewport content=width=device-width>
            <style>.card { max-width: 40rem; } @media print { .page { max-width: 100%; } }</style>
            <script>const example = '<style>@media (max-width: 20rem) {}</style>';</script>"#;
        let results = ResponsiveDesignCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["inline_width_media_query"],
            false
        );
    }

    #[test]
    fn external_stylesheets_make_source_only_assessment_explicitly_inconclusive() {
        let html = r#"<html><head><meta name=viewport content=width=device-width><link rel=stylesheet href=/app.css></head><body></body></html>"#;
        let results = ResponsiveDesignCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0]
            .description
            .contains("external stylesheets are not fetched"));
    }

    #[test]
    fn test_trailing_slash_consistent_pass() {
        let html = r#"<html><body>
            <a href="/about/">About</a>
            <a href="/contact/">Contact</a>
            <a href="/pricing/">Pricing</a>
            <a href="/blog/">Blog</a>
        </body></html>"#;
        let check = TrailingSlashCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn same_internal_path_linked_with_both_variants_warns() {
        let html = r#"<html><body>
            <a href="/about/">About</a>
            <a href="/about?source=nav">About duplicate</a>
            <a href="/docs/">Docs</a>
            <a href="/docs#top">Docs duplicate</a>
        </body></html>"#;
        let check = TrailingSlashCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].description.contains("observed"));
        assert!(results[0]
            .manual_fix
            .as_deref()
            .is_some_and(|fix| fix.contains("server") && fix.contains("redirect")));
        let why = results[0].why_it_matters.as_deref().unwrap_or_default();
        assert!(why.contains("may") || why.contains("can"));
        assert!(!why.contains("hurts SEO rankings"));
    }

    #[test]
    fn different_routes_may_intentionally_use_different_slash_styles() {
        let html = r#"<a href="/about/">About</a><a href="/contact">Contact</a><a href="/blog/">Blog</a><a href="/pricing">Pricing</a>"#;
        let results = TrailingSlashCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("no identical base path"));
    }

    #[test]
    fn path_case_is_preserved_when_comparing_slash_variants() {
        let html = r#"<a href="/Docs/">Uppercase route</a><a href="/docs">Lowercase route</a>"#;
        let results = TrailingSlashCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["conflicting_base_paths"],
            0
        );
    }

    #[test]
    fn first_document_base_href_is_applied_to_relative_links() {
        let html =
            r#"<base href="/docs/"><a href="about/">Docs about</a><a href="/about">Site about</a>"#;
        let results = TrailingSlashCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["conflicting_base_paths"],
            0
        );
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["document_base_applied"],
            true
        );
    }

    #[test]
    fn link_examples_inside_scripts_do_not_affect_slash_evidence() {
        let html = r#"<a href="/real/">Real</a><script>const a='<a href="/real">x</a>';</script>"#;
        let results = TrailingSlashCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }
}
