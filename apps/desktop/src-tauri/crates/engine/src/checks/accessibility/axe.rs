//! Axe verdicts and rule-level coverage. Rendered-DOM results supersede local
//! heuristics only when that specific rule proved an outcome.

use crate::browser::{AxeReport, AxeViolation, RuleOutcome};
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

/// Check-id prefix for the dynamic per-rule ids axe produces.
pub const CHECK_ID_PREFIX: &str = "accessibility.axe.";

/// First-party heuristics suppressed when corresponding axe rules return a verdict.
pub const SUPERSEDED_FIRST_PARTY_CHECKS: &[(&str, &[&str])] = &[
    ("accessibility.lang", &["html-has-lang", "valid-lang"]),
    ("accessibility.image_alt", &["image-alt"]),
    ("accessibility.form_labels", &["label"]),
    ("accessibility.link_text", &["link-name"]),
    ("accessibility.aria_usage", &["aria-hidden-focus"]),
    ("accessibility.color_contrast_hints", &["color-contrast"]),
];

/// The check id an axe rule reports under.
pub fn check_id_for_rule(rule: &str) -> String {
    format!("{CHECK_ID_PREFIX}{rule}")
}

/// The axe rule inside a check id, or `None` for an id from another family.
pub fn rule_for_check_id(check_id: &str) -> Option<&str> {
    check_id.strip_prefix(CHECK_ID_PREFIX)
}

/// First-party checks superseded by conclusive rendered-DOM verdicts.
pub fn superseded_first_party_check_ids(report: &AxeReport) -> Vec<&'static str> {
    SUPERSEDED_FIRST_PARTY_CHECKS
        .iter()
        .filter(|(_, rules)| {
            rules.iter().any(|rule| {
                matches!(
                    report.rule_outcome(rule),
                    RuleOutcome::Violated | RuleOutcome::Proved
                )
            })
        })
        .map(|(check_id, _)| *check_id)
        .collect()
}

/// One violation as a check result.
pub fn axe_violation_result(violation: &AxeViolation) -> CheckResult {
    let violation = violation.clone().sanitize_node_evidence();
    let severity = match violation.impact.as_str() {
        "critical" => Severity::Critical,
        "serious" => Severity::High,
        "moderate" => Severity::Medium,
        _ => Severity::Low,
    };
    let status = match violation.impact.as_str() {
        "critical" | "serious" => CheckStatus::Fail,
        _ => CheckStatus::Warn,
    };
    let first_node = violation.nodes.first();
    let first_selector = first_node
        .and_then(|node| node.target.first())
        .map(String::as_str);
    let first_failure = first_node
        .and_then(|node| node.failure_summary.as_deref())
        .unwrap_or(violation.help.as_str());
    let selector_note = first_selector
        .map(|selector| format!(" First affected selector: `{selector}`."))
        .unwrap_or_default();
    let fix_target = first_selector
        .map(|selector| format!("For `{selector}`: "))
        .unwrap_or_default();
    let plural = if violation.nodes_count == 1 { "" } else { "s" };
    let actionable_fix = format!(
        "{fix_target}{first_failure}. Review the axe rule guidance at {} and apply the fix to all {} affected element{plural}.",
        violation.help_url, violation.nodes_count,
    );

    CheckResult {
        check_id: check_id_for_rule(&violation.id),
        category: ScanCategory::Accessibility,
        title: violation.help.clone(),
        description: format!(
            "axe-core found {} rendered element{plural} that fail `{}`.{selector_note}",
            violation.nodes_count, violation.id,
        ),
        status,
        severity,
        fix_prompt: Some(actionable_fix.clone()),
        manual_fix: Some(actionable_fix),
        raw_data: Some(serde_json::json!({
            "axe_id": violation.id,
            "impact": violation.impact,
            "nodes": violation.nodes_count,
            "node_evidence": violation.nodes,
            "help_url": violation.help_url,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: Some(violation.description),
    }
}

/// Build an id-level rule result when no violation evidence is available.
/// Only `Proved` passes because it confirms the rule executed and cleared.
pub fn axe_rule_coverage_result(rule: &str, outcome: RuleOutcome) -> CheckResult {
    let (status, title, description, raw_outcome) = match outcome {
        RuleOutcome::Proved => (
            CheckStatus::Pass,
            format!("axe rule `{rule}` passed"),
            format!(
                "axe-core ran `{rule}` on the rendered page and reported no failing element."
            ),
            "proved",
        ),
        RuleOutcome::Undecided => (
            CheckStatus::Skipped,
            format!("axe rule `{rule}` needs review"),
            format!(
                "axe-core could not decide `{rule}` on the rendered page, so this run neither found nor ruled out a failure. Review the rule manually, or re-run once the page renders without the condition axe flagged as incomplete."
            ),
            "undecided",
        ),
        RuleOutcome::NotRun => (
            CheckStatus::Skipped,
            format!("axe rule `{rule}` did not run"),
            format!(
                "axe-core did not evaluate `{rule}` during this run, so its absence from the findings proves nothing about the page."
            ),
            "not_run",
        ),
        RuleOutcome::Violated => (
            CheckStatus::Fail,
            format!("axe rule `{rule}` reported a violation"),
            format!("axe-core reported `{rule}` violations on the rendered page."),
            "violated",
        ),
    };

    CheckResult {
        check_id: check_id_for_rule(rule),
        category: ScanCategory::Accessibility,
        title,
        description,
        status,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "axe_id": rule,
            "violations": 0,
            "rule_outcome": raw_outcome,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

/// Grade violations and emit coverage for every other executed axe rule.
///
/// Coverage rows distinguish proved absence from rules that did not conclude,
/// allowing fixed findings to be verified without affecting the score.
pub fn evaluate_axe_report(report: &AxeReport) -> Vec<CheckResult> {
    let mut results: Vec<CheckResult> =
        report.violations.iter().map(axe_violation_result).collect();
    results.extend(
        report
            .executed_rules()
            .into_iter()
            .filter(|rule| {
                !report
                    .violations
                    .iter()
                    .any(|violation| violation.id == *rule)
            })
            .map(|rule| axe_rule_coverage_result(rule, report.rule_outcome(rule))),
    );
    results
}

#[cfg(test)]
#[path = "axe_tests.rs"]
mod tests;
