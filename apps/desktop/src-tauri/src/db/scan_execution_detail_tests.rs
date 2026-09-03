//! Detail reads narrowed to one run of an execution.
//!
//! One scan's detail must not pay for its siblings' findings: the filter has
//! to reach the SQL, not be applied after every row has been read.

use super::*;
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};
use crate::core::normalized_scan::{normalize_web_scan, ScanRunKind};
use crate::core::scan_execution::{
    NewScanExecution, ScanAdmissionClass, ScanComponentStatus, ScanExecutionMode, ScanTrigger,
};
use crate::core::scanner::{ScanResult, ScanType};
use crate::db::test_helpers::{temp_db, TestDb};

const ENV: &str = "https://example.com";

fn check(check_id: &str, status: CheckStatus, severity: Severity) -> CheckResult {
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Security,
        title: check_id.into(),
        description: "detail".into(),
        status,
        severity,
        fix_prompt: Some("producer fix".into()),
        manual_fix: None,
        raw_data: Some(serde_json::json!({ "check": check_id })),
        confidence: IssueConfidence::Confirmed,
        confidence_reason: Some("observed".into()),
        why_it_matters: Some("impact".into()),
    }
}

fn scan(url: &str, issues: Vec<CheckResult>) -> ScanResult {
    ScanResult {
        url: url.into(),
        mode: "live".into(),
        scan_type: ScanType::Health,
        overall_score: 80,
        categories: Vec::new(),
        issues,
        detected_stack: None,
        duration_ms: 10,
        timestamp: "2026-07-21T00:00:00Z".into(),
        page_signals: None,
        site_facts: None,
    }
}

fn persist_run(
    db: &Database,
    execution_id: i64,
    project_id: i64,
    parent_run_id: Option<i64>,
    run_kind: ScanRunKind,
    result: &ScanResult,
) -> i64 {
    let site_id = db.get_or_create_site(&result.url).expect("site");
    let batch = normalize_web_scan(
        result,
        execution_id,
        parent_run_id,
        Some(project_id),
        site_id,
        run_kind,
        100,
    )
    .expect("normalize");
    db.persist_normalized_scan_run(batch).expect("persist")
}

/// One execution shaped like a real multi-page Web Scan: a parent run plus two
/// page children, each page carrying findings of its own.
struct Seeded {
    db: TestDb,
    execution_id: i64,
    parent_id: i64,
    page_ids: [i64; 2],
}

fn seeded_execution() -> Seeded {
    let db = temp_db();
    let project_id = db
        .upsert_project("p", "/tmp/execution-detail-filter", None)
        .expect("project");
    let execution_id = db
        .admit_scan_execution(
            NewScanExecution {
                project_id: Some(project_id),
                environment_id: None,
                environment_url: Some(ENV.into()),
                environment_scope_key: ENV.into(),
                requested_mode: ScanExecutionMode::Web,
                web_focus: Some(ScanType::Health),
                trigger: ScanTrigger::Manual,
                admission_class: ScanAdmissionClass::GeneralScan,
                idempotency_key: "detail-filter".into(),
                request_fingerprint: "v1:detail-filter".into(),
                now_ms: 100,
                web_status: Some(ScanComponentStatus::Planned),
                web_detail: None,
                code_status: None,
                code_detail: None,
            },
            900,
        )
        .expect("execution")
        .execution
        .id;
    let parent_id = persist_run(
        &db,
        execution_id,
        project_id,
        None,
        ScanRunKind::MultiParent,
        &scan(ENV, Vec::new()),
    );
    let first = persist_run(
        &db,
        execution_id,
        project_id,
        Some(parent_id),
        ScanRunKind::Page,
        &scan(
            &format!("{ENV}/one"),
            vec![check(
                "security.headers.csp",
                CheckStatus::Fail,
                Severity::Critical,
            )],
        ),
    );
    let second = persist_run(
        &db,
        execution_id,
        project_id,
        Some(parent_id),
        ScanRunKind::Page,
        &scan(
            &format!("{ENV}/two"),
            // A warn is actionable, a pass is not, whatever severity it carries.
            vec![
                check("security.headers.hsts", CheckStatus::Warn, Severity::High),
                check(
                    "security.headers.referrer_policy",
                    CheckStatus::Fail,
                    Severity::Medium,
                ),
                check(
                    "security.headers.x_frame_options",
                    CheckStatus::Fail,
                    Severity::Low,
                ),
                check(
                    "security.headers.permissions_policy",
                    CheckStatus::Fail,
                    Severity::Low,
                ),
                check(
                    "security.headers.coop",
                    CheckStatus::Pass,
                    Severity::Critical,
                ),
            ],
        ),
    );
    Seeded {
        db,
        execution_id,
        parent_id,
        page_ids: [first, second],
    }
}

fn producer_check_ids(run: &crate::core::scan_execution::ScanRunDetail) -> Vec<&str> {
    run.findings
        .iter()
        .map(|finding| finding.producer_check_id.as_str())
        .collect()
}

#[test]
fn a_run_filtered_detail_reads_only_that_run_and_its_findings() {
    let seeded = seeded_execution();
    let requested = seeded.page_ids[1];

    let detail = seeded
        .db
        .get_scan_execution_detail_for_run(seeded.execution_id, requested)
        .expect("filtered detail")
        .expect("the execution exists");

    assert_eq!(
        detail.runs.iter().map(|run| run.id).collect::<Vec<_>>(),
        vec![requested],
        "a scan detail must not carry its sibling runs"
    );
    assert_eq!(
        producer_check_ids(&detail.runs[0]),
        vec![
            "security.headers.hsts",
            "security.headers.referrer_policy",
            "security.headers.x_frame_options",
            "security.headers.permissions_policy",
            "security.headers.coop",
        ],
        "the requested run keeps every finding of its own, passes included"
    );
}

/// The test above can only see the response shape, and a read that loads every
/// run's findings and drops the siblings afterwards produces exactly that
/// shape. The saving is the SQLite work never done, so pin the guard itself:
/// the findings load has to sit behind the run filter, not in front of it.
#[test]
fn the_findings_load_sits_behind_the_run_filter_not_in_front_of_it() {
    const SOURCE: &str = include_str!("scan_execution_detail.rs");

    let call_sites = SOURCE
        .match_indices("load_normalized_findings(conn")
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(
        call_sites.len(),
        1,
        "one call site keeps this guardrail honest; a second one needs its own guard"
    );
    let preceding = &SOURCE[call_sites[0].saturating_sub(160)..call_sites[0]];
    assert!(
        preceding.contains("if run_filter.is_none_or("),
        "the findings read must be guarded by the run filter; loading every run's findings and \
         filtering the result afterwards returns the same shape while still paying for every \
         sibling run's rows"
    );
}

#[test]
fn an_unfiltered_detail_still_carries_every_run_with_its_findings() {
    let seeded = seeded_execution();

    let detail = seeded
        .db
        .get_scan_execution_detail(seeded.execution_id)
        .expect("whole detail")
        .expect("the execution exists");

    assert_eq!(
        detail.runs.iter().map(|run| run.id).collect::<Vec<_>>(),
        vec![seeded.parent_id, seeded.page_ids[0], seeded.page_ids[1]],
        "the unfiltered read is still the whole execution"
    );
    assert_eq!(
        detail
            .runs
            .iter()
            .map(|run| run.findings.len())
            .collect::<Vec<_>>(),
        vec![0, 1, 5],
        "every run keeps its findings when no run was requested"
    );
}

#[test]
fn a_run_filtered_detail_still_summarizes_the_whole_execution() {
    let seeded = seeded_execution();

    let whole = seeded
        .db
        .get_scan_execution_detail(seeded.execution_id)
        .expect("whole detail")
        .expect("the execution exists");
    let filtered = seeded
        .db
        .get_scan_execution_detail_for_run(seeded.execution_id, seeded.page_ids[0])
        .expect("filtered detail")
        .expect("the execution exists");

    assert_eq!(
        serde_json::to_value(&filtered.summary).expect("filtered summary"),
        serde_json::to_value(&whole.summary).expect("whole summary"),
        "narrowing the findings must not narrow the execution summary"
    );
    assert_eq!(
        filtered
            .summary
            .runs
            .iter()
            .map(|run| (
                run.id,
                run.issues_total,
                run.issues_critical,
                run.issues_high,
                run.issues_medium,
                run.issues_low
            ))
            .collect::<Vec<_>>(),
        vec![
            (seeded.parent_id, 0, 0, 0, 0, 0),
            (seeded.page_ids[0], 1, 1, 0, 0, 0),
            // The warn counts, the critical-severity pass does not.
            (seeded.page_ids[1], 4, 0, 1, 1, 2),
        ],
        "sibling run counts come from the stored run row, not from findings the caller read"
    );
}
