//! Shared projection count helpers used while normalizing scan results.

use std::collections::HashMap;

use crate::checks::{CheckResult, CheckStatus, Severity};

/// Severity counts after grouping by `check_id` (keep the highest severity per id).
/// Matches the frontend's grouped view used by Dashboard, Issues, and sidebar badges.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct GroupedCounts {
    pub(crate) total: u32,
    pub(crate) critical: u32,
    pub(crate) high: u32,
    pub(crate) medium: u32,
    pub(crate) low: u32,
}

pub(crate) fn grouped_issue_counts(issues: &[CheckResult]) -> GroupedCounts {
    let mut by_check: HashMap<&str, Severity> = HashMap::new();
    for issue in issues {
        if issue.status != CheckStatus::Fail && issue.status != CheckStatus::Warn {
            continue;
        }
        by_check
            .entry(&issue.check_id)
            .and_modify(|current| {
                if issue.severity.sort_rank() < current.sort_rank() {
                    *current = issue.severity;
                }
            })
            .or_insert(issue.severity);
    }
    let mut counts = GroupedCounts::default();
    for severity in by_check.into_values() {
        counts.total += 1;
        match severity {
            Severity::Critical => counts.critical += 1,
            Severity::High => counts.high += 1,
            Severity::Medium => counts.medium += 1,
            Severity::Low => counts.low += 1,
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::ScanCategory;

    fn issue(check_id: &str, severity: Severity, status: CheckStatus) -> CheckResult {
        CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Security,
            title: String::new(),
            description: String::new(),
            status,
            severity,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    }

    #[test]
    fn grouped_counts_collapse_duplicate_check_ids() {
        let issues = vec![
            issue("security.hsts", Severity::High, CheckStatus::Fail),
            issue("security.hsts", Severity::High, CheckStatus::Fail),
            issue("security.csp", Severity::Critical, CheckStatus::Fail),
            issue("seo.title", Severity::Low, CheckStatus::Warn),
            issue("perf.ttfb", Severity::Medium, CheckStatus::Pass),
        ];
        let counts = grouped_issue_counts(&issues);
        assert_eq!(counts.total, 3);
        assert_eq!(counts.critical, 1);
        assert_eq!(counts.high, 1);
        assert_eq!(counts.medium, 0);
        assert_eq!(counts.low, 1);
    }

    #[test]
    fn grouped_counts_keep_highest_severity_per_check() {
        let issues = vec![
            issue("security.cookie", Severity::Low, CheckStatus::Fail),
            issue("security.cookie", Severity::Critical, CheckStatus::Fail),
            issue("security.cookie", Severity::Medium, CheckStatus::Fail),
        ];
        let counts = grouped_issue_counts(&issues);
        assert_eq!(counts.total, 1);
        assert_eq!(counts.critical, 1);
        assert_eq!(counts.high, 0);
        assert_eq!(counts.medium, 0);
        assert_eq!(counts.low, 0);
    }

    #[test]
    fn grouped_counts_ignore_pass_and_skipped() {
        let issues = vec![
            issue("a", Severity::High, CheckStatus::Pass),
            issue("b", Severity::High, CheckStatus::Skipped),
            issue("c", Severity::High, CheckStatus::Fail),
        ];
        let counts = grouped_issue_counts(&issues);
        assert_eq!(counts.total, 1);
        assert_eq!(counts.high, 1);
    }
}
