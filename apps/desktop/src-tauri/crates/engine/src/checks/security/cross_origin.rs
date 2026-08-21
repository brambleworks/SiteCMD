//! Cross-origin isolation header checks. Only an isolating COOP value passes;
//! COEP and CORP remain contextual evidence.

use crate::checks::{Check, CheckResult, CheckStatus, PageContext, ScanCategory, Severity};

pub struct CrossOriginIsolationCheck;

impl Check for CrossOriginIsolationCheck {
    fn id(&self) -> &str {
        "security.headers.cross_origin"
    }

    fn category(&self) -> ScanCategory {
        ScanCategory::Security
    }

    fn run(&self, ctx: &PageContext) -> Vec<CheckResult> {
        if ctx.is_localhost {
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: "Cross-origin isolation headers".into(),
                description: "Skipped on localhost preview. Cross-origin headers are usually set by the deployed edge or reverse proxy, so verify them on a real deployment target.".into(),
                status: CheckStatus::Skipped,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(serde_json::json!({"reason": "localhost_preview_server"})),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }];
        }

        let headers = &ctx.response_headers;
        let coop = headers
            .get("cross-origin-opener-policy")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let coep = headers
            .get("cross-origin-embedder-policy")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let corp = headers
            .get("cross-origin-resource-policy")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let raw_data = Some(serde_json::json!({
            "coop": coop,
            "coep": coep,
            "corp": corp,
        }));

        if let Some(coop_value) = &coop {
            // The policy token is the part before any `; report-to=...` params.
            let token = coop_value
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let isolates = matches!(
                token.as_str(),
                "same-origin" | "same-origin-allow-popups" | "noopener-allow-popups"
            );
            if isolates {
                let mut extras = Vec::new();
                if coep.is_some() {
                    extras.push("COEP");
                }
                if corp.is_some() {
                    extras.push("CORP");
                }
                return vec![CheckResult {
                    check_id: self.id().into(),
                    category: self.category(),
                    title: "Cross-origin isolation headers".into(),
                    description: format!(
                        "Cross-Origin-Opener-Policy is set ({}){}. Other sites that open this page cannot keep a scriptable window reference to it.",
                        coop_value,
                        if extras.is_empty() {
                            String::new()
                        } else {
                            format!(", along with {}", extras.join(" and "))
                        },
                    ),
                    status: CheckStatus::Pass,
                    severity: Severity::Low,
                    fix_prompt: None,
                    manual_fix: None,
                    raw_data,
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    why_it_matters: None,
                }];
            }
            let (title, detail) = if token == "unsafe-none" {
                (
                    "Cross-Origin-Opener-Policy is set to unsafe-none",
                    "The COOP header is present but set to unsafe-none, which is the browser default: it explicitly opts out of isolation, so other sites keep a scriptable window reference to your pages.".to_string(),
                )
            } else {
                (
                    "Cross-Origin-Opener-Policy value is not recognized",
                    format!(
                        "The COOP header is set to \"{}\", which browsers do not recognize and therefore treat as unsafe-none (no isolation). Other sites keep a scriptable window reference to your pages.",
                        coop_value
                    ),
                )
            };
            return vec![CheckResult {
                check_id: self.id().into(),
                category: self.category(),
                title: title.into(),
                description: detail,
                status: CheckStatus::Warn,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: Some("Set `Cross-Origin-Opener-Policy: same-origin-allow-popups` at the layer that owns your headers. That value keeps OAuth and payment popups working; tighten to `same-origin` only if the site opens no cross-origin popups it needs to talk to.".into()),
                raw_data,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: Some("A page opened from a malicious link can silently swap your tab for a look-alike phishing page.".into()),
            }];
        }

        vec![CheckResult {
            check_id: self.id().into(),
            category: self.category(),
            title: "No Cross-Origin-Opener-Policy header".into(),
            description: "No Cross-Origin-Opener-Policy (COOP) header. Any site can open your pages with window.open() and keep a live window reference to them, which enables tab-nabbing (the opener swaps your tab for a phishing page) and some cross-site leak attacks. COOP severs that reference.".into(),
            status: CheckStatus::Warn,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: Some("Add `Cross-Origin-Opener-Policy: same-origin-allow-popups` at the layer that owns your headers (CDN, reverse proxy, or framework headers config). That value keeps OAuth and payment popups working; tighten to `same-origin` only if the site opens no cross-origin popups it needs to talk to.".into()),
            raw_data,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: Some("A page opened from a malicious link can silently swap your tab for a look-alike phishing page.".into()),
        }]
    }

    fn skip_in_predeploy(&self) -> bool {
        false // Mirrors security.headers: header checks still run against localhost, they just report as skipped.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{Check, CheckStatus, PageContext};
    use http::header::{HeaderMap, HeaderValue};

    fn ctx_with_headers(headers: HeaderMap) -> PageContext {
        PageContext {
            evaluation_time: chrono::DateTime::parse_from_rfc3339("2026-08-05T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            url: url::Url::parse("https://example.com").unwrap(),
            response_headers: headers,
            status_code: 200,
            body: String::new(),
            is_localhost: false,
            is_strict_localhost: false,
            http_version: Some("HTTP/2.0".to_string()),
            body_lower_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn missing_coop_warns() {
        let results = CrossOriginIsolationCheck.run(&ctx_with_headers(HeaderMap::new()));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].title.contains("Cross-Origin-Opener-Policy"));
    }

    #[test]
    fn coop_present_passes_and_mentions_extras() {
        let mut h = HeaderMap::new();
        h.insert(
            "cross-origin-opener-policy",
            HeaderValue::from_static("same-origin"),
        );
        h.insert(
            "cross-origin-resource-policy",
            HeaderValue::from_static("same-origin"),
        );
        let results = CrossOriginIsolationCheck.run(&ctx_with_headers(h));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("same-origin"));
        assert!(results[0].description.contains("CORP"));
    }

    #[test]
    fn coop_unsafe_none_is_not_protection() {
        let mut h = HeaderMap::new();
        h.insert(
            "cross-origin-opener-policy",
            HeaderValue::from_static("unsafe-none"),
        );
        let results = CrossOriginIsolationCheck.run(&ctx_with_headers(h));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].title.contains("unsafe-none"));
    }

    #[test]
    fn coop_unrecognized_value_is_not_protection() {
        let mut h = HeaderMap::new();
        h.insert(
            "cross-origin-opener-policy",
            HeaderValue::from_static("same-orign"),
        );
        let results = CrossOriginIsolationCheck.run(&ctx_with_headers(h));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert!(results[0].title.contains("not recognized"));
    }

    #[test]
    fn coop_with_report_to_param_passes() {
        let mut h = HeaderMap::new();
        h.insert(
            "cross-origin-opener-policy",
            HeaderValue::from_static("same-origin-allow-popups; report-to=\"coop\""),
        );
        let results = CrossOriginIsolationCheck.run(&ctx_with_headers(h));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Pass);
    }

    #[test]
    fn localhost_is_skipped() {
        let mut ctx = ctx_with_headers(HeaderMap::new());
        ctx.is_localhost = true;
        let results = CrossOriginIsolationCheck.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }
}
