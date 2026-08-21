//! Counts cross-site scripts in fetched HTML, not runtime requests.

use crate::checks::html_attrs::{attr_value, has_attr, tag_slices};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};
use std::collections::HashSet;

pub struct ThirdPartyScriptsCheck;

impl Check for ThirdPartyScriptsCheck {
    fn id(&self) -> &str {
        "performance.third_party"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let page_host = ctx.url.host_str().unwrap_or("").to_ascii_lowercase();

        let mut external_sites: HashSet<String> = HashSet::new();
        let mut external_origins: HashSet<String> = HashSet::new();
        let mut external_count = 0;
        // Parser-blocking candidates have none of async/defer/type=module.
        // This source observation does not prove the request succeeds or the
        // script executes on the scanned navigation.
        let mut sync_count = 0;
        let mut external_script_srcs: Vec<String> = Vec::new();

        for tag in tag_slices(&ctx.body, ctx.body_lower(), "script") {
            let Some(src) = attr_value(tag, "src").filter(|value| !value.trim().is_empty()) else {
                continue;
            };
            let Ok(resolved) = ctx.url.join(src.trim()) else {
                continue;
            };
            if !matches!(resolved.scheme(), "http" | "https") {
                continue;
            }
            let Some(host) = resolved.host_str().map(str::to_ascii_lowercase) else {
                continue;
            };
            if is_same_site(&host, &page_host) {
                continue;
            }

            external_sites.insert(registrable_site(&host));
            external_origins.insert(resolved.origin().ascii_serialization());
            external_count += 1;
            let is_module = attr_value(tag, "type")
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("module"));
            if !has_attr(tag, "async") && !has_attr(tag, "defer") && !is_module {
                sync_count += 1;
            }
            if external_script_srcs.len() < 10 {
                external_script_srcs.push(crate::log_sanitizer::evidence_safe_page_url(
                    resolved.as_str(),
                ));
            }
        }

        let (status, severity) = if external_count > 10 {
            (CheckStatus::Fail, Severity::Medium)
        } else if external_count > 5 {
            (CheckStatus::Warn, Severity::Low)
        } else {
            (CheckStatus::Pass, Severity::Low)
        };

        let mut sites: Vec<String> = external_sites.into_iter().collect();
        sites.sort();
        let mut origins: Vec<String> = external_origins.into_iter().collect();
        origins.sort();

        vec![CheckResult {
            check_id: "performance.third_party".into(),
            category: ScanCategory::Performance,
            title: if external_count > 5 {
                format!("{} cross-site script tags in source markup", external_count)
            } else {
                "Cross-site script tags".into()
            },
            description: if external_count == 0 {
                "No cross-site `<script src>` tag was found in the fetched HTML. Runtime injection, workers, modules imported by scripts, subresources, and same-site third-party services are outside this source check.".into()
            } else {
                format!(
                    "{} cross-site script tag{} from {} registrable site{} and {} origin{} appear{} in the fetched source markup: {}. Separate origins can require connection setup, though DNS caching, connection reuse/coalescing, and protocol behavior affect the cost.{}",
                    external_count,
                    if external_count == 1 { "" } else { "s" },
                    sites.len(),
                    if sites.len() == 1 { "" } else { "s" },
                    origins.len(),
                    if origins.len() == 1 { "" } else { "s" },
                    if external_count == 1 { "s" } else { "" },
                    sites.join(", "),
                    if sync_count > 0 {
                        format!(
                            " {} tag{} declare{} neither async, defer, nor type=module and can block HTML parsing if fetched and executed.",
                            sync_count,
                            if sync_count == 1 { "" } else { "s" },
                            if sync_count == 1 { "s" } else { "" },
                        )
                    } else {
                        " Every surfaced tag declares async, defer, or type=module, so none is parser-blocking under normal HTML script semantics.".to_string()
                    }
                )
            },
            status,
            severity,
            fix_prompt: None,
            manual_fix: if external_count > 5 {
                Some("Inventory each surfaced tag's owner, product purpose, consent/privacy requirements, dependency order, and measured network/main-thread cost. Remove stale or duplicate integrations. Use async, defer, modules, delayed loading, or self-hosting only when licensing and behavior permit, then test blocked/slow vendor failure states. Preconnect only to origins proven critical and otherwise discovered late.".into())
            } else {
                None
            },
            raw_data: Some(serde_json::json!({
                "external_script_count": external_count,
                "sync_script_count": sync_count,
                "registrable_sites": sites,
                "origins": origins,
                "external_script_srcs": external_script_srcs,
            })),
            confidence: if status == CheckStatus::Pass {
                crate::checks::IssueConfidence::High
            } else {
                crate::checks::IssueConfidence::NeedsReview
            },
            confidence_reason: (status != CheckStatus::Pass).then(|| "The source tags, sites, origins, and loading attributes are directly observed, but tag count does not measure actual requests, execution cost, connection reuse, ownership, consent state, or user value.".into()),
            why_it_matters: match status {
                CheckStatus::Warn | CheckStatus::Fail => {
                    if sync_count > 0 {
                        Some("If these tags fetch and execute, cross-site scripts can add connection, transfer, privacy, availability, and main-thread cost; parser-blocking candidates can also delay document parsing.".into())
                    } else {
                        Some("If these tags fetch and execute, cross-site scripts can add connection, transfer, privacy, availability, and main-thread cost even when they do not block HTML parsing.".into())
                    }
                }
                _ => None,
            },
        }]
    }
}

fn registrable_site(host: &str) -> String {
    psl::domain_str(host).unwrap_or(host).to_ascii_lowercase()
}

fn is_same_site(host1: &str, host2: &str) -> bool {
    registrable_site(host1) == registrable_site(host2)
}

#[cfg(test)]
mod tests {
    use super::is_same_site;
    use super::ThirdPartyScriptsCheck;
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
    fn async_and_defer_scripts_are_not_claimed_to_block_rendering() {
        let scripts: String = (0..6)
            .map(|i| {
                format!(r#"<script async src="https://widget{i}.vendor{i}.net/w.js"></script>"#)
            })
            .collect();
        let results = ThirdPartyScriptsCheck.run(&ctx(&scripts));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(
            !results[0].description.contains("block rendering")
                && results[0].description.contains("none is parser-blocking"),
            "async scripts must not be described as render-blocking: {}",
            results[0].description
        );
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["sync_script_count"],
            0
        );
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::NeedsReview
        );
    }

    #[test]
    fn synchronous_scripts_are_counted_and_described_as_blocking() {
        let scripts: String = (0..6)
            .map(|i| format!(r#"<script src="https://widget{i}.vendor{i}.net/w.js"></script>"#))
            .collect();
        let results = ThirdPartyScriptsCheck.run(&ctx(&scripts));
        assert!(
            results[0].description.contains("6 tags declare neither")
                && results[0].description.contains("can block HTML parsing"),
            "sync scripts must be called out: {}",
            results[0].description
        );
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["sync_script_count"],
            6
        );
    }

    #[test]
    fn fix_scopes_preconnect_to_render_critical_origins() {
        // Preconnect must remain tied to measured critical-path evidence.
        let scripts: String = (0..6)
            .map(|i| format!(r#"<script src="https://widget{i}.vendor{i}.net/w.js"></script>"#))
            .collect();
        let results = ThirdPartyScriptsCheck.run(&ctx(&scripts));
        let fix = results[0].manual_fix.as_deref().unwrap_or("");
        assert!(
            fix.contains("origins proven critical") && fix.contains("discovered late"),
            "fix must scope preconnect advice: {fix}"
        );
    }

    #[test]
    fn relative_paths_are_not_third_party_hosts() {
        let result = ThirdPartyScriptsCheck
            .run(&ctx(r#"<script src="js/app.js"></script>
                   <script src="assets/main.js"></script>
                   <script src="/static/bundle.js"></script>
                   <script src="./vendor.js"></script>
                   <script src="../lib/x.js"></script>
                   <script src="bundle.min.js"></script>"#))
            .remove(0);
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(
            result.raw_data.as_ref().unwrap()["external_script_count"],
            0
        );
    }

    #[test]
    fn absolute_and_protocol_relative_urls_are_resolved_by_the_check() {
        let result = ThirdPartyScriptsCheck
            .run(&ctx(r#"<script src="https://cdn.vendor.net/a.js"></script>
                   <script src="//fonts.gstatic.com/x.js"></script>
                   <script src="http://Analytics.VENDOR.org:8080/t.js"></script>"#))
            .remove(0);
        let raw = result.raw_data.as_ref().unwrap();
        assert_eq!(raw["external_script_count"], 3);
        assert_eq!(raw["origins"].as_array().map(Vec::len), Some(3));
        assert!(raw["origins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "http://analytics.vendor.org:8080"));
    }

    #[test]
    fn a_dotless_cross_host_is_still_observed_as_cross_site() {
        let result = ThirdPartyScriptsCheck
            .run(&ctx(r#"<script src="//localhost/x.js"></script>"#))
            .remove(0);
        assert_eq!(
            result.raw_data.as_ref().unwrap()["external_script_count"],
            1
        );
    }

    #[test]
    fn same_site_uses_the_public_suffix_list_instead_of_last_two_labels() {
        assert!(is_same_site("www.example.co.uk", "static.example.co.uk"));
        assert!(!is_same_site("www.example.co.uk", "evil.co.uk"));
        assert!(!is_same_site("github.com", "githubassets.com"));
    }

    #[test]
    fn unquoted_cross_site_scripts_are_counted_and_secret_query_is_redacted() {
        let scripts: String = (0..6)
            .map(|i| {
                format!(
                    "<script src=https://cdn{i}.vendor{i}.net/private/widget.js?token=secret{i}></script>"
                )
            })
            .collect();
        let result = ThirdPartyScriptsCheck.run(&ctx(&scripts)).remove(0);
        assert_eq!(result.status, CheckStatus::Warn);
        let serialized = serde_json::to_string(&result).expect("serialize result");
        assert!(
            serialized.contains("/private/widget.js"),
            "non-secret path evidence should remain locatable: {serialized}"
        );
        assert!(!serialized.contains("token"), "{serialized}");
        assert!(!serialized.contains("secret"), "{serialized}");
        assert!(result.description.contains("cross-site script tags"));
        assert!(result.description.contains("source markup"));
    }
}
