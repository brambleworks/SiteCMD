//! seo.meta_refresh: meta refresh redirects and timed reloads in the initial
//! HTML. Search engines treat a meta refresh as a weak redirect signal, and a
//! timed reload restarts reading for assistive-technology users.

use crate::checks::{
    Check, CheckResult, CheckStatus, IssueConfidence, PageContext, ScanCategory, Severity,
};

/// One parsed `<meta http-equiv="refresh">` directive.
struct RefreshDirective {
    delay_seconds: u64,
    target_url: Option<String>,
}

/// Parse a refresh `content` attribute: a delay, optionally followed by a
/// separator and a `url=` destination (quotes optional per the HTML spec).
fn parse_refresh_content(content: &str) -> Option<RefreshDirective> {
    let trimmed = content.trim();
    let (delay_part, rest) = match trimmed.find([';', ',']) {
        Some(index) => (&trimmed[..index], Some(&trimmed[index + 1..])),
        None => (trimmed, None),
    };
    // The spec allows a fractional delay; whole seconds are enough evidence.
    let delay_seconds = delay_part.trim().parse::<f64>().ok()?;
    if !delay_seconds.is_finite() || delay_seconds < 0.0 {
        return None;
    }
    let target_url = rest.and_then(|rest| {
        let rest = rest.trim();
        let value = if rest.len() >= 4 && rest[..4].eq_ignore_ascii_case("url=") {
            &rest[4..]
        } else {
            rest
        };
        let value = value.trim().trim_matches(['\'', '"']).trim();
        (!value.is_empty()).then(|| value.to_string())
    });
    Some(RefreshDirective {
        delay_seconds: delay_seconds as u64,
        target_url,
    })
}

/// Cap URL evidence so one tag cannot flood the result.
fn bounded_url(value: &str) -> String {
    const MAX_CHARS: usize = 200;
    if value.chars().count() <= MAX_CHARS {
        return value.to_string();
    }
    let cut: String = value.chars().take(MAX_CHARS).collect();
    format!("{cut}(truncated)")
}

pub struct MetaRefreshCheck;

impl Check for MetaRefreshCheck {
    fn id(&self) -> &str {
        "seo.meta_refresh"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Seo
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let scannable =
            crate::checks::seo::headings::NON_CONTENT_BLOCK_RE.replace_all(&ctx.body, " ");
        let lower = scannable.to_ascii_lowercase();
        let directives: Vec<RefreshDirective> =
            crate::checks::html_attrs::tag_slices(&scannable, &lower, "meta")
                .into_iter()
                .filter(|tag| {
                    crate::checks::html_attrs::attr_value(tag, "http-equiv")
                        .is_some_and(|value| value.trim().eq_ignore_ascii_case("refresh"))
                })
                .filter_map(|tag| {
                    crate::checks::html_attrs::attr_value(tag, "content")
                        .as_deref()
                        .and_then(parse_refresh_content)
                })
                .collect();

        let first = directives.first();
        let redirect = first.and_then(|directive| directive.target_url.as_deref());

        let (status, severity, title, description) = match (first, redirect) {
            (None, _) => (
                CheckStatus::Pass,
                Severity::Low,
                "No meta refresh".to_string(),
                "The initial HTML contains no <meta http-equiv=\"refresh\"> directive.".to_string(),
            ),
            (Some(directive), Some(target)) => (
                CheckStatus::Warn,
                Severity::Medium,
                "Meta refresh redirect detected".to_string(),
                format!(
                    "The initial HTML redirects to {} through a <meta http-equiv=\"refresh\"> tag after {} second{}. Search engines treat a meta refresh as a weaker redirect signal than an HTTP redirect and may not consolidate link signals to the destination.",
                    bounded_url(target),
                    directive.delay_seconds,
                    if directive.delay_seconds == 1 { "" } else { "s" },
                ),
            ),
            (Some(directive), None) => (
                CheckStatus::Warn,
                Severity::Low,
                "Timed page reload via meta refresh".to_string(),
                format!(
                    "The initial HTML instructs the browser to reload the page every {} second{} through a <meta http-equiv=\"refresh\"> tag. Each reload restarts reading position and repeats the document for assistive technology, and crawlers may record the page as unstable.",
                    directive.delay_seconds,
                    if directive.delay_seconds == 1 { "" } else { "s" },
                ),
            ),
        };

        let raw_data = match first {
            None => serde_json::json!({ "refresh_count": 0 }),
            Some(directive) => match &directive.target_url {
                Some(target) => serde_json::json!({
                    "refresh_count": directives.len(),
                    "delay_seconds": directive.delay_seconds,
                    "target_url": bounded_url(target),
                }),
                None => serde_json::json!({
                    "refresh_count": directives.len(),
                    "delay_seconds": directive.delay_seconds,
                }),
            },
        };

        vec![CheckResult {
            check_id: "seo.meta_refresh".into(),
            category: ScanCategory::Seo,
            title,
            description,
            status,
            severity,
            fix_prompt: match (status, redirect.is_some()) {
                (CheckStatus::Warn, true) => Some("Replace the meta refresh with a server-side HTTP redirect to the same destination: 301 for a permanent move, 302 for a temporary one. Remove the <meta http-equiv=\"refresh\"> tag once the HTTP redirect is in place.".into()),
                (CheckStatus::Warn, false) => Some("Remove the timed <meta http-equiv=\"refresh\"> reload. If the page shows live data, update the changed content in place instead of reloading the whole document.".into()),
                _ => None,
            },
            manual_fix: match (status, redirect.is_some()) {
                (CheckStatus::Warn, true) => Some("Configure the redirect where the server or host defines routes (server config, framework route, or hosting redirect rules) and delete the meta refresh tag. Verify the old URL answers with a 301 or 302 status afterward.".into()),
                (CheckStatus::Warn, false) => Some("Remove the refresh tag and, where the page needs fresh data, load updates in place. If a reload truly is required, give visitors a control to refresh on their own schedule.".into()),
                _ => None,
            },
            raw_data: Some(raw_data),
            confidence: IssueConfidence::High,
            confidence_reason: if status == CheckStatus::Pass {
                None
            } else {
                Some("The refresh directive is read directly from the served markup. Whether the navigation also happens for visitors with scripting or refresh disabled is outside this static check.".into())
            },
            why_it_matters: match (status, redirect.is_some()) {
                (CheckStatus::Warn, true) => Some("An HTTP redirect tells crawlers directly that the content moved and where its signals belong. A meta refresh leaves that consolidation uncertain, and visitors see an intermediate page load before reaching the destination.".into()),
                (CheckStatus::Warn, false) => Some("A timed reload interrupts reading and form entry, and restarts screen reader output from the top of the document. WCAG asks for a way to turn off, adjust, or extend time limits like this.".into()),
                _ => None,
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, IssueConfidence, PageContext, Severity};
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
    fn no_meta_refresh_passes() {
        let html = "<html><head><meta charset=\"utf-8\"></head><body></body></html>";
        let results = MetaRefreshCheck.run(&ctx(html));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].check_id, "seo.meta_refresh");
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].confidence, IssueConfidence::High);
    }

    #[test]
    fn meta_refresh_redirect_warns_at_medium() {
        let html = r#"<html><head><meta http-equiv="refresh" content="0; url=https://example.com/new"></head></html>"#;
        let results = MetaRefreshCheck.run(&ctx(html));
        let result = &results[0];
        assert_eq!(result.status, CheckStatus::Warn, "{}", result.description);
        assert_eq!(result.severity, Severity::Medium);
        assert_eq!(result.confidence, IssueConfidence::High);
        assert!(
            result.description.contains("https://example.com/new"),
            "{}",
            result.description
        );
        let raw = result.raw_data.as_ref().unwrap();
        assert_eq!(raw["delay_seconds"], 0);
        assert_eq!(raw["target_url"], "https://example.com/new");
    }

    #[test]
    fn unquoted_uppercase_refresh_without_url_is_a_timed_reload() {
        let html = "<html><head><meta http-equiv=REFRESH content=30></head></html>";
        let results = MetaRefreshCheck.run(&ctx(html));
        let result = &results[0];
        assert_eq!(result.status, CheckStatus::Warn, "{}", result.description);
        assert_eq!(result.severity, Severity::Low);
        assert!(
            result.description.contains("30"),
            "reload interval belongs in the description: {}",
            result.description
        );
        let raw = result.raw_data.as_ref().unwrap();
        assert_eq!(raw["delay_seconds"], 30);
        assert!(raw.get("target_url").is_none(), "{raw}");
    }

    #[test]
    fn refresh_inside_comments_and_scripts_is_ignored() {
        let html = r#"
            <!-- <meta http-equiv="refresh" content="0; url=/old"> -->
            <script>var tag = '<meta http-equiv="refresh" content="0; url=/js">';</script>
            <p>Body</p>
        "#;
        let results = MetaRefreshCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn emitted_copy_has_no_em_dashes() {
        for html in [
            r#"<meta http-equiv="refresh" content="5; url=/next">"#,
            r#"<meta http-equiv="refresh" content="300">"#,
        ] {
            let results = MetaRefreshCheck.run(&ctx(html));
            let result = &results[0];
            for text in [
                Some(result.title.as_str()),
                Some(result.description.as_str()),
                result.fix_prompt.as_deref(),
                result.manual_fix.as_deref(),
                result.why_it_matters.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                assert!(!text.contains('\u{2014}'), "em-dash in copy: {text}");
            }
        }
    }
}
