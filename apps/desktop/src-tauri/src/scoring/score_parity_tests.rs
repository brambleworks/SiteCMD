use crate::checks::{IssueConfidence, Severity};
use crate::core::code_scan::{canonical_code_check_id, score_report, CodeIssue, CodeScanReport};
use crate::core::types_work_items::{IssueGroup, IssueInstance};
use crate::scoring::calculator::{compute_current_score, health_score_from_severity};

// One logical finding, expressed once and materialized for both pipelines.
// `id` is `<rule>:<path>` for code findings; for web findings it is the
// check_id itself and `code` is false.
struct FindingSpec {
    id: &'static str,
    code: bool,
    category: &'static str,
    severity: Severity,
    confidence: IssueConfidence,
}

fn code(id: &'static str, severity: Severity, confidence: IssueConfidence) -> FindingSpec {
    FindingSpec {
        id,
        code: true,
        // Match the category assigned by production ingest.
        category: "code_quality",
        severity,
        confidence,
    }
}

fn web(id: &'static str, severity: Severity) -> FindingSpec {
    FindingSpec {
        id,
        code: false,
        category: "security",
        severity,
        confidence: IssueConfidence::High,
    }
}

fn code_issue(spec: &FindingSpec) -> CodeIssue {
    assert!(spec.code, "only code findings become CodeIssues");
    CodeIssue {
        id: spec.id.to_string(),
        check_id: canonical_code_check_id(spec.id),
        category: spec.category.to_string(),
        severity: spec.severity,
        title: "t".to_string(),
        description: "d".to_string(),
        relative_path: spec.id.split(':').nth(1).unwrap_or("src/x.ts").to_string(),
        absolute_path: "/tmp/x".to_string(),
        line: Some(1),
        source_excerpt: None,
        evidence: None,
        why_now: None,
        likely_fix: None,
        confidence: spec.confidence,
        confidence_reason: None,
        verify_hint: None,
    }
}

// Raw per-file counts by design: the report columns keep per-file semantics,
// the score dedups internally.
fn report(specs: &[FindingSpec]) -> CodeScanReport {
    let issues: Vec<CodeIssue> = specs.iter().filter(|s| s.code).map(code_issue).collect();
    let count = |severity: Severity| issues.iter().filter(|i| i.severity == severity).count();
    CodeScanReport {
        skipped_scopes: Default::default(),
        checked_at: "2026-07-20T00:00:00Z".to_string(),
        framework: None,
        issue_count: issues.len(),
        critical_count: count(Severity::Critical),
        high_count: count(Severity::High),
        medium_count: count(Severity::Medium),
        low_count: count(Severity::Low),
        issues,
    }
}

fn instance(spec: &FindingSpec) -> IssueInstance {
    IssueInstance {
        id: 1,
        source: if spec.code { "code_scan" } else { "web_scan" }.into(),
        signal_id: format!("sig:{}", spec.id),
        producer_check_id: None,
        url: None,
        page_url: None,
        severity: spec.severity,
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
        // The promoted column, exactly what the live cap gate reads.
        confidence: Some(spec.confidence),
        confidence_reason: None,
        domain: None,
        relative_path: None,
        line: None,
        producer_fix_prompt: None,
        producer_category: None,
    }
}

// One IssueGroup per finding, the way the work_items grouping stores them:
// Code occurrences share one rule-level check id while Web check ids are 1:1.
// Default status ("new") throughout - the fresh-scan case.
fn groups(specs: &[FindingSpec]) -> Vec<IssueGroup> {
    specs
        .iter()
        .map(|spec| IssueGroup {
            check_id: if spec.code {
                canonical_code_check_id(spec.id)
            } else {
                spec.id.to_string()
            },
            category: spec.category.into(),
            severity: spec.severity,
            title: "t".into(),
            description: "d".into(),
            instances: vec![instance(spec)],
            sources: vec![if spec.code { "code_scan" } else { "web_scan" }.into()],
            status: "new".parse().expect("valid issue status"),
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
        })
        .collect()
}

fn live_overall(specs: &[FindingSpec]) -> u32 {
    compute_current_score(&groups(specs), 0).overall as u32
}

#[test]
fn multi_file_rule_scores_identically_on_both_paths() {
    let specs = [
        code(
            "n-plus-one-query:src/a.ts",
            Severity::High,
            IssueConfidence::NeedsReview,
        ),
        code(
            "n-plus-one-query:src/b.ts",
            Severity::High,
            IssueConfidence::NeedsReview,
        ),
        code(
            "n-plus-one-query:src/c.ts",
            Severity::High,
            IssueConfidence::NeedsReview,
        ),
    ];
    let stored = score_report(&report(&specs));
    assert_eq!(stored, live_overall(&specs));
    // One deduped High row, NeedsReview -> weight 0.5 (B2). Despite 3 raw files
    // it is one half-weight high: -4.62 -> 95.
    assert_eq!(
        stored,
        health_score_from_severity(0.0, 0.5, 0.0, 0.0, false, false)
    );

    // Negative control: three DISTINCT rules stay three rows on both paths, and
    // High confidence weighs 1.0 -> three full-weight highs -> 76.
    let distinct = [
        code("rule-a:src/a.ts", Severity::High, IssueConfidence::High),
        code("rule-b:src/a.ts", Severity::High, IssueConfidence::High),
        code("rule-c:src/a.ts", Severity::High, IssueConfidence::High),
    ];
    let stored_distinct = score_report(&report(&distinct));
    assert_eq!(stored_distinct, live_overall(&distinct));
    assert_eq!(
        stored_distinct,
        health_score_from_severity(0.0, 3.0, 0.0, 0.0, false, false)
    );
    assert!(stored_distinct < stored);
}

#[test]
fn mixed_severities_per_rule_score_the_max_on_both_paths() {
    let specs = [
        code(
            "some-rule:src/a.ts",
            Severity::Low,
            IssueConfidence::NeedsReview,
        ),
        code(
            "some-rule:src/z.ts",
            Severity::Critical,
            IssueConfidence::NeedsReview,
        ),
    ];
    let stored = score_report(&report(&specs));
    let live = compute_current_score(&groups(&specs), 0);
    assert_eq!(stored, live.overall as u32);
    // One Critical row, both members NeedsReview -> weight 0.5: -7.70 -> 92, and
    // the zero-critical floor holds (no full-weight critical).
    assert_eq!(
        stored,
        health_score_from_severity(0.5, 0.0, 0.0, 0.0, false, false)
    );
    assert_eq!(
        (live.critical_count, live.low_count),
        (1, 0),
        "the deduped row counts once, at its max severity"
    );
}

#[test]
fn cap_candidate_confirmed_caps_both_paths_needs_review_caps_neither() {
    let confirmed = [
        code(
            "js-command-injection:src/a.ts",
            Severity::Critical,
            IssueConfidence::Confirmed,
        ),
        code(
            "js-command-injection:src/b.ts",
            Severity::Critical,
            IssueConfidence::NeedsReview,
        ),
    ];
    let stored = score_report(&report(&confirmed));
    let live = compute_current_score(&groups(&confirmed), 0);
    assert_eq!(stored, live.overall as u32);
    assert!(stored <= 49, "confirmed cap-class critical must cap");
    assert!(live.exploitable_capped);

    let needs_review = [
        code(
            "js-command-injection:src/a.ts",
            Severity::Critical,
            IssueConfidence::NeedsReview,
        ),
        code(
            "js-command-injection:src/b.ts",
            Severity::Critical,
            IssueConfidence::NeedsReview,
        ),
    ];
    let stored = score_report(&report(&needs_review));
    let live = compute_current_score(&groups(&needs_review), 0);
    assert_eq!(stored, live.overall as u32);
    assert_eq!(
        stored,
        health_score_from_severity(0.5, 0.0, 0.0, 0.0, false, false),
        "needs_review deducts as one half-weight critical row, no cap"
    );
    assert!(!live.exploitable_capped);
}

#[test]
fn cap_gates_never_combine_across_the_rows_members_on_either_path() {
    let specs = [
        code(
            "js-command-injection:src/a.ts",
            Severity::Critical,
            IssueConfidence::NeedsReview,
        ),
        code(
            "js-command-injection:src/b.ts",
            Severity::Low,
            IssueConfidence::Confirmed,
        ),
    ];
    let stored = score_report(&report(&specs));
    let live = compute_current_score(&groups(&specs), 0);
    assert_eq!(stored, live.overall as u32);
    assert_eq!(
        stored,
        health_score_from_severity(0.5, 0.0, 0.0, 0.0, false, false),
        "the row deducts as one half-weight critical, no cap"
    );
    assert_eq!(stored, 92);
    assert!(!live.exploitable_capped);

    // Positive control: the SAME member carrying both gates still caps both
    // paths, even with a weaker unconfirmed sibling merged into the row.
    let capping = [
        code(
            "js-command-injection:src/a.ts",
            Severity::Critical,
            IssueConfidence::Confirmed,
        ),
        code(
            "js-command-injection:src/b.ts",
            Severity::Low,
            IssueConfidence::NeedsReview,
        ),
    ];
    let stored = score_report(&report(&capping));
    let live = compute_current_score(&groups(&capping), 0);
    assert_eq!(stored, live.overall as u32);
    assert!(stored <= 49);
    assert!(live.exploitable_capped);
}

#[test]
fn web_and_code_mix_dedups_only_the_code_rules() {
    let specs = [
        web("security.hsts", Severity::High),
        web("security.csp", Severity::High),
        code(
            "n-plus-one-query:src/a.ts",
            Severity::High,
            IssueConfidence::NeedsReview,
        ),
        code(
            "n-plus-one-query:src/b.ts",
            Severity::High,
            IssueConfidence::NeedsReview,
        ),
        code(
            "unsafe-html:src/c.ts",
            Severity::Medium,
            IssueConfidence::NeedsReview,
        ),
    ];
    let live = compute_current_score(&groups(&specs), 0);
    assert_eq!(
        live.overall as u32,
        health_score_from_severity(0.0, 2.5, 0.5, 0.0, false, false)
    );
    assert_eq!((live.high_count, live.medium_count), (3, 1));

    // The code subset alone agrees with the stored code scan score.
    let code_only: Vec<FindingSpec> = vec![
        code(
            "n-plus-one-query:src/a.ts",
            Severity::High,
            IssueConfidence::NeedsReview,
        ),
        code(
            "n-plus-one-query:src/b.ts",
            Severity::High,
            IssueConfidence::NeedsReview,
        ),
        code(
            "unsafe-html:src/c.ts",
            Severity::Medium,
            IssueConfidence::NeedsReview,
        ),
    ];
    assert_eq!(score_report(&report(&code_only)), live_overall(&code_only));
}
