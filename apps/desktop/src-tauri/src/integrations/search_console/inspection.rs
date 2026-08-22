use super::types::{CanonicalInspection, IndexCoverageIssue, UrlInspectionResult};

/// Calls URL Inspection API to get Google's indexing status for a specific page.
/// Requires `https://www.googleapis.com/auth/webmasters.readonly` OAuth scope.
#[tracing::instrument(skip(oauth_token, site_url, page_url))]
pub async fn fetch_url_inspection(
    oauth_token: &str,
    site_url: &str,
    page_url: &str,
) -> Result<UrlInspectionResult, String> {
    let client = crate::http_client::client();
    let resp = client
        .post("https://searchconsole.googleapis.com/v1/urlInspection/index:inspect")
        .bearer_auth(oauth_token)
        .json(&serde_json::json!({
            "inspectionUrl": page_url,
            "siteUrl": site_url,
        }))
        .send()
        .await
        .map_err(|e| format!("GSC URL inspection request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = crate::http_client::read_text_limited(
            resp,
            crate::constants::INTEGRATION_ERROR_BODY_MAX_BYTES,
            crate::constants::BODY_READ_TIMEOUT,
        )
        .await
        .unwrap_or_default();
        return Err(format!("GSC URL inspection returned {}: {}", status, body));
    }

    let json: serde_json::Value = crate::http_client::read_json_limited(
        resp,
        crate::constants::GOOGLE_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await
    .map_err(|e| format!("Failed to parse GSC URL inspection response: {}", e))?;

    parse_url_inspection(&json, page_url)
}

#[tracing::instrument(skip(json, page_url))]
pub(crate) fn parse_url_inspection(
    json: &serde_json::Value,
    page_url: &str,
) -> Result<UrlInspectionResult, String> {
    let index_result = json.pointer("/inspectionResult/indexStatusResult");
    let mobile_result = json.pointer("/inspectionResult/mobileUsabilityResult");

    let verdict = index_result
        .and_then(|v| v.get("verdict"))
        .and_then(|v| v.as_str())
        .unwrap_or("NEUTRAL")
        .to_string();

    let coverage_state = index_result
        .and_then(|v| v.get("coverageState"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let indexing_state = index_result
        .and_then(|v| v.get("indexingState"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let page_fetch_state = index_result
        .and_then(|v| v.get("pageFetchState"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let robots_txt_state = index_result
        .and_then(|v| v.get("robotsTxtState"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let last_crawl_time = index_result
        .and_then(|v| v.get("lastCrawlTime"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let mobile_friendly = mobile_result
        .and_then(|v| v.get("verdict"))
        .and_then(|v| v.as_str())
        .map(|s| s == "MOBILE_FRIENDLY");

    let mobile_usability_issues = mobile_result
        .and_then(|v| v.get("issues"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|i| {
                    i.get("issueType")
                        .and_then(|t| t.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    let canonical_inspection = {
        let user = index_result
            .and_then(|v| v.get("userCanonical"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let google = index_result
            .and_then(|v| v.get("googleCanonical"))
            .and_then(|v| v.as_str())
            .map(String::from);
        match (&user, &google) {
            (Some(u), Some(g)) if !u.is_empty() && !g.is_empty() => Some(CanonicalInspection {
                mismatch: u != g,
                user_canonical: user,
                google_canonical: google,
            }),
            _ => None,
        }
    };

    Ok(UrlInspectionResult {
        page_url: page_url.to_string(),
        verdict,
        coverage_state,
        indexing_state,
        page_fetch_state,
        robots_txt_state,
        last_crawl_time,
        mobile_friendly,
        mobile_usability_issues,
        canonical_inspection,
    })
}

/// Inspects a batch of URLs and returns index-coverage issues.
/// Rate-limited: 100ms delay between calls to stay under GSC's 2000/day quota.
#[tracing::instrument(skip(oauth_token, urls, site_url), fields(url_count = urls.len()))]
pub async fn fetch_index_coverage_issues(
    oauth_token: &str,
    site_url: &str,
    urls: &[String],
) -> Result<Vec<IndexCoverageIssue>, String> {
    let mut issues = Vec::new();
    // An all-failed batch is an error, not evidence that coverage issues cleared.
    let mut any_ok = false;
    let mut last_err: Option<String> = None;
    for url in urls {
        match fetch_url_inspection(oauth_token, site_url, url).await {
            Ok(insp) => {
                any_ok = true;
                if let Some(reason) = classify_inspection(&insp) {
                    issues.push(IndexCoverageIssue {
                        page_url: insp.page_url.clone(),
                        reason: reason.to_string(),
                        detail: Some(insp.coverage_state.clone()),
                    });
                }
                if let Some(canon) = &insp.canonical_inspection {
                    if canon.mismatch {
                        issues.push(IndexCoverageIssue {
                            page_url: insp.page_url.clone(),
                            reason: "canonical-mismatch".into(),
                            detail: Some(format!(
                                "user: {}, google: {}",
                                canon.user_canonical.as_deref().unwrap_or(""),
                                canon.google_canonical.as_deref().unwrap_or(""),
                            )),
                        });
                    }
                }
                for m in &insp.mobile_usability_issues {
                    let reason = match m.as_str() {
                        "USES_INCOMPATIBLE_PLUGINS" | "CONTENT_WIDER_THAN_SCREEN" => {
                            "content-wider-than-screen"
                        }
                        "CLICKABLE_ELEMENTS_TOO_CLOSE" | "TAP_TARGETS_TOO_CLOSE" => {
                            "touch-target-size"
                        }
                        "SMALL_FONT_SIZE" => "text-too-small",
                        "VIEWPORT_NOT_CONFIGURED" => "mobile-viewport",
                        _ => continue,
                    };
                    issues.push(IndexCoverageIssue {
                        page_url: insp.page_url.clone(),
                        reason: reason.into(),
                        detail: None,
                    });
                }
            }
            Err(e) => {
                tracing::warn!("gsc: url inspection for {} failed: {}", url, e);
                last_err = Some(e);
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    // Every URL failed: surface the error so the caller does not mistake a
    // total fetch failure for "no issues found".
    if !any_ok {
        if let Some(err) = last_err {
            return Err(err);
        }
    }
    Ok(issues)
}

#[tracing::instrument(skip(insp))]
pub(crate) fn classify_inspection(insp: &UrlInspectionResult) -> Option<&'static str> {
    if matches!(
        insp.indexing_state.as_deref(),
        Some("BLOCKED_BY_ROBOTS_TXT")
    ) {
        return Some("blocked-by-robots");
    }
    let state = insp.coverage_state.as_str();
    if state.is_empty() {
        return None;
    }
    if state.contains("not indexed") || state.contains("Not indexed") {
        return Some("not-indexed");
    }
    if state.contains("crawl error") || state.contains("Crawl error") {
        return Some("crawl-error");
    }
    if state.contains("Duplicate") {
        return Some("duplicate-no-canonical");
    }
    None
}
