use super::*;

fn sample() -> CoreWebVitals {
    CoreWebVitals {
        lcp_ms: None,
        cls: None,
        fcp_ms: None,
        ttfb_ms: None,
        observed_long_task_blocking_ms: None,
        js_errors: Vec::new(),
        js_error_count: None,
    }
}

fn row<'a>(rows: &'a [CheckResult], check_id: &str) -> &'a CheckResult {
    rows.iter()
        .find(|row| row.check_id == check_id)
        .unwrap_or_else(|| panic!("{check_id} row present"))
}

#[test]
fn an_empty_sample_produces_no_rows() {
    // A browser engine that reports nothing must not be graded as perfect.
    assert!(evaluate_core_web_vitals(&sample()).is_empty());
}

#[test]
fn each_metric_grades_against_its_own_thresholds() {
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        lcp_ms: Some(2400.0),
        fcp_ms: Some(3200.0),
        ..sample()
    });
    assert_eq!(row(&rows, "performance.lcp").status, CheckStatus::Pass);
    assert_eq!(row(&rows, "performance.fcp").status, CheckStatus::Fail);
    assert!(row(&rows, "performance.fcp")
        .description
        .contains("3200ms - Poor"));
}

#[test]
fn layout_shift_reports_the_ratio_not_the_scaled_value() {
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        cls: Some(0.18),
        ..sample()
    });
    let cls = row(&rows, "performance.cls");
    assert_eq!(cls.status, CheckStatus::Warn);
    assert!(cls.description.starts_with("0.180 - Needs Improvement"));
    assert_eq!(cls.raw_data.as_ref().expect("raw data")["cls"], 0.18);
}

#[test]
fn the_navigation_ttfb_sample_grades_through_the_shared_ladder() {
    // One check id, one grading rule: the browser sample and the transport
    // probe differ only in the vantage they record.
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        ttfb_ms: Some(950.4),
        ..sample()
    });
    let ttfb = row(&rows, "performance.ttfb");
    let shared =
        &crate::checks::performance::ttfb::evaluate_ttfb(950, BROWSER_NAVIGATION_SOURCE)[0];
    assert_eq!(
        serde_json::to_value(ttfb).expect("row serializes"),
        serde_json::to_value(shared).expect("row serializes")
    );
    assert_eq!(
        ttfb.raw_data.as_ref().expect("raw data")["measurement_source"],
        "browser_navigation"
    );
}

#[test]
fn observed_blocking_states_that_it_is_not_lighthouse_tbt() {
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        observed_long_task_blocking_ms: Some(750.0),
        ..sample()
    });
    let blocking = row(&rows, "performance.long_task_blocking");
    assert_eq!(blocking.status, CheckStatus::Fail);
    assert_eq!(blocking.severity, Severity::Medium);
    assert_eq!(blocking.confidence, IssueConfidence::NeedsReview);
    assert!(blocking.description.contains("not Lighthouse TBT"));
    assert_eq!(
        blocking.raw_data.as_ref().expect("raw data")["rating"],
        "high_observation"
    );
}

#[test]
fn zero_observed_blocking_still_reports_a_measured_pass() {
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        observed_long_task_blocking_ms: Some(0.0),
        ..sample()
    });
    let blocking = row(&rows, "performance.long_task_blocking");
    assert_eq!(blocking.status, CheckStatus::Pass);
    assert_eq!(blocking.confidence, IssueConfidence::High);
    assert!(blocking.why_it_matters.is_none());
}

#[test]
fn javascript_errors_are_redacted_and_counted() {
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        js_error_count: Some(2),
        js_errors: vec!["TypeError at https://example.com/app.js?token=secret123".into()],
        ..sample()
    });
    let errors = row(&rows, "polish.js-errors");
    assert_eq!(errors.status, CheckStatus::Warn);
    assert_eq!(errors.category, ScanCategory::Polish);
    assert!(errors.title.contains("2 JavaScript errors"));
    assert!(!errors.description.contains("secret123"));
    assert_eq!(
        errors.raw_data.as_ref().expect("raw data")["error_count"],
        2
    );
}

fn cwv_from_json(json: &str) -> CoreWebVitals {
    serde_json::from_str(json).expect("cwv json")
}

#[test]
fn fired_metrics_state_the_problem_not_the_metric_name() {
    let slow = cwv_from_json(
        r#"{"lcp_ms":5000.0,"cls":0.4,"fcp_ms":3500.0,"ttfb_ms":2000.0,"observed_long_task_blocking_ms":800.0}"#,
    );
    for result in evaluate_core_web_vitals(&slow) {
        assert_eq!(result.status, CheckStatus::Fail, "{}", result.check_id);
        assert!(
            !result.title.contains("Webview") && !result.title.contains("- Lab"),
            "{}: internal jargon in title: {}",
            result.check_id,
            result.title
        );
        assert!(
            !result.title.starts_with("Largest Contentful")
                && !result.title.starts_with("Cumulative")
                && !result.title.starts_with("First Contentful")
                && !result.title.starts_with("Time to First")
                && !result.title.starts_with("Total Blocking"),
            "{}: fired title must state a problem, was: {}",
            result.check_id,
            result.title
        );
    }

    // Passing results keep the neutral metric-name title.
    let fast = cwv_from_json(r#"{"lcp_ms":1200.0,"cls":null,"fcp_ms":null,"ttfb_ms":300.0}"#);
    for result in evaluate_core_web_vitals(&fast) {
        assert_eq!(result.status, CheckStatus::Pass, "{}", result.check_id);
        assert!(
            result.title.contains("(LCP)") || result.title.contains("(TTFB)"),
            "{}: passing title should name the metric, was: {}",
            result.check_id,
            result.title
        );
    }
}

#[test]
fn actionable_findings_carry_complete_guidance() {
    let slow = cwv_from_json(
        r#"{"lcp_ms":5000.0,"cls":0.4,"fcp_ms":3500.0,"ttfb_ms":2000.0,"observed_long_task_blocking_ms":800.0}"#,
    );
    for result in evaluate_core_web_vitals(&slow) {
        assert_ne!(result.status, CheckStatus::Pass, "{}", result.check_id);
        assert!(
            result.fix_prompt.is_some(),
            "{} has no fix prompt",
            result.check_id
        );
        assert!(
            result.manual_fix.is_some(),
            "{} has no manual fix",
            result.check_id
        );
        assert!(
            result.why_it_matters.is_some(),
            "{} has no rationale",
            result.check_id
        );
    }
}

#[test]
fn javascript_error_evidence_redacts_user_and_credential_data() {
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        js_error_count: Some(1),
        js_errors: vec![
            "Failed for person@example.com at https://example.com/reset/short-token?token=supersecret password=hunter2".into(),
        ],
        ..sample()
    });
    let serialized = serde_json::to_string(row(&rows, "polish.js-errors")).expect("serialize row");

    for secret in [
        "person@example.com",
        "short-token",
        "supersecret",
        "hunter2",
    ] {
        assert!(
            !serialized.contains(secret),
            "runtime evidence leaked {secret}: {serialized}"
        );
    }
    assert!(serialized.contains("[email]"));
    assert!(serialized.contains("[redacted]"));
}

#[test]
fn a_sample_without_the_newer_fields_reports_only_what_it_measured() {
    let cwv = cwv_from_json(r#"{"lcp_ms":1200.0,"cls":0.02,"fcp_ms":900.0,"ttfb_ms":200.0}"#);
    let rows = evaluate_core_web_vitals(&cwv);
    assert!(rows
        .iter()
        .all(|row| row.check_id != "performance.long_task_blocking"));
    assert!(rows.iter().all(|row| row.check_id != "polish.js-errors"));
}

#[test]
fn no_javascript_errors_passes_without_remediation() {
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        js_error_count: Some(0),
        ..sample()
    });
    let errors = row(&rows, "polish.js-errors");
    assert_eq!(errors.status, CheckStatus::Pass);
    assert!(errors.manual_fix.is_none());
    assert!(errors.why_it_matters.is_none());
}

const ANALYZER_RUNTIME_REJECTION: &str = "Unhandled promise rejection: notification.is_permission_granted not allowed on window \"analyzer-1788391062761\", webview \"analyzer-1788391062761\", URL: https://example.com/";

#[test]
fn the_analyzer_runtime_rejection_is_not_a_page_error() {
    // The notification plugin's init script runs in every webview of the app
    // and its command is refused in the analyzer; the page did nothing wrong.
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        js_error_count: Some(1),
        js_errors: vec![ANALYZER_RUNTIME_REJECTION.into()],
        ..sample()
    });
    let errors = row(&rows, "polish.js-errors");
    assert_eq!(errors.status, CheckStatus::Pass);
    assert_eq!(
        errors.raw_data.as_ref().expect("raw data")["error_count"],
        0
    );
}

#[test]
fn a_page_error_beside_the_runtime_rejection_still_counts() {
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        js_error_count: Some(2),
        js_errors: vec![
            ANALYZER_RUNTIME_REJECTION.into(),
            "TypeError: undefined is not an object (evaluating 'window.dataLayer.push') (https://example.com/app.js:12)".into(),
        ],
        ..sample()
    });
    let errors = row(&rows, "polish.js-errors");
    assert_eq!(errors.status, CheckStatus::Warn);
    assert_eq!(errors.title, "JavaScript error during page load");
    let raw = errors.raw_data.as_ref().expect("raw data");
    assert_eq!(raw["error_count"], 1);
    assert_eq!(raw["errors"].as_array().map(Vec::len), Some(1));
    assert!(!errors.description.contains("not allowed on window"));
}

#[test]
fn every_runtime_marker_names_the_analyzer_not_the_page() {
    for message in [
        "Unhandled promise rejection: notification.is_permission_granted not allowed on window \"analyzer-1\"",
        "TypeError: window.__TAURI_INTERNALS__.invoke is not a function",
        "Unhandled promise rejection: plugin:notification|is_permission_granted failed",
    ] {
        assert!(is_analyzer_runtime_error(message), "{message}");
    }
    assert!(!is_analyzer_runtime_error(
        "TypeError: Cannot read properties of null (reading 'push')"
    ));
}

#[test]
fn a_bare_plugin_prefix_is_the_pages_own_error() {
    // A Vite or Rollup overlay error reads "[plugin:vite:import-analysis]",
    // and the observer appends the failing script URL to the message it
    // records. Dropping either would report a pass the run never observed.
    for message in [
        "[plugin:vite:import-analysis] Failed to resolve import \"./missing\" from \"src/main.ts\"",
        "Uncaught Error: rollup plugin: terser failed (https://example.com/app.js:12)",
        "TypeError: undefined is not an object (https://example.com/plugins/plugin:loader.js:3)",
    ] {
        assert!(!is_analyzer_runtime_error(message), "{message}");
    }
    // The pipe is what makes the name a Tauri command.
    assert!(is_analyzer_runtime_error(
        "plugin:window-state|restore_state failed"
    ));
    assert!(!is_analyzer_runtime_error("plugin:|restore_state failed"));
}

#[test]
fn a_vite_overlay_error_still_grades_as_a_page_error() {
    let rows = evaluate_core_web_vitals(&CoreWebVitals {
        js_error_count: Some(1),
        js_errors: vec![
            "[plugin:vite:import-analysis] Failed to resolve import \"./missing\"".into(),
        ],
        ..sample()
    });
    let errors = row(&rows, "polish.js-errors");
    assert_eq!(errors.status, CheckStatus::Warn);
    assert_eq!(
        errors.raw_data.as_ref().expect("raw data")["error_count"],
        1
    );
}

#[test]
fn the_observer_skips_the_same_runtime_markers_the_verdict_drops() {
    // The in-page observer and this verdict must agree, or a payload from a
    // runtime without the observer filter would grade differently.
    // Pinning the whole array literal proves the two lists are identical:
    // an extra marker in the page (a bare `plugin:`, say) would drop errors
    // this verdict counts, and a missing one would count errors it drops.
    let markers = format!(
        "var shkRuntimeMarkers = [{}];",
        ANALYZER_RUNTIME_ERROR_MARKERS
            .iter()
            .map(|marker| format!("\"{marker}\""))
            .collect::<Vec<String>>()
            .join(", ")
    );
    assert!(
        crate::browser::CWV_OBSERVER_SCRIPT.contains(&markers),
        "cwv_observer.js must skip exactly these markers: {markers}"
    );
    assert!(
        crate::browser::CWV_OBSERVER_SCRIPT.contains(r"/plugin:[A-Za-z0-9_.-]+\|/"),
        "cwv_observer.js must require the pipe that makes `plugin:` a Tauri command"
    );
}
