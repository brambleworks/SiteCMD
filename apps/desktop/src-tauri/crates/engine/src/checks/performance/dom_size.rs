//! Estimates DOM size from the fetched HTML element count.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::cmp::Reverse;
use std::sync::LazyLock;

static DOM_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[a-zA-Z][a-zA-Z0-9]*[\s/>]").unwrap());

pub struct DomSizeCheck;

impl Check for DomSizeCheck {
    fn id(&self) -> &str {
        "performance.dom_size"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Keep per-tag counts so remediation can target the dominant elements.
        use std::collections::HashMap;
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for m in DOM_TAG_RE.find_iter(&ctx.body) {
            // Strip the leading `<` and the trailing whitespace/`/`/`>`.
            let raw = m.as_str();
            let name: String = raw[1..raw.len() - 1].to_ascii_lowercase();
            *tag_counts.entry(name).or_insert(0) += 1;
        }
        let count: usize = tag_counts.values().sum();
        let mut top_tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
        // Break count ties by tag name so randomized HashMap iteration cannot
        // change evidence for identical inputs.
        top_tags.sort_by(|left, right| {
            Reverse(left.1)
                .cmp(&Reverse(right.1))
                .then(left.0.cmp(&right.0))
        });
        let top_tags_json: Vec<serde_json::Value> = top_tags
            .iter()
            .take(8)
            .map(|(name, n)| serde_json::json!({ "tag": name, "count": n }))
            .collect();

        let (status, severity) = if count > 1400 {
            (CheckStatus::Fail, Severity::Medium)
        } else if count > 800 {
            (CheckStatus::Warn, Severity::Low)
        } else {
            (CheckStatus::Pass, Severity::Low)
        };

        vec![CheckResult {
            check_id: "performance.dom_size".into(),
            category: ScanCategory::Performance,
            title: match status {
                CheckStatus::Fail => "DOM size over 1400 elements".into(),
                CheckStatus::Warn => "DOM size over 800 elements".into(),
                _ => "DOM size".into(),
            },
            description: match status {
                CheckStatus::Fail => format!(
                    "Page has ~{} DOM elements. Large DOMs slow down rendering, increase memory usage, and make style recalculations expensive. Aim for under 800 elements.",
                    count
                ),
                CheckStatus::Warn => format!(
                    "Page has ~{} DOM elements. This is getting large - simplify the page structure. Target under 800 elements.",
                    count
                ),
                _ => format!(
                    "Page has ~{} DOM elements. This is within the recommended range for good rendering performance.",
                    count
                ),
            },
            status,
            severity,
            fix_prompt: None,
            manual_fix: if count > 800 {
                Some("Reduce DOM complexity: flatten nested layouts, use CSS instead of wrapper divs, lazy-load below-the-fold content, and paginate long lists.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "element_count": count,
                // Top-8 most-used element types so an AI fix prompt can
                // tell which tag dominates the DOM (usually <div> /
                // <span>) and target it for flattening.
                "top_tags": top_tags_json,
            })),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: match status {
                    CheckStatus::Fail | CheckStatus::Warn => Some("Large DOMs slow rendering and make interactions feel sluggish.".into()),
                    _ => None,
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
    fn test_dom_size_small_page_pass() {
        let html = "<html><head></head><body><div><p>Hello</p></div></body></html>";
        let check = DomSizeCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn test_dom_size_large_page_fail() {
        // Generate 1500 divs - well above the 1400 threshold
        let divs: String = (0..1500).map(|i| format!("<div>{}</div>", i)).collect();
        let html = format!("<html><body>{}</body></html>", divs);
        let check = DomSizeCheck;
        let results = check.run(&ctx(&html));
        assert_eq!(results[0].status, CheckStatus::Fail);
    }

    #[test]
    fn test_dom_size_medium_page_warn() {
        // Generate 900 divs - between 800 and 1400
        let divs: String = (0..900).map(|i| format!("<div>{}</div>", i)).collect();
        let html = format!("<html><body>{}</body></html>", divs);
        let check = DomSizeCheck;
        let results = check.run(&ctx(&html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }
}
