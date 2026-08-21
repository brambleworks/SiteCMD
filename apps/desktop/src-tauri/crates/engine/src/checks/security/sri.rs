//! Inspect cross-origin scripts and stylesheets for Subresource Integrity.
//! Mutable endpoints are excluded; other omissions require review because source alone
//! cannot prove immutability or CORS support.

use crate::checks::html_attrs::{attr_value, has_attr, tag_slices};
use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

/// Checks for external scripts/stylesheets loaded without integrity attributes
pub struct SubresourceIntegrityCheck;

impl Check for SubresourceIntegrityCheck {
    fn id(&self) -> &str {
        "security.sri"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        let mut missing: Vec<String> = Vec::new();
        let mut rolling_excluded: Vec<serde_json::Value> = Vec::new();
        let lower = ctx.body_lower();

        for tag in tag_slices(&ctx.body, lower, "script") {
            let Some(src) = attr_value(tag, "src") else {
                continue;
            };
            if src.is_empty() || !is_cross_origin(&src, &ctx.url) {
                continue;
            }
            if let Some(reason) = sri_exclusion_reason(&src) {
                rolling_excluded.push(serde_json::json!({
                    "url": evidence_url(&src),
                    "reason": reason,
                }));
            } else if !has_attr(tag, "integrity") {
                missing.push(evidence_url(&src));
            }
        }

        for tag in tag_slices(&ctx.body, lower, "link") {
            let is_stylesheet = attr_value(tag, "rel").is_some_and(|rel| {
                rel.split_whitespace()
                    .any(|token| token.eq_ignore_ascii_case("stylesheet"))
            });
            if !is_stylesheet {
                continue;
            }
            let Some(href) = attr_value(tag, "href") else {
                continue;
            };
            if href.is_empty() || !is_cross_origin(&href, &ctx.url) {
                continue;
            }
            if let Some(reason) = sri_exclusion_reason(&href) {
                rolling_excluded.push(serde_json::json!({
                    "url": evidence_url(&href),
                    "reason": reason,
                }));
            } else if !has_attr(tag, "integrity") {
                missing.push(evidence_url(&href));
            }
        }

        let count = missing.len();
        let rolling_note = if !rolling_excluded.is_empty() {
            format!(
                " {} resource{} matched a known dynamic or rolling vendor endpoint and {} excluded from the missing-integrity count because a fixed hash can break when its response bytes change. The exact URL{} and reason{} are preserved in the evidence.",
                rolling_excluded.len(),
                if rolling_excluded.len() == 1 { "" } else { "s" },
                if rolling_excluded.len() == 1 { "was" } else { "were" },
                if rolling_excluded.len() == 1 { "" } else { "s" },
                if rolling_excluded.len() == 1 { "" } else { "s" },
            )
        } else {
            String::new()
        };

        vec![CheckResult {
            check_id: "security.sri".into(),
            category: ScanCategory::Security,
            title: if count == 0 {
                "Subresource integrity (SRI)".into()
            } else {
                "Cross-origin resources without integrity attributes".into()
            },
            description: if count == 0 {
                format!(
                    "All pinnable external scripts and stylesheets use integrity attributes, or no external resources found.{}",
                    rolling_note
                )
            } else {
                format!(
                    "{} cross-origin resource{} loaded without an integrity attribute. SRI can pin a stable, versioned resource to expected bytes, but this source check does not establish that each URL is immutable or that its server permits the CORS request SRI requires.{}",
                    count,
                    if count == 1 { "" } else { "s" },
                    rolling_note
                )
            },
            status: if count == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: if count > 0 {
                Some("Review each surfaced URL. When it identifies stable, versioned bytes and the origin supports CORS, generate a SHA-384 hash from the exact response and add integrity=\"sha384-...\" with crossorigin=\"anonymous\". For intentionally mutable vendor scripts, follow the vendor's trust and loading guidance instead of pinning a hash that will break on update; self-host a reviewed version only when the license and update process support it.".into())
            } else {
                None
            },
            raw_data: if count > 0 || !rolling_excluded.is_empty() {
                Some(serde_json::json!({
                    "missing_integrity": missing,
                    "count": count,
                    "excluded_rolling_resources": rolling_excluded,
                }))
            } else {
                None
            },
            confidence: if count > 0 {
                crate::checks::IssueConfidence::Confirmed
            } else {
                crate::checks::IssueConfidence::High
            },
            confidence_reason: (count > 0).then(|| "The integrity attribute is directly absent, but URL immutability, vendor update policy, and cross-origin SRI compatibility were not verified.".into()),
            why_it_matters: if count == 0 {
                None
            } else {
                Some("For a stable third-party script or stylesheet, SRI can prevent the browser from executing unexpectedly changed bytes after a CDN, account, or delivery-path compromise. It is not applicable to every mutable vendor resource.".into())
            },
        }]
    }
}

fn is_cross_origin(reference: &str, page_url: &url::Url) -> bool {
    let Ok(resolved) = page_url.join(reference.trim()) else {
        return false;
    };
    matches!(resolved.scheme(), "http" | "https") && resolved.origin() != page_url.origin()
}

/// Explain why a specific known endpoint is dynamic or rolling and therefore
/// cannot safely use one fixed integrity hash. Match paths, not whole hosts:
/// those hosts may also serve stable/versioned files that remain reviewable.
fn sri_exclusion_reason(url: &str) -> Option<&'static str> {
    let parsed = if url.starts_with("//") {
        url::Url::parse(&format!("https:{url}"))
    } else {
        url::Url::parse(url)
    }
    .ok()?;
    let host = parsed.host_str()?;
    let path = parsed.path();

    match host {
        "fonts.googleapis.com" => Some(
            "Google Fonts stylesheet responses can vary by request and user agent rather than identifying immutable bytes",
        ),
        "js.stripe.com" if path == "/v3" || path.starts_with("/v3/") => Some(
            "Stripe.js v3 is a vendor-managed rolling endpoint whose bytes can change in place",
        ),
        "www.googletagmanager.com" if path == "/gtm.js" || path == "/gtag/js" => Some(
            "Google Tag Manager and gtag scripts are vendor-managed endpoints whose bytes can change in place",
        ),
        "www.google-analytics.com" if path == "/analytics.js" || path == "/ga.js" => Some(
            "Google Analytics serves this versionless script from a vendor-managed endpoint whose bytes can change in place",
        ),
        "www.paypal.com" if path == "/sdk/js" => Some(
            "the PayPal JavaScript SDK response is assembled from query configuration and vendor-managed code",
        ),
        "www.paypalobjects.com" if path == "/api/checkout.js" => Some(
            "the legacy PayPal checkout SDK is a vendor-managed rolling endpoint whose bytes can change in place",
        ),
        _ => None,
    }
}

fn evidence_url(url: &str) -> String {
    let safe = crate::log_sanitizer::evidence_safe_url_reference(url);
    if safe.len() > 100 {
        let cut = crate::checks::floor_char_boundary(&safe, 97);
        format!("{}…", &safe[..cut])
    } else {
        safe
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
    fn test_sri_external_script_with_integrity_pass() {
        let html = r#"<script src="https://cdn.jsdelivr.net/lib.js" integrity="sha384-abc123" crossorigin="anonymous"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn test_sri_external_script_without_integrity_fail() {
        let html = r#"<script src="https://cdn.jsdelivr.net/lib.js"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(
            results[0].confidence,
            crate::checks::IssueConfidence::Confirmed
        );
        assert!(results[0].description.contains("stable, versioned"));
        assert!(!results[0].description.contains("could be injected"));
    }

    #[test]
    fn test_sri_first_party_script_ignored() {
        let html = r#"<script src="/assets/app.js"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "first-party scripts don't need SRI"
        );
    }

    #[test]
    fn test_sri_same_host_script_ignored() {
        let html = r#"<script src="https://example.com/app.js"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "same-host scripts don't need SRI"
        );
    }

    #[test]
    fn test_sri_external_stylesheet_without_integrity_fail() {
        let html = r#"<link rel="stylesheet" href="https://cdn.example.net/style.css">"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn unquoted_external_script_on_minified_html_is_flagged() {
        let html = "<script src=https://cdn.jsdelivr.net/lib.js></script>";
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn integrity_in_the_src_url_is_not_an_integrity_attribute() {
        let html = r#"<script src="https://cdn.example.net/lib.js?integrity=1"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
    }

    #[test]
    fn stripe_js_rolling_script_is_not_flagged() {
        let html = r#"<script src="https://js.stripe.com/v3"></script>
            <script src="https://www.google-analytics.com/analytics.js"></script>
            <script src="https://www.paypal.com/sdk/js?client-id=test"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
        assert!(
            results[0]
                .description
                .contains("excluded from the missing-integrity count"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn rolling_scripts_are_excluded_from_the_missing_count() {
        // One pinnable CDN script without SRI + one rolling Stripe script:
        // the count must be 1 and the description must explain the exclusion.
        let html = r#"<script src="https://js.stripe.com/v3"></script>
            <script src="https://cdn.jsdelivr.net/lib.js"></script>"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(
            results[0]
                .description
                .contains("1 cross-origin resource loaded"),
            "{}",
            results[0].description
        );
        assert!(
            results[0]
                .description
                .contains("excluded from the missing-integrity count"),
            "{}",
            results[0].description
        );
    }

    #[test]
    fn google_fonts_stylesheet_is_not_flagged() {
        let html = r#"<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;700&display=swap">"#;
        let check = SubresourceIntegrityCheck;
        let results = check.run(&ctx(html));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "Google Fonts CSS cannot use SRI and must not be flagged"
        );
    }

    #[test]
    fn stable_files_on_vendor_hosts_are_not_blanket_excluded() {
        let html = r#"<script src="https://www.googletagmanager.com/static/vendor-1.2.3.js"></script>
            <script src="https://www.paypalobjects.com/assets/checkout-4.5.6.js"></script>"#;
        let result = &SubresourceIntegrityCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(result.raw_data.as_ref().unwrap()["count"], 2);
    }

    #[test]
    fn different_port_is_cross_origin_and_query_secrets_are_not_persisted() {
        let html = r#"<script src="https://example.com:444/app.js?token=supersecret"></script>"#;
        let result = &SubresourceIntegrityCheck.run(&ctx(html))[0];
        let serialized = serde_json::to_string(result).expect("serialize result");

        assert_eq!(result.status, CheckStatus::Warn);
        assert_eq!(result.raw_data.as_ref().unwrap()["count"], 1);
        assert!(serialized.contains("https://example.com:444/app.js"));
        assert!(!serialized.contains("supersecret"));
        assert!(!serialized.contains("token="));
    }

    #[test]
    fn rolling_exclusion_is_preserved_as_evidence() {
        let html = r#"<script src="https://js.stripe.com/v3"></script>"#;
        let result = &SubresourceIntegrityCheck.run(&ctx(html))[0];
        assert_eq!(result.status, CheckStatus::Pass);
        let raw = result.raw_data.as_ref().expect("excluded URL evidence");
        assert_eq!(
            raw["excluded_rolling_resources"].as_array().unwrap().len(),
            1
        );
        assert!(raw["excluded_rolling_resources"][0]["url"]
            .as_str()
            .unwrap()
            .contains("js.stripe.com/v3"));
        assert!(raw["excluded_rolling_resources"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("change in place"));
    }
}
