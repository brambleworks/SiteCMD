//! Live-score and health-score curve tests.

use super::*;
use crate::core::types_work_items::{IssueGroup, IssueInstance};

fn g(check_id: &str, category: &str, severity: &str, status: &str) -> IssueGroup {
    IssueGroup {
        check_id: check_id.into(),
        category: category.into(),
        severity: severity.parse().expect("valid severity"),
        title: "t".into(),
        description: "d".into(),
        instances: Vec::<IssueInstance>::new(),
        sources: vec!["web_scan".into()],
        status: status.parse().expect("valid issue status"),
        snooze_until: None,
        block_reason: None,
        impact_score: 0.0,
        likely_causes: Vec::new(),
        suggested_integrations: Vec::new(),
        fix_locations: Vec::new(),
        transitive_causes: Vec::new(),
        downstream_effects: Vec::new(),
        recent_events: Vec::new(),
        enrichments: Vec::new(),
        correlation_evidence: Vec::new(),
        affected_pages: Vec::new(),
        cross_env_signal: None,
        cross_project_pattern: None,
        display_confidence: None,
        observation_count: 0,
        anomaly_score: None,
    }
}

#[test]
fn perfect_site_scores_100() {
    let s = compute_current_score(&[], 0);
    assert_eq!(s.overall, 100.0);
    assert_eq!(s.critical_count, 0);
}

#[test]
fn one_high_deducts_gently_no_cap() {
    // A lone high lands in the Good band (no 79 ceiling cliff).
    let s = compute_current_score(&[g("security.hsts", "security", "high", "new")], 0);
    assert_eq!(
        s.overall, 91.0,
        "one high should deduct gently, was {}",
        s.overall
    );
    assert!(!s.exploitable_capped);
    assert_eq!(s.high_count, 1);
}

#[test]
fn one_critical_deducts_but_does_not_tank() {
    let s = compute_current_score(
        &[g(
            "architecture.god-file",
            "architecture",
            "critical",
            "new",
        )],
        0,
    );
    assert_eq!(
        s.overall, 85.0,
        "one critical should not tank the score, was {}",
        s.overall
    );
    assert!(!s.exploitable_capped);
    assert_eq!(s.critical_count, 1);
}

#[test]
fn exploitable_security_issue_caps_in_the_red() {
    // A Critical cap-class issue with explicit verified confidence is
    // force-capped at 49. Static NeedsReview instances do not qualify.
    let mut verified = g(
        "code_scan.js-command-injection",
        "security",
        "critical",
        "new",
    );
    verified.instances = vec![instance_with_confidence("confirmed")];
    let s = compute_current_score(&[verified], 0);
    assert!(
        s.overall <= 49.0,
        "exploitable issue must cap in the red, was {}",
        s.overall
    );
    assert!(s.exploitable_capped);
}

fn instance_with_confidence(confidence: &str) -> IssueInstance {
    IssueInstance {
        id: 1,
        source: "code_scan".into(),
        signal_id: "code_scan:js-command-injection:src/api/export.ts".into(),
        producer_check_id: None,
        url: None,
        page_url: None,
        severity: Severity::Critical,
        title: "t".into(),
        description: "d".into(),
        category: None,
        check_status: None,
        fix_prompt: None,
        manual_fix: None,
        why_it_matters: None,
        detail_json: Some("{}".into()),
        first_seen_at: 0,
        last_seen_at: 0,
        confidence: Some(confidence.parse().expect("valid confidence")),
        confidence_reason: None,
        domain: None,
        relative_path: None,
        line: None,
        producer_fix_prompt: None,
        producer_category: None,
    }
}

#[test]
fn needs_review_exploitable_does_not_cap_the_live_score() {
    let mut needs_review = g(
        "code_scan.js-command-injection",
        "security",
        "critical",
        "new",
    );
    needs_review.instances = vec![instance_with_confidence("needs_review")];
    let s = compute_current_score(&[needs_review], 0);
    assert!(!s.exploitable_capped);
    assert_eq!(
        s.overall, 92.0,
        "needs_review exploitable deducts at half weight, was {}",
        s.overall
    );

    let mut confirmed = g(
        "code_scan.js-command-injection",
        "security",
        "critical",
        "new",
    );
    confirmed.instances = vec![instance_with_confidence("confirmed")];
    let s = compute_current_score(&[confirmed], 0);
    assert!(s.exploitable_capped);
    assert!(s.overall <= 49.0);
}

#[test]
fn noncritical_cap_class_group_does_not_cap_the_live_score() {
    let mut high = g(
        "code_scan.python-command-injection",
        "security",
        "high",
        "new",
    );
    let mut instance = instance_with_confidence("high");
    instance.severity = Severity::High;
    high.instances = vec![instance];

    let score = compute_current_score(&[high], 0);
    assert_eq!(score.overall, 91.0);
    assert!(!score.exploitable_capped);
}

#[test]
fn missing_or_instance_less_confidence_never_triggers_the_cap() {
    let empty = g(
        "code_scan.js-command-injection",
        "security",
        "critical",
        "new",
    );
    let empty_score = compute_current_score(&[empty], 0);
    assert_eq!(empty_score.overall, 85.0);
    assert!(!empty_score.exploitable_capped);

    let mut missing = g(
        "code_scan.js-command-injection",
        "security",
        "critical",
        "new",
    );
    let mut instance = instance_with_confidence("high");
    instance.confidence = None;
    missing.instances = vec![instance];
    let missing_score = compute_current_score(&[missing], 0);
    assert_eq!(missing_score.overall, 85.0);
    assert!(!missing_score.exploitable_capped);
}

#[test]
fn within_group_cap_gates_pair_per_instance() {
    let mut mixed = g(
        "code_scan.js-command-injection",
        "security",
        "critical",
        "new",
    );
    let critical_needs_review = instance_with_confidence("needs_review");
    let mut confirmed_low = instance_with_confidence("confirmed");
    confirmed_low.severity = Severity::Low;
    mixed.instances = vec![critical_needs_review, confirmed_low];
    let s = compute_current_score(&[mixed], 0);
    assert!(
        !s.exploitable_capped,
        "no single instance is both Critical and cap-confidence"
    );

    // Positive control: the SAME instance carrying Critical + Confirmed caps,
    // even with a weaker unconfirmed sibling in the group.
    let mut capping = g(
        "code_scan.js-command-injection",
        "security",
        "critical",
        "new",
    );
    let confirmed_critical = instance_with_confidence("confirmed");
    let mut needs_review_low = instance_with_confidence("needs_review");
    needs_review_low.severity = Severity::Low;
    capping.instances = vec![confirmed_critical, needs_review_low];
    let s = compute_current_score(&[capping], 0);
    assert!(s.exploitable_capped);
    assert!(s.overall <= 49.0);
}

#[test]
fn fixing_one_of_two_criticals_moves_the_score() {
    let two = compute_current_score(
        &[
            g("architecture.a", "architecture", "critical", "new"),
            g("architecture.b", "architecture", "critical", "new"),
        ],
        0,
    );
    let one = compute_current_score(&[g("architecture.a", "architecture", "critical", "new")], 0);
    assert!(
        one.overall > two.overall + 5.0,
        "fixing one of two criticals should move the score: {} -> {}",
        two.overall,
        one.overall
    );
}

#[test]
fn snoozed_ignored_blocked_verified_groups_do_not_penalize() {
    let groups = vec![
        g("a", "security", "critical", "snoozed"),
        g("b", "security", "high", "ignored"),
        g("c", "security", "medium", "blocked"),
        g("d", "security", "low", "verified"),
    ];
    let s = compute_current_score(&groups, 0);
    assert_eq!(s.overall, 100.0);
    assert_eq!(s.critical_count, 0);
    assert_eq!(s.high_count, 0);
}

#[test]
fn multi_file_code_issue_counts_and_penalizes_once() {
    let two_files = vec![
        g("code_scan.n-plus-one-query", "code_quality", "high", "new"),
        g("code_scan.n-plus-one-query", "code_quality", "high", "new"),
    ];
    let s = compute_current_score(&two_files, 0);
    assert_eq!(
        s.high_count, 1,
        "two locations of one rule count as one high"
    );

    let one_file = compute_current_score(
        &[g(
            "code_scan.n-plus-one-query",
            "code_quality",
            "high",
            "new",
        )],
        0,
    );
    assert_eq!(
        s.overall, one_file.overall,
        "the multi-file issue must not be penalized twice"
    );

    // Distinct web check_ids stay distinct (1:1 with their row).
    let two_web = compute_current_score(
        &[
            g("security.hsts", "security", "high", "new"),
            g("security.csp", "security", "high", "new"),
        ],
        0,
    );
    assert_eq!(two_web.high_count, 2);
}

#[test]
fn score_snapshot_carries_a_points_breakdown_that_matches_the_number() {
    // The score and its tier breakdown must stay coupled.
    let s = compute_current_score(
        &[
            g("security.hsts", "security", "high", "new"),
            g("security.csp", "security", "high", "new"),
            g("seo.title", "seo", "medium", "new"),
        ],
        0,
    );
    let b = &s.breakdown;
    assert_eq!(b.base, 100.0);
    assert_eq!(
        (b.eff_critical, b.eff_high, b.eff_medium, b.eff_low),
        (0.0, 2.0, 1.0, 0.0)
    );
    assert_eq!(b.critical_points, 0.0);
    assert_eq!(b.low_points, 0.0);
    assert!(b.high_points > 0.0 && b.medium_points > 0.0);
    assert!(!b.floor_applied);
    // overall == base - sum of tier points (no floor/cap applies here).
    let total = b.critical_points + b.high_points + b.medium_points + b.low_points;
    assert_eq!(s.overall, (100.0 - total).round().clamp(SCORE_FLOOR, 100.0));
}

#[test]
fn breakdown_flags_the_zero_critical_floor_only_when_it_applies() {
    // 20 highs deduct to ~21 but the zero-critical floor raises to 35; the
    // breakdown records that the floor was applied.
    let (score, applied) = health_score_with_breakdown(0.0, 20.0, 0.0, 0.0, false, false);
    assert_eq!(score, 35);
    assert!(applied.floor_applied);
    // Negative control: a full-weight critical disables the floor, so it is not
    // "applied" even though the deduction is far deeper.
    let (_, not_applied) = health_score_with_breakdown(0.0, 20.0, 0.0, 0.0, true, false);
    assert!(!not_applied.floor_applied);
    // Negative control: a light load never reaches the floor.
    let (_, light) = health_score_with_breakdown(0.0, 1.0, 0.0, 0.0, false, false);
    assert!(!light.floor_applied);
}

#[test]
fn code_category_issue_penalizes_its_own_bar_through_the_shared_curve() {
    let s = compute_current_score(
        &[g(
            "code_scan.n-plus-one-query",
            "code_quality",
            "high",
            "new",
        )],
        0,
    );
    let code_bar = s
        .per_category
        .get("code_quality")
        .copied()
        .expect("code_quality bar present in per_category");
    // One high through the shared curve: 100 - tier_deduction(9, 0.9, 1) = 91.
    assert_eq!(code_bar, 91.0);
    // The dominant real code categories all get seeded bars now.
    assert!(s.per_category.contains_key("dependencies"));
    assert!(s.per_category.contains_key("infrastructure"));
    // Untouched categories stay at 100.
    assert_eq!(s.per_category.get("security").copied(), Some(100.0));
    // And it tells ONE story with the overall: a single high anywhere scores
    // the same on the bar and (floor aside) the overall.
    let overall_one_high =
        compute_current_score(&[g("security.hsts", "security", "high", "new")], 0).overall;
    assert_eq!(code_bar, overall_one_high);
}

// Canonical score cases derived from the geometric severity curve, followed
// by the score floor, zero-critical floor, and exploitable cap.
#[test]
fn health_score_parity_table() {
    // (crit, high, med, low, full_weight_crit, exploit, expected)
    let cases: [(f64, f64, f64, f64, bool, bool, u32); 13] = [
        // Clean site.
        (0.0, 0.0, 0.0, 0.0, false, false, 100),
        // 1 confirmed critical: -15 -> 85 (gentle-score guarantee).
        (1.0, 0.0, 0.0, 0.0, true, false, 85),
        // 2 confirmed criticals: -28.5 -> 71.5 -> 72.
        (2.0, 0.0, 0.0, 0.0, true, false, 72),
        // 1 high: -9 -> 91 (floor irrelevant, > 35).
        (0.0, 1.0, 0.0, 0.0, false, false, 91),
        // 3 highs: -24.39 -> 76.
        (0.0, 3.0, 0.0, 0.0, false, false, 76),
        // sitecmd.com today (0 crit / 2 high / 24 med / 15 low, all
        // observed): -17.1 - 22.03 - 5.92 = -45.05 -> 55.
        (0.0, 2.0, 24.0, 15.0, false, false, 55),
        // Small-business web profile (0,0,4,12 all observed): -12.18 - 5.81
        // = -17.99 -> 82. Target: stays Good (>= 75).
        (0.0, 0.0, 4.0, 12.0, false, false, 82),
        (0.0, 0.0, 10.0, 15.0, false, false, 75),
        (2.0, 10.0, 0.0, 0.0, true, false, 13),
        (0.0, 20.0, 0.0, 0.0, false, false, 35),
        // NeedsReview-only criticals keep the floor: 10 eff criticals with
        // NO full-weight critical row -> -97.70 -> clamp 5 -> floor 35.
        (10.0, 0.0, 0.0, 0.0, false, false, 35),
        // One (well, ten) CONFIRMED criticals pierce the floor: same -97.70
        // > clamp SCORE_FLOOR 5, floor disabled by the full-weight critical.
        (10.0, 0.0, 0.0, 0.0, true, false, 5),
        // Exploitable-confirmed caps <= 49 even with a single finding: -15
        // > 85, then the cap lowers it to 49.
        (1.0, 0.0, 0.0, 0.0, true, true, 49),
    ];
    for (c, h, m, l, fwc, exploit, expected) in cases {
        assert_eq!(
            health_score_from_severity(c, h, m, l, fwc, exploit),
            expected,
            "health_score_from_severity({c},{h},{m},{l},fwc={fwc},exploit={exploit})"
        );
    }
}

// Whole-number effective counts preserve the integer curve exactly.
#[test]
fn whole_number_weights_reproduce_the_integer_curve() {
    // Independent reference: the pre-B2 integer geometric formula.
    fn integer_tier(base: f64, decay: f64, count: u32) -> f64 {
        if count == 0 {
            return 0.0;
        }
        base * (1.0 - decay.powi(count as i32)) / (1.0 - decay)
    }
    fn integer_curve(c: u32, h: u32, m: u32, l: u32) -> u32 {
        let deduction = integer_tier(15.0, 0.90, c)
            + integer_tier(9.0, 0.90, h)
            + integer_tier(4.0, 0.82, m)
            + integer_tier(1.5, 0.75, l);
        // Floor disabled (full-weight critical present or none needed): this
        // isolates the deduction curve, not the floor.
        (100.0 - deduction).round().clamp(SCORE_FLOOR, 100.0) as u32
    }
    for c in 0..4u32 {
        for h in 0..6u32 {
            for m in [0u32, 5, 12, 24] {
                for l in [0u32, 3, 15] {
                    let reference = integer_curve(c, h, m, l);
                    // has_full_weight_critical=true so the zero-critical
                    // floor never interferes with the deduction identity.
                    let actual = health_score_from_severity(
                        c as f64, h as f64, m as f64, l as f64, true, false,
                    );
                    assert_eq!(
                        actual, reference,
                        "f64 curve diverged from integer curve at ({c},{h},{m},{l})"
                    );
                }
            }
        }
    }
}

// Confidence weighting (B2) discounts NeedsReview: a half-weight critical
// deducts less than a full-weight one, and a half-weight critical alone
// keeps the zero-critical floor.
#[test]
fn confidence_weighting_discounts_needs_review() {
    let full = health_score_from_severity(1.0, 0.0, 0.0, 0.0, true, false);
    let half = health_score_from_severity(0.5, 0.0, 0.0, 0.0, false, false);
    assert_eq!(full, 85, "one full-weight critical");
    assert_eq!(
        half, 92,
        "one half-weight (NeedsReview) critical deducts less"
    );
    assert!(half > full);
}
