use super::*;
use crate::browser::{AxeNodeEvidence, AxeViolation};

fn violation(id: &str, impact: &str) -> AxeViolation {
    AxeViolation {
        id: id.into(),
        impact: impact.into(),
        description: "Images must have alternate text".into(),
        help: "Images must have alternate text".into(),
        help_url: "https://dequeuniversity.com/rules/axe/4.11/image-alt".into(),
        nodes_count: 3,
        nodes: vec![AxeNodeEvidence {
            target: vec!["main img".into()],
            html: "<img src=\"hero.png\">".into(),
            failure_summary: Some("Fix any of the following: Element has no alt attribute".into()),
        }],
    }
}

#[test]
fn impact_maps_to_status_and_severity() {
    assert_eq!(
        axe_violation_result(&violation("image-alt", "critical")).severity,
        Severity::Critical
    );
    assert_eq!(
        axe_violation_result(&violation("image-alt", "critical")).status,
        CheckStatus::Fail
    );
    assert_eq!(
        axe_violation_result(&violation("image-alt", "serious")).status,
        CheckStatus::Fail
    );
    let moderate = axe_violation_result(&violation("image-alt", "moderate"));
    assert_eq!(moderate.status, CheckStatus::Warn);
    assert_eq!(moderate.severity, Severity::Medium);
    let minor = axe_violation_result(&violation("image-alt", "minor"));
    assert_eq!(minor.severity, Severity::Low);
}

#[test]
fn a_violation_names_its_rule_and_first_selector() {
    let result = axe_violation_result(&violation("image-alt", "critical"));
    assert_eq!(result.check_id, "accessibility.axe.image-alt");
    assert!(result.description.contains("3 rendered elements"));
    assert!(result
        .description
        .contains("First affected selector: `main img`"));
    let fix = result.manual_fix.expect("fix guidance");
    assert!(fix.starts_with("For `main img`: "));
    assert!(fix.contains("all 3 affected elements"));
}

#[test]
fn one_affected_element_reads_singular() {
    let result = axe_violation_result(&AxeViolation {
        nodes_count: 1,
        ..violation("label", "serious")
    });
    assert!(result.description.contains("1 rendered element that fail"));
    assert!(result
        .manual_fix
        .expect("fix guidance")
        .contains("all 1 affected element."));
}

#[test]
fn a_proved_rule_passes_and_every_other_outcome_does_not() {
    // The coverage rule the four buckets exist to enforce: only a rule that
    // executed and cleared the page may be reported as a pass.
    assert_eq!(
        axe_rule_coverage_result("image-alt", RuleOutcome::Proved).status,
        CheckStatus::Pass
    );
    assert_eq!(
        axe_rule_coverage_result("color-contrast", RuleOutcome::Undecided).status,
        CheckStatus::Skipped
    );
    assert_eq!(
        axe_rule_coverage_result("image-alt", RuleOutcome::NotRun).status,
        CheckStatus::Skipped
    );
    assert_eq!(
        axe_rule_coverage_result("image-alt", RuleOutcome::Violated).status,
        CheckStatus::Fail
    );
}

#[test]
fn a_coverage_row_records_which_outcome_produced_it() {
    let row = axe_rule_coverage_result("color-contrast", RuleOutcome::Undecided);
    assert_eq!(row.check_id, "accessibility.axe.color-contrast");
    let raw = row.raw_data.expect("raw data");
    assert_eq!(raw["rule_outcome"], "undecided");
    assert!(row.description.contains("could not decide"));
}

#[test]
fn a_rule_supersedes_its_first_party_twin_only_when_it_reached_a_verdict() {
    let proved = AxeReport {
        passes: vec!["image-alt".into()],
        ..AxeReport::default()
    };
    assert_eq!(
        superseded_first_party_check_ids(&proved),
        vec!["accessibility.image_alt"]
    );

    let violated = AxeReport {
        violations: vec![violation("color-contrast", "serious")],
        ..AxeReport::default()
    };
    assert_eq!(
        superseded_first_party_check_ids(&violated),
        vec!["accessibility.color_contrast_hints"]
    );
}

#[test]
fn an_undecided_or_absent_rule_leaves_the_first_party_check_in_place() {
    // Dropping the twin here would delete the page's only coverage for the
    // defect and call the result clean.
    let report = AxeReport {
        incomplete: vec!["color-contrast".into()],
        passes: vec!["link-name".into()],
        ..AxeReport::default()
    };
    let superseded = superseded_first_party_check_ids(&report);
    assert_eq!(superseded, vec!["accessibility.link_text"]);
    assert!(!superseded.contains(&"accessibility.color_contrast_hints"));
    assert!(!superseded.contains(&"accessibility.lang"));
}

#[test]
fn either_language_rule_supersedes_the_language_check() {
    for rule in ["html-has-lang", "valid-lang"] {
        let report = AxeReport {
            passes: vec![rule.into()],
            ..AxeReport::default()
        };
        assert_eq!(
            superseded_first_party_check_ids(&report),
            vec!["accessibility.lang"],
            "{rule} supersedes the first-party language check"
        );
    }
}

#[test]
fn check_ids_round_trip_through_the_rule_prefix() {
    assert_eq!(
        check_id_for_rule("image-alt"),
        "accessibility.axe.image-alt"
    );
    assert_eq!(
        rule_for_check_id("accessibility.axe.image-alt"),
        Some("image-alt")
    );
    assert_eq!(rule_for_check_id("accessibility.lang"), None);
}

#[test]
fn violation_evidence_is_actionable_but_sanitized() {
    let result = axe_violation_result(&AxeViolation {
        id: "label".into(),
        impact: "serious".into(),
        description: "Ensures every form element has a label".into(),
        help: "Form elements must have labels".into(),
        help_url: "https://dequeuniversity.com/rules/axe/label".into(),
        nodes_count: 1,
        nodes: vec![AxeNodeEvidence {
            target: vec![r#"form input[data-email^="customer@example.com"]"#.into()],
            html: r#"<input id="email" type="email" value="customer@example.com">customer@example.com</input>"#.into(),
            failure_summary: Some(
                "Fix any of the following: Form element does not have an implicit or explicit label"
                    .into(),
            ),
        }],
    });

    let raw = result.raw_data.expect("axe raw evidence");
    let evidence = raw["node_evidence"]
        .as_array()
        .and_then(|nodes| nodes.first())
        .expect("first axe node evidence")
        .to_string();

    assert!(!evidence.contains("customer@example.com"));
    assert!(evidence.contains("[redacted]"));
    assert!(evidence.contains("Form element does not have"));
    assert!(result.fix_prompt.is_some());
    assert!(result.manual_fix.is_some());
    assert!(result.why_it_matters.is_some());
}

#[test]
fn a_report_grades_the_violations_in_order_then_every_other_rule_that_ran() {
    let report = AxeReport {
        violations: vec![
            violation("image-alt", "critical"),
            violation("link-name", "serious"),
        ],
        passes: vec!["label".into()],
        incomplete: vec!["color-contrast".into()],
        ..AxeReport::default()
    };
    let rows = evaluate_axe_report(&report);
    assert_eq!(
        rows.iter()
            .map(|row| row.check_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "accessibility.axe.image-alt",
            "accessibility.axe.link-name",
            "accessibility.axe.color-contrast",
            "accessibility.axe.label",
        ]
    );
    // The rows a fix is proven against: a rule that cleared the page, and one
    // that could not decide and therefore clears nothing.
    assert_eq!(rows[2].status, CheckStatus::Skipped);
    assert_eq!(rows[3].status, CheckStatus::Pass);
}
