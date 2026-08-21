use crate::checks::{CheckResult, CheckStatus, Severity};

// Severity counts include only active findings, never passing checks.
fn active_issue_count_by_severity(issues: &[CheckResult], severity: Severity) -> usize {
    issues
        .iter()
        .filter(|issue| {
            matches!(issue.status, CheckStatus::Fail | CheckStatus::Warn)
                && issue.severity == severity
        })
        .count()
}

fn issue(status: CheckStatus, severity: Severity) -> CheckResult {
    CheckResult {
        check_id: "security.test-check".into(),
        category: crate::checks::ScanCategory::Security,
        title: "Test check".into(),
        description: "Test check".into(),
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
fn active_issue_count_by_severity_excludes_passing_and_skipped_checks() {
    let issues = vec![
        issue(CheckStatus::Pass, Severity::Critical),
        issue(CheckStatus::Pass, Severity::High),
        issue(CheckStatus::Skipped, Severity::Critical),
        issue(CheckStatus::Fail, Severity::Critical),
        issue(CheckStatus::Warn, Severity::High),
        issue(CheckStatus::Fail, Severity::Medium),
    ];

    assert_eq!(
        active_issue_count_by_severity(&issues, Severity::Critical),
        1
    );
    assert_eq!(active_issue_count_by_severity(&issues, Severity::High), 1);

    let all_passing = vec![
        issue(CheckStatus::Pass, Severity::Critical),
        issue(CheckStatus::Pass, Severity::Critical),
    ];
    assert_eq!(
        active_issue_count_by_severity(&all_passing, Severity::Critical),
        0
    );
}
