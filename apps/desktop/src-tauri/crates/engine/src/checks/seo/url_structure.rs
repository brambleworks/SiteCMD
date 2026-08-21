//! Length-only review heuristic for decoded URL paths.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

fn decoded_path_char_count(path: &str) -> usize {
    let hex = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    };
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                decoded.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).chars().count()
}

pub struct UrlStructureCheck;

const URL_PATH_REVIEW_THRESHOLD: usize = 100;

impl Check for UrlStructureCheck {
    fn id(&self) -> &str {
        "seo.url_structure"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        // Measure the decoded path users see; encoded byte length overstates
        // non-Latin URLs. Case and separator style are not graded.
        let path_char_count = decoded_path_char_count(ctx.url.path());
        let exceeds_review_threshold = path_char_count > URL_PATH_REVIEW_THRESHOLD;

        vec![CheckResult {
            check_id: "seo.url_structure".into(),
            category: ScanCategory::Seo,
            title: if !exceeds_review_threshold {
                "URL structure".into()
            } else {
                format!(
                    "URL path exceeds the {}-character review threshold",
                    URL_PATH_REVIEW_THRESHOLD
                )
            },
            description: if !exceeds_review_threshold {
                format!(
                    "The decoded URL path contains {} characters, which does not exceed this check's {}-character review threshold. This length-only heuristic does not assess readability, indexing, canonicalization, or how a particular platform displays the URL.",
                    path_char_count, URL_PATH_REVIEW_THRESHOLD
                )
            } else {
                format!(
                    "The decoded URL path contains {} characters, exceeding this check's {}-character review threshold. Long paths may be harder to inspect or share in some interfaces, but this heuristic alone does not establish an SEO or usability defect.",
                    path_char_count, URL_PATH_REVIEW_THRESHOLD
                )
            },
            status: if !exceeds_review_threshold {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if !exceeds_review_threshold {
                None
            } else {
                Some(
                    "Review the route in context. If the path is unnecessarily verbose, shorten it while preserving a stable, descriptive destination; redirect the old URL and update internal links, sitemap entries, and canonical metadata. Do not change a valid route solely to satisfy this heuristic. Different-case paths such as `/My-Page` and `/my-page` are distinct URLs, so keep the chosen form consistent."
                        .into(),
                )
            },
            raw_data: Some(serde_json::json!({
                "decoded_path_character_count": path_char_count,
                "review_threshold": URL_PATH_REVIEW_THRESHOLD,
                "length_only_heuristic": true,
                "query_character_count_assessed": false,
                "platform_display_assessed": false
            })),
            confidence: if exceeds_review_threshold {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: exceeds_review_threshold.then(|| "The decoded path length is measured directly, but the 100-character threshold is a review heuristic; route intent, audience, readability, and platform-specific display were not evaluated.".into()),
            why_it_matters: if !exceeds_review_threshold {
                None
            } else {
                Some(
                    "Long paths may be harder to inspect, read aloud, or display in constrained interfaces; length alone is not an SEO defect."
                        .into(),
                )
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::{decoded_path_char_count, UrlStructureCheck};
    use crate::checks::{Check, CheckStatus, IssueConfidence, PageContext};

    fn ctx_at(path_url: &str) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse(path_url).unwrap(),
            response_headers: http::header::HeaderMap::new(),
            status_code: 200,
            body: String::new(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn decoded_path_char_count_counts_characters_not_encoding_bytes() {
        assert_eq!(decoded_path_char_count("/about/team"), 11);
        let japanese = "/%E3%81%82%E3%81%84%E3%81%86%E3%81%88%E3%81%8A";
        assert!(japanese.len() > 40, "sanity: encoded form is long");
        assert_eq!(decoded_path_char_count(japanese), 6);
    }

    #[test]
    fn url_structure_fix_does_not_claim_case_equivalence() {
        let long_path = format!("https://example.com/{}", "a".repeat(120));
        let results = UrlStructureCheck.run(&ctx_at(&long_path));
        let fix = results[0].manual_fix.as_deref().unwrap_or("");
        assert!(
            !fix.contains("as equivalent") && fix.contains("distinct URLs"),
            "fix must not claim case equivalence: {fix}"
        );
    }

    #[test]
    fn url_structure_is_labeled_as_a_contextual_length_heuristic() {
        let long_path = format!("https://example.com/{}", "a".repeat(120));
        let result = UrlStructureCheck.run(&ctx_at(&long_path));
        let result = &result[0];
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(result.confidence, IssueConfidence::NeedsReview);
        assert!(result.title.contains("review threshold"));
        assert!(result.description.contains("heuristic"));
        assert!(result
            .why_it_matters
            .as_deref()
            .is_some_and(|why| why.contains("not an SEO defect")));
        let raw = result.raw_data.as_ref().unwrap();
        assert_eq!(raw["decoded_path_character_count"], 121);
        assert_eq!(raw["review_threshold"], 100);
        assert_eq!(raw["length_only_heuristic"], true);
    }
}
