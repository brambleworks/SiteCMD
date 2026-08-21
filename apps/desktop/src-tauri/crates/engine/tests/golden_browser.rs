//! Exact cross-runtime fixtures for browser verdict grading.
//!
//! Regenerate with `cargo test -p sitecmd-engine --test golden_browser -- --ignored regenerate`.

use serde::Deserialize;
use sitecmd_engine::browser::{AxeReport, CoreWebVitals};
use sitecmd_engine::checks::accessibility::axe;
use sitecmd_engine::checks::performance::browser_vitals;
use sitecmd_engine::{CheckResult, CheckStatus, Severity};

const CORPUS: &str = include_str!("../fixtures/checks/golden_browser.json");

#[derive(Deserialize)]
struct Corpus {
    #[allow(dead_code)]
    comment: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    check: String,
    input: serde_json::Value,
    expected: Option<Vec<CheckResult>>,
}

#[derive(Deserialize)]
struct RuleCoverageInput {
    rule: String,
    report: AxeReport,
}

fn corpus() -> Corpus {
    serde_json::from_str(CORPUS).expect("golden_browser.json parses")
}

fn report_of(case: &Case) -> AxeReport {
    serde_json::from_value(case.input.clone()).expect("axe report input parses")
}

fn run_case(case: &Case) -> Vec<CheckResult> {
    match case.check.as_str() {
        "accessibility.axe" => axe::evaluate_axe_report(&report_of(case)),
        "accessibility.axe.coverage" => {
            let input: RuleCoverageInput =
                serde_json::from_value(case.input.clone()).expect("rule coverage input parses");
            vec![axe::axe_rule_coverage_result(
                &input.rule,
                input.report.rule_outcome(&input.rule),
            )]
        }
        "performance.browser_vitals" => {
            let sample: CoreWebVitals =
                serde_json::from_value(case.input.clone()).expect("core web vitals input parses");
            browser_vitals::evaluate_core_web_vitals(&sample)
        }
        other => panic!("no browser-check driver registered for corpus id '{other}'"),
    }
}

#[test]
fn golden_cases_reproduce_their_verdicts() {
    let corpus = corpus();
    assert!(!corpus.cases.is_empty(), "corpus has cases");
    for case in &corpus.cases {
        let expected = case.expected.as_ref().unwrap_or_else(|| {
            panic!(
                "case '{}' has no expected block; run the ignored `regenerate` test",
                case.name
            )
        });
        let actual = run_case(case);
        assert_eq!(
            actual.len(),
            expected.len(),
            "{}: result row count",
            case.name
        );
        for (index, (actual_row, expected_row)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                serde_json::to_value(actual_row).expect("actual row serializes"),
                serde_json::to_value(expected_row).expect("expected row serializes"),
                "{}[{index}] ({})",
                case.name,
                actual_row.check_id
            );
        }
    }
}

// Hand-authored assertions prevent fixture regeneration from masking regressions.
#[test]
fn headline_verdicts_match_the_documented_behavior() {
    let corpus = corpus();
    let rows_of = |name: &str| -> Vec<CheckResult> {
        let case = corpus
            .cases
            .iter()
            .find(|case| case.name == name)
            .unwrap_or_else(|| panic!("case '{name}' present"));
        run_case(case)
    };
    let only = |name: &str| -> CheckResult {
        let rows = rows_of(name);
        assert_eq!(rows.len(), 1, "{name}: one result row");
        rows[0].clone()
    };
    let row = |rows: &[CheckResult], check_id: &str| -> CheckResult {
        rows.iter()
            .find(|row| row.check_id == check_id)
            .unwrap_or_else(|| panic!("row '{check_id}' present"))
            .clone()
    };

    // axe impact drives the row: critical and serious are failures, anything
    // milder is a warning that still names the rule.
    let critical_rows = rows_of("critical_violation_fails_at_its_rule_id");
    let critical = row(&critical_rows, "accessibility.axe.image-alt");
    assert_eq!(critical.status, CheckStatus::Fail);
    assert_eq!(critical.severity, Severity::Critical);
    let moderate = only("moderate_violation_warns");
    assert_eq!(moderate.status, CheckStatus::Warn);
    assert_eq!(moderate.severity, Severity::Medium);

    let clean = rows_of("clean_report_emits_no_findings");
    assert_eq!(
        clean
            .iter()
            .map(|row| row.check_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "accessibility.axe.image-alt",
            "accessibility.axe.label",
            "accessibility.axe.link-name",
            "accessibility.axe.video-caption",
        ]
    );
    assert!(clean.iter().all(|row| row.status == CheckStatus::Pass));

    // Page content never rides out with the evidence.
    let redacted = serde_json::to_string(&only("violation_evidence_is_redacted"))
        .expect("violation row serializes");
    assert!(!redacted.contains("customer@example.com"));
    assert!(redacted.contains("[redacted]"));

    assert_eq!(
        only("passing_rule_proves_absence").status,
        CheckStatus::Pass
    );
    assert_eq!(
        only("inapplicable_rule_proves_absence").status,
        CheckStatus::Pass
    );
    assert_eq!(
        only("incomplete_rule_is_a_coverage_exception").status,
        CheckStatus::Skipped
    );
    assert_eq!(
        only("rule_that_never_executed_is_not_a_pass").status,
        CheckStatus::Skipped
    );

    // Core Web Vitals grade against the published thresholds.
    let good = rows_of("good_vitals_pass");
    for id in ["performance.lcp", "performance.cls", "performance.fcp"] {
        assert_eq!(row(&good, id).status, CheckStatus::Pass, "{id}");
    }
    let poor = rows_of("poor_vitals_fail");
    for id in ["performance.lcp", "performance.cls", "performance.fcp"] {
        assert_eq!(row(&poor, id).status, CheckStatus::Fail, "{id}");
    }

    // One TTFB id, one grading rule: a navigation sample grades on the same
    // web.dev ladder as the transport probe and records its vantage.
    let navigation = rows_of("navigation_ttfb_records_its_vantage");
    let ttfb = row(&navigation, "performance.ttfb");
    assert_eq!(ttfb.status, CheckStatus::Warn);
    assert_eq!(
        ttfb.raw_data
            .as_ref()
            .and_then(|data| data.get("measurement_source"))
            .and_then(serde_json::Value::as_str),
        Some("browser_navigation")
    );

    // A metric the browser engine never reported produces no row at all,
    // because absent is not zero.
    assert!(rows_of("unsupported_metrics_emit_no_rows").is_empty());

    // Load-time JavaScript errors warn with a redacted sample; a clean load
    // passes.
    let errors = rows_of("javascript_errors_warn_with_a_redacted_sample");
    let errors_row = row(&errors, "polish.js-errors");
    assert_eq!(errors_row.status, CheckStatus::Warn);
    let serialized = serde_json::to_string(&errors_row).expect("row serializes");
    assert!(!serialized.contains("hunter2"));
    assert_eq!(
        row(&rows_of("no_javascript_errors_passes"), "polish.js-errors").status,
        CheckStatus::Pass
    );
}

// Rewrite the expected blocks from current behavior. Ignored by default.
#[test]
#[ignore = "regenerates the golden fixture; run explicitly after an intentional change"]
fn regenerate() {
    let mut value: serde_json::Value =
        serde_json::from_str(CORPUS).expect("golden_browser.json parses");
    let cases: Vec<Case> =
        serde_json::from_value(value.get("cases").expect("cases array present").clone())
            .expect("cases parse");
    let out = value
        .get_mut("cases")
        .and_then(|cases| cases.as_array_mut())
        .expect("cases array");
    for (slot, case) in out.iter_mut().zip(&cases) {
        let rows = run_case(case);
        slot["expected"] = serde_json::to_value(&rows).expect("rows serialize");
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/checks/golden_browser.json"
    );
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&value).expect("corpus serializes")
    );
    std::fs::write(path, rendered).expect("write golden_browser.json");
    println!("regenerated {path}");
}
