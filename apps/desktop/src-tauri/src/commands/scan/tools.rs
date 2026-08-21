use crate::ai;
use crate::checks::CheckResult;
use crate::core::scanner;
use crate::db::Database;
use sitecmd_engine::checks::accessibility::axe;
use std::sync::Arc;
use tauri::{AppHandle, State};

use super::control::ScanControlState;
use crate::commands::validate_url_async;

/// Re-run only the requested checks without persisting a new scan result.
#[tauri::command]
#[tracing::instrument(skip(db, scan_control, url), fields(check_ids = ?check_ids, scan_request_id))]
pub async fn verify_scan_checks(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    scan_control: State<'_, ScanControlState>,
    project_id: Option<i64>,
    environment_url: Option<String>,
    url: String,
    check_ids: Vec<String>,
    scan_request_id: Option<u64>,
    idempotency_key: Option<String>,
) -> Result<scanner::VerifyChecksResult, scanner::ScanError> {
    super::verification::run_bounded_web_verification(
        Some(&app),
        db.inner().clone(),
        &scan_control,
        project_id,
        environment_url,
        url,
        check_ids,
        scan_request_id,
        idempotency_key,
    )
    .await
}

#[tracing::instrument(skip(app, scan_control, url), fields(check_ids = ?check_ids, scan_request_id))]
pub(crate) async fn verify_scan_checks_internal(
    app: Option<&AppHandle>,
    scan_control: &ScanControlState,
    url: String,
    check_ids: Vec<String>,
    scan_request_id: Option<u64>,
) -> Result<scanner::VerifyChecksResult, scanner::ScanError> {
    validate_url_async(&url)
        .await
        .map_err(scanner::ScanError::NetworkError)?;
    let scan_request_id = scan_control.begin_request(scan_request_id);
    let is_cancelled = || scan_control.is_cancelled(scan_request_id);
    let result = async {
        let mut result = scanner::verify_checks(&url, &check_ids, Some(&is_cancelled)).await?;
        if let Some(app) = app {
            append_browser_verification_results(app, &url, &check_ids, &mut result).await?;
        }
        ensure_verification_results_complete(&check_ids, &result.results)?;
        Ok(result)
    }
    .await;
    scan_control.finish_request(scan_request_id);
    result
}

// Shared with the synthesis pass inside `verify_checks`: both must agree on
// which producer IDs a verification is accountable for, and the set excludes
// historical aliases nothing emits anymore.
use crate::core::scanner::verify::required_web_verification_ids;

fn ensure_verification_results_complete(
    check_ids: &[String],
    results: &[CheckResult],
) -> Result<(), scanner::ScanError> {
    let observed: std::collections::BTreeSet<&str> = results
        .iter()
        .map(|result| result.check_id.as_str())
        .collect();
    let missing: Vec<String> = required_web_verification_ids(check_ids)
        .into_iter()
        .filter(|check_id| !observed.contains(check_id.as_str()))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(scanner::ScanError::ScanFailed(format!(
            "Verification produced no result for: {}",
            missing.join(", ")
        )))
    }
}

fn is_browser_verification_check(check_id: &str) -> bool {
    check_id.starts_with("accessibility.axe.")
        || matches!(
            check_id,
            "performance.lcp"
                | "performance.cls"
                | "performance.fcp"
                | "performance.ttfb"
                | "performance.long_task_blocking"
                | "polish.js-errors"
        )
}

fn result_matches_requested_check(
    result_id: &str,
    requested: &std::collections::BTreeSet<&str>,
) -> bool {
    requested.contains(result_id)
        || requested
            .contains(crate::core::correlation::resolve_check_id("web_scan", result_id).as_str())
}

async fn append_browser_verification_results(
    app: &AppHandle,
    url: &str,
    check_ids: &[String],
    result: &mut scanner::VerifyChecksResult,
) -> Result<(), scanner::ScanError> {
    let browser_ids: Vec<&str> = check_ids
        .iter()
        .map(String::as_str)
        .filter(|check_id| is_browser_verification_check(check_id))
        .collect();
    if browser_ids.is_empty() {
        return Ok(());
    }

    let include_accessibility = browser_ids
        .iter()
        .any(|check_id| check_id.starts_with("accessibility.axe."));
    let analysis = crate::webview::analyzer::analyze_url(app, url, include_accessibility).await;
    if include_accessibility {
        if let Some(error) = analysis.error.as_deref() {
            return Err(scanner::ScanError::ScanFailed(format!(
                "Rendered accessibility verification failed: {error}"
            )));
        }
    }

    let mut scan = scanner::ScanResult {
        url: url.to_string(),
        mode: "verification".to_string(),
        scan_type: scanner::ScanType::Health,
        overall_score: 100,
        categories: Vec::new(),
        issues: std::mem::take(&mut result.results),
        detected_stack: None,
        duration_ms: 0,
        timestamp: chrono::Utc::now().to_rfc3339(),
        page_signals: None,
        site_facts: None,
    };
    scanner::append_webview_results(
        &mut scan,
        analysis.accessibility.as_ref(),
        analysis.cwv.as_ref(),
    );

    // axe bucket arrays distinguish executed absences from incomplete or
    // unexecuted rules that cannot verify a fix.
    if let Some(report) = analysis.accessibility.as_ref() {
        for check_id in &browser_ids {
            let Some(rule) = axe::rule_for_check_id(check_id) else {
                continue;
            };
            if scan.issues.iter().any(|issue| issue.check_id == *check_id) {
                continue;
            }
            scan.issues.push(axe::axe_rule_coverage_result(
                rule,
                report.rule_outcome(rule),
            ));
        }
    }

    let requested: std::collections::BTreeSet<&str> =
        check_ids.iter().map(String::as_str).collect();
    scan.issues
        .retain(|issue| result_matches_requested_check(&issue.check_id, &requested));

    let missing: Vec<&str> = browser_ids
        .into_iter()
        .filter(|check_id| !scan.issues.iter().any(|issue| issue.check_id == *check_id))
        .collect();
    if !missing.is_empty() {
        return Err(scanner::ScanError::ScanFailed(format!(
            "Rendered verification produced no measurement for: {}",
            missing.join(", ")
        )));
    }

    result.results = scan.issues;
    Ok(())
}

async fn validate_webview_analysis_url(url: &str) -> Result<(), String> {
    validate_url_async(url).await
}

/// Run Layer 2 webview analysis (axe-core accessibility + Core Web Vitals) on a URL.
#[tauri::command]
#[tracing::instrument(skip(app, url))]
pub async fn run_webview_analysis(
    app: AppHandle,
    url: String,
) -> Result<crate::webview::analyzer::WebviewAnalysis, String> {
    validate_webview_analysis_url(&url).await?;
    tracing::info!(
        "Starting Layer 2 webview analysis for: {}",
        crate::log_sanitizer::log_safe_url_target(&url)
    );
    let result = crate::webview::analyzer::analyze_url(&app, &url, true).await;
    tracing::info!(
        "Webview analysis complete: {} accessibility violations, CWV: {:?}",
        result
            .accessibility
            .as_ref()
            .map_or(0, |report| report.violations.len()),
        result.cwv.is_some()
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_verification_results_complete, is_browser_verification_check,
        required_web_verification_ids, validate_webview_analysis_url,
    };
    use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

    fn pass(check_id: &str) -> CheckResult {
        CheckResult {
            check_id: check_id.to_string(),
            category: ScanCategory::Security,
            title: "Passed".to_string(),
            description: "Passed".to_string(),
            status: CheckStatus::Pass,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    }

    #[test]
    fn browser_verification_registry_covers_every_rendered_issue_family() {
        assert!(is_browser_verification_check("accessibility.axe.image-alt"));
        assert!(is_browser_verification_check("performance.lcp"));
        assert!(is_browser_verification_check("performance.cls"));
        assert!(is_browser_verification_check("performance.fcp"));
        assert!(is_browser_verification_check("performance.ttfb"));
        assert!(is_browser_verification_check(
            "performance.long_task_blocking"
        ));
        assert!(is_browser_verification_check("polish.js-errors"));
        assert!(!is_browser_verification_check("security.headers.csp"));
    }

    #[test]
    fn canonical_verification_requires_every_registered_web_producer_result() {
        let requested = vec!["security.csp".to_string()];
        let required = required_web_verification_ids(&requested);
        assert_eq!(
            required,
            std::collections::BTreeSet::from(["security.headers.csp".to_string()])
        );
        assert!(ensure_verification_results_complete(&requested, &[]).is_err());
        assert!(
            ensure_verification_results_complete(&requested, &[pass("security.headers.csp")])
                .is_ok()
        );
    }

    #[tokio::test]
    async fn security_regression_webview_analysis_rejects_private_network_targets() {
        assert!(validate_webview_analysis_url("http://192.168.1.10")
            .await
            .is_err());
        assert!(validate_webview_analysis_url("http://10.0.0.5")
            .await
            .is_err());
        assert!(validate_webview_analysis_url("http://127.0.0.1:5173")
            .await
            .is_ok());
    }
}

/// Build an AI fix prompt for a specific issue, optionally incorporating the detected tech stack.
#[tauri::command]
#[tracing::instrument(skip(issue, detected_stack, url))]
pub async fn build_prompt(
    issue: CheckResult,
    url: String,
    detected_stack: Option<serde_json::Value>,
) -> Result<String, String> {
    Ok(ai::build_fix_prompt(&issue, &url, detected_stack.as_ref()))
}

/// Build a detailed fix document for a single issue.
/// Returns a self-contained markdown guide with code examples and verification steps.
#[tauri::command]
#[tracing::instrument(skip(issue, detected_stack, url))]
pub async fn get_fix_document(
    issue: CheckResult,
    url: String,
    detected_stack: Option<serde_json::Value>,
) -> Result<String, String> {
    Ok(ai::build_fix_document(
        &issue,
        &url,
        detected_stack.as_ref(),
    ))
}

/// Export a scan result as a Markdown report string. Used for clipboard/file export.
fn render_scan_markdown(result: &scanner::ScanResult) -> String {
    let mut md = String::new();
    md.push_str("# SiteCMD Scan Report\n\n");
    md.push_str(&format!("**URL:** {}\n", result.url));
    md.push_str(&format!("**Score:** {}/100\n", result.overall_score));
    md.push_str(&format!("**Mode:** {}\n", result.mode));
    md.push_str(&format!("**Date:** {}\n", result.timestamp));
    md.push_str(&format!(
        "**Duration:** {}ms\n\n---\n\n",
        result.duration_ms
    ));

    let mut by_cat: std::collections::BTreeMap<String, Vec<&CheckResult>> =
        std::collections::BTreeMap::new();
    for issue in &result.issues {
        if issue.status == crate::checks::CheckStatus::Fail
            || issue.status == crate::checks::CheckStatus::Warn
        {
            by_cat
                .entry(format!("{:?}", issue.category))
                .or_default()
                .push(issue);
        }
    }
    for (category, issues) in &by_cat {
        md.push_str(&format!("## {}\n\n", category));
        for issue in issues {
            md.push_str(&format!(
                "### {} `{:?}`\n\n{}\n\n",
                issue.title, issue.severity, issue.description
            ));
            if let Some(fix) = &issue.manual_fix {
                md.push_str(&format!("**Fix:** {}\n\n", fix));
            }
            md.push_str("---\n\n");
        }
    }
    md.push_str(&format!(
        "\n*Generated by SiteCMD v{}*\n",
        env!("CARGO_PKG_VERSION")
    ));
    md
}

#[tauri::command]
#[tracing::instrument(skip(_db, result))]
pub async fn export_scan_markdown(
    _db: State<'_, Arc<Database>>,
    result: scanner::ScanResult,
) -> Result<String, String> {
    Ok(render_scan_markdown(&result))
}

#[cfg(test)]
mod export_tests {
    use super::*;

    #[test]
    fn markdown_footer_uses_the_build_version() {
        let result = scanner::ScanResult {
            url: "https://example.com".into(),
            mode: "live".into(),
            scan_type: scanner::ScanType::Health,
            overall_score: 100,
            categories: vec![],
            issues: vec![],
            detected_stack: None,
            duration_ms: 10,
            timestamp: "2026-08-17T00:00:00Z".into(),
            page_signals: None,
            site_facts: None,
        };

        let markdown = render_scan_markdown(&result);
        assert!(markdown.ends_with(&format!(
            "*Generated by SiteCMD v{}*\n",
            env!("CARGO_PKG_VERSION")
        )));
        assert!(!markdown.contains("v0.1.0"));
    }
}
