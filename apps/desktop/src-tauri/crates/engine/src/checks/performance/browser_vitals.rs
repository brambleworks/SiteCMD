//! Portable grading of runtime-supplied Web Vitals samples, sharing the TTFB ladder.

use crate::browser::CoreWebVitals;
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

/// Vantage recorded for a TTFB sample taken from a real page navigation,
/// which includes the page's own redirect, connection, and response path.
pub const BROWSER_NAVIGATION_SOURCE: &str = "browser_navigation";

// TTFB lives in `super::ttfb` so browser and probe samples share one ladder.
pub const LCP_CHECK_ID: &str = "performance.lcp";
pub const CLS_CHECK_ID: &str = "performance.cls";
pub const FCP_CHECK_ID: &str = "performance.fcp";
pub const LONG_TASK_BLOCKING_CHECK_ID: &str = "performance.long_task_blocking";
pub const JS_ERRORS_CHECK_ID: &str = "polish.js-errors";

/// Markers of errors the analyzer's own runtime raises inside the page: a
/// Tauri plugin init script runs in every webview of the app and its command
/// is refused in the analyzer, so its rejection names SiteCMD, never the
/// page. `cwv_observer.js` skips these messages as they happen and this
/// verdict drops them from a sample that still carries them.
pub const ANALYZER_RUNTIME_ERROR_MARKERS: [&str; 2] = ["not allowed on window", "__TAURI"];

/// A Tauri command is always `plugin:<name>|<command>`. The pipe is what
/// makes the name the analyzer's: a bare `plugin:` is the page's own, since a
/// Vite or Rollup overlay error reads `[plugin:vite:import-analysis] ...` and
/// the recorded message can carry the failing script's URL as well.
const TAURI_COMMAND_PREFIX: &str = "plugin:";

/// Whether a recorded error message describes the analyzer runtime rather
/// than the page under test. Dropping a page error here would turn
/// `polish.js-errors` into a pass the run never observed, so the Tauri
/// command shape has to match, not just its prefix.
pub fn is_analyzer_runtime_error(message: &str) -> bool {
    ANALYZER_RUNTIME_ERROR_MARKERS
        .iter()
        .any(|marker| message.contains(marker))
        || names_a_tauri_command(message)
}

fn names_a_tauri_command(message: &str) -> bool {
    message
        .match_indices(TAURI_COMMAND_PREFIX)
        .any(|(index, prefix)| {
            let rest = &message[index + prefix.len()..];
            let name_len = rest
                .find(|character: char| {
                    !character.is_ascii_alphanumeric() && !"_-.".contains(character)
                })
                .unwrap_or(rest.len());
            name_len > 0 && rest[name_len..].starts_with('|')
        })
}

/// Grade one sample. Metrics the browser engine did not report are absent
/// rather than zero, so an unsupported metric produces no row instead of a
/// perfect score.
pub fn evaluate_core_web_vitals(cwv: &CoreWebVitals) -> Vec<CheckResult> {
    let mut results = Vec::new();

    if let Some(lcp) = cwv.lcp_ms {
        results.push(metric_result(
            LCP_CHECK_ID,
            "Largest Contentful Paint (LCP)",
            "Largest content is slow to appear (LCP)",
            lcp,
            2500.0,
            4000.0,
            Unit::Milliseconds,
            "Optimize images, reduce server response time, minimize render-blocking resources.",
        ));
    }
    if let Some(cls) = cwv.cls {
        results.push(layout_shift_result(cls));
    }
    if let Some(fcp) = cwv.fcp_ms {
        results.push(metric_result(
            FCP_CHECK_ID,
            "First Contentful Paint (FCP)",
            "First content is slow to appear (FCP)",
            fcp,
            1800.0,
            3000.0,
            Unit::Milliseconds,
            "Reduce render-blocking resources, inline critical CSS, defer non-essential JavaScript.",
        ));
    }
    if let Some(ttfb) = cwv.ttfb_ms {
        // Same id, same thresholds, same guidance as the transport probe's
        // sample; only the recorded vantage differs.
        results.extend(super::ttfb::evaluate_ttfb(
            ttfb.max(0.0).round() as u64,
            BROWSER_NAVIGATION_SOURCE,
        ));
    }
    if let Some(observed_blocking) = cwv.observed_long_task_blocking_ms {
        results.push(long_task_blocking_result(observed_blocking));
    }
    if let Some(error_count) = cwv.js_error_count {
        let page_sample: Vec<String> = cwv
            .js_errors
            .iter()
            .filter(|message| !is_analyzer_runtime_error(message))
            .cloned()
            .collect();
        // The sample is capped, so only the runtime errors it actually holds
        // can be subtracted; the count never goes below the page's own errors.
        let dropped = u32::try_from(cwv.js_errors.len() - page_sample.len()).unwrap_or(u32::MAX);
        results.push(js_errors_result(
            error_count.saturating_sub(dropped),
            &page_sample,
        ));
    }

    results
}

enum Unit {
    Milliseconds,
    /// A unitless ratio carried in thousandths so one threshold ladder serves
    /// both kinds of metric.
    Thousandths,
}

fn layout_shift_result(cls: f64) -> CheckResult {
    let mut result = metric_result(
        CLS_CHECK_ID,
        "Cumulative Layout Shift (CLS)",
        "Page content shifts while loading (CLS)",
        cls * 1000.0,
        100.0,
        250.0,
        Unit::Thousandths,
        "Set explicit dimensions on images/videos, avoid inserting content above existing content.",
    );
    let rating = if cls <= 0.1 {
        "Good"
    } else if cls <= 0.25 {
        "Needs Improvement"
    } else {
        "Poor"
    };
    result.description = format!("{cls:.3} - {rating} (target: ≤0.1)");
    result.raw_data = Some(serde_json::json!({
        "cls": cls, "rating": rating, "threshold_good": 0.1, "threshold_poor": 0.25
    }));
    result
}

fn long_task_blocking_result(observed_blocking: f64) -> CheckResult {
    let mut result = metric_result(
        LONG_TASK_BLOCKING_CHECK_ID,
        "Observed post-FCP main-thread blocking",
        "Main thread stayed busy after first content appeared",
        observed_blocking,
        200.0,
        600.0,
        Unit::Milliseconds,
        "Break up long JavaScript tasks, defer non-critical scripts, and move heavy work off the main thread (web workers).",
    );
    result.severity = match result.status {
        CheckStatus::Fail => Severity::Medium,
        _ => Severity::Low,
    };
    let heuristic_band = match result.status {
        CheckStatus::Pass | CheckStatus::Skipped => "within_observation_threshold",
        CheckStatus::Warn => "elevated_observation",
        CheckStatus::Fail => "high_observation",
    };
    result.description = format!(
        "{observed_blocking:.0}ms observed after FCP (SiteCMD heuristic target: ≤200ms; this is not Lighthouse TBT)"
    );
    result.raw_data = Some(serde_json::json!({
        "value": observed_blocking,
        "rating": heuristic_band,
        "threshold_good": 200.0,
        "threshold_poor": 600.0,
        "threshold_basis": "sitecmd_heuristic_for_a_nonstandard_lab_sample",
        "measurement": "observed_post_fcp_long_tasks",
        "not_lighthouse_tbt": true,
    }));
    result.why_it_matters = (result.status != CheckStatus::Pass).then(|| {
        "Long main-thread tasks delay input handling and keep the page from becoming smoothly interactive after content first appears.".to_string()
    });
    if result.status != CheckStatus::Pass {
        result.confidence = IssueConfidence::NeedsReview;
        result.confidence_reason = Some("This lightweight lab sample ends when SiteCMD reads the page; it is not Lighthouse TBT and can vary between runs.".into());
    }
    result
}

/// Runtime JavaScript errors captured during load.
fn js_errors_result(error_count: u32, sample: &[String]) -> CheckResult {
    let passed = error_count == 0;
    let remediation = "Reproduce the same navigation with browser DevTools open, start with the first uncaught error or unhandled rejection, and map its stack to first-party or third-party code. Fix or isolate the root cause, add an error path and regression test for affected behavior, then reload with the console clear. A bare `Script error` often lacks detail because cross-origin error reporting is restricted, so inspect the Network panel and the owning script rather than guessing.";
    let safe_sample = sample
        .iter()
        .map(|error| crate::log_sanitizer::redact_issue_evidence(error))
        .collect::<Vec<_>>();
    let plural = if error_count == 1 { "" } else { "s" };
    let sample_note = if safe_sample.is_empty() {
        String::new()
    } else {
        format!(
            " First error{}: {}",
            if safe_sample.len() == 1 { "" } else { "s" },
            safe_sample.join(" | ")
        )
    };

    CheckResult {
        check_id: JS_ERRORS_CHECK_ID.into(),
        category: ScanCategory::Polish,
        title: if passed {
            "No JavaScript errors on load".into()
        } else if error_count == 1 {
            "JavaScript error during page load".into()
        } else {
            format!("{error_count} JavaScript errors during page load")
        },
        description: if passed {
            "The page loaded without any runtime JavaScript errors.".into()
        } else {
            format!(
                "The page emitted {error_count} uncaught JavaScript error{plural} or unhandled promise rejection{plural} during this lab navigation.{sample_note} This establishes a runtime failure, but does not establish which feature or visitor cohort is affected."
            )
        },
        status: if passed {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        severity: Severity::Medium,
        fix_prompt: (!passed).then(|| remediation.into()),
        manual_fix: (!passed).then(|| remediation.into()),
        raw_data: Some(serde_json::json!({
            "error_count": error_count,
            "errors": safe_sample,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: (!passed).then(|| {
            "Uncaught load-time errors can interrupt initialization, leave state incomplete, or hide failed work. The actual impact can range from non-critical third-party code to a broken customer flow and must be traced from the observed stack and behavior.".to_string()
        }),
    }
}

/// One metric graded against its good/poor thresholds. `title` names the
/// metric for passing results; `fired_title` states the problem when the
/// metric misses its target, because a bare metric name is not a finding.
#[allow(clippy::too_many_arguments)]
fn metric_result(
    check_id: &str,
    title: &str,
    fired_title: &str,
    value: f64,
    good_threshold: f64,
    poor_threshold: f64,
    unit: Unit,
    fix_hint: &str,
) -> CheckResult {
    let (status, severity, rating) = if value <= good_threshold {
        (CheckStatus::Pass, Severity::Low, "Good")
    } else if value <= poor_threshold {
        (CheckStatus::Warn, Severity::Medium, "Needs Improvement")
    } else {
        (CheckStatus::Fail, Severity::High, "Poor")
    };
    let title = if status == CheckStatus::Pass {
        title
    } else {
        fired_title
    };

    let description = match unit {
        Unit::Milliseconds => format!("{value:.0}ms - {rating} (target: ≤{good_threshold:.0}ms)"),
        Unit::Thousandths => format!(
            "{:.3} - {rating} (target: ≤{:.1})",
            value / 1000.0,
            good_threshold / 1000.0
        ),
    };

    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Performance,
        title: title.into(),
        description,
        status,
        severity,
        fix_prompt: (value > good_threshold).then(|| fix_hint.into()),
        manual_fix: (value > good_threshold).then(|| fix_hint.into()),
        raw_data: Some(serde_json::json!({
            "value": value, "rating": rating,
            "threshold_good": good_threshold, "threshold_poor": poor_threshold
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: (value > good_threshold)
            .then(|| metric_why(check_id).map(str::to_string))
            .flatten(),
    }
}

fn metric_why(check_id: &str) -> Option<&'static str> {
    Some(match check_id {
        LCP_CHECK_ID => "Largest Contentful Paint measures when the page's main visible content finishes rendering. A slow LCP makes the page feel unavailable even if smaller UI appeared earlier.",
        CLS_CHECK_ID => "Unexpected layout movement interrupts reading and can move a control between pointer-down and activation, leading to accidental clicks.",
        FCP_CHECK_ID => "First Contentful Paint marks the end of the blank-screen phase. A slow FCP gives visitors no visual confirmation that navigation is progressing.",
        LONG_TASK_BLOCKING_CHECK_ID => "Long main-thread tasks delay input handling and keep the page from becoming smoothly interactive after content first appears.",
        _ => return None,
    })
}

#[cfg(test)]
#[path = "browser_vitals_tests.rs"]
mod tests;
