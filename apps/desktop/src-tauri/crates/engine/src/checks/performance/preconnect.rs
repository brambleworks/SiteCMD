//! Preconnect coverage for external render-critical origins.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

static PRECONNECT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<link\s[^>]*rel\s*=\s*["'](?:preconnect|dns-prefetch)["'][^>]*href\s*=\s*["']([^"']+)["']"#).unwrap()
});
static PRECONNECT_RE2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<link\s[^>]*href\s*=\s*["']([^"']+)["'][^>]*rel\s*=\s*["'](?:preconnect|dns-prefetch)["']"#).unwrap()
});
/// Render-critical script and stylesheet/preload origins.
static SCRIPT_SRC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<script\s[^>]*src\s*=\s*["'](https?://[^"']+)["'][^>]*>"#).unwrap()
});
/// async/defer as attributes, or type=module; matched against the tag with
/// the src value blanked out (same approach as render_blocking.rs).
static ASYNC_DEFER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\s(async|defer)[\s/>=]|type\s*=\s*["']?module\b"#).unwrap());
static LINK_TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)<link\s[^>]*>"#).unwrap());
static FETCH_REL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)rel\s*=\s*["'](?:stylesheet|preload)["']"#).unwrap());
static HREF_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)href\s*=\s*["'](https?://[^"']+)["']"#).unwrap());

/// Checks for preconnect hints to third-party origins
pub struct PreconnectCheck;

impl Check for PreconnectCheck {
    fn id(&self) -> &str {
        "performance.preconnect"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let page_host = ctx.url.host_str().unwrap_or("").to_lowercase();
        let page_domain = extract_domain(&page_host);

        // Find preconnect links
        let mut preconnected: HashSet<String> = HashSet::new();
        for cap in PRECONNECT_RE.captures_iter(&ctx.body) {
            if let Some(domain) = extract_domain_from_url(&cap[1]) {
                preconnected.insert(domain);
            }
        }
        for cap in PRECONNECT_RE2.captures_iter(&ctx.body) {
            if let Some(domain) = extract_domain_from_url(&cap[1]) {
                preconnected.insert(domain);
            }
        }

        // Find all external subresource domains, splitting render-critical
        // origins (stylesheets, preloads, synchronous scripts) from deferred
        // ones (async/defer/module scripts). Only render-critical origins
        // benefit from preconnect enough to grade on.
        let mut external_domains: HashSet<String> = HashSet::new();
        let mut critical_domains: HashSet<String> = HashSet::new();
        let external_domain_of = |url: &str| -> Option<String> {
            let domain = extract_domain_from_url(url)?;
            let base = extract_domain(&domain);
            (base != page_domain && !base.is_empty()).then_some(domain)
        };

        for cap in SCRIPT_SRC_RE.captures_iter(&ctx.body) {
            let full_tag = cap.get(0).map(|m| m.as_str()).unwrap_or("");
            let src = &cap[1];
            if let Some(domain) = external_domain_of(src) {
                // Blank the src value so URL text can't satisfy the match.
                let attrs_only = full_tag.replace(src, " ");
                if !ASYNC_DEFER_RE.is_match(&attrs_only) {
                    critical_domains.insert(domain.clone());
                }
                external_domains.insert(domain);
            }
        }
        for tag in LINK_TAG_RE.find_iter(&ctx.body) {
            let tag_str = tag.as_str();
            if FETCH_REL_RE.is_match(tag_str) {
                if let Some(cap) = HREF_URL_RE.captures(tag_str) {
                    if let Some(domain) = external_domain_of(&cap[1]) {
                        critical_domains.insert(domain.clone());
                        external_domains.insert(domain);
                    }
                }
            }
        }

        // Missing hints only count for render-critical origins.
        let missing: Vec<String> = critical_domains
            .iter()
            .filter(|d| !preconnected.contains(d.as_str()))
            .cloned()
            .collect::<Vec<_>>();

        let external_count = external_domains.len();
        let critical_count = critical_domains.len();

        if external_count == 0 {
            return vec![CheckResult {
                check_id: "performance.preconnect".into(),
                category: ScanCategory::Performance,
                title: "Preconnect hints".into(),
                description: "No external origins detected - preconnect hints are not needed."
                    .into(),
                status: CheckStatus::Pass,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        // Warn only when two or more render-critical origins are unhinted;
        // hinting those exact origins (the fix) clears the check on re-scan.
        let (status, severity) = if missing.len() >= 2 {
            (CheckStatus::Warn, Severity::Low)
        } else {
            (CheckStatus::Pass, Severity::Low)
        };

        let mut sorted_missing = missing.clone();
        sorted_missing.sort();
        sorted_missing.truncate(5);
        // Sort HashSet-derived evidence so identical inputs remain deterministic
        // across processes and runtimes.
        let mut sorted_preconnected: Vec<String> = preconnected.into_iter().collect();
        sorted_preconnected.sort();

        vec![CheckResult {
            check_id: "performance.preconnect".into(),
            category: ScanCategory::Performance,
            title: if status == CheckStatus::Pass {
                "Preconnect hints".into()
            } else {
                "Render-critical origins missing preconnect hints".into()
            },
            description: if missing.is_empty() {
                format!(
                    "All render-critical external origins ({} of {} external origin{}) are covered by preconnect or dns-prefetch hints; deferred script origins don't need them.",
                    critical_count,
                    external_count,
                    if external_count == 1 { "" } else { "s" },
                )
            } else if status == CheckStatus::Pass {
                format!(
                    "{} render-critical origin lacks a preconnect hint ({} external origin{} total). A single unhinted origin is a minor cost; deferred script origins don't need hints.",
                    missing.len(),
                    external_count,
                    if external_count == 1 { "" } else { "s" },
                )
            } else {
                format!(
                    "{} render-critical origin{} (stylesheets, synchronous scripts, preloads) missing preconnect hints: {}{}",
                    missing.len(),
                    if missing.len() == 1 { "" } else { "s" },
                    sorted_missing.join(", "),
                    if missing.len() > 5 { " (and more)" } else { "" }
                )
            },
            status,
            severity,
            fix_prompt: None,
            manual_fix: if status != CheckStatus::Pass {
                Some(format!(
                    "Preconnect the origins that deliver render-critical assets (fonts, CSS, synchronous scripts) - browsers hold each preconnected socket open, so keep it to the few origins that matter: {}. Example: <link rel=\"preconnect\" href=\"https://example-cdn.com\">. Add the crossorigin attribute only when the asset is fetched with CORS (fonts, ES modules); use dns-prefetch or nothing for async/deferred origins.",
                    sorted_missing.join(", ")
                ))
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "preconnected": sorted_preconnected,
                "missing": sorted_missing,
                "external_count": external_count,
                "critical_count": critical_count,
            })),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: if status == CheckStatus::Pass {
                None
            } else {
                Some("Missing preconnect hints add DNS+TLS latency for each render-critical external origin.".into())
            },
        }]
    }
}

fn extract_domain_from_url(url: &str) -> Option<String> {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("//")
        .split('/')
        .next()
        .map(|h| h.split(':').next().unwrap_or(h).to_lowercase())
        .filter(|h| !h.is_empty() && h.contains('.'))
}

fn extract_domain(host: &str) -> String {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else {
        host.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn anchor_links_are_not_resource_origins() {
        let html = r#"<footer>
            <a href="https://twitter.com/acme">Twitter</a>
            <a href="https://github.com/acme">GitHub</a>
            <a href="https://www.linkedin.com/company/acme">LinkedIn</a>
            <a href="https://youtube.com/@acme">YouTube</a>
        </footer>"#;
        let results = PreconnectCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("No external origins"));
    }

    #[test]
    fn script_and_stylesheet_origins_are_counted() {
        let html = r#"<head>
            <script src="https://cdn-a.com/app.js"></script>
            <script src="https://cdn-b.com/lib.js"></script>
            <link rel="stylesheet" href="https://cdn-c.com/site.css">
            <script src="https://cdn-d.com/vendor.js"></script>
        </head>"#;
        let results = PreconnectCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        let fix = results[0].manual_fix.as_deref().unwrap_or("");
        assert!(
            fix.contains("render-critical") && fix.contains("crossorigin attribute only"),
            "advice must scope preconnect and crossorigin: {fix}"
        );
    }

    #[test]
    fn deferred_script_origins_never_trigger_the_warn() {
        let html = r#"<head>
            <script async src="https://analytics-a.com/a.js"></script>
            <script defer src="https://analytics-b.com/b.js"></script>
            <script async src="https://analytics-c.com/c.js"></script>
            <script type="module" src="https://esm-d.com/d.js"></script>
            <script defer src="https://widget-e.com/e.js"></script>
            <script async src="https://chat-f.com/f.js"></script>
        </head>"#;
        let results = PreconnectCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn hinting_the_render_critical_origins_clears_the_warn() {
        let html = r#"<head>
            <link rel="preconnect" href="https://fonts-cdn.com">
            <link rel="preconnect" href="https://app-cdn.com">
            <link rel="stylesheet" href="https://fonts-cdn.com/site.css">
            <script src="https://app-cdn.com/app.js"></script>
            <script async src="https://analytics-a.com/a.js"></script>
            <script async src="https://analytics-b.com/b.js"></script>
            <script async src="https://analytics-c.com/c.js"></script>
        </head>"#;
        let results = PreconnectCheck.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
    }

    #[test]
    fn a_single_unhinted_critical_origin_is_not_a_warn() {
        // The fix says "the one or two origins that matter"; one unhinted
        // origin is within that budget and must not fire.
        let html = r#"<head>
            <link rel="stylesheet" href="https://fonts-cdn.com/site.css">
        </head>"#;
        let results = PreconnectCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn canonical_and_alternate_links_are_not_resource_origins() {
        let html = r#"<head>
            <link rel="canonical" href="https://other-domain.com/page">
            <link rel="alternate" href="https://de.example.org/page" hreflang="de">
        </head>"#;
        let results = PreconnectCheck.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("No external origins"));
    }
}
