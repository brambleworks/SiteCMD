//! seo.link_count: how many links the initial HTML carries. Very large counts
//! can exceed what search engines crawl from one page and thin the signal each
//! internal link passes.

use crate::checks::{
    Check, CheckResult, CheckStatus, IssueConfidence, PageContext, ScanCategory, Severity,
};

/// Counts above this ask for a review. Search-engine guidance has long put
/// "reasonable" per-page link counts well under four figures; index pages and
/// sitemaps legitimately run high, so the verdict stays a review signal.
pub const LINK_REVIEW_THRESHOLD: usize = 1000;

pub struct LinkCountCheck;

impl Check for LinkCountCheck {
    fn id(&self) -> &str {
        "seo.link_count"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let lower = scannable.to_ascii_lowercase();
        let link_count = crate::checks::html_attrs::tag_slices(&scannable, &lower, "a")
            .into_iter()
            .filter(|tag| crate::checks::html_attrs::has_attr(tag, "href"))
            .count();
        let over_threshold = link_count > LINK_REVIEW_THRESHOLD;

        vec![CheckResult {
            check_id: "seo.link_count".into(),
            category: ScanCategory::Seo,
            title: if over_threshold {
                "Very high link count needs review".into()
            } else {
                "Link count".into()
            },
            description: if over_threshold {
                format!(
                    "The initial HTML contains {} links, above the {}-link review threshold. Search engines limit how much of a single page they crawl, and every added link thins the internal-link signal the others pass. An index or archive page can be legitimate at this size.",
                    link_count, LINK_REVIEW_THRESHOLD
                )
            } else {
                format!("The initial HTML contains {} links.", link_count)
            },
            status: if over_threshold {
                CheckStatus::Warn
            } else {
                CheckStatus::Pass
            },
            severity: Severity::Low,
            fix_prompt: over_threshold.then(|| "Review whether this page needs every link it carries. Move repeated boilerplate link blocks behind fewer entry links, split very large listings across pages, and keep the links that describe this page's own content.".to_string()),
            manual_fix: over_threshold.then(|| "Open the page and identify where the links concentrate: navigation, footer, tag clouds, or a long listing. Trim repeated blocks to single entry points and paginate listings so each page stays focused. Keep the change only where the page's purpose allows it.".to_string()),
            raw_data: Some(serde_json::json!({
                "link_count": link_count,
                "review_threshold": LINK_REVIEW_THRESHOLD,
            })),
            confidence: if over_threshold {
                IssueConfidence::NeedsReview
            } else {
                IssueConfidence::High
            },
            confidence_reason: over_threshold.then(|| "The link count is read directly from the served markup, but whether it is a problem depends on the page's role: index, sitemap, and archive pages carry large counts on purpose.".to_string()),
            why_it_matters: over_threshold.then(|| "Crawlers budget how much of a page they process, so links late in a very long page may never be followed. Fewer, more deliberate links also concentrate ranking signal on the pages that matter.".to_string()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, IssueConfidence, PageContext};
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

    fn page_with_links(count: usize) -> String {
        let links: String = (0..count)
            .map(|index| format!("<a href=\"/page-{index}\">Page {index}</a>"))
            .collect();
        format!("<html><body>{links}</body></html>")
    }

    #[test]
    fn ordinary_link_counts_pass() {
        let results = LinkCountCheck.run(&ctx(&page_with_links(40)));
        assert_eq!(results.len(), 1);
        let result = &results[0];
        assert_eq!(result.check_id, "seo.link_count");
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(result.confidence, IssueConfidence::High);
        assert_eq!(result.raw_data.as_ref().unwrap()["link_count"], 40);
    }

    #[test]
    fn a_count_at_the_review_threshold_still_passes() {
        let results = LinkCountCheck.run(&ctx(&page_with_links(LINK_REVIEW_THRESHOLD)));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn a_count_above_the_review_threshold_warns_for_review() {
        let results = LinkCountCheck.run(&ctx(&page_with_links(LINK_REVIEW_THRESHOLD + 1)));
        let result = &results[0];
        assert_eq!(result.status, CheckStatus::Warn, "{}", result.description);
        assert_eq!(result.severity, crate::checks::Severity::Low);
        assert_eq!(result.confidence, IssueConfidence::NeedsReview);
        assert!(
            result
                .description
                .contains(&(LINK_REVIEW_THRESHOLD + 1).to_string()),
            "{}",
            result.description
        );
    }

    #[test]
    fn anchors_without_href_and_non_content_blocks_are_not_links() {
        let html = r#"
            <a name="top"></a>
            <!-- <a href="/commented">old</a> -->
            <script>var tpl = '<a href="/js">x</a>';</script>
            <a href="/real">Real</a>
        "#;
        let results = LinkCountCheck.run(&ctx(html));
        assert_eq!(results[0].raw_data.as_ref().unwrap()["link_count"], 1);
    }
}
