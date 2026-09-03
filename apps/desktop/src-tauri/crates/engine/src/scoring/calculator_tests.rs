use super::*;
use crate::vocab::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

fn result(category: ScanCategory, status: CheckStatus, severity: Severity) -> CheckResult {
    CheckResult {
        check_id: "compliance.ccpa_notice".into(),
        category,
        title: "t".into(),
        description: "d".into(),
        status,
        severity,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: None,
        why_it_matters: None,
    }
}

#[test]
fn a_category_with_an_open_low_issue_never_reads_as_a_perfect_100() {
    // A Warn at NeedsReview confidence is an effective count of 0.25, whose
    // deduction rounds away. "100" beside "1 issue" is not a verdict SiteCMD
    // can defend, so the ceiling holds it at 99.
    let (overall, categories) = calculate_scores(&[result(
        ScanCategory::Compliance,
        CheckStatus::Warn,
        Severity::Low,
    )]);
    let compliance = categories
        .iter()
        .find(|c| c.category == ScanCategory::Compliance)
        .expect("compliance bar");
    assert_eq!(compliance.issues_total, 1);
    assert_eq!(compliance.score, 99);
    assert_eq!(overall, 99);
}

#[test]
fn a_category_with_only_passes_still_scores_100() {
    let (overall, categories) = calculate_scores(&[result(
        ScanCategory::Compliance,
        CheckStatus::Pass,
        Severity::Low,
    )]);
    assert_eq!(categories[0].score, 100);
    assert_eq!(categories[0].issues_total, 0);
    assert_eq!(overall, 100);
}

#[test]
fn the_breakdown_reports_the_ceiling_only_when_it_moved_the_score() {
    // A Warn at NeedsReview confidence is 0.25, whose 0.42 points round back to
    // 100, so the ceiling is what produces the 99 and the breakdown says so.
    let (held, breakdown) = health_score_with_breakdown(0.0, 0.0, 0.0, 0.25, false, false);
    assert_eq!(held, 99);
    assert!(breakdown.ceiling_applied);

    // The lightest live-score group weighs 0.5, which deducts 0.80 and reaches
    // 99 on its own. Reporting the ceiling there would name the wrong reason.
    let (arithmetic, breakdown) = health_score_with_breakdown(0.0, 0.0, 0.0, 0.5, false, false);
    assert_eq!(arithmetic, 99);
    assert!(!breakdown.ceiling_applied);

    // Nothing open, nothing to hold down.
    let (clean, breakdown) = health_score_with_breakdown(0.0, 0.0, 0.0, 0.0, false, false);
    assert_eq!(clean, 100);
    assert!(!breakdown.ceiling_applied);

    // A capped or floored score is explained by the cap or the floor.
    let (capped, breakdown) = health_score_with_breakdown(1.0, 0.0, 0.0, 0.0, true, true);
    assert_eq!(capped, 49);
    assert!(!breakdown.ceiling_applied);
    let (floored, breakdown) = health_score_with_breakdown(0.0, 20.0, 0.0, 0.0, false, false);
    assert_eq!(floored, 35);
    assert!(breakdown.floor_applied);
    assert!(!breakdown.ceiling_applied);
}

#[test]
fn the_ceiling_does_not_disturb_the_deduction_curve() {
    // One confirmed high still deducts the documented 9 points.
    assert_eq!(
        health_score_from_severity(0.0, 1.0, 0.0, 0.0, false, false),
        91
    );
    // The exploitable cap still wins over the open-issue ceiling.
    assert_eq!(
        health_score_from_severity(1.0, 0.0, 0.0, 0.0, true, true),
        49
    );
    // And the zero-critical floor still raises a deep advisory backlog.
    assert_eq!(
        health_score_from_severity(0.0, 20.0, 0.0, 0.0, false, false),
        35
    );
}
