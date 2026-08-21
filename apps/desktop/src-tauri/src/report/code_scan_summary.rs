use crate::checks::Severity;
#[cfg(feature = "desktop")]
use crate::commands::scan::describe_code_scan_domain_trend;
use crate::core::code_scan::CodeScanDomain;
use crate::db::Database;

use super::{CodeScanDomainSummary, CodeScanSummary, ReportIssue};

fn normalize_report_url(url: &str) -> &str {
    url.trim_end_matches('/')
}

fn code_scan_domain_label(domain: CodeScanDomain) -> &'static str {
    match domain {
        CodeScanDomain::Database => "Database Analysis",
        CodeScanDomain::AiSafety => "AI Safety",
        CodeScanDomain::Security => "Security",
        CodeScanDomain::Architecture => "Architecture",
        CodeScanDomain::Operations => "Operations",
        CodeScanDomain::SupplyChain => "Dependencies",
        CodeScanDomain::AiScaffolding => "AI Setup",
    }
}

#[cfg(feature = "desktop")]
pub(super) fn build_code_scan_report_summary(
    db: &Database,
    project_id: i64,
    site_url: &str,
) -> Result<Option<CodeScanSummary>, String> {
    let normalized_site_url = normalize_report_url(site_url);
    let mut exact_code_history = Vec::new();
    let mut project_wide_code_history = Vec::new();
    for summary in db.get_code_scan_history(project_id, 50)? {
        match summary.environment_url.as_deref() {
            Some(url) if normalize_report_url(url) == normalized_site_url => {
                exact_code_history.push(summary)
            }
            None => project_wide_code_history.push(summary),
            _ => {}
        }
    }

    let relevant_code_history = if !exact_code_history.is_empty() {
        exact_code_history
    } else {
        project_wide_code_history
    };

    let Some(latest_code_scan) = relevant_code_history.first() else {
        return Ok(None);
    };
    let detail = db.get_code_scan_detail(latest_code_scan.id)?;
    let Some(detail) = detail else {
        return Ok(None);
    };

    let previous_code_scan = relevant_code_history.get(1);
    let previous_score = relevant_code_history
        .get(1)
        .map(|entry| entry.overall_score);
    let code_trend = match previous_score {
        Some(prev) if detail.overall_score > prev => "up",
        Some(prev) if detail.overall_score < prev => "down",
        _ => "stable",
    };
    let domain_trend = previous_code_scan.and_then(|previous| {
        describe_code_scan_domain_trend(
            &detail.domain_summaries,
            &previous.domain_summaries,
            latest_code_scan.top_domain,
            previous.top_domain,
        )
    });

    let mut domains: Vec<_> = detail
        .domain_summaries
        .iter()
        .map(|entry| CodeScanDomainSummary {
            name: code_scan_domain_label(entry.domain).to_string(),
            issue_count: entry.issue_count,
        })
        .collect();
    domains.sort_by(|a, b| {
        b.issue_count
            .cmp(&a.issue_count)
            .then_with(|| a.name.cmp(&b.name))
    });

    let top_issues = detail
        .issues
        .iter()
        .filter(|issue| matches!(issue.severity, Severity::Critical | Severity::High))
        .take(10)
        .map(|issue| ReportIssue {
            title: issue.title.clone(),
            category: format!(
                "{} · {}",
                code_scan_domain_label(issue.domain),
                issue.category
            ),
            severity: issue.severity,
            description: issue.description.clone(),
        })
        .collect();

    Ok(Some(CodeScanSummary {
        current_score: detail.overall_score,
        previous_score,
        trend: code_trend.to_string(),
        issue_count: detail.issue_count,
        critical_count: detail.critical_count,
        high_count: detail.high_count,
        medium_count: detail.medium_count,
        low_count: detail.low_count,
        checked_at: detail.checked_at,
        framework: detail.framework,
        top_domain: latest_code_scan
            .top_domain
            .map(|domain| code_scan_domain_label(domain).to_string()),
        top_domain_count: latest_code_scan.top_domain_count,
        domain_trend,
        domains,
        top_issues,
    }))
}
