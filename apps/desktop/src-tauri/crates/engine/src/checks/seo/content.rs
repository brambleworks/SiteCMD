//! Thin-content estimate from server-rendered body text.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

static SCRIPT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script[\s>].*?</script>").unwrap());
static STYLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<style[\s>].*?</style>").unwrap());
static NAV_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<nav[\s>].*?</nav>").unwrap());
static FOOTER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<footer[\s>].*?</footer>").unwrap());
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

/// Checks for thin content (pages with very few words)
pub struct ThinContentCheck;

impl Check for ThinContentCheck {
    fn id(&self) -> &str {
        "seo.thin_content"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let path = ctx.url.path().to_lowercase();
        // Whole path segments only: substring matching skipped /cartoons
        // because it contains /cart.
        let skip_segments = [
            "login",
            "signin",
            "sign-in",
            "register",
            "signup",
            "sign-up",
            "search",
            "404",
            "500",
            "error",
            "cart",
            "checkout",
            "reset-password",
        ];
        let is_functional_page = path
            .split('/')
            .any(|segment| skip_segments.contains(&segment));
        if is_functional_page {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Initial-HTML text estimate not graded".into(),
                description: "The URL path contains a recognized functional page route segment such as login, search, cart, checkout, or an error code. Low text can be appropriate on those routes, so SiteCMD does not grade content depth from word count. The path heuristic does not establish the page's actual purpose.".into(),
                status: CheckStatus::Skipped,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({
                    "reason": "functional_path_segment",
                    "page_purpose_verified": false,
                })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The path segment is directly observed, but the route's purpose and rendered content were not evaluated.".into()),
                why_it_matters: None,
            }];
        }

        // JS shells are graded by seo.js_only_content instead.
        if crate::checks::seo::geo::js_shell_signature(&ctx.body, ctx.body_lower()).is_some() {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Initial-HTML text estimate not graded".into(),
                description: "SiteCMD observed its JavaScript-shell signature and deferred the source-text concern to the JS-only content check. It did not execute the application or assert that the rendered page is empty, so a separate thin-content grade would double-count the same source signal.".into(),
                status: CheckStatus::Skipped,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({ "reason": "js_shell_deferred_to_js_only_content" })),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The shell signature is direct source evidence, but the rendered DOM and page purpose were not evaluated.".into()),
                why_it_matters: None,
            }];
        }

        let word_count = count_body_words(&ctx.body);

        let low_text_signal = word_count < 300;
        let status = if low_text_signal {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        };

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: if word_count < 100 {
                "Very low initial-HTML text estimate needs review".into()
            } else if low_text_signal {
                "Low initial-HTML text estimate needs review".into()
            } else {
                "Initial-HTML text estimate".into()
            },
            description: if low_text_signal {
                format!("The source heuristic estimated about {} word-equivalents after removing scripts, styles, nav, footer, and tags from the initial HTML. This does not measure rendered or hidden-state content, content usefulness, page purpose, language-specific segmentation, originality, search intent, or quality. Short product, contact, utility, media, and transactional pages can fully satisfy users.", word_count)
            } else {
                format!("The source heuristic estimated about {} word-equivalents after removing scripts, styles, nav, footer, and tags from the initial HTML. No low-text review signal fired at this check's 300-word heuristic, but the count does not establish quality, usefulness, originality, or search performance.", word_count)
            },
            status,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if low_text_signal {
                Some("Review the rendered page in its actual language and context, and confirm what task it must complete. If an informational or commercial page genuinely leaves important questions unanswered, add original, specific content that helps the user: purpose, audience, evidence/details, limitations, decisions, or next steps as applicable. Do not add filler to cross a word threshold; consolidate or noindex true duplicates only when that matches the URL strategy.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "initial_html_word_equivalent_estimate": word_count,
                "review_threshold": 300,
                "scripts_styles_nav_footer_removed": true,
                "cjk_character_heuristic": "two_characters_per_word_equivalent",
                "rendered_dom_inspected": false,
                "page_purpose_verified": false,
                "content_quality_verified": false,
            })),
            confidence: if low_text_signal {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: if low_text_signal {
                Some("The bounded initial-HTML estimate is reproducible, but the threshold is heuristic and SiteCMD cannot infer page purpose, language segmentation, rendered content, or usefulness from word count.".into())
            } else {
                None
            },
            why_it_matters: if low_text_signal {
                Some("If an informational page genuinely lacks the detail needed to complete its user task, adding useful specific content can help. The observed count alone does not establish that problem or a ranking effect.".into())
            } else {
                None
            },
        }]
    }
}

fn count_body_words(html: &str) -> usize {
    let lower = html.to_ascii_lowercase();
    let body_start = lower.find("<body").unwrap_or(0);
    // Search for the closing tag only after the opening one. Searching both
    // independently let a stray "</body>" before the real <body> produce
    // body_start > body_end and panic the slice below.
    let body_end = lower[body_start..]
        .find("</body>")
        .map(|rel| body_start + rel)
        .unwrap_or(html.len());
    let body_html = &html[body_start..body_end];

    let cleaned = SCRIPT_RE.replace_all(body_html, " ");
    let cleaned = STYLE_RE.replace_all(&cleaned, " ");
    let cleaned = NAV_RE.replace_all(&cleaned, " ");
    let cleaned = FOOTER_RE.replace_all(&cleaned, " ");
    let text = TAG_RE.replace_all(&cleaned, " ");

    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">");

    // CJK scripts don't separate words with spaces, so a whitespace
    // count read every Japanese/Chinese/Korean page as one "word" and
    // failed it as thin. Count CJK characters
    // separately at ~2 characters per word.
    let cjk_chars = text.chars().filter(|c| is_cjk(*c)).count();
    let non_cjk_text: String = text
        .chars()
        .map(|c| if is_cjk(c) { ' ' } else { c })
        .collect();
    let spaced_words = non_cjk_text
        .split_whitespace()
        .filter(|w: &&str| w.len() > 1)
        .count();
    spaced_words + cjk_chars / 2
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF     // Hiragana + Katakana
        | 0x3400..=0x4DBF   // CJK extension A
        | 0x4E00..=0x9FFF   // CJK unified ideographs
        | 0xAC00..=0xD7AF   // Hangul syllables
        | 0xF900..=0xFAFF   // CJK compatibility ideographs
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, PageContext};

    fn ctx_at(url: &str, body: &str) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse(url).unwrap(),
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
    fn cartoons_page_is_not_skipped_as_a_cart() {
        let thin = "<body><p>Only a few words here.</p></body>";
        let results = ThinContentCheck.run(&ctx_at("https://example.com/cartoons", thin));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(!results[0].description.contains("Skipped"));
    }

    #[test]
    fn cart_page_is_still_skipped() {
        let thin = "<body><p>Your cart is empty.</p></body>";
        let results = ThinContentCheck.run(&ctx_at("https://example.com/shop/cart", thin));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(results[0].description.contains("functional page"));
    }

    #[test]
    fn js_shell_page_defers_to_the_js_only_content_check() {
        let shell = r#"<html><body><div id="root"></div><script src="/assets/main.js"></script></body></html>"#;
        let results = ThinContentCheck.run(&ctx_at("https://example.com/", shell));
        assert_eq!(results[0].status, CheckStatus::Skipped);
        assert!(
            results[0].description.contains("JS-only content check"),
            "{}",
            results[0].description
        );

        // The signature that suppresses thin_content is the one that fires
        // js_only_content, so exactly one of the two grades the page.
        let js_only =
            crate::checks::seo::geo::JsOnlyContentCheck.run(&ctx_at("https://example.com/", shell));
        assert_eq!(js_only[0].status, CheckStatus::Warn);
        assert_eq!(
            js_only[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(js_only[0]
            .description
            .contains("source-level shell heuristic"));
    }

    #[test]
    fn server_rendered_thin_page_is_still_graded_as_thin() {
        // A real (non-shell) page with little text keeps the thin finding.
        let thin = "<body><main><p>Only a few words here.</p></main></body>";
        let results = ThinContentCheck.run(&ctx_at("https://example.com/about-us", thin));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(results[0].description.contains("estimate"));
    }

    #[test]
    fn count_body_words_survives_closing_tag_before_opening_tag() {
        let malformed = "<p>x</body>y</p><body>alpha beta gamma delta</body>";
        let wellformed = "<body>alpha beta gamma delta</body>";
        assert_eq!(count_body_words(malformed), count_body_words(wellformed));
        assert!(count_body_words(malformed) >= 4);
    }

    #[test]
    fn cjk_content_is_counted_as_words() {
        let sentence = "これは日本語で書かれた記事の本文でありサイトの内容を詳しく説明しています"; // 36 chars
        let body = format!("<body><article>{}</article></body>", sentence.repeat(20));
        let results = ThinContentCheck.run(&ctx_at("https://example.jp/article", &body));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }
}
