//! Baseline and provenance policy for scheduled regression comparisons.

use crate::{
    core::{
        scan_execution::{ScanExecutionMode, ScanExecutionStatus, ScanTrigger},
        types_work_items::ScoreSnapshot,
    },
    db::Database,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct FullScoreBaseline {
    pub(super) score: u32,
    pub(super) critical: usize,
}

pub(super) fn scan_provenance_matches_previous(
    db: &Database,
    previous_run_id: Option<i64>,
    current_run_id: Option<i64>,
) -> bool {
    let Some(previous_run_id) = previous_run_id else {
        return true;
    };
    let Some(current_run_id) = current_run_id else {
        return false;
    };
    match db.scan_runs_have_matching_score_provenance(previous_run_id, current_run_id) {
        Ok(matches) => matches,
        Err(error) => {
            tracing::warn!(
                previous_run_id,
                current_run_id,
                "Scheduled scan could not compare run provenance: {error}"
            );
            false
        }
    }
}

pub(super) fn scheduled_completion_status(
    execution_status: ScanExecutionStatus,
    web_scope_complete: Option<bool>,
) -> &'static str {
    if execution_status == ScanExecutionStatus::Complete && web_scope_complete.unwrap_or(true) {
        "complete"
    } else {
        "partial"
    }
}

pub(super) fn has_complete_full_comparison_baseline(
    web_completed: bool,
    web_baseline_comparable: bool,
    code_completed: bool,
    code_baseline_comparable: bool,
) -> bool {
    (web_completed || code_completed)
        && (!web_completed || web_baseline_comparable)
        && (!code_completed || code_baseline_comparable)
}

pub(super) fn full_baseline_execution_id(
    web_execution_id: Option<i64>,
    code_execution_id: Option<i64>,
) -> Option<i64> {
    match (web_execution_id, code_execution_id) {
        (Some(web), Some(code)) if web == code => Some(web),
        (Some(web), None) => Some(web),
        (None, Some(code)) => Some(code),
        _ => None,
    }
}

pub(super) fn load_full_score_baseline(
    db: &Database,
    web_run_id: Option<i64>,
    code_run_id: Option<i64>,
) -> Option<FullScoreBaseline> {
    let execution_for_run = |run_id| match db.get_scan_run_execution_id(run_id) {
        Ok(execution_id) => execution_id,
        Err(error) => {
            tracing::warn!(
                run_id,
                "Could not resolve scheduled baseline execution: {error}"
            );
            None
        }
    };
    let execution_id = full_baseline_execution_id(
        web_run_id.and_then(execution_for_run),
        code_run_id.and_then(execution_for_run),
    )?;
    let detail = match db.get_scan_execution_detail(execution_id) {
        Ok(Some(detail)) => detail,
        Ok(None) => return None,
        Err(error) => {
            tracing::warn!(
                execution_id,
                "Could not load scheduled Full Scan baseline: {error}"
            );
            return None;
        }
    };
    let summary = detail.summary;
    if summary.trigger != ScanTrigger::Scheduled
        || summary.requested_mode != ScanExecutionMode::Full
        || summary.status != ScanExecutionStatus::Complete
    {
        return None;
    }
    let score = summary
        .score
        .filter(|score| score.is_finite() && (0.0..=100.0).contains(score))?
        .round() as u32;
    Some(FullScoreBaseline {
        score,
        critical: summary.critical_count? as usize,
    })
}

pub(super) fn should_notify_score_change(
    prev_score: Option<u32>,
    new_score: u32,
    prev_critical: Option<usize>,
    new_critical: usize,
) -> bool {
    if let Some(prev) = prev_score {
        let drop = prev as i32 - new_score as i32;
        let critical_increased = prev_critical.is_some_and(|count| new_critical > count);
        drop >= 10 || (critical_increased && drop > 0)
    } else {
        new_critical > 0
    }
}

pub(super) fn should_send_scheduler_notification(
    blame_notified: bool,
    prev_score: Option<u32>,
    new_score: u32,
    prev_critical: Option<usize>,
    new_critical: usize,
) -> bool {
    !blame_notified
        && should_notify_score_change(prev_score, new_score, prev_critical, new_critical)
}

pub(super) fn should_send_full_scheduler_notification(
    comparison_eligible: bool,
    blame_notified: bool,
    uncovered_component_regression: bool,
    previous: Option<&FullScoreBaseline>,
    current: &ScoreSnapshot,
) -> bool {
    comparison_eligible
        && should_notify_score_change(
            previous.map(|snapshot| snapshot.score),
            current.overall.round() as u32,
            previous.map(|snapshot| snapshot.critical),
            current.critical_count,
        )
        && (!blame_notified || uncovered_component_regression)
}

pub(super) fn scan_completion_event_type(should_notify: bool) -> &'static str {
    if should_notify {
        "score_drop"
    } else {
        "scan_complete"
    }
}
