use super::*;
use crate::checks::{CheckStatus, Severity};
use crate::core::normalized_scan::ScanRunKind;
use crate::core::scanner::ScanResult;
use crate::core::types_work_items::ScoreSnapshot;

use super::comparison::full_baseline_execution_id;
use super::notifications::hostname_for_url;

#[tokio::test(start_paused = true)]
async fn scheduler_restarts_after_panic() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let attempt = Arc::new(AtomicUsize::new(0));
    let attempt_clone = Arc::clone(&attempt);

    let handle = tokio::spawn(crate::core::supervised_loop::supervised_loop(
        "test",
        std::time::Duration::from_millis(10),
        move || {
            let n = attempt_clone.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                panic!("synthetic panic");
            }
        },
    ));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    handle.abort();

    let ran = attempt.load(Ordering::SeqCst);
    assert!(
        ran >= 2,
        "expected restart after panic, got {ran} iterations"
    );
}

#[tokio::test(start_paused = true)]
async fn scheduler_async_restarts_after_panic() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let attempt = Arc::new(AtomicUsize::new(0));
    let attempt_clone = Arc::clone(&attempt);

    let handle = tokio::spawn(crate::core::supervised_loop::supervised_loop_async(
        "test_async",
        move || {
            let attempt = Arc::clone(&attempt_clone);
            async move {
                let n = attempt.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    panic!("synthetic async panic");
                }
                // Simulate a long-running loop that ticks once then yields back
                // long enough for the test to abort.
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        },
    ));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    handle.abort();

    let ran = attempt.load(Ordering::SeqCst);
    assert!(
        ran >= 2,
        "expected restart after panic, got {ran} iterations"
    );
}

#[test]
fn scheduled_occurrence_builds_one_full_execution_and_one_stable_action_key() {
    let mut schedule = ScanSchedule {
        id: Some(9),
        project_id: 3,
        environment_id: 4,
        frequency: "daily".into(),
        time_of_day: "09:00".into(),
        day_of_week: None,
        scan_type: ScheduledScanType::Full,
        last_run_at: None,
        next_run_at: Some("2026-07-22 09:00:00".into()),
    };
    let scope = || {
        vec![
            "https://example.com/".to_string(),
            "https://example.com/pricing".to_string(),
        ]
    };
    let first = build_scheduled_execution_request(
        &schedule,
        "https://example.com",
        scope(),
        Some("/tmp/example".into()),
        30,
    )
    .expect("scheduled request");
    let retry = build_scheduled_execution_request(
        &schedule,
        "https://example.com",
        scope(),
        Some("/tmp/example".into()),
        30,
    )
    .expect("scheduled retry");

    assert_eq!(first.requested_mode, ScanExecutionMode::Full);
    assert_eq!(first.trigger, ScanTrigger::Scheduled);
    assert_eq!(first.web_focus, Some(ScanType::Health));
    assert_eq!(first.axe_enabled, Some(true));
    assert_eq!(first.retention, Some(30));
    assert_eq!(first.idempotency_key, retry.idempotency_key);

    schedule.next_run_at = Some("2026-07-23 09:00:00".into());
    let next = build_scheduled_execution_request(
        &schedule,
        "https://example.com",
        scope(),
        Some("/tmp/example".into()),
        30,
    )
    .expect("next occurrence");
    assert_ne!(first.idempotency_key, next.idempotency_key);

    assert_eq!(first.urls, scope());
}

#[test]
fn scheduled_runs_enable_axe_only_for_accessibility_and_full() {
    for (scan_type, expected) in [
        (ScheduledScanType::Health, false),
        (ScheduledScanType::Security, false),
        (ScheduledScanType::Accessibility, true),
        (ScheduledScanType::Polish, false),
        (ScheduledScanType::Code, false),
        (ScheduledScanType::Full, true),
    ] {
        let schedule = ScanSchedule {
            id: Some(9),
            project_id: 3,
            environment_id: 4,
            frequency: "daily".into(),
            time_of_day: "09:00".into(),
            day_of_week: None,
            scan_type,
            last_run_at: None,
            next_run_at: Some("2026-07-22 09:00:00".into()),
        };
        let request = build_scheduled_execution_request(
            &schedule,
            "https://example.com",
            vec!["https://example.com/".into()],
            Some("/tmp/example".into()),
            30,
        )
        .expect("scheduled request");

        assert_eq!(request.axe_enabled, Some(expected), "{scan_type}");
    }
}

#[test]
fn scheduled_web_baselines_match_the_execution_shape() {
    assert_eq!(
        scheduled_web_run_kind(&["https://example.com/".into()]),
        ScanRunKind::Single
    );
    assert_eq!(
        scheduled_web_run_kind(&[
            "https://example.com/".into(),
            "https://example.com/pricing".into(),
        ]),
        ScanRunKind::MultiParent
    );
}

#[test]
fn scheduled_web_completion_summarizes_multi_page_sessions() {
    use crate::core::scanner::{MultiScanResult, PageScanSummary};

    let multi_result = MultiScanResult {
        session_id: 42,
        total_pages: 2,
        completed_pages: 2,
        overall_score: 73,
        duration_ms: 900,
        incomplete_detail: None,
        page_results: vec![
            PageScanSummary {
                url: "https://example.com/".into(),
                score: 80,
                issues_count: 3,
                issues_critical: 1,
                issues_high: 1,
                issues_medium: 1,
                issues_low: 0,
                duration_ms: 400,
                scan_id: 11,
            },
            PageScanSummary {
                url: "https://example.com/pricing".into(),
                score: 66,
                issues_count: 2,
                issues_critical: 0,
                issues_high: 1,
                issues_medium: 0,
                issues_low: 1,
                duration_ms: 500,
                scan_id: 12,
            },
        ],
        new_issue_count: Some(4),
        resolved_issue_count: Some(1),
        site_issues: Vec::new(),
    };

    let summary = summarize_scheduled_web_result(
        None,
        Some(&multi_result),
        None,
        None,
        Some(42),
        "2026-08-29T12:00:00Z",
    )
    .expect("multi-page completion summary");

    assert_eq!(summary.scan_id, Some(42));
    assert_eq!(summary.score, 73);
    assert_eq!(summary.counts.total, 5);
    assert_eq!(summary.counts.critical, 1);
    assert_eq!(summary.counts.high, 2);
    assert_eq!(summary.timestamp, "2026-08-29T12:00:00Z");
    assert_eq!(summary.regression_scan_ids, vec![11, 12]);
    assert!(summary.comparison_eligible);
}

#[test]
fn scheduled_web_completion_excludes_partial_page_sets_from_comparisons() {
    use crate::core::scanner::{MultiScanResult, PageScanSummary};

    let multi_result = MultiScanResult {
        session_id: 42,
        total_pages: 2,
        completed_pages: 1,
        overall_score: 61,
        duration_ms: 900,
        incomplete_detail: Some("1 of 2 selected pages completed.".into()),
        page_results: vec![
            PageScanSummary {
                url: "https://example.com/".into(),
                score: 61,
                issues_count: 2,
                issues_critical: 0,
                issues_high: 1,
                issues_medium: 1,
                issues_low: 0,
                duration_ms: 400,
                scan_id: 11,
            },
            PageScanSummary {
                url: "https://example.com/pricing".into(),
                score: 0,
                issues_count: 0,
                issues_critical: 0,
                issues_high: 0,
                issues_medium: 0,
                issues_low: 0,
                duration_ms: 0,
                scan_id: -1,
            },
        ],
        new_issue_count: Some(1),
        resolved_issue_count: Some(0),
        site_issues: Vec::new(),
    };

    let summary = summarize_scheduled_web_result(
        None,
        Some(&multi_result),
        None,
        None,
        Some(42),
        "2026-08-29T12:00:00Z",
    )
    .expect("partial completion summary remains reportable");

    assert!(!summary.comparison_eligible);
    assert!(!summary.scope_complete);
    assert_eq!(summary.completed_pages, 1);
    assert_eq!(summary.total_pages, 2);
    assert_eq!(summary.regression_scan_ids, vec![11]);
}

#[test]
fn scheduled_web_completion_excludes_incomplete_browser_coverage() {
    use crate::core::scanner::MultiScanResult;

    let multi_result = MultiScanResult {
        session_id: 43,
        total_pages: 2,
        completed_pages: 2,
        overall_score: 70,
        duration_ms: 900,
        incomplete_detail: Some("Browser analysis failed: unavailable".into()),
        page_results: Vec::new(),
        new_issue_count: None,
        resolved_issue_count: None,
        site_issues: Vec::new(),
    };

    let summary = summarize_scheduled_web_result(
        None,
        Some(&multi_result),
        Some("Browser analysis failed: unavailable"),
        None,
        Some(43),
        "2026-08-29T12:00:00Z",
    )
    .expect("partial browser completion summary");

    assert!(!summary.scope_complete);
    assert!(!summary.comparison_eligible);
    assert_eq!(
        summary.incomplete_detail.as_deref(),
        Some("Browser analysis failed: unavailable")
    );
}

#[test]
fn scheduled_completion_status_reports_incomplete_runs_as_partial() {
    assert_eq!(
        scheduled_completion_status(
            crate::core::scan_execution::ScanExecutionStatus::Complete,
            Some(false),
        ),
        "partial"
    );
    assert_eq!(
        scheduled_completion_status(
            crate::core::scan_execution::ScanExecutionStatus::Partial,
            Some(true),
        ),
        "partial"
    );
    assert_eq!(
        scheduled_completion_status(
            crate::core::scan_execution::ScanExecutionStatus::Complete,
            Some(true),
        ),
        "complete"
    );
}

#[test]
fn full_comparisons_require_a_baseline_for_every_completed_component() {
    assert!(has_complete_full_comparison_baseline(
        true, true, true, true,
    ));
    assert!(has_complete_full_comparison_baseline(
        true, true, false, false,
    ));
    assert!(!has_complete_full_comparison_baseline(
        true, true, true, false,
    ));
    assert!(!has_complete_full_comparison_baseline(
        true, false, true, true,
    ));
    assert!(!has_complete_full_comparison_baseline(
        false, false, false, false,
    ));
}

#[test]
fn full_comparison_baselines_must_come_from_one_execution() {
    assert_eq!(full_baseline_execution_id(Some(7), Some(7)), Some(7));
    assert_eq!(full_baseline_execution_id(Some(7), None), Some(7));
    assert_eq!(full_baseline_execution_id(None, Some(7)), Some(7));
    assert_eq!(full_baseline_execution_id(Some(7), Some(8)), None);
    assert_eq!(full_baseline_execution_id(None, None), None);
}

#[test]
fn scheduled_web_completion_includes_actionable_site_findings() {
    use crate::checks::{CheckResult, IssueConfidence, ScanCategory};
    use crate::core::scanner::MultiScanResult;

    let site_result = |status| CheckResult {
        check_id: "seo.duplicate_title".into(),
        category: ScanCategory::Seo,
        title: "Duplicate title".into(),
        description: String::new(),
        status,
        severity: Severity::Critical,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    };
    let multi_result = MultiScanResult {
        session_id: 42,
        total_pages: 2,
        completed_pages: 2,
        overall_score: 73,
        duration_ms: 900,
        incomplete_detail: None,
        page_results: Vec::new(),
        new_issue_count: Some(1),
        resolved_issue_count: Some(0),
        site_issues: vec![
            site_result(CheckStatus::Fail),
            site_result(CheckStatus::Skipped),
            site_result(CheckStatus::Pass),
        ],
    };

    let summary = summarize_scheduled_web_result(
        None,
        Some(&multi_result),
        None,
        None,
        None,
        "2026-08-29T12:00:00Z",
    )
    .expect("multi-page completion summary");

    assert_eq!(summary.counts.total, 1);
    assert_eq!(summary.counts.critical, 1);
}

#[test]
fn scheduled_web_completion_counts_only_actionable_single_page_findings() {
    use crate::checks::{CheckResult, IssueConfidence, ScanCategory};
    use crate::core::scanner::ScanResult;

    let issue = |status, severity| CheckResult {
        check_id: "security.headers".into(),
        category: ScanCategory::Security,
        title: "Security headers".into(),
        description: String::new(),
        status,
        severity,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    };
    let result = ScanResult {
        url: "https://example.com/".into(),
        mode: "live".into(),
        scan_type: ScanType::Security,
        overall_score: 91,
        categories: Vec::new(),
        issues: vec![
            issue(CheckStatus::Fail, Severity::Critical),
            issue(CheckStatus::Warn, Severity::High),
            issue(CheckStatus::Skipped, Severity::Critical),
            issue(CheckStatus::Pass, Severity::Critical),
        ],
        detected_stack: None,
        duration_ms: 400,
        timestamp: "2026-08-29T12:00:00Z".into(),
        page_signals: None,
        site_facts: None,
    };

    let summary = summarize_scheduled_web_result(
        Some(&result),
        None,
        None,
        Some(11),
        None,
        "2026-08-29T12:00:00Z",
    )
    .expect("single-page completion summary");

    assert_eq!(summary.counts.total, 2);
    assert_eq!(summary.counts.critical, 1);
    assert_eq!(summary.counts.high, 1);
}

#[test]
fn scheduled_web_completion_excludes_incomplete_single_page_coverage() {
    let result = ScanResult {
        url: "https://example.com/".into(),
        mode: "live".into(),
        scan_type: ScanType::Health,
        overall_score: 91,
        categories: Vec::new(),
        issues: Vec::new(),
        detected_stack: None,
        duration_ms: 400,
        timestamp: "2026-08-29T12:00:00Z".into(),
        page_signals: None,
        site_facts: None,
    };

    let summary = summarize_scheduled_web_result(
        Some(&result),
        None,
        Some("Browser analysis failed: unavailable"),
        Some(11),
        None,
        "2026-08-29T12:00:00Z",
    )
    .expect("incomplete single-page completion summary");

    assert!(!summary.scope_complete);
    assert!(!summary.comparison_eligible);
}

#[test]
fn scheduler_reaches_engines_only_through_execution_admission() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/background/scan_scheduler.rs"),
    )
    .expect("read scan_scheduler.rs");

    for bypass in [
        "run_code_scan_internal",
        "run_scan_low_priority",
        "post_scan_persist",
        "scan_multi_for_execution",
        "scan_url_for_execution",
    ] {
        assert!(
            !source.contains(bypass),
            "scan_scheduler.rs calls {bypass} directly; scheduled runs must go through \
             run_scan_execution_internal so admission can reserve quota"
        );
    }
    assert!(
        source.contains("run_scan_execution_internal"),
        "scan_scheduler.rs must drive scans through execution admission"
    );
    assert!(
        source.contains("mark_schedule_run"),
        "scan_scheduler.rs must advance the schedule after every attempt"
    );
}

#[test]
fn score_drop_notification_requires_meaningful_drop_or_new_critical() {
    assert!(should_notify_score_change(Some(90), 80, Some(0), 0));
    assert!(should_notify_score_change(Some(90), 89, Some(0), 1));
    assert!(!should_notify_score_change(Some(90), 89, Some(1), 1));
    assert!(!should_notify_score_change(Some(90), 89, Some(0), 0));
    assert!(!should_notify_score_change(Some(90), 95, Some(0), 1));
    assert!(should_notify_score_change(None, 95, None, 1));
    assert!(!should_notify_score_change(None, 95, None, 0));
}

#[test]
fn scheduler_notification_is_suppressed_when_blame_already_notified() {
    assert!(!should_send_scheduler_notification(
        true,
        Some(90),
        80,
        Some(0),
        0
    ));
    assert!(!should_send_scheduler_notification(
        true,
        Some(90),
        89,
        Some(0),
        1
    ));
    assert!(!should_send_scheduler_notification(true, None, 95, None, 1));
    // Without a blame ping the threshold decision is unchanged.
    assert!(should_send_scheduler_notification(
        false,
        Some(90),
        80,
        Some(0),
        0
    ));
    assert!(should_send_scheduler_notification(
        false,
        Some(90),
        89,
        Some(0),
        1
    ));
    assert!(!should_send_scheduler_notification(
        false,
        Some(90),
        89,
        Some(1),
        1
    ));
    assert!(should_send_scheduler_notification(false, None, 95, None, 1));
}

#[test]
fn full_scheduler_notification_compares_unified_snapshots() {
    fn snapshot(overall: f64, critical_count: usize) -> ScoreSnapshot {
        ScoreSnapshot {
            overall,
            per_category: std::collections::HashMap::new(),
            critical_count,
            high_count: 0,
            medium_count: 0,
            low_count: 0,
            exploitable_capped: false,
            breakdown: Default::default(),
            computed_at: 0,
        }
    }

    let previous = FullScoreBaseline {
        score: 88,
        critical: 1,
    };
    assert!(!should_send_full_scheduler_notification(
        true,
        false,
        false,
        Some(&previous),
        &snapshot(87.0, 1)
    ));
    assert!(should_send_full_scheduler_notification(
        true,
        false,
        false,
        Some(&previous),
        &snapshot(87.0, 2)
    ));
    assert!(should_send_full_scheduler_notification(
        true,
        false,
        false,
        Some(&previous),
        &snapshot(78.0, 1)
    ));
}

#[test]
fn full_scheduler_notification_keeps_uncovered_component_regressions() {
    let snapshot = |overall| ScoreSnapshot {
        overall,
        per_category: std::collections::HashMap::new(),
        critical_count: 0,
        high_count: 0,
        medium_count: 0,
        low_count: 0,
        exploitable_capped: false,
        breakdown: Default::default(),
        computed_at: 0,
    };
    let previous = FullScoreBaseline {
        score: 90,
        critical: 0,
    };
    let current = snapshot(79.0);

    assert!(!should_send_full_scheduler_notification(
        true,
        true,
        false,
        Some(&previous),
        &current,
    ));
    assert!(should_send_full_scheduler_notification(
        true,
        true,
        true,
        Some(&previous),
        &current,
    ));
}

#[test]
fn scan_completion_event_type_tracks_notification_state() {
    assert_eq!(scan_completion_event_type(true), "score_drop");
    assert_eq!(scan_completion_event_type(false), "scan_complete");
}

#[test]
fn hostname_for_url_prefers_host_and_falls_back_to_input() {
    assert_eq!(
        hostname_for_url("https://www.example.com/path"),
        "www.example.com"
    );
    assert_eq!(hostname_for_url("not a url"), "not a url");
}
