//! Adapt desktop issue groups to the portable scoring model while retaining
//! desktop-only impact-ranking weights.

use crate::checks::Severity;
use crate::core::types_work_items::{IssueGroup, ScoreSnapshot};
use sitecmd_engine::scoring::calculator::{compute_score, ScoreInputGroup, ScoreInputMember};

/// Desktop-only severity penalty for impact ranking over deduplicated groups.
/// Portable score calculation uses the geometric curve instead.
pub(crate) fn group_severity_penalty(severity: Severity) -> f64 {
    match severity {
        Severity::Critical => 15.0,
        Severity::High => 8.0,
        Severity::Medium => 4.0,
        Severity::Low => 1.0,
    }
}

/// Preserve per-instance severity and confidence for the portable scorer.
fn input_group(group: &IssueGroup) -> ScoreInputGroup {
    ScoreInputGroup {
        check_id: group.check_id.clone(),
        category: group.category.clone(),
        severity: group.severity,
        status: group.status,
        snooze_until: group.snooze_until,
        members: group
            .instances
            .iter()
            .map(|instance| ScoreInputMember {
                severity: instance.severity,
                confidence: instance.confidence,
            })
            .collect(),
    }
}

/// Computes the live score through the shared engine model, including
/// diminishing severity deductions and the explicit exploitable cap.
pub fn compute_current_score(groups: &[IssueGroup], now_ms: i64) -> ScoreSnapshot {
    let inputs: Vec<ScoreInputGroup> = groups.iter().map(input_group).collect();
    compute_score(&inputs, now_ms)
}
