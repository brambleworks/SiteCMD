use std::collections::HashMap;

use crate::checks::Severity;
use crate::core::code_scan::{code_issue_domain, code_scan_domain_rank, CodeIssue};
use crate::db::{CodeScanDomainSummary, CodeScanSummary};

/// Group code issues into the domain summaries returned with scan results.
#[tracing::instrument(skip(issues))]
pub(crate) fn build_domain_summaries(issues: &[CodeIssue]) -> Vec<CodeScanDomainSummary> {
    let mut buckets: HashMap<crate::core::code_scan::CodeScanDomain, (u32, u32, u32, u32, u32)> =
        HashMap::new();
    for issue in issues {
        let entry = buckets
            .entry(code_issue_domain(issue))
            .or_insert((0, 0, 0, 0, 0));
        entry.0 += 1;
        match issue.severity {
            Severity::Critical => entry.1 += 1,
            Severity::High => entry.2 += 1,
            Severity::Medium => entry.3 += 1,
            Severity::Low => entry.4 += 1,
        }
    }
    buckets
        .into_iter()
        .map(
            |(domain, (issue_count, critical_count, high_count, medium_count, low_count))| {
                CodeScanDomainSummary {
                    domain,
                    issue_count,
                    critical_count,
                    high_count,
                    medium_count,
                    low_count,
                }
            },
        )
        .collect()
}

/// Select the highest-count summary, breaking ties by canonical domain order.
#[tracing::instrument(skip(summaries))]
pub(crate) fn top_code_scan_domain_from_summaries(
    summaries: &[CodeScanDomainSummary],
) -> Option<(crate::core::code_scan::CodeScanDomain, usize)> {
    summaries
        .iter()
        .map(|summary| (summary.domain, summary.issue_count as usize))
        .max_by_key(|(domain, count)| (*count, std::cmp::Reverse(code_scan_domain_rank(*domain))))
}

#[tracing::instrument(skip(history, environment_url))]
pub(crate) fn select_relevant_previous_code_scan_summary(
    history: Vec<CodeScanSummary>,
    environment_url: Option<&str>,
) -> Option<CodeScanSummary> {
    let normalized_target = environment_url.map(|value| value.trim_end_matches('/'));
    let mut project_wide = None;
    let mut other = None;

    for entry in history {
        match (
            normalized_target,
            entry
                .environment_url
                .as_deref()
                .map(|value| value.trim_end_matches('/')),
        ) {
            (Some(target), Some(entry_url)) if entry_url == target => return Some(entry),
            (_, None) if project_wide.is_none() => project_wide = Some(entry),
            _ if other.is_none() => other = Some(entry),
            _ => {}
        }
    }

    project_wide.or(other)
}

fn code_scan_domain_label(domain: crate::core::code_scan::CodeScanDomain) -> &'static str {
    match domain {
        crate::core::code_scan::CodeScanDomain::Database => "Database Analysis",
        crate::core::code_scan::CodeScanDomain::AiSafety => "AI Safety",
        crate::core::code_scan::CodeScanDomain::Security => "Security",
        crate::core::code_scan::CodeScanDomain::Architecture => "Architecture",
        crate::core::code_scan::CodeScanDomain::Operations => "Operations",
        crate::core::code_scan::CodeScanDomain::SupplyChain => "Dependencies",
        crate::core::code_scan::CodeScanDomain::AiScaffolding => "AI Setup",
    }
}

#[tracing::instrument(skip(current, previous, current_top_domain, previous_top_domain))]
pub(crate) fn describe_code_scan_domain_trend(
    current: &[CodeScanDomainSummary],
    previous: &[CodeScanDomainSummary],
    current_top_domain: Option<crate::core::code_scan::CodeScanDomain>,
    previous_top_domain: Option<crate::core::code_scan::CodeScanDomain>,
) -> Option<String> {
    use crate::core::code_scan::CodeScanDomain;

    if current.is_empty() && previous.is_empty() {
        return None;
    }

    let get_count = |summaries: &[CodeScanDomainSummary], domain: CodeScanDomain| -> i32 {
        summaries
            .iter()
            .find(|summary| summary.domain == domain)
            .map(|summary| summary.issue_count as i32)
            .unwrap_or(0)
    };

    let mut strongest_domain = None;
    let mut strongest_delta: i32 = 0;

    for domain in [
        CodeScanDomain::Database,
        CodeScanDomain::AiSafety,
        CodeScanDomain::Security,
        CodeScanDomain::Architecture,
        CodeScanDomain::Operations,
        CodeScanDomain::SupplyChain,
        CodeScanDomain::AiScaffolding,
    ] {
        let delta = get_count(current, domain) - get_count(previous, domain);
        if delta.abs() > strongest_delta.abs() {
            strongest_domain = Some(domain);
            strongest_delta = delta;
        }
    }

    if let Some(domain) = strongest_domain {
        if strongest_delta > 0 {
            return Some(format!(
                "{} grew by {}",
                code_scan_domain_label(domain),
                strongest_delta
            ));
        }
        if strongest_delta < 0 {
            return Some(format!(
                "{} eased by {}",
                code_scan_domain_label(domain),
                strongest_delta.abs()
            ));
        }
    }

    match (current_top_domain, previous_top_domain) {
        (Some(current_top_domain), Some(previous_top_domain))
            if current_top_domain != previous_top_domain =>
        {
            Some(format!(
                "{} is now leading",
                code_scan_domain_label(current_top_domain)
            ))
        }
        _ => Some("Domain mix stable".to_string()),
    }
}
