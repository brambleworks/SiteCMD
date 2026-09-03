//! Flags large executable inline blocks with unminified formatting.

use crate::checks::html_attrs::{attr_value, has_attr, raw_text_elements, slice_offset};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

/// Whether a script tag contains executable JavaScript rather than data.
fn is_executable_script(tag: &str) -> bool {
    match attr_value(tag, "type") {
        None => true,
        Some(script_type) => matches!(
            script_type.trim().to_ascii_lowercase().as_str(),
            "" | "module" | "text/javascript" | "application/javascript"
        ),
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

        let lower = ctx.body_lower();

        for (tag, content) in raw_text_elements(&ctx.body, lower, "script") {
            // An external script's element content is empty; its code lives at
            // the src URL and is never inspected here. Reading past the empty
            // body previously merged the next block into this one and graded
            // page markup as inline script.
            if has_attr(tag, "src") || !is_executable_script(tag) {
                continue;
            }
            if content.len() >= min_size && looks_unminified(content) {
                candidate_scripts += 1;
                candidate_total_bytes += content.len();
                if block_locations.len() < 5 {
                    let start = slice_offset(&ctx.body, tag);
                    block_locations.push(serde_json::json!({
                        "kind": "script",
                        "line": line_number_at(&ctx.body, start),
                        "byte_offset": start,
                        "size_bytes": content.len(),
                        "preview": preview(content),
                    }));
                }
            }
        }

        for (tag, content) in raw_text_elements(&ctx.body, lower, "style") {
            if content.len() >= min_size
                && looks_unminified(content)
                && !declarations_look_minified(content)
            {
                candidate_styles += 1;
                candidate_total_bytes += content.len();
                if block_locations.len() < 5 {
                    let start = slice_offset(&ctx.body, tag);
                    block_locations.push(serde_json::json!({
                        "kind": "style",
                        "line": line_number_at(&ctx.body, start),
                        "byte_offset": start,
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
                    "heuristic": "at_least_500_bytes_more_than_10_lines_and_short_average_lines_with_formatting_signal_and_for_styles_multi_line_rule_bodies",
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

/// Whether a stylesheet's declarations are already minified, whatever the
/// layout around them is. `looks_unminified` reads line count, indentation,
/// and comments, so a build that emits one minified rule per line under a
/// `/* section */` comment matched it while carrying no removable declaration
/// whitespace at all. The removable bytes live inside rule bodies, so that is
/// what this measures: a body that never spans a line has nothing left to
/// strip. Only innermost bodies count, since an `@media` wrapper spans lines
/// whenever the rules it holds are on separate lines.
fn declarations_look_minified(css: &str) -> bool {
    // (start byte offset, whether a nested block was seen inside it)
    let mut open: Vec<(usize, bool)> = Vec::new();
    let mut bodies = 0usize;
    let mut multi_line_bodies = 0usize;
    // A brace inside a quoted value (`content: "{"`) or a comment is not a
    // block boundary, so both are skipped rather than counted.
    let mut quote: Option<u8> = None;
    let mut in_comment = false;
    let bytes = css.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                in_comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        match quote {
            // A backslash escapes the next byte inside a CSS string.
            Some(_) if byte == b'\\' => index += 1,
            Some(active) if byte == active => quote = None,
            Some(_) => {}
            None if matches!(byte, b'"' | b'\'') => quote = Some(byte),
            None if byte == b'/' && bytes.get(index + 1) == Some(&b'*') => {
                in_comment = true;
                index += 2;
                continue;
            }
            None if byte == b'{' => open.push((index, false)),
            None if byte == b'}' => {
                if let Some((start, has_nested)) = open.pop() {
                    if !has_nested {
                        bodies += 1;
                        if bytes[start..index].contains(&b'\n') {
                            multi_line_bodies += 1;
                        }
                    }
                    if let Some(parent) = open.last_mut() {
                        parent.1 = true;
                    }
                }
            }
            None => {}
        }
        index += 1;
    }
    // Too few rules to read a build style from; leave the verdict to the
    // whitespace heuristic.
    bodies >= 3 && multi_line_bodies * 5 < bodies
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
    fn an_empty_external_script_never_absorbs_the_markup_that_follows_it() {
        // Two adjacent external scripts and then a large HTML body. The old
        // `(.+?)` body group could not close on its own end tag, so the match
        // ran to the next `</script>` and page markup was graded as inline
        // script (wordpress.org, drupal.org, github.com in the live corpus).
        let filler: String = (0..60)
            .map(|i| format!("  <li class=\"item-{i}\"><a href=\"/page/{i}\">Item {i}</a></li>\n"))
            .collect();
        // The page markup sits between the two external scripts, exactly as on
        // wordpress.org: the first tag's body ran to the second tag's closing
        // `</script>` and swallowed everything in between.
        let html = format!(
            "<script defer src=\"/a.js\"></script>\n<ul>\n{filler}</ul>\n<script src=\"/b.js\"></script>"
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
    fn an_inline_script_after_an_empty_external_script_is_measured_on_its_own() {
        let code: String = (0..40)
            .map(|i| format!("    var field_{i} = {i}; // init\n"))
            .collect();
        let html = format!("<script src=\"/a.js\"></script>\n<script>\n{code}\n</script>");
        let result = super::UnminifiedCodeCheck.run(&ctx(&html))[0].clone();

        assert_eq!(result.status, CheckStatus::Warn, "{}", result.description);
        let raw = result.raw_data.as_ref().expect("raw data");
        assert_eq!(raw["candidate_script_blocks"], 1);
        assert_eq!(raw["candidate_total_bytes"], (code.len() + 2) as u64);
        let preview = raw["block_locations"][0]["preview"]
            .as_str()
            .expect("preview");
        assert!(
            preview.starts_with("var field_0"),
            "the reported block must be the inline script, not the tag before it: {preview}"
        );
    }

    #[test]
    fn a_json_ld_block_after_an_empty_external_script_is_still_data() {
        let html = format!(
            "<script defer src=\"/a.js\"></script><script type=\"application/ld+json\">{{\n{}\n}}</script>",
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
    fn one_minified_css_rule_per_line_is_not_unminified_formatting() {
        // The smarthomeu.com shape: minified declarations, one rule per line,
        // six-space indentation and `/* section */` comments around them.
        let rules: String = (0..30)
            .map(|i| format!("      .u-{i}{{margin:{i}px;padding:{i}px;color:#111}}\n"))
            .collect();
        let css = format!("\n      /* Fonts */\n{rules}      @media(max-width:767px){{.page{{padding-top:56px}}.navbar-brand img{{height:40px}}}}\n    ");
        assert!(css.len() >= 500, "fixture must clear the size floor");
        assert!(
            super::looks_unminified(&css),
            "the whitespace heuristic must still match, or this test proves nothing"
        );
        let results = super::UnminifiedCodeCheck.run(&ctx(&format!("<style>{css}</style>")));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn css_with_declarations_on_their_own_lines_is_still_flagged() {
        let rules: String = (0..15)
            .map(|i| {
                format!(
                    "      .u-{i} {{\n        margin: {i}px;\n        color: #111111;\n      }}\n"
                )
            })
            .collect();
        let results = super::UnminifiedCodeCheck.run(&ctx(&format!("<style>\n{rules}</style>")));
        assert_eq!(
            results[0].status,
            CheckStatus::Warn,
            "{}",
            results[0].description
        );
        assert_eq!(
            results[0].raw_data.as_ref().expect("raw data")["candidate_style_blocks"],
            1
        );
    }

    #[test]
    fn a_brace_in_a_comment_or_a_string_is_not_a_rule_body() {
        // `content: "{"` and a commented-out rule are text, not structure. A
        // lexical brace count would read them as unclosed bodies and hand the
        // wrong ratio to the minified-declaration test.
        let minified: String = (0..30)
            .map(|i| format!("      .u-{i}{{margin:{i}px;content:\"{{\"}}\n"))
            .collect();
        let css =
            format!("\n      /* a commented rule: .x {{ color: red;\n         }} */\n{minified}");
        assert!(
            super::declarations_look_minified(&css),
            "30 single-line rule bodies must still read as minified declarations"
        );

        let pretty: String = (0..15)
            .map(|i| format!("      .u-{i} {{\n        content: \"{{\";\n      }}\n"))
            .collect();
        assert!(
            !super::declarations_look_minified(&pretty),
            "a brace inside a quoted value must not close the rule that holds it"
        );
    }

    #[test]
    fn commented_out_markup_is_not_measured_as_inline_code() {
        let code: String = (0..40)
            .map(|i| format!("    var field_{i} = {i}; // init\n"))
            .collect();
        let html = format!("<!-- <script>\n{code}\n</script> -->");
        let results = super::UnminifiedCodeCheck.run(&ctx(&html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
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
