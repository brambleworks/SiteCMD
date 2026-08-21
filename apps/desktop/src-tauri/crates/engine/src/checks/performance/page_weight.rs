//! HTML document byte-size grading; asset transfer weight is scored separately.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

pub const CHECK_ID: &str = "performance.page_weight";

/// HTML documents above this warn (1 MB, decimal - matches the displayed unit).
const HTML_WARN_BYTES: usize = 1_000_000;

/// HTML documents above this fail (3 MB, decimal - matches the displayed unit).
const HTML_FAIL_BYTES: usize = 3_000_000;

pub fn html_size_result(body_size: usize) -> CheckResult {
    let size_text = super::assets::format_bytes(body_size as u64);

    let (status, severity, title, description) = if body_size > HTML_FAIL_BYTES {
        (
            CheckStatus::Fail,
            Severity::High,
            "HTML document over 3 MB",
            format!(
                "The fetched HTML document alone is {}. Browsers can render progressively while HTML streams, but a document this large can add substantial download, tokenization, DOM-construction, and memory work when it is transferred.",
                size_text
            ),
        )
    } else if body_size > HTML_WARN_BYTES {
        (
            CheckStatus::Warn,
            Severity::Medium,
            "HTML document over 1 MB",
            format!(
                "The fetched HTML document alone is {}. Large documents can add download and parse work when transferred, even though browsers may render progressively while the HTML streams.",
                size_text
            ),
        )
    } else {
        (
            CheckStatus::Pass,
            Severity::Low,
            "HTML document size",
            format!(
                "The HTML document is {}. Asset transfer weight (images, scripts, stylesheets) is measured separately by the asset sampler.",
                size_text
            ),
        )
    };

    CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Performance,
        title: title.into(),
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix: if status == CheckStatus::Pass {
            None
        } else {
            Some(
                "Profile the production response and DOM before changing rendering. Remove unintended debug output, duplicated markup, and oversized embedded data; paginate or defer genuinely non-critical sections only when doing so preserves accessibility, crawlable content, and user workflows. Re-measure transfer, parsing, and rendering on representative devices."
                    .into(),
            )
        },
        raw_data: Some(serde_json::json!({
            "html_size_bytes": body_size,
            "html_size_kb_decimal": body_size as f64 / 1000.0,
            "warn_threshold_bytes": HTML_WARN_BYTES,
            "fail_threshold_bytes": HTML_FAIL_BYTES,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: if status == CheckStatus::Pass {
            None
        } else {
            // Streaming HTML adds download and parse cost while browsers may
            // still render progressively and preload subresources.
            Some(
                "A navigation that transfers oversized HTML incurs download and parse work before the whole document is available. Caching, streaming, device speed, and document structure determine the actual paint and interaction impact."
                    .into(),
            )
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_documents_pass_and_mention_the_asset_sampler() {
        let result = html_size_result(48 * 1024);
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.description.contains("measured separately"));
        assert!(result.manual_fix.is_none());
    }

    #[test]
    fn one_megabyte_is_the_warn_boundary() {
        assert_eq!(html_size_result(HTML_WARN_BYTES).status, CheckStatus::Pass);
        let result = html_size_result(HTML_WARN_BYTES + 1);
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(result.severity, Severity::Medium);
        assert!(result.manual_fix.is_some());
        assert!(result.why_it_matters.is_some());
        assert!(!result.description.contains("to every visit"));
        assert!(!result
            .why_it_matters
            .as_deref()
            .unwrap()
            .contains("Every visit"));
    }

    #[test]
    fn three_megabytes_is_the_fail_boundary() {
        assert_eq!(html_size_result(HTML_FAIL_BYTES).status, CheckStatus::Warn);
        let result = html_size_result(HTML_FAIL_BYTES + 1);
        assert_eq!(result.status, CheckStatus::Fail);
        assert_eq!(result.severity, Severity::High);
    }

    #[test]
    fn copy_does_not_claim_rendering_is_fully_blocked() {
        for result in [
            html_size_result(HTML_WARN_BYTES + 1),
            html_size_result(HTML_FAIL_BYTES + 1),
        ] {
            assert!(
                !result.description.contains("Nothing renders")
                    && result.description.contains("progressively"),
                "copy must reflect progressive rendering: {}",
                result.description
            );
            let why = result.why_it_matters.expect("why_it_matters");
            assert!(
                !why.contains("blocks everything"),
                "why_it_matters must not claim total blocking: {why}"
            );
        }
    }

    #[test]
    fn raw_data_reports_exact_byte_size() {
        let result = html_size_result(2048);
        let raw = result.raw_data.expect("raw_data");
        assert_eq!(raw["html_size_bytes"], 2048);
    }

    #[test]
    fn raw_units_are_decimal_and_name_their_unit_system() {
        let result = html_size_result(1_500_000);
        let raw = result.raw_data.expect("raw data");
        assert_eq!(raw["html_size_kb_decimal"], 1500.0);
        assert!(raw.get("html_size_kb").is_none());
        assert_eq!(raw["warn_threshold_bytes"], HTML_WARN_BYTES);
        assert_eq!(raw["fail_threshold_bytes"], HTML_FAIL_BYTES);
    }
}
