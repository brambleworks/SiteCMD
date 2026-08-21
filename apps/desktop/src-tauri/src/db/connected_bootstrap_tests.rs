//! Connected bootstrap projection tests.

use super::*;
use crate::checks::{CheckResult, CheckStatus, ScanCategory};
use crate::core::code_scan::{CodeIssue, CodeScanReport};
use crate::core::normalized_scan::{
    normalize_code_scan, normalize_web_scan, CheckOutcome, ClaimBasis, ScanCoverageKind,
    ScanCoverageManifest, ScanEvidenceSource, ScanRunKind,
};
use crate::core::scan_execution::{
    NewScanExecution, ScanAdmissionClass, ScanComponentStatus, ScanExecutionMode, ScanTrigger,
};
use crate::core::scanner::{ScanResult, ScanType};
use crate::db::test_helpers::{temp_db, TestDb};
use crate::db::{ConnectedSubmissionRequest, IssueLifecycle, PendingRotation};
use sitecmd_engine::sync::ProjectFingerprintKey;

const SITE: &str = "https://example.com";

fn seeded() -> (TestDb, i64) {
    let db = temp_db();
    let project_id = db
        .upsert_project("Bootstrap", "/tmp/bootstrap", Some("nextjs"))
        .expect("upsert project");
    (db, project_id)
}

fn execution(db: &TestDb, project_id: i64, scope_key: &str, key: &str, code: bool) -> i64 {
    db.admit_scan_execution(
        NewScanExecution {
            project_id: Some(project_id),
            environment_id: None,
            environment_url: (!code).then(|| scope_key.to_string()),
            environment_scope_key: scope_key.into(),
            requested_mode: if code {
                ScanExecutionMode::Code
            } else {
                ScanExecutionMode::Web
            },
            web_focus: (!code).then_some(ScanType::Health),
            trigger: ScanTrigger::Manual,
            admission_class: ScanAdmissionClass::GeneralScan,
            idempotency_key: key.into(),
            request_fingerprint: format!("v1:{key}"),
            now_ms: 100,
            web_status: (!code).then_some(ScanComponentStatus::Planned),
            web_detail: None,
            code_status: code.then_some(ScanComponentStatus::Planned),
            code_detail: None,
        },
        900,
    )
    .expect("admit execution")
    .execution
    .id
}

fn web_result(url: &str, checks: &[(&str, CheckStatus)]) -> ScanResult {
    ScanResult {
        url: url.into(),
        mode: "live".into(),
        scan_type: ScanType::Health,
        overall_score: 80,
        categories: Vec::new(),
        issues: checks
            .iter()
            .map(|(check_id, status)| CheckResult {
                check_id: (*check_id).into(),
                category: ScanCategory::Security,
                title: (*check_id).into(),
                description: "detail".into(),
                status: *status,
                severity: Severity::High,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: IssueConfidence::Confirmed,
                confidence_reason: None,
                why_it_matters: None,
            })
            .collect(),
        detected_stack: None,
        duration_ms: 10,
        timestamp: "2026-08-06T00:00:00Z".into(),
        page_signals: None,
        site_facts: None,
    }
}

// Persist one complete web run of `url` inside its own execution.
fn web_scan(
    db: &TestDb,
    project_id: i64,
    key: &str,
    url: &str,
    checks: &[(&str, CheckStatus)],
) -> i64 {
    let site_id = db.get_or_create_site(SITE).expect("site");
    let execution_id = execution(db, project_id, SITE, key, false);
    let batch = normalize_web_scan(
        &web_result(url, checks),
        execution_id,
        None,
        Some(project_id),
        site_id,
        ScanRunKind::Single,
        100,
    )
    .expect("normalize web");
    db.persist_normalized_scan_run(batch).expect("persist web");
    execution_id
}

// Persist several page runs inside ONE execution, the way a site scan does.
fn web_session(
    db: &TestDb,
    project_id: i64,
    key: &str,
    pages: &[(&str, &[(&str, CheckStatus)])],
) -> i64 {
    let site_id = db.get_or_create_site(SITE).expect("site");
    let execution_id = execution(db, project_id, SITE, key, false);
    for (url, checks) in pages {
        let mut batch = normalize_web_scan(
            &web_result(url, checks),
            execution_id,
            None,
            Some(project_id),
            site_id,
            ScanRunKind::Page,
            100,
        )
        .expect("normalize page");
        // A page run's lifecycle scope is the site, exactly as the multi-scan
        // command sets it; the page itself is the finding's location.
        batch.environment_scope_key = crate::db::normalize_env_url(Some(SITE));
        db.persist_normalized_scan_run(batch).expect("persist page");
    }
    execution_id
}

fn bounded_web_verification(
    db: &TestDb,
    project_id: i64,
    key: &str,
    url: &str,
    checks: &[(&str, CheckStatus)],
) -> i64 {
    let site_id = db.get_or_create_site(SITE).expect("site");
    let execution_id = execution(db, project_id, SITE, key, false);
    let result = web_result(url, checks);
    let canonical_checks = checks
        .iter()
        .map(|(check_id, status)| (canonical(check_id), *status))
        .collect::<Vec<_>>();
    let outcomes = canonical_checks
        .iter()
        .map(|(check_id, status)| CheckOutcome {
            route: Some(url),
            check_id,
            status: *status,
        })
        .collect::<Vec<_>>();
    let mut batch = normalize_web_scan(
        &result,
        execution_id,
        None,
        Some(project_id),
        site_id,
        ScanRunKind::Single,
        100,
    )
    .expect("normalize bounded verification");
    batch.environment_url = Some(SITE.into());
    batch.environment_scope_key = crate::db::normalize_env_url(Some(SITE));
    batch.coverage = ScanCoverageManifest::derive(
        ScanCoverageKind::CheckSet,
        vec![url.into()],
        &outcomes,
        ClaimBasis::PerRoute,
    );
    db.persist_normalized_scan_run(batch)
        .expect("persist bounded verification");
    db.execute(move |conn| {
        conn.execute(
            "UPDATE scan_executions
                SET trigger = 'verification', admission_class = 'bounded_verification'
              WHERE id = :execution_id",
            rusqlite::named_params! { ":execution_id": execution_id },
        )
    })
    .expect("database worker")
    .expect("mark bounded verification");
    execution_id
}

fn code_issue(id: &str, path: &str, line: u32, severity: Severity) -> CodeIssue {
    CodeIssue {
        id: format!("{id}:{path}:{line}"),
        check_id: String::new(),
        category: "code_quality".into(),
        severity,
        title: id.into(),
        description: "detail".into(),
        relative_path: path.into(),
        absolute_path: format!("/tmp/bootstrap/{path}"),
        line: Some(line),
        source_excerpt: None,
        evidence: None,
        why_now: None,
        likely_fix: None,
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: None,
        verify_hint: None,
    }
}

// One instance the scanner is sure about, for the collapse rule.
fn confirmed(issue: CodeIssue) -> CodeIssue {
    CodeIssue {
        confidence: IssueConfidence::Confirmed,
        ..issue
    }
}

fn code_scan(db: &TestDb, project_id: i64, key: &str, scope_key: &str, issues: Vec<CodeIssue>) {
    code_scan_with_commit(db, project_id, key, scope_key, issues, Some("abc123"));
}

fn code_scan_with_commit(
    db: &TestDb,
    project_id: i64,
    key: &str,
    scope_key: &str,
    issues: Vec<CodeIssue>,
    commit_sha: Option<&str>,
) {
    let execution_id = execution(db, project_id, scope_key, key, true);
    let report = CodeScanReport {
        checked_at: "2026-08-06T00:00:00Z".into(),
        framework: Some("nextjs".into()),
        issue_count: issues.len(),
        critical_count: 0,
        high_count: 0,
        medium_count: 0,
        low_count: 0,
        issues,
        skipped_scopes: Default::default(),
    };
    let mut batch = normalize_code_scan(
        &report,
        execution_id,
        project_id,
        None,
        scope_key.to_string(),
        "/tmp/bootstrap".into(),
        80,
        10,
        100,
    )
    .expect("normalize code");
    batch.diagnostics.code_commit_sha = commit_sha.map(str::to_string);
    batch.diagnostics.code_tree_clean = Some(true);
    db.persist_normalized_scan_run(batch).expect("persist code");
}

// The canonical group id a web producer's check id resolves to. Scans emit
// producer ids; lifecycle and bootstrap both speak canonical ones.
fn canonical(producer_check_id: &str) -> String {
    crate::core::correlation::resolve_check_id("web_scan", producer_check_id)
}

fn group<'a>(set: &'a BootstrapSet, check_id: &str) -> Option<&'a BootstrapGroup> {
    set.groups.iter().find(|group| group.check_id == check_id)
}

fn evidence(set: &BootstrapSet, source: ScanEvidenceSource) -> Option<&SourceEvidence> {
    set.evidence.iter().find(|found| found.source == source)
}

fn routes(occurrences: &[LastKnownOccurrenceRecord]) -> Vec<String> {
    occurrences
        .iter()
        .filter_map(|occurrence| match &occurrence.identity.location {
            OccurrenceLocation::Page { url } => Some(url.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_untouched_active_issue_is_declared_without_ever_having_a_lifecycle_row() {
    let (db, project_id) = seeded();
    web_scan(
        &db,
        project_id,
        "untouched",
        SITE,
        &[("security.headers.csp", CheckStatus::Fail)],
    );

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    let found = group(&set, &canonical("security.headers.csp")).expect("group present");
    assert_eq!(
        found.state,
        BootstrapState::Active,
        "the overlay is an override table; absence is the ordinary case, not a gap"
    );
    assert_eq!(found.sources, vec![ScanEvidenceSource::WebScan]);
    assert!(found.last_known_occurrences.is_empty());
}

#[test]
fn a_terminal_group_whose_findings_are_gone_survives_as_a_row() {
    let (db, project_id) = seeded();
    db.set_issue_state(
        project_id,
        SITE,
        "security.csp",
        IssueLifecycle::Blocked {
            reason: Some("intended".into()),
        },
        4_000,
    )
    .expect("block");

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    let found = group(&set, "security.csp").expect("row-only group survives");
    assert_eq!(
        found.state,
        BootstrapState::Blocked {
            reason: Some("intended".into())
        },
        "a dismissal the projection can no longer see is exactly what the \
         service has to be told, or it will alert on it"
    );
    assert_eq!(found.state_changed_at, 4_000);
}

#[test]
fn a_scan_fixed_group_nobody_decided_about_is_not_revived() {
    let (db, project_id) = seeded();
    web_scan(
        &db,
        project_id,
        "present",
        SITE,
        &[("security.headers.csp", CheckStatus::Fail)],
    );
    web_scan(
        &db,
        project_id,
        "absent",
        SITE,
        &[("security.headers.csp", CheckStatus::Pass)],
    );

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    assert!(
        group(&set, &canonical("security.headers.csp")).is_none(),
        "the resolved row is history; declaring it would report a fix as an \
         open issue the user has to look at again"
    );
}

#[test]
fn the_stored_state_travels_rather_than_the_effective_one() {
    let (db, project_id) = seeded();
    db.set_issue_state(
        project_id,
        SITE,
        "seo.title",
        IssueLifecycle::Snoozed { until: 1_000 },
        900,
    )
    .expect("snooze");

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    assert_eq!(
        group(&set, "seo.title").expect("group").state,
        BootstrapState::Snoozed { until: 1_000 },
        "the server evaluates snooze expiry at read time; collapsing it here \
         would discard the policy and read as a reopening nobody decided"
    );
}

#[test]
fn a_regression_is_readable_here_even_though_nothing_can_declare_one() {
    let (db, project_id) = seeded();
    web_scan(
        &db,
        project_id,
        "seen",
        SITE,
        &[("security.headers.csp", CheckStatus::Fail)],
    );
    db.set_issue_state(
        project_id,
        SITE,
        &canonical("security.headers.csp"),
        IssueLifecycle::Verified {
            by: VerifiedBy::LocalScan,
        },
        2_000,
    )
    .expect("verify");
    // Seeing it again after a scan proved it gone is the regression.
    web_scan(
        &db,
        project_id,
        "again",
        SITE,
        &[("security.headers.csp", CheckStatus::Fail)],
    );

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    assert_eq!(
        group(&set, &canonical("security.headers.csp"))
            .expect("group")
            .state,
        BootstrapState::Regressed,
        "reading a regression is what a producer must do; declaring one is \
         what it must not, which is why only the read vocabulary has it"
    );
}

#[test]
fn a_snoozed_row_with_no_deadline_stops_the_derivation() {
    let (db, project_id) = seeded();
    db.execute(move |conn| {
        conn.execute(
            "INSERT INTO project_issue_states
                (project_id, env_url, check_id, status, last_status_changed_at)
             VALUES (?1, ?2, 'seo.title', 'snoozed', 1000)",
            rusqlite::params![project_id, SITE],
        )
    })
    .expect("execute")
    .expect("raw insert");

    let error = db
        .derive_bootstrap_set(project_id, SITE)
        .expect_err("an undeclarable dismissal must stop the submission");
    assert!(
        error.to_string().contains("no deadline"),
        "every repair invents a decision the user did not make: got {error}"
    );
}

#[test]
fn only_a_group_awaiting_verification_carries_the_places_to_look() {
    let (db, project_id) = seeded();
    web_session(
        &db,
        project_id,
        "history",
        &[
            (
                "https://example.com/",
                &[("seo.canonical", CheckStatus::Fail)],
            ),
            (
                "https://example.com/pricing",
                &[("seo.canonical", CheckStatus::Fail)],
            ),
            (
                "https://example.com/docs",
                &[("security.headers.csp", CheckStatus::Fail)],
            ),
        ],
    );
    db.set_issue_state(
        project_id,
        SITE,
        &canonical("seo.canonical"),
        IssueLifecycle::Verified {
            by: VerifiedBy::UserClaim,
        },
        5_000,
    )
    .expect("claim fixed");

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    let claimed = group(&set, &canonical("seo.canonical")).expect("claimed group");
    assert_eq!(
        routes(&claimed.last_known_occurrences),
        vec![
            "https://example.com/".to_string(),
            "https://example.com/pricing".to_string(),
        ],
        "one site scan writes one run per page; taking the newest RUN would \
         hand the verifier a single route and call it the whole set"
    );
    let untouched = group(&set, &canonical("security.headers.csp")).expect("active group");
    assert!(
        untouched.last_known_occurrences.is_empty(),
        "an active group is not awaiting verification and has nothing to prove absent"
    );
}

#[test]
fn a_group_whose_history_aged_out_declares_no_places_to_look() {
    let (db, project_id) = seeded();
    db.set_issue_state(
        project_id,
        SITE,
        &canonical("seo.canonical"),
        IssueLifecycle::Verified {
            by: VerifiedBy::LocalScan,
        },
        5_000,
    )
    .expect("verify");

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    let found = group(&set, &canonical("seo.canonical")).expect("group");
    assert!(
        found.last_known_occurrences.is_empty(),
        "omitting is honest; a guessed location would have the verifier prove \
         absence somewhere the issue never was"
    );
    assert!(found.sources.is_empty());
}

#[test]
fn a_verified_group_recovers_its_source_from_the_scan_that_last_saw_it() {
    let (db, project_id) = seeded();
    let scope_key = format!("project:{project_id}");
    code_scan(
        &db,
        project_id,
        "code-history",
        &scope_key,
        vec![code_issue(
            "n-plus-one-query",
            "src/db.ts",
            12,
            Severity::Medium,
        )],
    );
    let check_id = crate::core::code_scan::canonical_code_check_id("n-plus-one-query");
    db.set_issue_state(
        project_id,
        &scope_key,
        &check_id,
        IssueLifecycle::Verified {
            by: VerifiedBy::UserClaim,
        },
        6_000,
    )
    .expect("claim fixed");

    let set = db
        .derive_bootstrap_set(project_id, &scope_key)
        .expect("derive");
    let found = group(&set, &check_id).expect("group");
    assert_eq!(
        found.sources,
        vec![ScanEvidenceSource::CodeScan],
        "a code-only project keys its rows the way the lifecycle overlay does, \
         and the scan that last saw the group still names its scanner"
    );
    assert_eq!(found.last_known_occurrences.len(), 1);
    assert_eq!(
        found.last_known_occurrences[0].identity.location,
        OccurrenceLocation::File {
            rule: "n-plus-one-query".into(),
            path: "src/db.ts".into(),
        }
    );
}

#[test]
fn code_instances_in_one_file_collapse_and_keep_their_count() {
    let (db, project_id) = seeded();
    let scope_key = format!("project:{project_id}");
    code_scan(
        &db,
        project_id,
        "collapse",
        &scope_key,
        vec![
            code_issue("n-plus-one-query", "src/db.ts", 12, Severity::Low),
            confirmed(code_issue(
                "n-plus-one-query",
                "src/db.ts",
                48,
                Severity::Critical,
            )),
            code_issue("n-plus-one-query", "src/api.ts", 7, Severity::Low),
        ],
    );

    let set = db
        .derive_bootstrap_set(project_id, &scope_key)
        .expect("derive");
    let code = evidence(&set, ScanEvidenceSource::CodeScan).expect("code evidence");
    let occurrences = &code.occurrences;
    assert_eq!(
        occurrences.len(),
        2,
        "two findings in one file are one occurrence once the line is dropped"
    );
    let db_file = occurrences
        .iter()
        .find(|occurrence| {
            occurrence.identity.location
                == OccurrenceLocation::File {
                    rule: "n-plus-one-query".into(),
                    path: "src/db.ts".into(),
                }
        })
        .expect("collapsed occurrence");
    assert_eq!(
        db_file.identity.instance_count, 2,
        "multiplicity is preserved as a count rather than lost with the line"
    );
    assert_eq!(
        db_file.severity,
        Severity::Critical,
        "the occurrence holds an instance this severe, so the strongest is the true one"
    );
    assert_eq!(
        db_file.confidence,
        IssueConfidence::Confirmed,
        "both facts are existential: one instance the scanner is sure about \
         makes the occurrence one the scanner is sure about"
    );
}

#[test]
fn a_cross_page_finding_has_no_route_of_its_own() {
    let (db, project_id) = seeded();
    let site_id = db.get_or_create_site(SITE).expect("site");
    let execution_id = execution(&db, project_id, SITE, "cross-page", false);
    let batch = crate::core::normalized_scan::normalize_multi_page_parent(
        &[CheckResult {
            check_id: "seo.duplicate_title_across_pages".into(),
            category: ScanCategory::Seo,
            title: "duplicate titles".into(),
            description: "detail".into(),
            status: CheckStatus::Fail,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: IssueConfidence::Confirmed,
            confidence_reason: None,
            why_it_matters: None,
        }],
        execution_id,
        Some(project_id),
        site_id,
        SITE.to_string(),
        vec![SITE.to_string()],
        vec![SITE.to_string()],
        1,
        Some(80),
        10,
        100,
        200,
        ScanType::Health,
        false,
        true,
    )
    .expect("normalize cross page");
    db.persist_normalized_scan_run(batch).expect("persist");

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    let web = evidence(&set, ScanEvidenceSource::WebScan).expect("web evidence");
    assert_eq!(
        web.occurrences[0].identity.location,
        OccurrenceLocation::Whole,
        "giving a cross-page finding the environment root would attach it to a \
         route that never carried it, and let a scan of that route resolve it"
    );
}

#[test]
fn evidence_is_the_latest_complete_scan_of_each_source_separately() {
    let (db, project_id) = seeded();
    let scope_key = format!("project:{project_id}");
    code_scan(
        &db,
        project_id,
        "code-first",
        &scope_key,
        vec![code_issue(
            "hardcoded-secret",
            "src/env.ts",
            3,
            Severity::High,
        )],
    );
    // A later web scan of a different scope must not become the code answer.
    web_scan(
        &db,
        project_id,
        "web-later",
        SITE,
        &[("security.headers.csp", CheckStatus::Fail)],
    );

    let code_set = db
        .derive_bootstrap_set(project_id, &scope_key)
        .expect("derive code scope");
    assert!(
        evidence(&code_set, ScanEvidenceSource::WebScan).is_none(),
        "the web scan belongs to another environment scope"
    );
    let code = evidence(&code_set, ScanEvidenceSource::CodeScan).expect("code evidence");
    assert_eq!(code.occurrences.len(), 1);

    let web_set = db
        .derive_bootstrap_set(project_id, SITE)
        .expect("derive web");
    assert!(evidence(&web_set, ScanEvidenceSource::CodeScan).is_none());
    assert_eq!(
        evidence(&web_set, ScanEvidenceSource::WebScan)
            .expect("web evidence")
            .occurrences
            .len(),
        1
    );
}

#[test]
fn newer_evidence_replaces_older_evidence_of_the_same_source() {
    let (db, project_id) = seeded();
    web_scan(
        &db,
        project_id,
        "first",
        SITE,
        &[("security.headers.csp", CheckStatus::Fail)],
    );
    web_scan(
        &db,
        project_id,
        "second",
        SITE,
        &[
            ("security.headers.csp", CheckStatus::Fail),
            ("seo.title", CheckStatus::Fail),
        ],
    );

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    let web = evidence(&set, ScanEvidenceSource::WebScan).expect("web evidence");
    assert_eq!(web.run_ids.len(), 1);
    assert_eq!(
        web.occurrences.len(),
        2,
        "a snapshot is one scan's observation, and the newest scan is the one \
         whose coverage the service will evaluate"
    );
}

#[test]
fn evidence_carries_the_watermark_its_scan_was_taken_against() {
    let (db, project_id) = seeded();
    db.connect_site(project_id, SITE, "site_9f2c81d0a4b3", 100)
        .expect("connect");
    db.record_pulled_event_sequence(project_id, SITE, 42, 100)
        .expect("pull events");
    web_scan(
        &db,
        project_id,
        "after-pull",
        SITE,
        &[("security.headers.csp", CheckStatus::Fail)],
    );

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    assert_eq!(
        evidence(&set, ScanEvidenceSource::WebScan)
            .expect("web evidence")
            .based_on_event_sequence,
        42,
        "the basis is captured on the execution when the scan starts looking"
    );
}

#[test]
fn deriving_what_would_be_sent_does_not_require_a_connection() {
    let (db, project_id) = seeded();
    web_scan(
        &db,
        project_id,
        "unconnected",
        SITE,
        &[("security.headers.csp", CheckStatus::Fail)],
    );

    let set = db.derive_bootstrap_set(project_id, SITE).expect("derive");
    assert_eq!(set.groups.len(), 1);
    assert!(
        db.get_connected_site(project_id, SITE)
            .expect("read binding")
            .is_none(),
        "a preview computable only after connecting would arrive too late to \
         inform the decision to connect"
    );
}

#[test]
fn an_environment_is_required() {
    let (db, project_id) = seeded();
    let error = db
        .derive_bootstrap_set(project_id, "")
        .expect_err("no environment, no site");
    assert!(error.to_string().contains("environment is required"));
}

#[test]
fn bootstrap_prefers_full_web_evidence_but_later_sync_uses_bounded_verification() {
    let (db, project_id) = seeded();
    web_session(
        &db,
        project_id,
        "full-web-before-verification",
        &[
            (
                "https://example.com/",
                &[("security.headers.csp", CheckStatus::Pass)],
            ),
            (
                "https://example.com/docs",
                &[("security.headers.csp", CheckStatus::Fail)],
            ),
        ],
    );
    bounded_web_verification(
        &db,
        project_id,
        "one-page-verification",
        "https://example.com/docs",
        &[("security.headers.csp", CheckStatus::Pass)],
    );

    let request = |include_groups| ConnectedSubmissionRequest {
        site_id: "site_verification_scope".into(),
        submission_sequence: 1,
        include_groups,
        fingerprint_key: None,
        fingerprint_key_version: 1,
        pending_rotation: None,
        deployed_commit: None,
    };
    let bootstrap = db
        .build_connected_submission(project_id, SITE, request(true))
        .expect("build bootstrap submission");
    let later_sync = db
        .build_connected_submission(project_id, SITE, request(false))
        .expect("build post-bootstrap submission");

    assert_eq!(
        bootstrap
            .snapshots
            .web
            .expect("bootstrap Web snapshot")
            .coverage
            .routes,
        vec!["/", "/docs"],
        "a hidden one-page verification must not shrink initial connected scope"
    );
    assert_eq!(
        later_sync
            .snapshots
            .web
            .expect("post-bootstrap Web snapshot")
            .coverage
            .routes,
        vec!["/docs"],
        "post-bootstrap sync must keep the newest bounded verification usable"
    );
}

#[test]
fn the_connected_payload_builder_is_the_single_privacy_boundary() {
    let (db, project_id) = seeded();
    web_session(
        &db,
        project_id,
        "payload-preview",
        &[(
            "https://example.com/pricing/?campaign=preview",
            &[
                ("security.headers.csp", CheckStatus::Fail),
                ("performance.lcp", CheckStatus::Fail),
            ],
        )],
    );

    let submission = db
        .build_connected_submission(
            project_id,
            SITE,
            ConnectedSubmissionRequest {
                site_id: "site_9f2c81d0a4b3".into(),
                submission_sequence: 1,
                include_groups: true,
                fingerprint_key: None,
                fingerprint_key_version: 1,
                pending_rotation: None,
                deployed_commit: None,
            },
        )
        .expect("build payload");

    let groups = &submission
        .groups
        .as_ref()
        .expect("bootstrap groups")
        .entries;
    let csp = canonical("security.headers.csp");
    assert!(groups.iter().any(|group| group.check == csp));
    assert!(
        groups.iter().all(|group| group.check != "performance.lcp"),
        "measurement checks are samples, never lifecycle groups"
    );
    let web = submission.snapshots.web.as_ref().expect("web snapshot");
    let occurrence = web
        .occurrences
        .iter()
        .find(|occurrence| occurrence.check == csp)
        .expect("CSP occurrence");
    let route = occurrence.route.as_ref().expect("page route");
    assert_eq!(route.route, "/pricing/");
    assert!(route.query_dependent);

    let rendered = submission.render_for_inspection().expect("render");
    for forbidden in ["campaign=preview", "description", "raw_data", "detail_json"] {
        assert!(!rendered.contains(forbidden), "payload leaked {forbidden}");
    }
}

#[test]
fn connected_payload_does_not_relabel_browser_ttfb_as_transport() {
    let (db, project_id) = seeded();
    let site_id = db.get_or_create_site(SITE).expect("site");
    let execution_id = execution(&db, project_id, SITE, "browser-ttfb", false);
    let mut result = web_result(
        "https://example.com/",
        &[("performance.ttfb", CheckStatus::Pass)],
    );
    result.issues[0].raw_data = Some(serde_json::json!({
        "measurement_source": "browser_navigation",
        "ttfb_ms": 320,
    }));
    let batch = normalize_web_scan(
        &result,
        execution_id,
        None,
        Some(project_id),
        site_id,
        ScanRunKind::Single,
        100,
    )
    .expect("normalize browser TTFB");
    db.persist_normalized_scan_run(batch)
        .expect("persist browser TTFB");

    let submission = db
        .build_connected_submission(
            project_id,
            SITE,
            ConnectedSubmissionRequest {
                site_id: "site_browser_ttfb".into(),
                submission_sequence: 1,
                include_groups: false,
                fingerprint_key: None,
                fingerprint_key_version: 1,
                pending_rotation: None,
                deployed_commit: None,
            },
        )
        .expect("build connected payload");

    let samples = &submission
        .snapshots
        .web
        .expect("web snapshot")
        .measurement_samples;
    assert!(
        samples
            .iter()
            .all(|sample| sample.check != "performance.ttfb"),
        "a browser-navigation value cannot enter the manifest's transport TTFB series"
    );
}

#[test]
fn a_redirected_desktop_occurrence_keeps_final_identity_and_authored_scope_separate() {
    const AUTHORED_URL: &str = "https://example.com/pricing";
    const EFFECTIVE_URL: &str = "https://example.com/pricing/";
    let (db, project_id) = seeded();
    let site_id = db.get_or_create_site(SITE).expect("site");
    let execution_id = execution(&db, project_id, SITE, "redirected-payload", false);
    let mut batch = normalize_web_scan(
        &web_result(
            EFFECTIVE_URL,
            &[("security.headers.csp", CheckStatus::Fail)],
        ),
        execution_id,
        None,
        Some(project_id),
        site_id,
        ScanRunKind::Single,
        100,
    )
    .expect("normalize redirected Web scan");
    batch.environment_url = Some(SITE.into());
    batch.environment_scope_key = crate::db::normalize_env_url(Some(SITE));
    batch.coverage.page_urls = vec![AUTHORED_URL.into()];
    batch.diagnostics.page_url = Some(AUTHORED_URL.into());
    db.persist_normalized_scan_run(batch)
        .expect("persist redirected Web scan");

    let submission = db
        .build_connected_submission(
            project_id,
            SITE,
            ConnectedSubmissionRequest {
                site_id: "site_redirected".into(),
                submission_sequence: 1,
                include_groups: false,
                fingerprint_key: None,
                fingerprint_key_version: 1,
                pending_rotation: None,
                deployed_commit: None,
            },
        )
        .expect("build redirected payload");
    let web = submission.snapshots.web.expect("Web snapshot");
    let occurrence = web.occurrences.first().expect("Web occurrence");

    assert_eq!(
        occurrence.route.as_ref().map(|route| route.route.as_str()),
        Some("/pricing/")
    );
    assert_eq!(occurrence.scope_route.as_deref(), Some("/pricing"));
    assert_eq!(web.coverage.routes, vec!["/pricing"]);
}

#[test]
fn a_redirected_bootstrap_tombstone_keeps_final_identity_and_authored_scope() {
    const AUTHORED_URL: &str = "https://example.com/pricing";
    const EFFECTIVE_URL: &str = "https://example.com/pricing/";
    let (db, project_id) = seeded();
    let site_id = db.get_or_create_site(SITE).expect("site");
    let execution_id = execution(&db, project_id, SITE, "redirected-tombstone", false);
    let mut batch = normalize_web_scan(
        &web_result(
            EFFECTIVE_URL,
            &[("security.headers.csp", CheckStatus::Fail)],
        ),
        execution_id,
        None,
        Some(project_id),
        site_id,
        ScanRunKind::Single,
        100,
    )
    .expect("normalize redirected Web scan");
    batch.environment_url = Some(SITE.into());
    batch.environment_scope_key = crate::db::normalize_env_url(Some(SITE));
    batch.coverage.page_urls = vec![AUTHORED_URL.into()];
    batch.diagnostics.page_url = Some(AUTHORED_URL.into());
    db.persist_normalized_scan_run(batch)
        .expect("persist redirected Web scan");

    let check_id = canonical("security.headers.csp");
    db.set_issue_state(
        project_id,
        SITE,
        &check_id,
        IssueLifecycle::Verified {
            by: VerifiedBy::UserClaim,
        },
        5_000,
    )
    .expect("claim fixed");

    let submission = db
        .build_connected_submission(
            project_id,
            SITE,
            ConnectedSubmissionRequest {
                site_id: "site_redirected".into(),
                submission_sequence: 1,
                include_groups: true,
                fingerprint_key: None,
                fingerprint_key_version: 1,
                pending_rotation: None,
                deployed_commit: None,
            },
        )
        .expect("build redirected bootstrap payload");
    let group = submission
        .groups
        .expect("bootstrap groups")
        .entries
        .into_iter()
        .find(|group| group.check == check_id)
        .expect("claimed group");
    let tombstone = serde_json::to_value(
        group
            .last_known_occurrences
            .first()
            .expect("last-known occurrence"),
    )
    .expect("serialize tombstone");

    assert_eq!(
        tombstone,
        serde_json::json!({
            "route": "/pricing/",
            "query_dependent": false,
            "scope_routes": ["/pricing"]
        })
    );
}

#[test]
fn a_pending_rotation_is_used_only_by_a_completing_code_snapshot() {
    let (db, project_id) = seeded();
    let scope_key = format!("project:{project_id}");
    code_scan(
        &db,
        project_id,
        "epoch",
        &scope_key,
        vec![code_issue(
            "n-plus-one-query",
            "src/db.ts",
            12,
            Severity::Low,
        )],
    );

    let current = ProjectFingerprintKey::from_bytes([1_u8; 32]);
    let candidate = ProjectFingerprintKey::from_bytes([2_u8; 32]);
    let request = |pending: Option<PendingRotation>| ConnectedSubmissionRequest {
        site_id: "site_epoch".into(),
        submission_sequence: 1,
        include_groups: false,
        fingerprint_key: Some(current.clone()),
        fingerprint_key_version: 3,
        pending_rotation: pending,
        deployed_commit: None,
    };

    // A complete project scan under a pending rotation is the completing
    // snapshot: it travels under the CANDIDATE key and version.
    let completing = db
        .build_connected_submission(
            project_id,
            &scope_key,
            request(Some(PendingRotation {
                key: candidate.clone(),
                version: 4,
            })),
        )
        .expect("build completing");
    let code = completing.snapshots.code.expect("code snapshot");
    assert_eq!(code.versions.fingerprint_key_version, 4);
    assert_eq!(code.key_commitment, candidate.commitment());

    // No pending rotation: the current epoch stamps the snapshot, from the
    // request rather than any constant.
    let steady = db
        .build_connected_submission(project_id, &scope_key, request(None))
        .expect("build steady");
    let code = steady.snapshots.code.expect("code snapshot");
    assert_eq!(code.versions.fingerprint_key_version, 3);
    assert_eq!(code.key_commitment, current.commitment());
}

#[test]
fn connected_code_basis_comes_from_the_persisted_scan_checkout() {
    let (db, project_id) = seeded();
    let scope_key = format!("project:{project_id}");
    code_scan(
        &db,
        project_id,
        "scan-provenance",
        &scope_key,
        vec![code_issue(
            "n-plus-one-query",
            "src/db.ts",
            12,
            Severity::Low,
        )],
    );
    let key = ProjectFingerprintKey::from_bytes([3_u8; 32]);
    let request = |deployed_commit: &str| ConnectedSubmissionRequest {
        site_id: "site_scan_provenance".into(),
        submission_sequence: 1,
        include_groups: false,
        fingerprint_key: Some(key.clone()),
        fingerprint_key_version: 1,
        pending_rotation: None,
        deployed_commit: Some(deployed_commit.into()),
    };

    let different_head = db
        .build_connected_submission(project_id, &scope_key, request("def456"))
        .expect("build from stored scan provenance")
        .snapshots
        .code
        .expect("code snapshot");
    assert_eq!(
        different_head.code_basis.commit_sha.as_deref(),
        Some("abc123")
    );
    assert_eq!(
        different_head.code_basis.kind,
        sitecmd_engine::sync::CodeBasisKind::Unknown
    );

    let matching_head = db
        .build_connected_submission(project_id, &scope_key, request("abc123"))
        .expect("build exact stored scan provenance")
        .snapshots
        .code
        .expect("code snapshot");
    assert_eq!(
        matching_head.code_basis.commit_sha.as_deref(),
        Some("abc123")
    );
    assert_eq!(
        matching_head.code_basis.kind,
        sitecmd_engine::sync::CodeBasisKind::ExactCheckout
    );
}

#[test]
fn a_non_git_code_scan_still_emits_presence_with_an_explicit_unknown_basis() {
    let (db, project_id) = seeded();
    let scope_key = format!("project:{project_id}");
    code_scan_with_commit(
        &db,
        project_id,
        "non-git-provenance",
        &scope_key,
        vec![code_issue(
            "n-plus-one-query",
            "src/db.ts",
            12,
            Severity::Low,
        )],
        None,
    );

    let snapshot = db
        .build_connected_submission(
            project_id,
            &scope_key,
            ConnectedSubmissionRequest {
                site_id: "site_non_git".into(),
                submission_sequence: 1,
                include_groups: false,
                fingerprint_key: Some(ProjectFingerprintKey::from_bytes([4_u8; 32])),
                fingerprint_key_version: 1,
                pending_rotation: None,
                deployed_commit: Some("def456".into()),
            },
        )
        .expect("build non-git snapshot")
        .snapshots
        .code
        .expect("code findings are retained without git");

    assert_eq!(
        snapshot.code_basis.kind,
        sitecmd_engine::sync::CodeBasisKind::Unknown
    );
    assert_eq!(snapshot.code_basis.commit_sha, None);
    assert!(!snapshot.occurrences.is_empty());
    assert!(snapshot
        .occurrences
        .iter()
        .all(|occurrence| occurrence.provenance.commit_sha.is_none()));
    let wire = serde_json::to_value(snapshot).expect("wire snapshot");
    assert!(wire["code_basis"]["commit_sha"].is_null());
    assert!(wire["occurrences"][0]["provenance"]["commit_sha"].is_null());
}
