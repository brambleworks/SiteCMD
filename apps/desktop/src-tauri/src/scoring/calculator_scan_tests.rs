use super::*;
use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};

fn make_result(category: ScanCategory, severity: Severity, status: CheckStatus) -> CheckResult {
    CheckResult {
        check_id: "test".to_string(),
        title: "Test".to_string(),
        description: "Test check".to_string(),
        category,
        severity,
        status,
        manual_fix: None,
        fix_prompt: None,
        raw_data: None,
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

#[test]
fn needs_review_exploitable_does_not_cap_the_snapshot_score() {
    let mut needs_review = make_result(
        ScanCategory::Security,
        Severity::Critical,
        CheckStatus::Fail,
    );
    needs_review.check_id = "code_scan.js-command-injection".to_string();
    needs_review.confidence = crate::checks::IssueConfidence::NeedsReview;
    let (uncapped, _) = calculate_scores(&[needs_review.clone()]);
    assert!(
        uncapped > 49,
        "needs_review exploitable must not cap the snapshot score, was {uncapped}"
    );

    let mut confirmed = needs_review;
    confirmed.confidence = crate::checks::IssueConfidence::Confirmed;
    let (capped, _) = calculate_scores(&[confirmed]);
    assert!(capped <= 49, "confirmed exploitable must cap, was {capped}");
}

#[test]
fn test_all_pass_scores_100() {
    let results = vec![
        make_result(ScanCategory::Security, Severity::Low, CheckStatus::Pass),
        make_result(ScanCategory::Performance, Severity::Low, CheckStatus::Pass),
        make_result(ScanCategory::Seo, Severity::Low, CheckStatus::Pass),
    ];
    let (overall, _) = calculate_scores(&results);
    assert_eq!(overall, 100);
}

#[test]
fn test_one_critical_deducts_but_does_not_tank() {
    let results = vec![
        make_result(
            ScanCategory::Security,
            Severity::Critical,
            CheckStatus::Fail,
        ),
        make_result(ScanCategory::Performance, Severity::Low, CheckStatus::Pass),
        make_result(ScanCategory::Seo, Severity::Low, CheckStatus::Pass),
        make_result(ScanCategory::Compliance, Severity::Low, CheckStatus::Pass),
        make_result(
            ScanCategory::Accessibility,
            Severity::Low,
            CheckStatus::Pass,
        ),
        make_result(ScanCategory::Polish, Severity::Low, CheckStatus::Pass),
    ];
    let (overall, _) = calculate_scores(&results);
    assert_eq!(
        overall, 85,
        "one critical should not tank the score, got {}",
        overall
    );
}

#[test]
fn test_one_high_deducts_gently() {
    let results = vec![
        make_result(ScanCategory::Security, Severity::High, CheckStatus::Fail),
        make_result(ScanCategory::Performance, Severity::Low, CheckStatus::Pass),
        make_result(ScanCategory::Seo, Severity::Low, CheckStatus::Pass),
        make_result(ScanCategory::Compliance, Severity::Low, CheckStatus::Pass),
        make_result(
            ScanCategory::Accessibility,
            Severity::Low,
            CheckStatus::Pass,
        ),
        make_result(ScanCategory::Polish, Severity::Low, CheckStatus::Pass),
    ];
    let (overall, _) = calculate_scores(&results);
    assert_eq!(
        overall, 91,
        "one high should deduct gently, got {}",
        overall
    );
}

#[test]
fn test_warn_applies_half_penalty() {
    let pass_only = vec![make_result(
        ScanCategory::Security,
        Severity::Medium,
        CheckStatus::Pass,
    )];
    let with_warn = vec![make_result(
        ScanCategory::Security,
        Severity::Medium,
        CheckStatus::Warn,
    )];
    let (score_pass, _) = calculate_scores(&pass_only);
    let (score_warn, _) = calculate_scores(&with_warn);
    assert!(score_warn < score_pass, "Warn should reduce score vs Pass");
}

#[test]
fn warn_contributes_half_weight_to_the_overall_curve() {
    let warns: Vec<CheckResult> = (0..4)
        .map(|_| make_result(ScanCategory::Security, Severity::High, CheckStatus::Warn))
        .collect();
    let fails: Vec<CheckResult> = (0..4)
        .map(|_| make_result(ScanCategory::Security, Severity::High, CheckStatus::Fail))
        .collect();
    let (warn_overall, _) = calculate_scores(&warns);
    let (fail_overall, _) = calculate_scores(&fails);
    assert!(
        warn_overall > fail_overall,
        "4 warns ({warn_overall}) must score strictly higher than 4 fails ({fail_overall})"
    );
    // 4 High warns = 4 * 0.5 = 2.0 effective high; 4 High fails = 4.0 effective.
    assert_eq!(
        warn_overall,
        health_score_from_severity(0.0, 2.0, 0.0, 0.0, false, false)
    );
    assert_eq!(
        fail_overall,
        health_score_from_severity(0.0, 4.0, 0.0, 0.0, false, false)
    );
}

#[test]
fn needs_review_warn_composes_confidence_and_status_weights_to_a_quarter() {
    let mut warn = make_result(ScanCategory::Security, Severity::High, CheckStatus::Warn);
    warn.confidence = crate::checks::IssueConfidence::NeedsReview;
    let (overall, _) = calculate_scores(std::slice::from_ref(&warn));
    assert_eq!(
        overall,
        health_score_from_severity(0.0, 0.25, 0.0, 0.0, false, false),
        "a NeedsReview Warn is 0.5 (confidence) * 0.5 (warn) = 0.25 effective high"
    );
}

#[test]
fn test_higher_severity_reduces_overall_more() {
    let medium_fail = vec![make_result(
        ScanCategory::Security,
        Severity::Medium,
        CheckStatus::Fail,
    )];
    let low_fail = vec![make_result(
        ScanCategory::Security,
        Severity::Low,
        CheckStatus::Fail,
    )];
    let (medium_score, _) = calculate_scores(&medium_fail);
    let (low_score, _) = calculate_scores(&low_fail);
    assert!(
        medium_score < low_score,
        "a medium ({medium_score}) should reduce the score more than a low ({low_score})"
    );
}

#[test]
fn test_empty_results() {
    let (overall, categories) = calculate_scores(&[]);
    assert_eq!(overall, 0);
    assert!(categories.is_empty());
}

#[test]
fn test_category_score_counts() {
    let results = vec![
        make_result(ScanCategory::Seo, Severity::Critical, CheckStatus::Fail),
        make_result(ScanCategory::Seo, Severity::High, CheckStatus::Fail),
        make_result(ScanCategory::Seo, Severity::Medium, CheckStatus::Warn),
        make_result(ScanCategory::Seo, Severity::Low, CheckStatus::Pass),
    ];
    let (_, cats) = calculate_scores(&results);
    let seo = cats
        .iter()
        .find(|c| c.category == ScanCategory::Seo)
        .expect("Seo category must be present in test results");
    assert_eq!(seo.issues_critical, 1);
    assert_eq!(seo.issues_high, 1);
    assert_eq!(seo.issues_medium, 1);
    assert_eq!(seo.issues_passed, 1);
    assert_eq!(seo.issues_total, 3);
}

#[test]
fn test_category_score_floors_at_score_floor_not_zero() {
    let results: Vec<CheckResult> = (0..10)
        .map(|_| {
            make_result(
                ScanCategory::Security,
                Severity::Critical,
                CheckStatus::Fail,
            )
        })
        .collect();
    let (_, cats) = calculate_scores(&results);
    let sec = cats
        .iter()
        .find(|c| c.category == ScanCategory::Security)
        .expect("Security category must be present in test results");
    assert_eq!(sec.score, 5);
}
