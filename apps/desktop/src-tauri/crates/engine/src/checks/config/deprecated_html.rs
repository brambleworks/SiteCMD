use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

/// Checks for deprecated HTML elements
pub struct DeprecatedHtmlCheck;

/// Pre-compiled regex patterns for each deprecated element: (regex, element_name, fix_suggestion)
static DEPRECATED_PATTERNS: LazyLock<Vec<(Regex, &'static str, &'static str)>> = LazyLock::new(
    || {
        [
            ("center", "Use semantic content elements and CSS alignment"),
            ("font", "Use semantic text elements and CSS typography/color"),
            ("marquee", "Prefer static content; if motion is necessary, implement accessible CSS motion that honors prefers-reduced-motion"),
            // blink is an obsolete Netscape extension unsupported by modern browsers.
            ("blink", "Remove the blinking effect; <blink> is obsolete and unsupported by current major browsers"),
            ("big", "Use CSS font-size instead"),
            ("strike", "Use <s> for no-longer-accurate text, <del> for a documented deletion, or CSS for a purely visual treatment"),
            ("tt", "Use <code>, <kbd>, <samp>, or <var> only when that semantic meaning applies; otherwise use CSS typography"),
            ("frame", "Redesign the frameset as normal documents and layout; use <iframe> only for a genuinely embedded standalone resource"),
            ("frameset", "Replace the framed document architecture with ordinary pages and CSS layout"),
            ("applet", "Remove the obsolete applet and replace the underlying feature with a currently supported implementation"),
            ("basefont", "Move document typography defaults to CSS"),
            ("dir", "Use <ul> when the content is an unordered list, preserving correct list semantics"),
            ("isindex", "Build an explicit, labeled <form> and suitable <input> only if the page still needs this interaction"),
        ]
        .into_iter()
        .map(|(element, fix)| {
            // The tag name must be followed by whitespace, `/`, or `>`.
            // `\b` matched at a hyphen, so custom elements like
            // <font-awesome-icon> and <dir-pagination-controls> were
            // flagged as <font>/<dir>.
            let re = Regex::new(&format!(r"(?i)<{}[\s/>]", regex::escape(element))).unwrap();
            (re, element, fix)
        })
        .collect()
    },
);

impl Check for DeprecatedHtmlCheck {
    fn id(&self) -> &str {
        "config.deprecated_html"
    }

    fn category(&self) -> ScanCategory {
        // Obsolete markup is an accessibility and HTML-correctness concern;
        // practical impact varies by element and browser.
        ScanCategory::Accessibility
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut found: Vec<serde_json::Value> = Vec::new();
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");

        for (re, element, fix) in DEPRECATED_PATTERNS.iter() {
            let count = re.find_iter(&scannable).count();
            if count > 0 {
                found.push(serde_json::json!({
                    "element": format!("<{}>", element),
                    "count": count,
                    "fix": fix,
                }));
            }
        }

        let total_elements: usize = found
            .iter()
            .map(|f| f["count"].as_u64().unwrap_or(0) as usize)
            .sum();

        vec![CheckResult {
            check_id: "config.deprecated_html".into(),
            category: ScanCategory::Accessibility,
            title: if found.is_empty() {
                "Deprecated HTML elements".into()
            } else {
                "Deprecated HTML elements found".into()
            },
            description: if found.is_empty() {
                "None of the scanned obsolete HTML elements were found in page markup outside comments, scripts, and styles. This check does not establish overall HTML conformance.".into()
            } else {
                let names: Vec<String> = found
                    .iter()
                    .map(|f| f["element"].as_str().unwrap_or("").to_string())
                    .collect();
                format!(
                    "Found {} occurrence{} of scanned obsolete or nonconforming HTML elements: {}. Browser behavior and user impact vary by element, but these constructs should be reviewed and replaced without changing the intended semantics.",
                    total_elements,
                    if total_elements == 1 { "" } else { "s" },
                    names.join(", ")
                )
            },
            status: if found.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if !found.is_empty() {
                Some("Review each occurrence in context and use the raw-data guidance as a starting point. Replace it with current semantic HTML, CSS, or a supported implementation as appropriate; do not translate every obsolete element mechanically. Test keyboard use, assistive-technology semantics, layout, and behavior after the change.".into())
            } else {
                None
            },
            raw_data: if !found.is_empty() {
                Some(serde_json::json!({ "deprecated_elements": found }))
            } else {
                None
            },
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if !found.is_empty() {
                Some("Obsolete markup can carry the wrong semantics, depend on legacy browser behavior, complicate maintenance, or create accessibility problems. The actual impact depends on the specific element and how the page uses it.".into())
            } else {
                None
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, PageContext};

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

    #[test]
    fn custom_elements_are_not_deprecated_tags() {
        let body = r#"<html><body><font-awesome-icon icon="user"></font-awesome-icon><dir-pagination-controls></dir-pagination-controls><big-calendar></big-calendar></body></html>"#;
        let results = DeprecatedHtmlCheck.run(&ctx_with_body(body));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "custom elements must not match: {}",
            results[0].description
        );
    }

    #[test]
    fn deprecated_tag_text_in_comments_or_scripts_is_not_reported() {
        let body = r#"<html><body><!-- example: <font color=red> --><script>const sample = '<center>text</center>';</script><p>Current markup</p></body></html>"#;
        let results = DeprecatedHtmlCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn pass_copy_is_bounded_to_the_scanned_elements() {
        let results = DeprecatedHtmlCheck.run(&ctx_with_body("<main>Current markup</main>"));
        assert!(results[0]
            .description
            .contains("None of the scanned obsolete HTML elements"));
        assert!(results[0]
            .description
            .contains("does not establish overall HTML conformance"));
        assert!(!results[0].description.contains("uses modern HTML"));
    }

    #[test]
    fn real_deprecated_tags_still_flagged() {
        let body = r#"<html><body><font color="red">hi</font><center>mid</center><marquee/></body></html>"#;
        let results = DeprecatedHtmlCheck.run(&ctx_with_body(body));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].description.contains("<font>"));
        assert!(results[0].description.contains("<center>"));
        assert!(results[0].description.contains("<marquee>"));
        assert!(!results[0].description.contains("may not render correctly"));
    }
}
