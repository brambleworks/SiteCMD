//! Portable favicon declaration and availability verdicts.
//! Fetch declared icons because markup alone cannot prove they are usable.

use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};
use crate::probe::{BodyPolicy, ProbeOutcome, ProbeRequest};

/// First `<link>` with an `icon` rel token, excluding `apple-touch-icon`.
/// Uses the shared tokenizer for unquoted and minified markup.
pub fn favicon_href(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    crate::checks::html_attrs::tag_slices(body, &lower, "link")
        .into_iter()
        .find(|tag| {
            crate::checks::html_attrs::attr_value(tag, "rel").is_some_and(|rel| {
                rel.split_whitespace()
                    .any(|token| token.eq_ignore_ascii_case("icon"))
            })
        })
        .and_then(|tag| crate::checks::html_attrs::attr_value(tag, "href"))
        .filter(|href| !href.is_empty())
}

/// The probe request for one icon URL: status and headers only, since the
/// verdict grades availability and media type, never the image bytes.
pub fn favicon_probe_request(url: &str) -> ProbeRequest {
    ProbeRequest::get(url).body(BodyPolicy::None)
}

/// Next favicon evaluation step after parsing page markup.
pub enum FaviconStep {
    Done(Vec<CheckResult>),
    /// Probe the declared icon URL (already resolved against the page URL).
    ProbeDeclared {
        url: String,
        safe_href: String,
    },
    /// No declaration: probe the conventional `/favicon.ico` fallback.
    ProbeFallback {
        url: String,
    },
}

/// Read the page markup and decide what to probe. `resolve` joins a relative
/// href against the page URL, returning None when it cannot be resolved.
pub fn plan_favicon(
    body: &str,
    origin_with_port: &str,
    resolve: impl Fn(&str) -> Option<String>,
) -> FaviconStep {
    let Some(href) = favicon_href(body) else {
        return FaviconStep::ProbeFallback {
            url: format!("{origin_with_port}/favicon.ico"),
        };
    };
    if href.to_ascii_lowercase().starts_with("data:image/") {
        return FaviconStep::Done(favicon_result(
            "Favicon".into(),
            "The favicon is declared as an inline image data URI. This source check does not decode the payload or validate its dimensions and appearance.".into(),
            CheckStatus::Pass,
            None,
            Some(serde_json::json!({"href": "data:", "inline": true})),
            None,
            None,
        ));
    }
    if href.to_ascii_lowercase().starts_with("data:") {
        return FaviconStep::Done(favicon_result(
            "Inline favicon does not declare an image media type".into(),
            "The favicon href is a data URI whose declared media type is not image/*. The scanner did not decode it, so browser handling requires review.".into(),
            CheckStatus::Warn,
            Some("Use a valid image data URI with the correct media type, or deploy a normal icon URL. Then inspect the rendered declaration and tab icon in supported browsers.".into()),
            Some(serde_json::json!({"href": "data:", "inline": true, "image_media_type": false})),
            Some("A non-image or malformed inline resource may be ignored as a favicon, but this source-only check does not decode the payload.".into()),
            Some("Only the declared data-URI media type was inspected; the payload and actual browser decoding were not.".into()),
        ));
    }
    let safe_href = crate::log_sanitizer::evidence_safe_url_reference(&href);
    match resolve(&href) {
        Some(url) => FaviconStep::ProbeDeclared { url, safe_href },
        None => FaviconStep::Done(favicon_result(
            "Favicon link is malformed".into(),
            format!(
                "The favicon link href '{}' does not resolve to a valid URL.",
                safe_href
            ),
            CheckStatus::Warn,
            Some(
                "Fix the href in the <link rel=\"icon\"> tag so it points at a real image file."
                    .into(),
            ),
            Some(serde_json::json!({"href": safe_href})),
            Some("Browsers cannot resolve this icon declaration, although another declaration or the conventional /favicon.ico fallback may still supply an icon.".into()),
            None,
        )),
    }
}

/// The two runner-side reasons a planned probe produced no HTTP response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaviconProbeSkip {
    /// The transport never returned a usable response.
    Failed,
    /// The target is outside the permitted page-subresource network policy,
    /// so the runner deliberately did not request it.
    Disallowed,
}

/// Grade the declared icon's probe outcome.
pub fn evaluate_declared(
    safe_href: &str,
    probed_url: &str,
    outcome: Result<ProbeOutcome, FaviconProbeSkip>,
) -> Vec<CheckResult> {
    let response = match probe_response(outcome) {
        Ok(response) => response,
        Err(FaviconProbeSkip::Failed) => return favicon_result(
            "Favicon request did not complete".into(),
            "The scanner could not complete the declared favicon request, so it produced no availability or image-format verdict.".into(),
            CheckStatus::Skipped,
            None,
            Some(serde_json::json!({"href": safe_href, "reason": "request_failed"})),
            None,
            Some("The request failed before a response was available; re-scan or inspect it in browser developer tools.".into()),
        ),
        Err(FaviconProbeSkip::Disallowed) => return favicon_result(
            "Favicon target was not probed".into(),
            "The declared favicon target is outside the scanner's permitted page-subresource network policy, so SiteCMD did not request it or assess its image response.".into(),
            CheckStatus::Skipped,
            None,
            Some(serde_json::json!({"href": crate::log_sanitizer::evidence_safe_page_url(probed_url), "reason": "disallowed_page_subresource_target"})),
            None,
            Some("The target was intentionally not fetched, so no favicon-availability conclusion is possible.".into()),
        ),
    };
    let (status, content_type) = response;
    match favicon_probe_verdict(status, content_type.as_deref()) {
        FaviconProbeVerdict::UsableResponse => favicon_result(
            "Favicon response".into(),
            format!("The declared favicon URL returned HTTP {} with an image Content-Type. The scan does not decode the bytes or validate dimensions, transparency, or appearance.", status),
            CheckStatus::Pass,
            None,
            Some(serde_json::json!({"href": safe_href, "status_code": status, "content_type": content_type})),
            None,
            None,
        ),
        FaviconProbeVerdict::Missing => favicon_result(
            "Declared favicon URL is missing".into(),
            format!("The declared favicon URL at '{}' returned HTTP {} during this scan. A browser receiving the same response cannot load that icon, although another icon declaration or fallback may be used.", safe_href, status),
            CheckStatus::Warn,
            Some("Deploy the intended image at the declared URL or correct/remove the stale declaration. Then verify the final response, Content-Type, decoding, and tab appearance in supported browsers.".into()),
            Some(serde_json::json!({"href": safe_href, "status_code": status, "content_type": content_type})),
            Some("A missing declared icon can leave browsers using another declaration or a generic fallback and creates an avoidable failed request when that URL is fetched.".into()),
            None,
        ),
        FaviconProbeVerdict::Review => favicon_result(
            "Declared favicon response needs review".into(),
            format!("The declared favicon URL returned HTTP {} with Content-Type '{}'. That response does not provide enough evidence for SiteCMD to confirm a usable image; authentication, bot handling, a missing image type, or an empty/special response may be involved.", status, content_type.as_deref().unwrap_or("not provided")),
            CheckStatus::Warn,
            Some("Open the exact URL through the deployed page and inspect the final status, Content-Type, response body, and browser decoding. Correct the file, response metadata, authentication, or declaration only as the observed cause requires.".into()),
            Some(serde_json::json!({"href": safe_href, "status_code": status, "content_type": content_type})),
            Some("The scanner could not confirm that the declared response is a usable icon; browser behavior must be checked before calling it broken.".into()),
            Some("The response was observed, but status and Content-Type alone do not establish whether supported browsers can decode and use it.".into()),
        ),
    }
}

/// Grade the conventional `/favicon.ico` fallback's probe outcome, used when
/// the page declares no icon at all.
pub fn evaluate_fallback(outcome: Result<ProbeOutcome, FaviconProbeSkip>) -> Vec<CheckResult> {
    let response = match probe_response(outcome) {
        Ok(response) => response,
        Err(FaviconProbeSkip::Failed) => return favicon_result(
            "Favicon fallback request did not complete".into(),
            "No favicon link tag was found, and the scanner could not complete its /favicon.ico fallback request. It cannot conclude that the site has no icon.".into(),
            CheckStatus::Skipped,
            None,
            Some(serde_json::json!({"fallback": "/favicon.ico", "reason": "request_failed"})),
            None,
            Some("The fallback request failed before a response was available; re-scan or inspect it in browser developer tools.".into()),
        ),
        Err(FaviconProbeSkip::Disallowed) => return favicon_result(
            "Favicon fallback was not probed".into(),
            "No favicon link tag was found, and the conventional fallback target was outside the permitted page-subresource policy. No favicon-availability verdict was produced.".into(),
            CheckStatus::Skipped,
            None,
            Some(serde_json::json!({"fallback": "/favicon.ico", "reason": "disallowed_page_subresource_target"})),
            None,
            Some("The target was intentionally not fetched, so no favicon-availability conclusion is possible.".into()),
        ),
    };
    let (status, content_type) = response;
    match favicon_probe_verdict(status, content_type.as_deref()) {
        FaviconProbeVerdict::UsableResponse => favicon_result(
            "Conventional favicon response".into(),
            format!("No favicon link tag was found, but /favicon.ico returned HTTP {} with an image Content-Type. Browsers may use that conventional fallback; the scan does not decode or visually inspect it.", status),
            CheckStatus::Pass,
            None,
            Some(serde_json::json!({"fallback": "/favicon.ico", "status_code": status, "content_type": content_type})),
            None,
            None,
        ),
        FaviconProbeVerdict::Missing => favicon_result(
            "No favicon declaration or conventional file found".into(),
            format!("No favicon link tag was found and /favicon.ico returned HTTP {}. The scanned HTML and conventional fallback therefore provide no icon evidence.", status),
            CheckStatus::Warn,
            Some("Choose the browser/install surfaces the product supports, deploy suitable icon files, and add accurate link declarations. Verify the final responses and appearance instead of adding an empty placeholder solely to clear the check.".into()),
            Some(serde_json::json!({"fallback": "/favicon.ico", "status_code": status, "content_type": content_type})),
            Some("Without a declared or conventional icon, browsers commonly use a generic fallback, which weakens visual identification in tabs and bookmarks.".into()),
            None,
        ),
        FaviconProbeVerdict::Review => favicon_result(
            "Conventional favicon response needs review".into(),
            format!("No favicon link tag was found. /favicon.ico returned HTTP {} with Content-Type '{}', which is not enough to confirm a usable icon response.", status, content_type.as_deref().unwrap_or("not provided")),
            CheckStatus::Warn,
            Some("Inspect /favicon.ico in browser developer tools and verify the final status, Content-Type, response bytes, decoding, and tab appearance. Add an explicit accurate icon declaration when the conventional fallback is not intentional.".into()),
            Some(serde_json::json!({"fallback": "/favicon.ico", "status_code": status, "content_type": content_type})),
            Some("The fallback response may be usable, blocked, empty, or a rewrite; this probe cannot distinguish those cases from status and Content-Type alone.".into()),
            Some("No declaration was present and the fallback response was not a clearly successful image response, so supported-browser verification is required.".into()),
        ),
    }
}

/// Reduce an outcome to (status, content type), mapping a transport failure
/// onto the runner-side skip reason the verdicts already distinguish.
fn probe_response(
    outcome: Result<ProbeOutcome, FaviconProbeSkip>,
) -> Result<(u16, Option<String>), FaviconProbeSkip> {
    match outcome {
        Err(skip) => Err(skip),
        Ok(ProbeOutcome::Failure(_)) => Err(FaviconProbeSkip::Failed),
        Ok(ProbeOutcome::Response(response)) => Ok((response.status, response.content_type)),
    }
}

fn favicon_result(
    title: String,
    description: String,
    status: CheckStatus,
    manual_fix: Option<String>,
    raw_data: Option<serde_json::Value>,
    why_it_matters: Option<String>,
    confidence_reason: Option<String>,
) -> Vec<CheckResult> {
    let confidence = if confidence_reason.is_some() {
        crate::checks::IssueConfidence::NeedsReview
    } else {
        crate::checks::IssueConfidence::High
    };
    vec![CheckResult {
        check_id: "config.favicon".into(),
        category: ScanCategory::Polish,
        title,
        description,
        status,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix,
        raw_data,
        confidence,
        confidence_reason,
        why_it_matters,
    }]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaviconProbeVerdict {
    UsableResponse,
    Missing,
    Review,
}

fn favicon_probe_verdict(status: u16, content_type: Option<&str>) -> FaviconProbeVerdict {
    if matches!(status, 404 | 410) {
        return FaviconProbeVerdict::Missing;
    }
    let image_type = content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase()
            .starts_with("image/")
    });
    if (200..300).contains(&status) && status != 204 && image_type {
        FaviconProbeVerdict::UsableResponse
    } else {
        FaviconProbeVerdict::Review
    }
}

#[cfg(test)]
#[path = "favicon_tests.rs"]
mod tests;
