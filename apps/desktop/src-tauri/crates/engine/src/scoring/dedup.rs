//! Shared score-input deduplication.
//!
//! Code findings collapse by rule using maximum severity. Cap eligibility and
//! confidence remain tied to the same concrete finding.

use crate::cap::is_score_cap_candidate_check;
use crate::identity::code_rule_id;
use crate::vocab::{IssueConfidence, Severity};
use std::cmp::Ordering;
use std::collections::HashMap;

/// Weight NeedsReview findings at half; all other observations retain full weight.
pub fn confidence_weight(confidence: Option<IssueConfidence>) -> f64 {
    match confidence {
        Some(IssueConfidence::NeedsReview) => 0.5,
        _ => 1.0,
    }
}

/// One pre-dedup finding supplied to the score.
pub struct ScoreFinding<'a> {
    pub check_id: &'a str,
    pub category: &'a str,
    pub severity: Severity,
    pub cap_confidence: bool,
    /// Confidence weight of this member (see `confidence_weight`). The curve
    /// counts the highest-confidence member AT each deduped row's max
    /// severity, never a weaker sibling's confidence.
    pub weight: f64,
    /// Whether this member alone is a full-weight Critical finding.
    pub full_weight_critical: bool,
}

/// One deduped display row, aggregated across its members.
pub struct ScoreRow<'a> {
    /// Representative member check ID.
    pub check_id: &'a str,
    /// Category of the member that set the row's severity.
    pub category: &'a str,
    /// MAX severity across members.
    pub severity: Severity,
    /// Whether one member independently satisfies every exploitable-cap gate.
    pub cap_eligible: bool,
    /// Maximum confidence weight among members at the row's maximum severity.
    pub weight: f64,
    /// Whether one member is independently a full-weight critical.
    pub full_weight_critical: bool,
}

/// Whether one finding meets the mechanical exploitable-cap gates on its own:
/// a cap-candidate check at the finding's severity, carrying explicit
/// High/Confirmed confidence.
fn finding_cap_eligible(finding: &ScoreFinding<'_>) -> bool {
    is_score_cap_candidate_check(finding.check_id, finding.severity) && finding.cap_confidence
}

/// Collapse findings into display rows, preserving first-seen order.
pub fn dedup_score_rows<'a>(
    findings: impl IntoIterator<Item = ScoreFinding<'a>>,
) -> Vec<ScoreRow<'a>> {
    let mut index_by_key: HashMap<&'a str, usize> = HashMap::new();
    let mut rows: Vec<ScoreRow<'a>> = Vec::new();
    for finding in findings {
        let key = code_rule_id(finding.check_id).unwrap_or(finding.check_id);
        let cap_eligible = finding_cap_eligible(&finding);
        match index_by_key.get(key) {
            None => {
                index_by_key.insert(key, rows.len());
                rows.push(ScoreRow {
                    check_id: finding.check_id,
                    category: finding.category,
                    severity: finding.severity,
                    cap_eligible,
                    weight: finding.weight,
                    full_weight_critical: finding.full_weight_critical,
                });
            }
            Some(&i) => {
                let row = &mut rows[i];
                match finding.severity.sort_rank().cmp(&row.severity.sort_rank()) {
                    // Strictly more severe: this member now owns the row's
                    // severity, so its own confidence weight replaces the
                    // previous owner's (severity and weight stay paired).
                    Ordering::Less => {
                        row.severity = finding.severity;
                        row.category = finding.category;
                        row.weight = finding.weight;
                    }
                    // Same severity: the highest-confidence member AT the
                    // row's max severity sets the weight.
                    Ordering::Equal => row.weight = row.weight.max(finding.weight),
                    // Weaker member: never contributes severity or weight
                    // (a Confirmed Low sibling must not inflate a NeedsReview
                    // Critical row to full weight).
                    Ordering::Greater => {}
                }
                row.cap_eligible |= cap_eligible;
                row.full_weight_critical |= finding.full_weight_critical;
            }
        }
    }
    rows
}

/// Deduplicated display counts, confidence-weighted curve counts, and cap flags.
/// Display and effective counts differ for reduced-confidence rows.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DedupedSeverityCounts {
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub eff_critical: f64,
    pub eff_high: f64,
    pub eff_medium: f64,
    pub eff_low: f64,
    pub has_cap_eligible: bool,
    pub has_full_weight_critical: bool,
}

/// Count deduped rows by severity (integer + confidence-weighted) and resolve
/// the exploitable-cap and full-weight-critical flags.
pub fn severity_counts(rows: &[ScoreRow<'_>]) -> DedupedSeverityCounts {
    let mut counts = DedupedSeverityCounts::default();
    for row in rows {
        match row.severity {
            Severity::Critical => {
                counts.critical += 1;
                counts.eff_critical += row.weight;
            }
            Severity::High => {
                counts.high += 1;
                counts.eff_high += row.weight;
            }
            Severity::Medium => {
                counts.medium += 1;
                counts.eff_medium += row.weight;
            }
            Severity::Low => {
                counts.low += 1;
                counts.eff_low += row.weight;
            }
        }
        if row.cap_eligible {
            counts.has_cap_eligible = true;
        }
        if row.full_weight_critical {
            counts.has_full_weight_critical = true;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(check_id: &str, severity: Severity, cap_confidence: bool) -> ScoreFinding<'_> {
        // Default: full-weight observation; full_weight_critical follows severity.
        ScoreFinding {
            check_id,
            category: "security",
            severity,
            cap_confidence,
            weight: 1.0,
            full_weight_critical: severity == Severity::Critical,
        }
    }

    fn fw(check_id: &str, severity: Severity, weight: f64) -> ScoreFinding<'_> {
        ScoreFinding {
            check_id,
            category: "security",
            severity,
            cap_confidence: false,
            weight,
            full_weight_critical: severity == Severity::Critical && weight >= 1.0,
        }
    }

    #[test]
    fn duplicate_code_group_evidence_collapses_to_one_row_web_ids_stay_distinct() {
        let rows = dedup_score_rows([
            f("code_scan.n-plus-one-query", Severity::High, false),
            f("code_scan.n-plus-one-query", Severity::High, false),
            f("security.hsts", Severity::High, false),
            f("security.csp", Severity::High, false),
        ]);
        assert_eq!(rows.len(), 3, "one code row + two web rows");
        let counts = severity_counts(&rows);
        assert_eq!(counts.high, 3);
        assert_eq!(counts.critical + counts.medium + counts.low, 0);
    }

    #[test]
    fn row_severity_is_the_max_across_files_not_the_first_seen() {
        // The highest severity wins regardless of evidence order.
        let rows = dedup_score_rows([
            f("code_scan.some-rule", Severity::Low, false),
            f("code_scan.some-rule", Severity::Critical, false),
        ]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].severity, Severity::Critical);
        let counts = severity_counts(&rows);
        assert_eq!((counts.critical, counts.low), (1, 0));
    }

    #[test]
    fn severity_never_downgrades_when_a_weaker_member_comes_later() {
        // Negative control for the max logic: Critical first, Low later.
        let rows = dedup_score_rows([
            f("code_scan.some-rule", Severity::Critical, false),
            f("code_scan.some-rule", Severity::Low, false),
        ]);
        assert_eq!(rows[0].severity, Severity::Critical);
    }

    #[test]
    fn cap_eligibility_needs_one_member_meeting_every_gate_itself() {
        let rows = dedup_score_rows([
            f("code_scan.js-command-injection", Severity::Critical, true),
            f("code_scan.js-command-injection", Severity::Low, false),
        ]);
        assert!(severity_counts(&rows).has_cap_eligible);

        let rows = dedup_score_rows([
            f("code_scan.js-command-injection", Severity::Critical, false),
            f("code_scan.js-command-injection", Severity::Low, true),
        ]);
        assert!(!severity_counts(&rows).has_cap_eligible);

        // Negative controls: drop one gate at a time on a single member.
        let rows = dedup_score_rows([f("code_scan.js-command-injection", Severity::High, true)]);
        assert!(!severity_counts(&rows).has_cap_eligible);

        let rows = dedup_score_rows([f("code_scan.god-route", Severity::Critical, true)]);
        assert!(!severity_counts(&rows).has_cap_eligible);

        let rows = dedup_score_rows([f(
            "code_scan.js-command-injection",
            Severity::Critical,
            false,
        )]);
        assert!(!severity_counts(&rows).has_cap_eligible);
    }

    #[test]
    fn category_follows_the_member_that_set_the_max_severity() {
        let rows = dedup_score_rows([
            ScoreFinding {
                check_id: "code_scan.some-rule",
                category: "code_quality",
                severity: Severity::Low,
                cap_confidence: false,
                weight: 1.0,
                full_weight_critical: false,
            },
            ScoreFinding {
                check_id: "code_scan.some-rule",
                category: "security",
                severity: Severity::Critical,
                cap_confidence: false,
                weight: 1.0,
                full_weight_critical: true,
            },
        ]);
        assert_eq!(rows[0].category, "security");
    }

    #[test]
    fn row_weight_is_the_highest_confidence_member_and_feeds_effective_counts() {
        let all_needs_review = dedup_score_rows([
            fw("code_scan.some-rule", Severity::High, 0.5),
            fw("code_scan.some-rule", Severity::High, 0.5),
            fw("code_scan.some-rule", Severity::High, 0.5),
        ]);
        let counts = severity_counts(&all_needs_review);
        assert_eq!(counts.high, 1, "one integer display row");
        assert_eq!(counts.eff_high, 0.5, "weighted by the row's confidence");

        let with_confident_member = dedup_score_rows([
            fw("code_scan.some-rule", Severity::High, 0.5),
            fw("code_scan.some-rule", Severity::High, 1.0),
        ]);
        let counts = severity_counts(&with_confident_member);
        assert_eq!(counts.high, 1);
        assert_eq!(
            counts.eff_high, 1.0,
            "highest-confidence member sets weight"
        );
    }

    #[test]
    fn row_weight_pairs_with_the_max_severity_member_never_a_weaker_sibling() {
        let rows = dedup_score_rows([
            fw("code_scan.some-rule", Severity::Critical, 0.5),
            fw("code_scan.some-rule", Severity::Low, 1.0),
        ]);
        let counts = severity_counts(&rows);
        assert_eq!((counts.critical, counts.low), (1, 0));
        assert_eq!(
            counts.eff_critical, 0.5,
            "weight follows the max-severity member, not the confident Low"
        );
        assert_eq!(counts.eff_low, 0.0);

        let rows = dedup_score_rows([
            fw("code_scan.some-rule", Severity::Low, 1.0),
            fw("code_scan.some-rule", Severity::Critical, 0.5),
        ]);
        assert_eq!(severity_counts(&rows).eff_critical, 0.5);

        // Positive control: a Confirmed sibling AT the same max severity does
        // lift the row to full weight.
        let rows = dedup_score_rows([
            fw("code_scan.some-rule", Severity::Critical, 0.5),
            fw("code_scan.some-rule", Severity::Critical, 1.0),
        ]);
        assert_eq!(severity_counts(&rows).eff_critical, 1.0);
    }

    #[test]
    fn full_weight_critical_pairs_per_member_never_stitched() {
        let stitched = dedup_score_rows([
            fw("code_scan.some-rule", Severity::Critical, 0.5),
            fw("code_scan.some-rule", Severity::Low, 1.0),
        ]);
        assert!(!severity_counts(&stitched).has_full_weight_critical);

        // A single confirmed Critical member registers.
        let real = dedup_score_rows([fw("code_scan.some-rule", Severity::Critical, 1.0)]);
        assert!(severity_counts(&real).has_full_weight_critical);
    }

    #[test]
    fn rows_preserve_first_seen_order() {
        let rows = dedup_score_rows([
            f("security.hsts", Severity::Low, false),
            f("code_scan.rule-b", Severity::High, false),
            f("security.csp", Severity::Medium, false),
            f("code_scan.rule-b", Severity::High, false),
        ]);
        let keys: Vec<&str> = rows.iter().map(|row| row.check_id).collect();
        assert_eq!(
            keys,
            vec!["security.hsts", "code_scan.rule-b", "security.csp"]
        );
    }
}
