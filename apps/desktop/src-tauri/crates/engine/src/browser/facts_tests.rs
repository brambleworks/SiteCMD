use super::*;

/// Adapters decode the payload before parsing it so they can read its
/// document identity from the same value; these fixtures are written as the
/// raw payload text, so they decode the same way.
fn report_from_json(json: &str) -> Result<AxeReport, String> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| format!("axe payload was not valid JSON: {error}"))?;
    axe_report_from_value(value)
}

fn violation(id: &str) -> AxeViolation {
    AxeViolation {
        id: id.into(),
        impact: "serious".into(),
        description: "description".into(),
        help: "help".into(),
        help_url: "https://dequeuniversity.com/rules/axe/4.11/image-alt".into(),
        nodes_count: 1,
        nodes: Vec::new(),
    }
}

#[test]
fn axe_evidence_redacts_ids_and_attributes_containing_angle_brackets() {
    let violation = AxeViolation {
        nodes: vec![AxeNodeEvidence {
            target: vec![r#"form #客户-123 input[value="private"]"#.into()],
            html: r#"<input id="客户-123" value="private > data">private text</input>"#.into(),
            failure_summary: None,
        }],
        ..violation("label")
    }
    .sanitize_node_evidence();

    let node = &violation.nodes[0];
    assert!(!node.target[0].contains("客户-123"));
    assert!(!node.target[0].contains("private"));
    assert!(!node.html.contains("客户-123"));
    assert!(!node.html.contains("private"));
    assert!(node.html.ends_with('>'));
}

#[test]
fn a_rule_that_reported_nodes_is_violated() {
    let report = AxeReport {
        violations: vec![violation("image-alt")],
        ..AxeReport::default()
    };
    assert_eq!(report.rule_outcome("image-alt"), RuleOutcome::Violated);
}

#[test]
fn passes_and_inapplicable_both_prove_absence() {
    // A rule with nothing on the page to evaluate proves absence just as
    // firmly as a rule that examined nodes and cleared them.
    let report = AxeReport {
        passes: vec!["label".into()],
        inapplicable: vec!["video-caption".into()],
        ..AxeReport::default()
    };
    assert_eq!(report.rule_outcome("label"), RuleOutcome::Proved);
    assert_eq!(report.rule_outcome("video-caption"), RuleOutcome::Proved);
}

#[test]
fn incomplete_is_not_a_pass_and_absence_is_not_a_pass() {
    // The whole point of the bucket arrays: neither of these may be read as
    // "the page is clean for this rule".
    let report = AxeReport {
        incomplete: vec!["color-contrast".into()],
        ..AxeReport::default()
    };
    assert_eq!(
        report.rule_outcome("color-contrast"),
        RuleOutcome::Undecided
    );
    assert_eq!(report.rule_outcome("html-has-lang"), RuleOutcome::NotRun);
}

#[test]
fn executed_rules_spans_every_bucket_once() {
    let report = AxeReport {
        violations: vec![violation("image-alt")],
        passes: vec!["label".into(), "label".into()],
        incomplete: vec!["color-contrast".into()],
        inapplicable: vec!["video-caption".into()],
    };
    assert_eq!(
        report.executed_rules(),
        vec!["color-contrast", "image-alt", "label", "video-caption"]
    );
}

#[test]
fn a_payload_error_is_a_failed_run_not_an_empty_one() {
    let error = report_from_json(r#"{"error":"axe-core not loaded"}"#).unwrap_err();
    assert_eq!(error, "axe-core not loaded");
}

#[test]
fn a_parsed_report_carries_every_bucket_and_sanitizes_evidence() {
    let report = report_from_json(
        r##"{
            "violations": [{
                "id": "image-alt",
                "impact": "critical",
                "description": "Images must have alternate text",
                "help": "Images must have alternate text",
                "help_url": "https://example.com/image-alt",
                "nodes_count": 2,
                "nodes": [{
                    "target": ["#customer-42 img"],
                    "html": "<img src=\"/private/receipt.png\">",
                    "failure_summary": "Fix any of the following: Element has no alt attribute"
                }]
            }],
            "passes": ["label"],
            "incomplete": ["color-contrast"],
            "inapplicable": ["video-caption"]
        }"##,
    )
    .expect("report parses");

    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].nodes_count, 2);
    assert_eq!(report.passes, vec!["label"]);
    assert_eq!(report.incomplete, vec!["color-contrast"]);
    assert_eq!(report.inapplicable, vec!["video-caption"]);
    let node = &report.violations[0].nodes[0];
    assert!(!node.target[0].contains("customer-42"));
    assert!(!node.html.contains("receipt.png"));
}

#[test]
fn a_report_without_bucket_arrays_still_parses_as_uncovered() {
    let report = report_from_json(r#"{"violations":[]}"#).expect("legacy report parses");
    assert!(report.executed_rules().is_empty());
    assert_eq!(report.rule_outcome("image-alt"), RuleOutcome::NotRun);
}
