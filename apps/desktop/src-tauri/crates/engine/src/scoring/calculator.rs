//! Portable 0-100 SiteCMD score calculator.
//!
//! Active findings use diminishing per-severity deductions. Confirmed critical
//! exposure classes cap the score. Category scores use the same curve.

use crate::cap::is_score_cap_candidate_check;
use crate::scoring::dedup;
use crate::vocab::{
    CheckResult, CheckStatus, IssueConfidence, IssueStatus, ScanCategory, Severity,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Stable web category display order, not a weight table.
const SCAN_CATEGORY_ORDER: [ScanCategory; 7] = [
    ScanCategory::Security,
    ScanCategory::Performance,
    ScanCategory::Seo,
    ScanCategory::Accessibility,
    ScanCategory::Compliance,
    ScanCategory::Config,
    ScanCategory::Polish,
];

/// Hard score cap for confirmed exploitable findings.
/// The frontend reads the resulting snapshot flag instead of duplicating it.
pub const EXPLOITABLE_SCORE_CAP: f64 = 49.0;

/// Hard minimum; only full-weight criticals may pass the zero-critical floor.
pub const SCORE_FLOOR: f64 = 5.0;

/// Protective floor used unless a full-weight critical or exploitable finding
/// makes the critical score band reachable.
const ZERO_CRITICAL_SCORE_FLOOR: f64 = 35.0;

/// Per-severity `(first cost, decay)` values, ordered critical to low.
/// Geometric decay bounds long backlogs.
const SEVERITY_DEDUCTION: [(f64, f64); 4] = [
    (15.0, 0.90), // critical
    (9.0, 0.90),  // high
    (4.0, 0.82),  // medium
    (1.5, 0.75),  // low
];

/// Apply a diminishing-returns deduction to a confidence-weighted count.
fn tier_deduction(base: f64, decay: f64, count: f64) -> f64 {
    if count <= 0.0 {
        return 0.0;
    }
    base * (1.0 - decay.powf(count)) / (1.0 - decay)
}

/// Apply the shared deduction curve in critical-to-low order.
fn tier_deductions(counts: [f64; 4]) -> [f64; 4] {
    let mut points = [0.0f64; 4];
    for (i, (&(base, decay), count)) in SEVERITY_DEDUCTION.iter().zip(counts).enumerate() {
        points[i] = tier_deduction(base, decay, count);
    }
    points
}

/// A perfect score means nothing is open, so any active issue keeps the score
/// off 100 even when its confidence-weighted deduction rounds away.
const OPEN_ISSUE_SCORE_CEILING: f64 = 99.0;

/// True when any severity tier carries an active, confidence-weighted finding.
fn has_active_issue(counts: [f64; 4]) -> bool {
    counts.iter().any(|count| *count > 0.0)
}

/// Computes a category score with the overall curve and floor, and the
/// open-issue ceiling; the exploitable cap stays an overall-score decision.
fn category_subscore(counts: [f64; 4]) -> u32 {
    let deduction: f64 = tier_deductions(counts).iter().sum();
    let score = (100.0 - deduction).round().clamp(SCORE_FLOOR, 100.0);
    if has_active_issue(counts) {
        score.min(OPEN_ISSUE_SCORE_CEILING) as u32
    } else {
        score as u32
    }
}

/// Computes the canonical SiteCMD Score from confidence-weighted issue counts.
///
/// Deduction is followed by the hard score floor, the no-critical floor, the
/// exploitable cap, and the open-issue ceiling. The frontend consumes this
/// result over IPC.
pub fn health_score_from_severity(
    critical: f64,
    high: f64,
    medium: f64,
    low: f64,
    has_full_weight_critical: bool,
    has_exploitable: bool,
) -> u32 {
    health_score_with_breakdown(
        critical,
        high,
        medium,
        low,
        has_full_weight_critical,
        has_exploitable,
    )
    .0
}

/// Computes the score and per-tier deduction breakdown together.
pub fn health_score_with_breakdown(
    critical: f64,
    high: f64,
    medium: f64,
    low: f64,
    has_full_weight_critical: bool,
    has_exploitable: bool,
) -> (u32, ScoreBreakdown) {
    let counts = [critical, high, medium, low];
    let points = tier_deductions(counts);
    let deduction: f64 = points.iter().sum();
    let mut score = (100.0 - deduction).round().clamp(SCORE_FLOOR, 100.0);
    let mut floor_applied = false;
    if !has_full_weight_critical && !has_exploitable {
        if score < ZERO_CRITICAL_SCORE_FLOOR {
            floor_applied = true;
        }
        score = score.max(ZERO_CRITICAL_SCORE_FLOOR);
    }
    if has_exploitable {
        score = score.min(EXPLOITABLE_SCORE_CAP);
    }
    let mut ceiling_applied = false;
    if has_active_issue(counts) {
        let held = score.min(OPEN_ISSUE_SCORE_CEILING);
        // Only report the ceiling when it moved the score. Most inputs already
        // land at or below it, and saying "held at 99" about a 99 the curve
        // produced on its own would explain a number with the wrong reason.
        ceiling_applied = held < score;
        score = held;
    }
    let breakdown = ScoreBreakdown {
        base: 100.0,
        critical_points: points[0],
        high_points: points[1],
        medium_points: points[2],
        low_points: points[3],
        eff_critical: critical,
        eff_high: high,
        eff_medium: medium,
        eff_low: low,
        floor_applied,
        ceiling_applied,
    };
    (score as u32, breakdown)
}

/// Confidence-weighted score deductions and effective counts by severity.
/// This is produced with the score to prevent UI reimplementation or drift;
/// the exploitable cap remains on `ScoreSnapshot`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "camelCase")]
pub struct ScoreBreakdown {
    /// The starting score every deduction is subtracted from (always 100.0).
    pub base: f64,
    /// Points lost to the critical tier (geometric, confidence-weighted).
    pub critical_points: f64,
    /// Points lost to the high tier.
    pub high_points: f64,
    /// Points lost to the medium tier.
    pub medium_points: f64,
    /// Points lost to the low tier.
    pub low_points: f64,
    /// Confidence-weighted effective critical count the deduction used.
    pub eff_critical: f64,
    /// Confidence-weighted effective high count.
    pub eff_high: f64,
    /// Confidence-weighted effective medium count.
    pub eff_medium: f64,
    /// Confidence-weighted effective low count.
    pub eff_low: f64,
    /// True when the zero-critical protective floor raised the final score
    /// (no full-weight critical, not exploitable, deduction otherwise below 35).
    pub floor_applied: bool,
    /// True when the open-issue ceiling, not the deduction arithmetic, set the
    /// final score. It only moves a score whose deductions round back to the
    /// base, which needs a tier count below 0.5: the Web Scan path reaches that
    /// (a Warn at NeedsReview confidence weighs 0.25), while `compute_score`'s
    /// lightest group weighs 0.5 and already lands on 99 by arithmetic, so a
    /// live snapshot never carries it. The UI reads this instead of inferring
    /// the ceiling from the numbers.
    pub ceiling_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "camelCase")]
pub struct ScoreSnapshot {
    pub overall: f64,
    pub per_category: HashMap<String, f64>,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    /// Whether an active finding met the mechanical critical-score cap gates.
    #[serde(default)]
    pub exploitable_capped: bool,
    /// Points lost per tier, defaulted so older persisted payloads deserialize.
    #[serde(default)]
    pub breakdown: ScoreBreakdown,
    pub computed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export_to = "ipc-bindings.ts"))]
#[serde(rename_all = "camelCase")]
pub struct CategoryScore {
    pub category: ScanCategory,
    pub score: u32,
    pub issues_total: u32,
    pub issues_critical: u32,
    pub issues_high: u32,
    pub issues_medium: u32,
    pub issues_low: u32,
    pub issues_passed: u32,
}

/// Combine verdict and confidence weights for one active result.
fn result_effective_weight(result: &CheckResult) -> f64 {
    let confidence = dedup::confidence_weight(Some(result.confidence));
    match result.status {
        CheckStatus::Warn => confidence * 0.5,
        _ => confidence, // Fail (Pass/Skipped are filtered out before this)
    }
}

/// Calculate category scores and overall score from check results.
///
/// A focused scan's issue counts are already scoped to that focus, so the
/// caller's scan type never enters the computation.
pub fn calculate_scores(results: &[CheckResult]) -> (u32, Vec<CategoryScore>) {
    calculate_scores_with_identity(results, |result| result.check_id.as_str())
}

/// As [`calculate_scores`], with `identity` naming the defect each result
/// reports. Two checks that grade one defect return one identity, and the
/// overall score then charges that defect once rather than once per check.
/// Category bars are untouched: each still reports what its own checks saw.
///
/// The desktop app passes the same canonical identity the issue store files a
/// finding under, so a defect that shows as one row in the list also costs one
/// deduction. A portable caller with no such mapping gets the check id, which
/// makes the collapse a no-op.
pub fn calculate_scores_with_identity<'a>(
    results: &'a [CheckResult],
    identity: impl Fn(&'a CheckResult) -> &'a str,
) -> (u32, Vec<CategoryScore>) {
    let mut categories: Vec<CategoryScore> = Vec::new();

    for cat in &SCAN_CATEGORY_ORDER {
        let cat_results: Vec<&CheckResult> =
            results.iter().filter(|r| r.category == *cat).collect();

        if cat_results.is_empty() {
            continue;
        }

        // Category scores use effective counts; display fields keep raw counts.
        let mut eff = [0.0f64; 4]; // critical, high, medium, low
        let mut issues_critical = 0u32;
        let mut issues_high = 0u32;
        let mut issues_medium = 0u32;
        let mut issues_low = 0u32;
        let mut issues_passed = 0u32;

        for result in &cat_results {
            match result.status {
                CheckStatus::Pass => {
                    issues_passed += 1;
                }
                CheckStatus::Fail | CheckStatus::Warn => {
                    eff[result.severity.sort_rank() as usize] += result_effective_weight(result);
                    match result.severity {
                        Severity::Critical => issues_critical += 1,
                        Severity::High => issues_high += 1,
                        Severity::Medium => issues_medium += 1,
                        Severity::Low => issues_low += 1,
                    }
                }
                CheckStatus::Skipped => {}
            }
        }

        let cat_score = category_subscore(eff);
        let issues_total = issues_critical + issues_high + issues_medium + issues_low;

        categories.push(CategoryScore {
            category: *cat,
            score: cat_score,
            issues_total,
            issues_critical,
            issues_high,
            issues_medium,
            issues_low,
            issues_passed,
        });
    }

    // No checks means unknown (0), not clean (100).
    if categories.is_empty() {
        return (0, categories);
    }

    // Route web results through the canonical SiteCMD score, collapsing the
    // results that report one defect so it deducts once. Cap eligibility and
    // the full-weight-critical flag stay tied to each result's own check id,
    // never stitched across the members of a collapsed row.
    let rows = dedup::dedup_score_rows(
        results
            .iter()
            .filter(|r| !matches!(r.status, CheckStatus::Pass | CheckStatus::Skipped))
            .map(|r| {
                let weight = result_effective_weight(r);
                dedup::ScoreFinding {
                    check_id: r.check_id.as_str(),
                    category: r.category.as_str(),
                    severity: r.severity,
                    cap_confidence: r.confidence.can_trigger_score_cap(),
                    weight,
                    // A half-weight (Warn or NeedsReview) critical must NOT arm
                    // the floor-piercing full-weight-critical flag.
                    full_weight_critical: r.severity == Severity::Critical && weight >= 1.0,
                    identity: Some(identity(r)),
                }
            }),
    );
    let counts = dedup::severity_counts(&rows);
    let overall = health_score_from_severity(
        counts.eff_critical,
        counts.eff_high,
        counts.eff_medium,
        counts.eff_low,
        counts.has_full_weight_critical,
        counts.has_cap_eligible,
    );

    (overall, categories)
}

// Portable live-score computation over lifecycle groups

/// Lifecycle categories seeded at 100 for dashboard rendering.
/// This is a membership list, not a weight table.
const SCORED_CATEGORIES: &[&str] = &[
    "security",
    "performance",
    "seo",
    "accessibility",
    "compliance",
    "config",
    "polish",
    "code_quality",
    "dependencies",
    "infrastructure",
    "operations",
    "supply-chain",
];

/// Score inputs for one group member, keeping severity and confidence paired.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreInputMember {
    pub severity: Severity,
    /// Missing confidence receives full weight.
    #[serde(default)]
    pub confidence: Option<IssueConfidence>,
}

/// Portable deduplicated lifecycle group consumed by desktop and hosted scorers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreInputGroup {
    pub check_id: String,
    /// Category the group penalizes (one of `SCORED_CATEGORIES`, or any other
    /// string - an unknown category still gets its own bar so the issue is
    /// never scored silently against nothing).
    pub category: String,
    /// Max severity across the group's members.
    pub severity: Severity,
    pub status: IssueStatus,
    #[serde(default)]
    pub snooze_until: Option<i64>,
    #[serde(default)]
    pub members: Vec<ScoreInputMember>,
}

impl ScoreInputGroup {
    /// Return whether the effective status contributes a penalty at `now_ms`.
    /// Expired snoozes become active again.
    pub fn is_active_for_scoring(&self, now_ms: i64) -> bool {
        !self
            .status
            .effective(self.snooze_until, now_ms)
            .is_inactive_for_scoring()
    }

    /// Require one member to satisfy severity, check class, and confidence
    /// together; cap eligibility is never stitched across group members.
    fn can_trigger_cap(&self) -> bool {
        self.members.iter().any(|member| {
            member.severity == Severity::Critical
                && member
                    .confidence
                    .is_some_and(IssueConfidence::can_trigger_score_cap)
                && is_score_cap_candidate_check(&self.check_id, member.severity)
        })
    }

    /// The group's confidence weight for the effective count: the
    /// highest-confidence member (max weight), or full weight when the group
    /// has no members (absence of confidence means observed, not uncertain).
    fn confidence_weight(&self) -> f64 {
        self.members
            .iter()
            .map(|member| dedup::confidence_weight(member.confidence))
            .reduce(f64::max)
            .unwrap_or(1.0)
    }

    /// Require one member to carry both Critical severity and full confidence.
    /// Member-less groups retain the protective floor.
    fn has_full_weight_critical(&self) -> bool {
        self.members.iter().any(|member| {
            member.severity == Severity::Critical
                && dedup::confidence_weight(member.confidence) >= 1.0
        })
    }
}

/// Computes a live score from deduplicated, active groups using diminishing
/// severity deductions and the explicit exploitable cap. `now_ms` controls the
/// active-set predicate and snapshot timestamp.
pub fn compute_score(groups: &[ScoreInputGroup], now_ms: i64) -> ScoreSnapshot {
    // Deduplicate code locations before counting; severity uses the maximum
    // while cap eligibility remains tied to each member's own evidence.
    let rows = dedup::dedup_score_rows(
        groups
            .iter()
            .filter(|g| g.is_active_for_scoring(now_ms))
            .map(|g| dedup::ScoreFinding {
                check_id: &g.check_id,
                category: &g.category,
                severity: g.severity,
                cap_confidence: g.can_trigger_cap(),
                weight: g.confidence_weight(),
                full_weight_critical: g.has_full_weight_critical(),
                // Stored groups already carry the canonical check id.
                identity: None,
            }),
    );
    // Category bars use the overall curve and retain unseeded categories.
    let mut per_cat_counts: HashMap<&str, [f64; 4]> = SCORED_CATEGORIES
        .iter()
        .map(|&c| (c, [0.0f64; 4]))
        .collect();
    for row in &rows {
        per_cat_counts.entry(row.category).or_insert([0.0; 4])
            [row.severity.sort_rank() as usize] += row.weight;
    }
    let per_category: HashMap<String, f64> = per_cat_counts
        .iter()
        .map(|(cat, counts)| (cat.to_string(), category_subscore(*counts) as f64))
        .collect();

    let counts = dedup::severity_counts(&rows);

    // Persist integer display counts beside the confidence-weighted curve.
    let (overall_u32, breakdown) = health_score_with_breakdown(
        counts.eff_critical,
        counts.eff_high,
        counts.eff_medium,
        counts.eff_low,
        counts.has_full_weight_critical,
        counts.has_cap_eligible,
    );

    ScoreSnapshot {
        overall: overall_u32 as f64,
        per_category,
        critical_count: counts.critical as usize,
        high_count: counts.high as usize,
        medium_count: counts.medium as usize,
        low_count: counts.low as usize,
        exploitable_capped: counts.has_cap_eligible,
        breakdown,
        computed_at: now_ms,
    }
}

#[cfg(test)]
#[path = "calculator_tests.rs"]
mod tests;
