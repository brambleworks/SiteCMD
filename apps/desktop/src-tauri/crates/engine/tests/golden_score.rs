//! Cross-runtime score fixtures with exact assertions except for `powf` output.
//!
//! Regenerate with `cargo test -p sitecmd-engine --test golden_score -- --ignored regenerate`.

use serde::Deserialize;
use sitecmd_engine::scoring::calculator::{compute_score, ScoreInputGroup, ScoreSnapshot};

const CORPUS: &str = include_str!("../fixtures/score/golden.json");
const POINTS_TOLERANCE: f64 = 1e-9;

#[derive(Deserialize)]
struct Corpus {
    #[allow(dead_code)]
    comment: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    now_ms: i64,
    groups: Vec<ScoreInputGroup>,
    expected: Option<ScoreSnapshot>,
}

fn corpus() -> Corpus {
    serde_json::from_str(CORPUS).expect("golden.json parses")
}

fn assert_snapshot_matches(name: &str, actual: &ScoreSnapshot, expected: &ScoreSnapshot) {
    assert_eq!(actual.overall, expected.overall, "{name}: overall");
    assert_eq!(
        actual.per_category, expected.per_category,
        "{name}: per_category"
    );
    assert_eq!(
        (
            actual.critical_count,
            actual.high_count,
            actual.medium_count,
            actual.low_count
        ),
        (
            expected.critical_count,
            expected.high_count,
            expected.medium_count,
            expected.low_count
        ),
        "{name}: row counts"
    );
    assert_eq!(
        actual.exploitable_capped, expected.exploitable_capped,
        "{name}: exploitable_capped"
    );
    assert_eq!(
        actual.computed_at, expected.computed_at,
        "{name}: computed_at"
    );
    let a = &actual.breakdown;
    let e = &expected.breakdown;
    assert_eq!(a.base, e.base, "{name}: breakdown.base");
    assert_eq!(
        a.floor_applied, e.floor_applied,
        "{name}: breakdown.floor_applied"
    );
    assert_eq!(
        a.ceiling_applied, e.ceiling_applied,
        "{name}: breakdown.ceiling_applied"
    );
    assert_eq!(
        (a.eff_critical, a.eff_high, a.eff_medium, a.eff_low),
        (e.eff_critical, e.eff_high, e.eff_medium, e.eff_low),
        "{name}: breakdown effective counts"
    );
    for (label, actual_points, expected_points) in [
        ("critical_points", a.critical_points, e.critical_points),
        ("high_points", a.high_points, e.high_points),
        ("medium_points", a.medium_points, e.medium_points),
        ("low_points", a.low_points, e.low_points),
    ] {
        assert!(
            (actual_points - expected_points).abs() <= POINTS_TOLERANCE,
            "{name}: breakdown.{label} {actual_points} vs {expected_points} exceeds {POINTS_TOLERANCE}"
        );
    }
}

#[test]
fn golden_cases_reproduce_their_snapshots() {
    let corpus = corpus();
    assert!(!corpus.cases.is_empty(), "corpus has cases");
    for case in &corpus.cases {
        let expected = case.expected.as_ref().unwrap_or_else(|| {
            panic!(
                "case '{}' has no expected block; run the ignored `regenerate` test",
                case.name
            )
        });
        let actual = compute_score(&case.groups, case.now_ms);
        assert_snapshot_matches(&case.name, &actual, expected);
    }
}

#[test]
fn headline_scores_match_the_documented_model() {
    let corpus = corpus();
    let overall = |name: &str| -> f64 {
        let case = corpus
            .cases
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("case '{name}' present"));
        compute_score(&case.groups, case.now_ms).overall
    };
    // Clean slate.
    assert_eq!(overall("clean_slate_scores_100"), 100.0);
    // One confirmed critical deducts 15 (gentle score, no cliff).
    assert_eq!(overall("one_confirmed_critical_gentle_score_85"), 85.0);
    // Three NeedsReview files of one rule = one half-weight high row: 95.
    assert_eq!(
        overall("three_needs_review_files_one_half_weight_high_95"),
        95.0
    );
    // A confirmed cap-class critical caps at exactly EXPLOITABLE_SCORE_CAP.
    assert_eq!(overall("confirmed_cap_candidate_critical_caps_at_49"), 49.0);
    assert_eq!(overall("env_exposure_web_check_caps_at_49"), 49.0);
    // NeedsReview never caps: one half-weight critical row deducts to 92.
    assert_eq!(overall("needs_review_cap_candidate_does_not_cap_92"), 92.0);
    assert_eq!(
        overall("cap_and_weight_never_stitch_across_sibling_groups_92"),
        92.0
    );
    // 20 confident highs deduct past 35 but the zero-critical floor holds.
    assert_eq!(overall("zero_critical_advisory_wall_floors_at_35"), 35.0);
    // Confirmed criticals disable the floor; ten of them clamp at SCORE_FLOOR.
    assert_eq!(
        overall("ten_confirmed_criticals_pierce_the_floor_to_5"),
        5.0
    );
    // Snooze expiry re-arms scoring; an active snooze suppresses it.
    assert_eq!(overall("expired_snooze_counts_again"), 85.0);
    assert_eq!(overall("active_snooze_excluded"), 100.0);
    // Ignored/blocked/verified are inactive; regressed counts as one high.
    assert_eq!(overall("inactive_statuses_excluded_regressed_counts"), 91.0);
    // An unlisted category still penalizes a bar of its own.
    assert_eq!(overall("unknown_category_gets_its_own_bar"), 91.0);
    // A member-less critical row is full weight but never pierces the floor.
    assert_eq!(
        overall("memberless_critical_full_weight_but_keeps_floor_flag_off"),
        85.0
    );
}

#[test]
#[ignore]
fn regenerate() {
    let mut value: serde_json::Value = serde_json::from_str(CORPUS).expect("golden.json parses");
    let cases: Vec<Case> =
        serde_json::from_value(value.get("cases").expect("cases array present").clone())
            .expect("cases parse");
    let out = value
        .get_mut("cases")
        .and_then(|c| c.as_array_mut())
        .expect("cases array");
    for (slot, case) in out.iter_mut().zip(&cases) {
        let snapshot = compute_score(&case.groups, case.now_ms);
        slot["expected"] = serde_json::to_value(&snapshot).expect("snapshot serializes");
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/score/golden.json");
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&value).expect("corpus serializes")
    );
    std::fs::write(path, rendered).expect("write golden.json");
    println!("regenerated {path}");
}
