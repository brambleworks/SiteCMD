//! Bounded web-manifest identity checks, not full browser install validation.

use crate::checks::html_attrs::{attr_value, tag_slices};
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::probe::{ProbeFailureClass, ProbeOutcome, ProbeRequest};

const CHECK_ID: &str = "config.web_manifest";

/// What the runtime should do after reading the page's manifest
/// declaration: either the verdict is complete, or the declared manifest
/// URL needs one bounded fetch.
pub enum WebManifestStep {
    Done(Vec<CheckResult>),
    Probe { safe_href: String, url: url::Url },
}

/// Why the runtime never executed the planned probe. The runtime keeps its
/// network policy; the refusal is graded here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebManifestProbeSkip {
    Disallowed { safe_url: String },
}

/// The manifest fetch: the JSON body IS the evidence, so a 2xx body is
/// required and a failed read is a probe failure.
pub fn manifest_request(url: &url::Url) -> ProbeRequest {
    ProbeRequest::get(url.as_str())
}

struct ManifestIdentitySummary {
    has_name: bool,
    icon_source_count: usize,
}

fn manifest_identity_summary(manifest: &serde_json::Value) -> Option<ManifestIdentitySummary> {
    let object = manifest.as_object()?;
    let has_name = ["name", "short_name"].iter().any(|key| {
        object
            .get(*key)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    });
    let icon_source_count = object
        .get("icons")
        .and_then(serde_json::Value::as_array)
        .map(|icons| {
            icons
                .iter()
                .filter(|icon| {
                    icon.as_object()
                        .and_then(|entry| entry.get("src"))
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|src| !src.trim().is_empty())
                })
                .count()
        })
        .unwrap_or(0);
    Some(ManifestIdentitySummary {
        has_name,
        icon_source_count,
    })
}

/// The `href` of the page's `<link rel="manifest">`, if it declares one.
pub fn manifest_href(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    tag_slices(body, &lower, "link")
        .into_iter()
        .find(|tag| {
            attr_value(tag, "rel").is_some_and(|rel| {
                rel.split_whitespace()
                    .any(|token| token.eq_ignore_ascii_case("manifest"))
            })
        })
        .and_then(|tag| attr_value(tag, "href"))
        .filter(|href| !href.is_empty())
}

fn pass_without_manifest() -> Vec<CheckResult> {
    vec![CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Polish,
        title: "Web app manifest".into(),
        description: "No web app manifest is declared. That is normal for a site that does not target an installed-app experience; this check does not recommend adding a manifest solely to clear a scan.".into(),
        status: CheckStatus::Pass,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }]
}

fn manifest_result(
    title: String,
    description: String,
    status: CheckStatus,
    manual_fix: Option<String>,
    raw_data: serde_json::Value,
    confidence: IssueConfidence,
    confidence_reason: Option<String>,
    why_it_matters: Option<String>,
) -> Vec<CheckResult> {
    vec![CheckResult {
        check_id: CHECK_ID.into(),
        category: ScanCategory::Polish,
        title,
        description,
        status,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix,
        raw_data: Some(raw_data),
        confidence,
        confidence_reason,
        why_it_matters,
    }]
}

/// Decide whether the page declares a fetchable manifest.
pub fn plan_web_manifest(body: &str, page_url: &url::Url) -> WebManifestStep {
    let Some(href) = manifest_href(body) else {
        return WebManifestStep::Done(pass_without_manifest());
    };
    let safe_href = crate::log_sanitizer::evidence_safe_url_reference(&href);

    let Ok(url) = page_url.join(&href) else {
        return WebManifestStep::Done(manifest_result(
            "Web app manifest link is malformed".into(),
            format!(
                "The page declares a manifest at '{}', but that href does not resolve to a valid URL.",
                safe_href
            ),
            CheckStatus::Warn,
            Some("Fix the href in the <link rel=\"manifest\"> tag so it points at your manifest file, usually /manifest.json or /site.webmanifest.".into()),
            serde_json::json!({"href": safe_href}),
            IssueConfidence::High,
            None,
            Some("User agents cannot fetch manifest metadata through a malformed href. Whether that affects users depends on which installed-app behavior the site intends to support.".into()),
        ));
    };

    WebManifestStep::Probe { safe_href, url }
}

/// Grade the manifest fetch (or the runtime's refusal to run it).
pub fn evaluate_web_manifest(
    safe_href: &str,
    outcome: Result<ProbeOutcome, WebManifestProbeSkip>,
) -> Vec<CheckResult> {
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(WebManifestProbeSkip::Disallowed { safe_url }) => {
            return manifest_result(
                "Web app manifest target was not probed".into(),
                "The declared manifest target is outside the scanner's permitted page-subresource network policy, so SiteCMD did not request it and cannot assess its response.".into(),
                CheckStatus::Skipped,
                None,
                serde_json::json!({
                    "href": safe_url,
                    "reason": "disallowed_page_subresource_target"
                }),
                IssueConfidence::High,
                None,
                None,
            );
        }
    };

    let response = match outcome {
        ProbeOutcome::Response(response) => response,
        // Distinguish an oversized response from an incomplete exchange.
        ProbeOutcome::Failure(failure) if failure.class == ProbeFailureClass::BodyCapExceeded => {
            return manifest_result(
                "Web app manifest response could not be read".into(),
                format!(
                    "The declared manifest at '{}' returned a successful status, but its response body could not be read within the scan's size and time limits. Its JSON and members were not assessed.",
                    safe_href
                ),
                CheckStatus::Skipped,
                None,
                serde_json::json!({"href": safe_href, "reason": "body_read_failed"}),
                IssueConfidence::High,
                None,
                None,
            );
        }
        ProbeOutcome::Failure(_) => {
            return manifest_result(
                "Web app manifest".into(),
                format!("The scanner could not complete a request for the declared manifest at '{}', so its availability and contents were not assessed.", safe_href),
                CheckStatus::Skipped,
                None,
                serde_json::json!({"href": safe_href}),
                IssueConfidence::High,
                None,
                None,
            );
        }
    };

    if !(200..300).contains(&response.status) {
        let status_code = response.status;
        let confirmed_missing = matches!(status_code, 404 | 410);
        return manifest_result(
            format!("Declared web app manifest returned HTTP {}", status_code),
            format!(
                "SiteCMD's unauthenticated GET for the declared manifest at '{}' returned HTTP {}. {}",
                safe_href,
                status_code,
                if confirmed_missing {
                    "That response directly shows the file was not available at this URL during the scan."
                } else {
                    "This may be transient or differ from a credentialed browser request, so the scan does not claim the manifest is permanently unavailable."
                }
            ),
            CheckStatus::Warn,
            Some("Fetch the exact deployed URL in browser developer tools using the intended manifest credentials mode. If it should be public, make it return the manifest response consistently; otherwise correct or remove the declaration and verify redirects, authentication, and rewrite rules.".into()),
            serde_json::json!({"href": safe_href, "status_code": status_code}),
            if confirmed_missing { IssueConfidence::High } else { IssueConfidence::NeedsReview },
            (!confirmed_missing).then(|| "The status is directly observed, but it may be transient and SiteCMD does not reproduce a signed-in browser's cookie or credential context.".into()),
            Some("A user agent that receives the same non-success response cannot use this manifest's installed-app metadata. The scan does not establish which browsers or install behavior the site promises.".into()),
        );
    }

    let Some(body) = response.body else {
        return manifest_result(
            "Web app manifest response could not be read".into(),
            format!(
                "The declared manifest at '{}' returned a successful status, but its response body could not be read within the scan's size and time limits. Its JSON and members were not assessed.",
                safe_href
            ),
            CheckStatus::Skipped,
            None,
            serde_json::json!({"href": safe_href, "reason": "body_read_failed"}),
            IssueConfidence::High,
            None,
            None,
        );
    };

    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&body.text) else {
        return manifest_result(
            "Web app manifest is not valid JSON".into(),
            format!(
                "The response at '{}' loaded but is not valid JSON, so user agents cannot process it as a web app manifest.",
                safe_href
            ),
            CheckStatus::Warn,
            Some("Inspect the exact response body and Content-Type, then fix invalid JSON or a rewrite that returned HTML/error content. Validate the deployed response as strict JSON before re-scanning.".into()),
            serde_json::json!({"href": safe_href, "reason": "invalid_json"}),
            IssueConfidence::High,
            None,
            Some("A user agent cannot use members from an unparseable response. The effect depends on which installed-app behavior the site intends to support.".into()),
        );
    };

    let Some(summary) = manifest_identity_summary(&manifest) else {
        return manifest_result(
            "Web app manifest is not a JSON object".into(),
            format!(
                "The response at '{}' is valid JSON, but its top-level value is not an object, so it is not a valid web app manifest document.",
                safe_href
            ),
            CheckStatus::Warn,
            Some("Return one top-level JSON object containing the intended manifest members, then validate the exact deployed response and re-scan.".into()),
            serde_json::json!({"href": safe_href, "reason": "non_object_json"}),
            IssueConfidence::High,
            None,
            Some("User agents process manifest members from a JSON object; a different top-level JSON type cannot provide those members.".into()),
        );
    };
    let has_name = summary.has_name;
    let icon_count = summary.icon_source_count;

    let mut missing = Vec::new();
    if !has_name {
        missing.push("a name or short_name");
    }
    if icon_count == 0 {
        missing.push("an icon entry with a nonblank src");
    }

    if missing.is_empty() {
        manifest_result(
            "Web app manifest".into(),
            format!(
                "Manifest at '{}' returned readable JSON with a top-level object, a nonblank name/short_name, and {} icon source{}. This check does not validate full browser install criteria, icon responses or sizes, or installed-app behavior.",
                safe_href,
                icon_count,
                if icon_count == 1 { "" } else { "s" }
            ),
            CheckStatus::Pass,
            None,
            serde_json::json!({"href": safe_href, "icon_count": icon_count}),
            IssueConfidence::High,
            None,
            None,
        )
    } else {
        manifest_result(
            "Web app manifest lacks common identity fields".into(),
            format!(
                "Manifest at '{}' returned a readable JSON object but is missing {}. Name and icon sources are commonly used for installed-app identity; this check does not treat them as the complete installability criteria for every browser.",
                safe_href,
                missing.join(" and ")
            ),
            CheckStatus::Warn,
            Some("If the product targets an installed-app experience, add truthful nonblank name/short_name and icon entries with resolvable src URLs, then check the current criteria for the browsers you support (including required sizes, start URL, display mode, security context, and other applicable members). If it does not, remove an accidental manifest declaration instead.".into()),
            serde_json::json!({"href": safe_href, "icon_count": icon_count, "has_name": has_name}),
            IssueConfidence::High,
            None,
            Some("Missing identity members can prevent or degrade installed-app presentation in browsers that require or use them. The finding does not prove that installation is unavailable everywhere.".into()),
        )
    }
}

#[cfg(test)]
#[path = "web_manifest_tests.rs"]
mod tests;
