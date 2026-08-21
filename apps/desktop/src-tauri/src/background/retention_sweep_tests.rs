use super::*;

#[test]
fn sweep_summary_is_silent_when_nothing_was_removed() {
    assert_eq!(sweep_summary(&RetentionStats::default()), None);
}

#[test]
fn sweep_summary_reports_the_named_store_counts() {
    let stats = RetentionStats {
        dismissed_alerts: 2,
        old_events: 5,
        resolved_signal_items: 1,
        ..Default::default()
    };
    assert_eq!(
        sweep_summary(&stats).as_deref(),
        Some(
            "Retention sweep removed 2 dismissed alert(s), 5 old event(s), \
             1 resolved signal item(s)"
        )
    );
}

#[test]
fn sweep_summary_logs_when_only_an_unnamed_store_was_pruned() {
    let stats = RetentionStats {
        abandoned_scan_executions: 3,
        old_score_snapshots: 1,
        ..Default::default()
    };
    assert_eq!(
        sweep_summary(&stats).as_deref(),
        Some(
            "Retention sweep removed 0 dismissed alert(s), 0 old event(s), \
             0 resolved signal item(s)"
        )
    );
}
