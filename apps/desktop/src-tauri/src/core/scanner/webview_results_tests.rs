use super::*;
use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};
use crate::core::analysis_types::{AxeNodeEvidence, AxeViolation};

fn cwv_from_json(json: &str) -> CoreWebVitals {
    serde_json::from_str(json).expect("cwv json")
}

fn violation(rule: &str) -> AxeViolation {
    AxeViolation {
        id: rule.into(),
        impact: "critical".into(),
        description: "Images must have alternate text".into(),
        help: "Images must have alternate text".into(),
        help_url: "https://example.com/image-alt".into(),
        nodes_count: 2,
        nodes: vec![AxeNodeEvidence {
            target: vec!["main img".into()],
            html: "<img src=\"hero.png\">".into(),
            failure_summary: None,
        }],
    }
}

fn scan_result(issues: Vec<CheckResult>) -> ScanResult {
    ScanResult {
        url: "https://example.com".into(),
        mode: "live".into(),
        scan_type: crate::core::scanner::ScanType::Health,
        overall_score: 100,
        categories: Vec::new(),
        issues,
        detected_stack: None,
        duration_ms: 0,
        timestamp: String::new(),
        page_signals: None,
        site_facts: None,
    }
}

fn issue(check_id: &str, category: ScanCategory, status: CheckStatus) -> CheckResult {
    CheckResult {
        check_id: check_id.into(),
        category,
        title: check_id.into(),
        description: "first-party finding".into(),
        status,
        severity: Severity::Medium,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

fn first_party_twins() -> ScanResult {
    scan_result(vec![
        issue(
            "accessibility.image_alt",
            ScanCategory::Accessibility,
            CheckStatus::Fail,
        ),
        issue(
            "accessibility.color_contrast_hints",
            ScanCategory::Accessibility,
            CheckStatus::Fail,
        ),
        issue(
            "accessibility.headings",
            ScanCategory::Accessibility,
            CheckStatus::Pass,
        ),
    ])
}

fn has(result: &ScanResult, check_id: &str) -> bool {
    result.issues.iter().any(|issue| issue.check_id == check_id)
}

#[test]
fn browser_ttfb_replaces_the_static_sample_under_one_canonical_id() {
    let mut result = scan_result(vec![CheckResult {
        raw_data: Some(serde_json::json!({
            "ttfb_ms": 1200,
            "measurement_source": "http_probe"
        })),
        ..issue(
            "performance.ttfb",
            ScanCategory::Performance,
            CheckStatus::Warn,
        )
    }]);

    append_webview_results(
        &mut result,
        None,
        Some(&cwv_from_json(r#"{"ttfb_ms":300.0}"#)),
    );

    let ttfb = result
        .issues
        .iter()
        .filter(|issue| issue.check_id == "performance.ttfb")
        .collect::<Vec<_>>();
    assert_eq!(ttfb.len(), 1, "TTFB must be one issue and one score input");
    assert_eq!(
        ttfb[0]
            .raw_data
            .as_ref()
            .and_then(|data| data.get("measurement_source"))
            .and_then(serde_json::Value::as_str),
        Some("browser_navigation")
    );
    assert_eq!(
        ttfb[0]
            .raw_data
            .as_ref()
            .and_then(|data| data.get("transport_ttfb_ms"))
            .and_then(serde_json::Value::as_u64),
        Some(1200),
        "the connected payload still needs the honest transport observation"
    );
}

#[test]
fn appended_axe_results_arrive_severity_policy_normalized() {
    let mut result = scan_result(Vec::new());
    let report = AxeReport {
        violations: vec![violation("image-alt")],
        ..AxeReport::default()
    };

    append_webview_results(&mut result, Some(&report), None);

    let axe = result
        .issues
        .iter()
        .find(|issue| issue.check_id == "accessibility.axe.image-alt")
        .expect("axe result");
    assert_eq!(axe.severity, Severity::High);
}

#[test]
fn a_rule_that_reached_a_verdict_supersedes_its_first_party_twin() {
    let mut result = first_party_twins();
    let report = AxeReport {
        violations: vec![violation("image-alt")],
        passes: vec!["color-contrast".into()],
        ..AxeReport::default()
    };

    append_webview_results(&mut result, Some(&report), None);

    assert!(
        !has(&result, "accessibility.image_alt"),
        "a violated rule replaces the markup heuristic that re-detects it"
    );
    assert!(
        !has(&result, "accessibility.color_contrast_hints"),
        "a passing rule replaces it too - axe saw the computed styles"
    );
    assert!(has(&result, "accessibility.axe.image-alt"));
    assert!(
        has(&result, "accessibility.headings"),
        "non-overlapping first-party checks must survive"
    );
}

#[test]
fn a_rule_that_could_not_decide_leaves_the_first_party_twin_in_place() {
    let mut result = first_party_twins();
    let report = AxeReport {
        passes: vec!["image-alt".into()],
        incomplete: vec!["color-contrast".into()],
        ..AxeReport::default()
    };

    append_webview_results(&mut result, Some(&report), None);

    assert!(
        has(&result, "accessibility.color_contrast_hints"),
        "an undecided rule proves nothing, so its twin keeps the coverage"
    );
    assert!(!has(&result, "accessibility.image_alt"));
}

#[test]
fn a_rule_that_never_executed_leaves_the_first_party_twin_in_place() {
    let mut result = first_party_twins();
    append_webview_results(&mut result, Some(&AxeReport::default()), None);

    assert!(has(&result, "accessibility.image_alt"));
    assert!(has(&result, "accessibility.color_contrast_hints"));
}

#[test]
fn first_party_accessibility_checks_survive_when_axe_did_not_run() {
    let mut result = first_party_twins();
    append_webview_results(&mut result, None, None);
    assert!(
        has(&result, "accessibility.image_alt"),
        "without an axe run the static checks are the only coverage"
    );
}
