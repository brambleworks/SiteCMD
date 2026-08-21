use super::*;

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
    )
    .expect("scheduled request");
    let retry = build_scheduled_execution_request(
        &schedule,
        "https://example.com",
        scope(),
        Some("/tmp/example".into()),
    )
    .expect("scheduled retry");

    assert_eq!(first.requested_mode, ScanExecutionMode::Full);
    assert_eq!(first.trigger, ScanTrigger::Scheduled);
    assert_eq!(first.web_focus, Some(ScanType::Health));
    assert_eq!(first.idempotency_key, retry.idempotency_key);

    schedule.next_run_at = Some("2026-07-23 09:00:00".into());
    let next = build_scheduled_execution_request(
        &schedule,
        "https://example.com",
        scope(),
        Some("/tmp/example".into()),
    )
    .expect("next occurrence");
    assert_ne!(first.idempotency_key, next.idempotency_key);

    assert_eq!(first.urls, scope());
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
    assert!(should_notify_score_change(Some(90), 80, 0));
    assert!(should_notify_score_change(Some(90), 89, 1));
    assert!(!should_notify_score_change(Some(90), 89, 0));
    assert!(!should_notify_score_change(Some(90), 95, 1));
    assert!(should_notify_score_change(None, 95, 1));
    assert!(!should_notify_score_change(None, 95, 0));
}

#[test]
fn scheduler_notification_is_suppressed_when_blame_already_notified() {
    assert!(!should_send_scheduler_notification(true, Some(90), 80, 0));
    assert!(!should_send_scheduler_notification(true, Some(90), 89, 1));
    assert!(!should_send_scheduler_notification(true, None, 95, 1));
    // Without a blame ping the threshold decision is unchanged.
    assert!(should_send_scheduler_notification(false, Some(90), 80, 0));
    assert!(should_send_scheduler_notification(false, Some(90), 89, 1));
    assert!(!should_send_scheduler_notification(false, Some(90), 89, 0));
    assert!(should_send_scheduler_notification(false, None, 95, 1));
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
