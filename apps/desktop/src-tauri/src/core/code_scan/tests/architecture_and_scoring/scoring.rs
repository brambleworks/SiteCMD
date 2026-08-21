use super::super::*;
use crate::checks::IssueConfidence;

fn report_with(critical: usize, high: usize, medium: usize, low: usize) -> CodeScanReport {
    let mut issues = Vec::new();
    let mut push_tier = |severity: Severity, count: usize, tag: &str| {
        for i in 0..count {
            let id = format!("synthetic-{tag}-{i}:src/file-{tag}-{i}.ts");
            issues.push(CodeIssue {
                check_id: crate::core::code_scan::canonical_code_check_id(&id),
                id,
                category: "architecture".to_string(),
                severity,
                title: "Synthetic".to_string(),
                description: "Synthetic".to_string(),
                relative_path: format!("src/file-{tag}-{i}.ts"),
                absolute_path: format!("/tmp/src/file-{tag}-{i}.ts"),
                line: Some(1),
                source_excerpt: None,
                evidence: None,
                why_now: None,
                likely_fix: None,
                confidence: IssueConfidence::High,
                confidence_reason: None,
                verify_hint: None,
            });
        }
    };
    push_tier(Severity::Critical, critical, "critical");
    push_tier(Severity::High, high, "high");
    push_tier(Severity::Medium, medium, "medium");
    push_tier(Severity::Low, low, "low");
    CodeScanReport {
        skipped_scopes: Default::default(),
        checked_at: "2026-04-11T00:00:00Z".into(),
        framework: None,
        issue_count: critical + high + medium + low,
        critical_count: critical,
        high_count: high,
        medium_count: medium,
        low_count: low,
        issues,
    }
}

#[test]
fn score_report_clean_project_scores_100() {
    assert_eq!(score_report(&report_with(0, 0, 0, 0)), 100);
}

#[test]
fn score_report_one_high_deducts_gently() {
    // No "Good ceiling" cap any more: a lone high just deducts and lands in Good.
    assert_eq!(score_report(&report_with(0, 1, 0, 0)), 91);
}

#[test]
fn score_report_one_critical_does_not_tank() {
    assert_eq!(score_report(&report_with(1, 0, 0, 0)), 85);
    assert!(
        score_report(&report_with(5, 0, 0, 0)) < 60,
        "five criticals should land low"
    );
}

#[test]
fn score_report_zero_critical_wall_rests_on_the_poor_floor() {
    let score = score_report(&report_with(0, 16, 4, 15));
    assert_eq!(
        score, 35,
        "a zero-critical wall must rest on the 35 Poor floor, got {}",
        score,
    );
}

#[test]
fn score_report_severe_projects_still_drop_low() {
    // A wall of 50 distinct high rules still lands clearly in the red, just not
    // pinned to 0.
    let score = score_report(&report_with(0, 50, 0, 0));
    assert!(
        score < 45,
        "expected a clearly-poor score with 50 highs, got {}",
        score
    );
}

#[test]
fn score_report_realistic_first_scan_mix_does_not_floor_to_zero() {
    let score = score_report(&report_with(3, 33, 29, 5));
    assert!(
        (5..=25).contains(&score),
        "expected a severe-but-readable score for 3/33/29/5 mix, got {}",
        score,
    );
}
