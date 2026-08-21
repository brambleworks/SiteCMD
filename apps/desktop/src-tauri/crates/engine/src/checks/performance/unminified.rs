//! Flags large executable inline blocks with unminified formatting.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::sync::LazyLock;

static UNMIN_SCRIPT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script(\s[^>]*)?>(.+?)</script>").unwrap());
static UNMIN_STYLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<style(?:\s[^>]*)?>(.+?)</style>").unwrap());
static SCRIPT_TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)type\s*=\s*["']?([a-z0-9/+.-]+)"#).unwrap());

/// Whether a script tag contains executable JavaScript rather than data.
fn is_executable_script(attrs: &str) -> bool {
    match SCRIPT_TYPE_RE.captures(attrs) {
        None => true,
        Some(caps) => {
            let script_type = caps[1].to_ascii_lowercase();
            matches!(
                script_type.as_str(),
                "module" | "text/javascript" | "application/javascript"
            )
        }
    }
}

pub struct UnminifiedCodeCheck;

impl Check for UnminifiedCodeCheck {
    fn id(&self) -> &str {
        "performance.unminified"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut candidate_scripts = 0;
        let mut candidate_styles = 0;
        let mut candidate_total_bytes: usize = 0;
        // Retain bounded locations so fix prompts can identify each block.
        let mut block_locations: Vec<serde_json::Value> = Vec::new();

        // Skip tiny inline blocks (<500 bytes) - those are fine
        let min_size = 500;

        for cap in UNMIN_SCRIPT_RE.captures_iter(&ctx.body) {
            let attrs = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            if !is_executable_script(attrs) {
                continue;
            }
            let content = &cap[2];
            if content.len() >= min_size && looks_unminified(content) {
                candidate_scripts += 1;
                candidate_total_bytes += content.len();
                if block_locations.len() < 5 {
                    let m = cap.get(0).expect("captures_iter yields full match");
                    block_locations.push(serde_json::json!({
                        "kind": "script",
                        "line": line_number_at(&ctx.body, m.start()),
                        "byte_offset": m.start(),
                        "size_bytes": content.len(),
                        "preview": preview(content),
                    }));
                }
            }
        }

        for cap in UNMIN_STYLE_RE.captures_iter(&ctx.body) {
            let content = &cap[1];
            if content.len() >= min_size && looks_unminified(content) {
                candidate_styles += 1;
                candidate_total_bytes += content.len();
                if block_locations.len() < 5 {
                    let m = cap.get(0).expect("captures_iter yields full match");
                    block_locations.push(serde_json::json!({
                        "kind": "style",
                        "line": line_number_at(&ctx.body, m.start()),
                        "byte_offset": m.start(),
                        "size_bytes": content.len(),
                        "preview": preview(content),
                    }));
                }
            }
        }

        let total = candidate_scripts + candidate_styles;

        let (status, severity) = if candidate_total_bytes > 50_000 {
            (CheckStatus::Fail, Severity::Medium)
        } else if total > 0 {
            (CheckStatus::Warn, Severity::Low)
        } else {
            (CheckStatus::Pass, Severity::Low)
        };

        vec![CheckResult {
            check_id: "performance.unminified".into(),
            category: ScanCategory::Performance,
            title: if total == 0 {
                "Inline code formatting".into()
            } else {
                "Large inline code appears unminified".into()
            },
            description: if total == 0 {
                "No executable inline script or style block of at least 500 bytes matched this check's whitespace heuristic.".into()
            } else {
                format!(
                    "The whitespace heuristic matched {} large inline block{} ({} script{}, {} style{}) totaling {} bytes. This formatting signal does not prove production minification is disabled or measure the transfer or runtime savings of changing it.",
                    total, if total == 1 { "" } else { "s" },
                    candidate_scripts, if candidate_scripts == 1 { "" } else { "s" },
                    candidate_styles, if candidate_styles == 1 { "" } else { "s" },
                    candidate_total_bytes,
                )
            },
            status,
            severity,
            fix_prompt: None,
            manual_fix: if total > 0 {
                Some(
                    "Inspect each block location and trace it to its source or generator. Compare source bytes, compressed transferred bytes, parse/evaluation cost, cache reuse, and critical-path role before changing it. Remove unused content first; minify or externalize a block only when a production measurement shows a benefit, then verify CSP hashes/nonces, execution order, initial styling, caching, and source-map symbolication."
                        .into(),
                )
            } else {
                None
            },
            raw_data: if total > 0 {
                Some(serde_json::json!({
                    "candidate_script_blocks": candidate_scripts,
                    "candidate_style_blocks": candidate_styles,
                    "candidate_total_bytes": candidate_total_bytes,
                    "heuristic": "at_least_500_bytes_more_than_10_lines_and_short_average_lines_with_formatting_signal",
                    // Up to 5 location records so AI fix prompts can grep
                    // the offending markup. Each entry has kind, line,
                    // byte_offset, size_bytes, and a 120-char preview.
                    "block_locations": block_locations,
                }))
            } else {
                None
            },
            confidence: if total > 0 {
                crate::checks::IssueConfidence::NeedsReview
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: (total > 0).then(|| "The block sizes and formatting are direct observations, but the whitespace heuristic cannot prove build mode or minification state and does not measure transfer savings, parse cost, cache reuse, or critical-path impact.".into()),
            why_it_matters: if total > 0 {
                Some("If the surfaced formatting corresponds to avoidable bytes or work, it can increase document transfer, parsing, or evaluation cost; this source heuristic does not establish that impact by itself.".into())
            } else {
                None
            },
        }]
    }
}

/// 1-indexed line number of the byte offset within `body`.
fn line_number_at(body: &str, byte_offset: usize) -> usize {
    body[..byte_offset.min(body.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

/// First ~120 chars of the block, with newlines normalized to spaces so
/// the preview fits one JSON line.
fn preview(content: &str) -> String {
    let normalized: String = content
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let trimmed = normalized.trim();
    if trimmed.len() > 120 {
        let cut = crate::checks::floor_char_boundary(trimmed, 120);
        format!("{}…", &trimmed[..cut])
    } else {
        trimmed.to_string()
    }
}

/// Heuristic: code is probably unminified if it has many newlines relative to its size
fn looks_unminified(code: &str) -> bool {
    let lines = code.lines().count();
    let avg_line_len = code.len().checked_div(lines).unwrap_or(code.len());

    // Minified code typically has very long lines (>500 chars avg)
    // Unminified code has short lines (<120 chars avg) with many lines
    if lines > 10 && avg_line_len < 120 {
        // Also check for common unminified indicators
        let has_comments = code.contains("//") || code.contains("/*");
        let has_indentation = code.contains("    ") || code.contains("\t");
        return has_comments || has_indentation || avg_line_len < 80;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::preview;
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

    /// A >500 byte block with short indented lines - unminified-looking.
    fn big_pretty_block(kind: &str) -> String {
        (0..40)
            .map(|i| format!("    \"{kind}_field_{i}\": \"value {i}\",\n"))
            .collect()
    }

    #[test]
    fn json_ld_blocks_are_not_unminified_code() {
        let html = format!(
            "<script type=\"application/ld+json\">{{\n{}\n}}</script>",
            big_pretty_block("schema")
        );
        let results = super::UnminifiedCodeCheck.run(&ctx(&html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn text_templates_are_not_unminified_code() {
        let html = format!(
            "<script type=\"text/x-template\">\n{}\n</script>",
            big_pretty_block("tpl")
        );
        let results = super::UnminifiedCodeCheck.run(&ctx(&html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn executable_scripts_are_still_flagged() {
        let code: String = (0..40)
            .map(|i| format!("    var field_{i} = {i}; // init\n"))
            .collect();
        let plain = format!("<script>\n{code}\n</script>");
        assert_eq!(
            super::UnminifiedCodeCheck.run(&ctx(&plain))[0].status,
            CheckStatus::Warn
        );
        let typed = format!("<script type=\"text/javascript\">\n{code}\n</script>");
        assert_eq!(
            super::UnminifiedCodeCheck.run(&ctx(&typed))[0].status,
            CheckStatus::Warn
        );
    }

    #[test]
    fn heuristic_match_is_not_presented_as_proven_unminified_code() {
        let code: String = (0..40)
            .map(|i| format!("    var field_{i} = {i}; // init\n"))
            .collect();
        let result = super::UnminifiedCodeCheck.run(&ctx(&format!("<script>\n{code}\n</script>")))
            [0]
        .clone();

        assert_eq!(
            result.confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
        assert!(
            result.title.contains("appears unminified"),
            "{}",
            result.title
        );
        assert!(
            result.description.contains("whitespace heuristic"),
            "{}",
            result.description
        );
        assert!(
            !result.description.contains("Minify these files"),
            "{}",
            result.description
        );
        assert!(
            result
                .confidence_reason
                .as_deref()
                .unwrap_or_default()
                .contains("does not measure transfer savings"),
            "{:?}",
            result.confidence_reason
        );
    }

    #[test]
    fn preview_does_not_panic_on_multibyte_at_boundary() {
        // A run of 3-byte chars (エ = 3 bytes) so a char boundary lands astride
        // byte 120. Truncating with a raw byte slice would panic here.
        let content = "エ".repeat(100);
        let truncated = preview(&content);
        assert!(truncated.ends_with('…'));
        // Truncated well below the 100-char source, and valid UTF-8 (no panic).
        assert!(truncated.chars().count() < 100);
    }

    #[test]
    fn preview_leaves_short_content_untouched() {
        assert_eq!(preview("  var x = 1;  "), "var x = 1;");
    }
}
