use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

static SCRIPT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script[\s>].*?</script>").unwrap());
static COMMENT_OR_STYLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<!--.*?-->|<style[\s>].*?</style>").expect("valid comment/style regex")
});
static BODY_OPEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<body(?:\s[^<>]*?)?>").expect("valid body-open regex"));
static BODY_CLOSE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</body\s*>").expect("valid body-close regex"));
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static OPEN_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<[a-z][a-z0-9:_-]*(?:\s[^<>]*?)?/?>").expect("valid opening-tag regex")
});

fn visible_body_markup(body: &str) -> String {
    let visible_document =
        crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(body, " ");
    let visible_lower = visible_document.to_ascii_lowercase();
    let body_start = BODY_OPEN_RE
        .find(&visible_lower)
        .map(|found| found.end())
        .unwrap_or(0);
    let body_end = BODY_CLOSE_RE
        .find(&visible_lower[body_start..])
        .map(|found| body_start + found.start())
        .unwrap_or(visible_lower.len());
    visible_document[body_start..body_end].to_string()
}

pub struct SemanticHtmlCheck;

impl Check for SemanticHtmlCheck {
    fn id(&self) -> &str {
        "seo.semantic_html"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();
        let scannable = crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(lower, " ");
        let count =
            |tag: &str| crate::checks::html_attrs::tag_slices(&scannable, &scannable, tag).len();
        let main_count = count("main");
        let article_count = count("article");
        let section_count = count("section");
        let nav_count = count("nav");
        let header_count = count("header");
        let footer_count = count("footer");

        let has_main = main_count > 0;
        let has_article = article_count > 0;
        let has_section = section_count > 0;
        let has_nav = nav_count > 0;
        let has_header = header_count > 0;
        let has_footer = footer_count > 0;

        let semantic_count: u8 = has_main as u8
            + has_article as u8
            + has_section as u8
            + has_nav as u8
            + has_header as u8
            + has_footer as u8;

        let has_primary_content = has_main || has_article;

        if has_primary_content {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Semantic HTML structure".into(),
                description: format!(
                    "The initial HTML contains <main> or <article> plus {} of the six scanned semantic element types. This confirms element presence only; it does not validate the document outline, accessible names, landmark uniqueness, nesting, runtime DOM, or search interpretation.",
                    semantic_count
                ),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "main_count": main_count, "article_count": article_count,
                    "section_count": section_count, "nav_count": nav_count,
                    "header_count": header_count, "footer_count": footer_count,
                    "structure_validated": false,
                })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: "No main or article element observed".into(),
            description: "No <main> or <article> opening tag was observed outside comments, scripts, and styles in the initial HTML. A simple document can still be valid without these optional elements, runtime markup may differ, and this source observation is not evidence of an SEO or accessibility failure.".into(),
            status: CheckStatus::Skipped,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({
                "main_count": main_count, "article_count": article_count,
                "section_count": section_count, "nav_count": nav_count,
                "header_count": header_count, "footer_count": footer_count,
                "runtime_dom_inspected": false,
            })),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("Element absence is bounded to the fetched HTML, and the page's complexity, runtime DOM, existing landmark roles, and need for these optional elements were not evaluated.".into()),
            why_it_matters: None,
        }]
    }
}

pub struct SourceCitationsCheck;

impl Check for SourceCitationsCheck {
    fn id(&self) -> &str {
        "seo.source_citations"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let cleaned = visible_body_markup(&ctx.body);
        let cleaned_lower = cleaned.to_ascii_lowercase();
        let text_content = TAG_RE.replace_all(&cleaned, " ");
        let word_count: usize = text_content
            .split_whitespace()
            .filter(|w| w.len() > 1)
            .count();

        if word_count < 300 {
            return vec![];
        }

        let page_host = ctx.url.host_str().unwrap_or("");
        let full_scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let full_scannable_lower = full_scannable.to_ascii_lowercase();
        let document_base =
            crate::checks::html_attrs::tag_slices(&full_scannable, &full_scannable_lower, "base")
                .into_iter()
                .find_map(|tag| crate::checks::html_attrs::attr_value(tag, "href"))
                .and_then(|href| ctx.url.join(&href).ok());
        let resolution_base = document_base.as_ref().unwrap_or(&ctx.url);
        let outbound_count = crate::checks::html_attrs::tag_slices(&cleaned, &cleaned_lower, "a")
            .into_iter()
            .filter_map(|tag| crate::checks::html_attrs::attr_value(tag, "href"))
            .filter_map(|href| resolution_base.join(&href).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .filter(|url| {
                url.host_str()
                    .is_some_and(|target| !same_registrable_site(page_host, target))
            })
            .count();

        if outbound_count > 0 {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Source citations".into(),
                description: format!(
                    "The initial HTML contains {} cross-site HTTP(S) link{}. This check does not verify that a link is a citation, supports a nearby claim, points to a primary/authoritative source, remains reachable, or is visible at runtime.",
                    outbound_count
                    , if outbound_count == 1 { "" } else { "s" }
                ),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({"outbound_links": outbound_count, "visible_text_word_estimate": word_count, "document_base_applied": document_base.is_some(), "citation_semantics_verified": false})),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Cross-site link presence is directly observed, but the relationship between each link and the page's claims was not evaluated.".into()),
                why_it_matters: None,
            }];
        }

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: "Source citations".into(),
            description: "This source-level heuristic estimated at least 300 visible words but found no cross-site HTTP(S) links. Many pages do not make externally sourced factual claims and do not need citations, so no citation defect is inferred. Review manually only when the page relies on claims that readers should be able to verify.".into(),
            status: CheckStatus::Skipped,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({"outbound_links": outbound_count, "visible_text_word_estimate": word_count, "document_base_applied": document_base.is_some(), "citation_need_assessed": false})),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("Word count and link absence do not reveal whether the page makes externally verifiable claims, cites sources in another form, or gains links at runtime.".into()),
            why_it_matters: None,
        }]
    }
}

fn same_registrable_site(first: &str, second: &str) -> bool {
    let first = first.trim_end_matches('.').to_ascii_lowercase();
    let second = second.trim_end_matches('.').to_ascii_lowercase();
    match (psl::domain_str(&first), psl::domain_str(&second)) {
        (Some(first), Some(second)) => first.eq_ignore_ascii_case(second),
        _ => first == second,
    }
}

pub struct JsOnlyContentCheck;

/// Return the word count for a script-backed SPA shell with under 30 visible words.
pub fn js_shell_signature(body: &str, lower: &str) -> Option<usize> {
    debug_assert_eq!(body.len(), lower.len());
    let visible_body = visible_body_markup(body);

    let text = TAG_RE.replace_all(&visible_body, " ");
    let word_count: usize = text.split_whitespace().filter(|w| w.len() > 1).count();

    let has_js_shell = OPEN_TAG_RE.find_iter(&visible_body).any(|tag| {
        crate::checks::html_attrs::attr_value(tag.as_str(), "id").is_some_and(|id| {
            matches!(
                id.as_str(),
                "root" | "app" | "__next" | "__nuxt" | "__svelte"
            )
        })
    });

    let script_source = COMMENT_OR_STYLE_RE.replace_all(body, " ");
    let has_scripts = SCRIPT_RE.is_match(&script_source);

    (word_count < 30 && has_js_shell && has_scripts).then_some(word_count)
}

impl Check for JsOnlyContentCheck {
    fn id(&self) -> &str {
        "seo.js_only_content"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let lower = ctx.body_lower();

        if let Some(word_count) = js_shell_signature(&ctx.body, lower) {
            vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Possible minimal HTML application shell".into(),
                description: format!(
                    "The fetched body has an observed SPA mount id, at least one script, and an estimated {} visible words before JavaScript. This is a source-level shell heuristic: it does not execute the app, measure rendered content, identify the page's purpose, or establish how a particular crawler processes it.",
                    word_count
                ),
                status: CheckStatus::Warn,
                severity: Severity::Medium,
                fix_prompt: None,
                manual_fix: Some("First compare the raw logged-out HTML response with the rendered DOM and confirm whether this URL is a content/discovery page or an intentionally client-only application surface. If important public content is absent from the response, use the installed framework's current SSR, static generation, or prerendering approach for that route and keep critical metadata/content available without a successful client render. Re-fetch production HTML and test the rendered page before closing the issue.".into()),
                raw_data: Some(serde_json::json!({"visible_text_word_estimate": word_count, "spa_mount_marker": true, "script_present": true, "rendered_dom_inspected": false})),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The mount marker, scripts, and small source-text estimate are direct observations, but the client-rendered DOM and page intent were not evaluated.".into()),
                why_it_matters: Some("When important public content exists only after a client render, clients that do not execute the application successfully may receive little usable content. The actual impact depends on the route and consumer.".into()),
            }]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::Check;

    fn ctx_with_body(body: &str) -> PageContext {
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

    /// ~360 words of >1-letter body text, enough to clear the 300-word
    /// content-page gate in SourceCitationsCheck.
    fn long_prose() -> String {
        "alpha beta gamma delta epsilon zeta ".repeat(60)
    }

    #[test]
    fn semantic_layout_with_primary_content_passes() {
        let body = "<html><body><header></header><nav></nav><main><h1>Doc</h1></main><footer></footer></body></html>";
        let results = SemanticHtmlCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0]
            .description
            .contains("does not validate the document outline"));
    }

    #[test]
    fn div_only_page_is_inconclusive_not_an_seo_failure() {
        let body = r#"<html><body><div class="wrap"><div>Widgets</div></div></body></html>"#;
        let results = SemanticHtmlCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].title.contains("No main or article"));
        assert!(results[0].manual_fix.is_none());
    }

    #[test]
    fn main_alone_is_not_penalized_for_omitting_optional_page_regions() {
        let body = "<html><body><main><h1>Doc</h1></main></body></html>";
        let results = SemanticHtmlCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn semantic_tag_text_inside_script_does_not_count() {
        let body = r#"<html><body><script>const sample = '<main><article>x</article></main>';</script><div>Page</div></body></html>"#;
        let results = SemanticHtmlCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }

    #[test]
    fn short_pages_are_not_graded_for_citations() {
        let body = "<html><body><p>Widgets for sale.</p></body></html>";
        let results = SourceCitationsCheck.run(&ctx_with_body(body));
        assert!(results.is_empty());
    }

    #[test]
    fn content_page_with_external_citations_passes_with_the_count() {
        let body = format!(
            r#"<html><body><p>{}</p>
            <a href="https://www.w3.org/TR/WCAG22/">WCAG</a>
            <a href="https://datatracker.ietf.org/doc/html/rfc9110">RFC 9110</a>
            </body></html>"#,
            long_prose()
        );
        let results = SourceCitationsCheck.run(&ctx_with_body(&body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0]
            .description
            .contains("2 cross-site HTTP(S) links"));
        assert!(results[0].description.contains("does not verify"));
    }

    #[test]
    fn no_external_links_is_not_automatically_a_citation_defect() {
        let body = format!(
            r#"<html><body><p>{}</p>
            <a href="https://example.com/docs">Docs</a>
            <a href="https://example.com/blog">Blog</a>
            </body></html>"#,
            long_prose()
        );
        let results = SourceCitationsCheck.run(&ctx_with_body(&body));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0]
            .description
            .contains("no cross-site HTTP(S) links"));
        assert!(results[0].manual_fix.is_none());
    }

    #[test]
    fn host_name_substrings_do_not_hide_external_links() {
        let body = format!(
            r#"<html><body><p>{}</p><a href="https://notexample.com/research">Research</a></body></html>"#,
            long_prose()
        );
        let results = SourceCitationsCheck.run(&ctx_with_body(&body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].raw_data.as_ref().unwrap()["outbound_links"], 1);
    }

    #[test]
    fn links_inside_script_examples_are_not_counted() {
        let body = format!(
            r#"<html><body><p>{}</p><script>const sample = '<a href="https://outside.example/x">x</a>';</script></body></html>"#,
            long_prose()
        );
        let results = SourceCitationsCheck.run(&ctx_with_body(&body));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert_eq!(results[0].raw_data.as_ref().unwrap()["outbound_links"], 0);
    }

    #[test]
    fn document_base_is_applied_before_classifying_relative_links() {
        let body = format!(
            r#"<html><head><base href="https://research.example.net/sources/"></head><body><p>{}</p><a href="study">Study</a></body></html>"#,
            long_prose()
        );
        let results = SourceCitationsCheck.run(&ctx_with_body(&body));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].raw_data.as_ref().unwrap()["outbound_links"], 1);
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["document_base_applied"],
            true
        );
    }

    #[test]
    fn javascript_shell_with_no_server_rendered_text_warns_needs_review() {
        let body = r#"<html><body><div id="root"></div><script src="/assets/main.js"></script></body></html>"#;
        let results = JsOnlyContentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Medium);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
    }

    #[test]
    fn javascript_shell_detects_unquoted_mount_id() {
        let body = r#"<html><body><div id=root></div><script src=/app.js></script></body></html>"#;
        let results = JsOnlyContentCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn mount_id_text_inside_script_is_not_a_shell() {
        let body = r#"<html><body><div>Page</div><script>const sample = 'id="root"';</script></body></html>"#;
        let results = JsOnlyContentCheck.run(&ctx_with_body(body));
        assert!(results.is_empty());
    }

    #[test]
    fn comments_do_not_supply_visible_words_or_a_script_element() {
        let noisy_comment = "comment words that are not rendered ".repeat(20);
        let real_script = format!(
            "<html><body><!-- {} --><div id=\"root\"></div><script src=\"/app.js\"></script></body></html>",
            noisy_comment
        );
        assert_eq!(
            JsOnlyContentCheck.run(&ctx_with_body(&real_script))[0].status,
            CheckStatus::Warn
        );

        let fake_script =
            "<html><body><div id=\"root\"></div><!-- <script src=/app.js></script> --></body></html>";
        assert!(JsOnlyContentCheck
            .run(&ctx_with_body(fake_script))
            .is_empty());
    }

    #[test]
    fn mount_id_matching_preserves_case() {
        let body =
            r#"<html><body><div id="ROOT"></div><script src="/app.js"></script></body></html>"#;
        assert!(JsOnlyContentCheck.run(&ctx_with_body(body)).is_empty());
    }

    #[test]
    fn server_rendered_app_with_a_root_container_is_not_flagged() {
        // SSR frameworks keep the mount div but ship real content in it;
        // the check must grade the words, not the container id.
        let body = format!(
            r#"<html><body><div id="root"><main><p>{}</p></main></div><script src="/assets/main.js"></script></body></html>"#,
            long_prose()
        );
        let results = JsOnlyContentCheck.run(&ctx_with_body(&body));
        assert!(results.is_empty());
    }

    #[test]
    fn static_page_without_scripts_is_not_flagged() {
        let body = "<html><body><h1>Widgets</h1></body></html>";
        let results = JsOnlyContentCheck.run(&ctx_with_body(body));
        assert!(results.is_empty());
    }
}
