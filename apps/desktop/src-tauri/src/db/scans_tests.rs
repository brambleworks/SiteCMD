use crate::core::scanner::ScanResult;
use crate::db::test_helpers::temp_db;
use crate::db::work_items::WorkItemInput;
use crate::db::work_items::WorkItemMetadata;

#[test]
fn count_issues_by_category_tallies_in_one_pass() {
    use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

    fn issue(category: ScanCategory, severity: Severity, status: CheckStatus) -> CheckResult {
        CheckResult {
            check_id: "x".into(),
            category,
            title: "t".into(),
            description: "d".into(),
            status,
            severity,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    }

    let issues = vec![
        issue(
            ScanCategory::Security,
            Severity::Critical,
            CheckStatus::Fail,
        ),
        issue(ScanCategory::Security, Severity::High, CheckStatus::Warn),
        issue(
            ScanCategory::Security,
            Severity::Critical,
            CheckStatus::Skipped,
        ),
        issue(ScanCategory::Security, Severity::Medium, CheckStatus::Pass),
        issue(ScanCategory::Performance, Severity::Low, CheckStatus::Fail),
    ];

    let counts = super::count_issues_by_category(&issues);
    let sec = counts
        .get(&ScanCategory::Security)
        .copied()
        .unwrap_or_default();
    // total counts non-pass, non-skipped issues (Fail Critical + Warn High).
    assert_eq!(sec.total, 2);
    // Skipped checks contribute to neither failure nor pass counts,
    // matching scoring::calculator::calculate_scores.
    assert_eq!(sec.critical, 1);
    assert_eq!(sec.high, 1);
    assert_eq!(sec.medium, 0);
    assert_eq!(sec.passed, 1);

    let perf = counts
        .get(&ScanCategory::Performance)
        .copied()
        .unwrap_or_default();
    assert_eq!(perf.total, 1);
    assert_eq!(perf.low, 1);
}

fn make_scan_result(score: u32, ts: &str) -> ScanResult {
    ScanResult {
        page_signals: None,
        site_facts: None,
        url: "https://example.com".to_string(),
        mode: "full".to_string(),
        scan_type: crate::core::scanner::ScanType::Health,
        overall_score: score,
        categories: vec![],
        issues: vec![],
        detected_stack: None,
        duration_ms: 1000,
        timestamp: ts.to_string(),
    }
}

fn make_snapshot_issue(check_id: &str) -> crate::checks::CheckResult {
    crate::checks::CheckResult {
        check_id: check_id.into(),
        category: crate::checks::ScanCategory::Security,
        title: format!("Check {check_id}"),
        description: "test".into(),
        status: crate::checks::CheckStatus::Fail,
        severity: crate::checks::Severity::High,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

fn make_work_item(check_id: &str, severity: &str, scan_id: i64, project_id: i64) -> WorkItemInput {
    WorkItemInput {
        project_id,
        env_url: "https://example.com".to_string(),
        source: "web_scan".to_string(),
        signal_id: format!("web_scan:{}:https://example.com", check_id),
        check_id: check_id.to_string(),
        category: "performance".to_string(),
        severity: severity.parse().expect("valid severity"),
        title: format!("Check {}", check_id),
        description: "test".to_string(),
        detail_json: None,
        scan_ref: Some(scan_id),
        page_url: Some("https://example.com".to_string()),
        fix_prompt: None,
        manual_fix: None,
        why_it_matters: None,
        observed_at: 1_000,
        metadata: WorkItemMetadata::default(),
    }
}

// Each immutable scan snapshot retains the guidance emitted in that scan;
// a later scan gets its refreshed copy without rewriting history.
#[test]
fn get_scan_detail_round_trips_guidance_per_immutable_snapshot() {
    use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").unwrap();
    let original = CheckResult {
        check_id: "security.csp".into(),
        category: ScanCategory::Security,
        severity: Severity::High,
        status: CheckStatus::Fail,
        title: "Check security.csp".into(),
        description: "test".into(),
        fix_prompt: Some("Add a Content-Security-Policy header.".into()),
        manual_fix: Some("Set the CSP header in your server config.".into()),
        raw_data: None,
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: Some("Without CSP, injected scripts run unrestricted.".into()),
    };
    let mut first_scan = make_scan_result(80, "2025-01-01T00:00:00Z");
    first_scan.issues = vec![original];
    let scan_id = db.save_scan(site_id, &first_scan).unwrap();

    let detail = db.get_scan_detail(scan_id).unwrap().expect("scan detail");
    assert_eq!(detail.issues.len(), 1);
    let issue = &detail.issues[0];
    // Value-pins the columns adjacent to the guidance params so a
    // positional transposition anywhere in the upsert row fails here.
    assert_eq!(issue.title, "Check security.csp");
    assert_eq!(issue.description, "test");
    assert_eq!(
        issue.fix_prompt.as_deref(),
        Some("Add a Content-Security-Policy header.")
    );
    assert_eq!(
        issue.manual_fix.as_deref(),
        Some("Set the CSP header in your server config.")
    );
    assert_eq!(
        issue.why_it_matters.as_deref(),
        Some("Without CSP, injected scripts run unrestricted.")
    );

    let mut updated = first_scan.issues[0].clone();
    updated.fix_prompt = Some("Updated fix prompt.".into());
    updated.manual_fix = Some("Updated manual fix.".into());
    updated.why_it_matters = Some("Updated why it matters.".into());
    let mut second_scan = make_scan_result(82, "2025-01-02T00:00:00Z");
    second_scan.issues = vec![updated];
    let second_id = db.save_scan(site_id, &second_scan).unwrap();

    let refreshed = db
        .get_scan_detail(second_id)
        .unwrap()
        .expect("second scan detail");
    let issue = &refreshed.issues[0];
    assert_eq!(issue.fix_prompt.as_deref(), Some("Updated fix prompt."));
    assert_eq!(issue.manual_fix.as_deref(), Some("Updated manual fix."));
    assert_eq!(
        issue.why_it_matters.as_deref(),
        Some("Updated why it matters.")
    );
    let original_again = db.get_scan_detail(scan_id).unwrap().expect("first scan");
    assert_eq!(
        original_again.issues[0].fix_prompt.as_deref(),
        Some("Add a Content-Security-Policy header.")
    );
}

#[test]
fn get_scan_detail_preserves_status_confidence_and_confidence_reason() {
    use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

    let db = temp_db();
    let project_id = db
        .upsert_project("test", "/tmp/test-issue-verdict", None)
        .unwrap();
    let site_id = db.get_or_create_site("https://example.com").unwrap();
    let finding = CheckResult {
        check_id: "performance.third-party-script".into(),
        category: ScanCategory::Performance,
        title: "Third-party script needs review".into(),
        description: "A cross-origin script was observed.".into(),
        status: CheckStatus::Warn,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: Some("Confirm the script is necessary and measure its cost.".into()),
        raw_data: Some(serde_json::json!({"host": "cdn.example.test"})),
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some(
            "The static response does not establish runtime cost or business necessity.".into(),
        ),
        why_it_matters: Some("Third-party code can add latency and operational risk.".into()),
    };
    let mut scan = make_scan_result(92, "2025-01-01T00:00:00Z");
    scan.issues = vec![finding.clone()];
    let scan_id = db.save_scan(site_id, &scan).unwrap();
    let input = crate::commands::scan::work_items::check_result_to_work_item_input(
        &finding,
        project_id,
        "https://example.com",
        scan_id,
        1_000,
        None,
    );
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://example.com",
        vec![input],
        1_000,
    )
    .unwrap();

    let detail = db.get_scan_detail(scan_id).unwrap().expect("scan detail");
    let restored = detail.issues.first().expect("persisted finding");
    assert_eq!(restored.status, CheckStatus::Warn);
    assert_eq!(restored.confidence, IssueConfidence::NeedsReview);
    assert_eq!(restored.confidence_reason, finding.confidence_reason);
    assert_eq!(restored.raw_data, finding.raw_data);
}

#[test]
fn get_scan_detail_preserves_producer_identity_and_fix_prompt() {
    use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

    let db = temp_db();
    let project_id = db
        .upsert_project("test", "/tmp/test-producer-fields", None)
        .unwrap();
    let site_id = db.get_or_create_site("https://example.com").unwrap();
    let finding = CheckResult {
        check_id: "security.headers.csp".into(),
        category: ScanCategory::Security,
        title: "Content Security Policy needs review".into(),
        description: "The response policy needs review.".into(),
        status: CheckStatus::Warn,
        severity: Severity::Low,
        fix_prompt: Some("Preserve this producer-authored remediation exactly.".into()),
        manual_fix: None,
        raw_data: None,
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some("The policy was observed but requires context.".into()),
        why_it_matters: None,
    };
    let mut scan = make_scan_result(75, "2025-01-01T00:00:00Z");
    scan.issues = vec![finding.clone()];
    let scan_id = db.save_scan(site_id, &scan).unwrap();
    let input = crate::commands::scan::work_items::check_result_to_work_item_input(
        &finding,
        project_id,
        "https://example.com",
        scan_id,
        1_000,
        None,
    );
    assert_eq!(input.check_id, "security.csp");
    assert_ne!(input.fix_prompt, finding.fix_prompt);
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://example.com",
        vec![input],
        1_000,
    )
    .unwrap();

    let detail = db.get_scan_detail(scan_id).unwrap().expect("scan detail");
    let restored = detail.issues.first().expect("persisted finding");
    assert_eq!(restored.check_id, finding.check_id);
    assert_eq!(restored.fix_prompt, finding.fix_prompt);
}

#[test]
fn get_scan_detail_preserves_producer_category() {
    use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

    let db = temp_db();
    let project_id = db
        .upsert_project("test", "/tmp/test-producer-category", None)
        .unwrap();
    let site_id = db.get_or_create_site("https://example.com").unwrap();
    let finding = CheckResult {
        check_id: "config.favicon".into(),
        category: ScanCategory::Config,
        title: "Favicon configuration".into(),
        description: "The deployed favicon needs review.".into(),
        status: CheckStatus::Warn,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: None,
        confidence: IssueConfidence::NeedsReview,
        confidence_reason: Some("A default asset can still be intentional.".into()),
        why_it_matters: None,
    };
    let mut scan = make_scan_result(95, "2025-01-01T00:00:00Z");
    scan.issues = vec![finding.clone()];
    let scan_id = db.save_scan(site_id, &scan).unwrap();
    let input = crate::commands::scan::work_items::check_result_to_work_item_input(
        &finding,
        project_id,
        "https://example.com",
        scan_id,
        1_000,
        None,
    );
    assert_eq!(input.category, "compliance");
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://example.com",
        vec![input],
        1_000,
    )
    .unwrap();

    let detail = db.get_scan_detail(scan_id).unwrap().expect("scan detail");
    assert_eq!(detail.issues[0].category, ScanCategory::Config);
}

#[test]
fn scan_issue_snapshots_are_immutable_across_rescans_and_include_passes() {
    use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

    fn finding(title: &str, status: CheckStatus) -> CheckResult {
        CheckResult {
            check_id: if status == CheckStatus::Pass {
                "seo.title"
            } else {
                "security.headers.csp"
            }
            .into(),
            category: if status == CheckStatus::Pass {
                ScanCategory::Seo
            } else {
                ScanCategory::Security
            },
            title: title.into(),
            description: format!("Description for {title}"),
            status,
            severity: if status == CheckStatus::Pass {
                Severity::Low
            } else {
                Severity::High
            },
            fix_prompt: (status != CheckStatus::Pass).then(|| format!("Producer fix for {title}")),
            manual_fix: None,
            raw_data: Some(serde_json::json!({ "title": title })),
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    }

    let db = temp_db();
    let project_id = db
        .upsert_project("test", "/tmp/test-immutable-scan-issues", None)
        .unwrap();
    let site_id = db.get_or_create_site("https://example.com").unwrap();

    let original_fail = finding("Original CSP finding", CheckStatus::Fail);
    let original_pass = finding("Original title pass", CheckStatus::Pass);
    let mut first = make_scan_result(70, "2025-01-01T00:00:00Z");
    first.issues = vec![original_fail.clone(), original_pass.clone()];
    let first_id = db.save_scan(site_id, &first).unwrap();
    let first_work_item = crate::commands::scan::work_items::check_result_to_work_item_input(
        &original_fail,
        project_id,
        "https://example.com",
        first_id,
        1_000,
        None,
    );
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://example.com",
        vec![first_work_item],
        1_000,
    )
    .unwrap();

    let changed_fail = finding("Changed CSP finding", CheckStatus::Warn);
    let mut second = make_scan_result(80, "2025-01-02T00:00:00Z");
    second.issues = vec![changed_fail.clone()];
    let second_id = db.save_scan(site_id, &second).unwrap();
    let second_work_item = crate::commands::scan::work_items::check_result_to_work_item_input(
        &changed_fail,
        project_id,
        "https://example.com",
        second_id,
        2_000,
        None,
    );
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://example.com",
        vec![second_work_item],
        2_000,
    )
    .unwrap();

    let restored_first = db.get_scan_detail(first_id).unwrap().expect("first scan");
    assert_eq!(
        serde_json::to_value(&restored_first.issues).unwrap(),
        serde_json::to_value(&first.issues).unwrap()
    );
    let restored_second = db.get_scan_detail(second_id).unwrap().expect("second scan");
    assert_eq!(
        serde_json::to_value(&restored_second.issues).unwrap(),
        serde_json::to_value(&second.issues).unwrap()
    );
}

#[test]
fn session_scan_detail_preserves_page_url_and_all_category_scores() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").unwrap();
    let session_id = db.create_scan_session(site_id, 1, false).unwrap();
    let mut scan = make_scan_result(88, "2025-01-01T00:00:00Z");
    scan.url = "https://example.com/about".into();
    scan.categories = vec![
        crate::scoring::calculator::CategoryScore {
            category: crate::checks::ScanCategory::Config,
            score: 91,
            issues_total: 1,
            issues_critical: 0,
            issues_high: 0,
            issues_medium: 0,
            issues_low: 1,
            issues_passed: 4,
        },
        crate::scoring::calculator::CategoryScore {
            category: crate::checks::ScanCategory::Polish,
            score: 73,
            issues_total: 2,
            issues_critical: 0,
            issues_high: 0,
            issues_medium: 1,
            issues_low: 1,
            issues_passed: 28,
        },
    ];
    scan.issues = vec![make_snapshot_issue("security.csp")];

    let scan_id = db
        .save_scan_with_session(site_id, session_id, "https://example.com/about", &scan)
        .unwrap();
    let restored = db.get_scan_detail(scan_id).unwrap().expect("scan detail");

    assert_eq!(restored.url, "https://example.com/about");
    assert_eq!(restored.issues.len(), 1);
    let config = restored
        .categories
        .iter()
        .find(|category| category.category == crate::checks::ScanCategory::Config)
        .expect("config score");
    let polish = restored
        .categories
        .iter()
        .find(|category| category.category == crate::checks::ScanCategory::Polish)
        .expect("polish score");
    assert_eq!(config.score, 91);
    assert_eq!(polish.score, 73);
}

#[test]
fn get_scan_detail_reports_corrupt_issue_evidence_instead_of_silently_dropping_it() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").unwrap();
    let mut scan = make_scan_result(80, "2025-01-01T00:00:00Z");
    scan.issues = vec![make_snapshot_issue("security.csp")];
    let scan_id = db.save_scan(site_id, &scan).unwrap();
    db.execute(move |conn| {
        conn.execute(
            "UPDATE scan_findings SET raw_data = '{not-valid-json' WHERE run_id = ?1",
            rusqlite::params![scan_id],
        )
    })
    .unwrap()
    .unwrap();

    assert!(
        db.get_scan_detail(scan_id).is_err(),
        "corrupt persisted evidence must be reported, not returned as an empty raw_data field"
    );
}

#[test]
fn get_scan_detail_reports_corrupt_issue_enum_instead_of_reclassifying_it() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").unwrap();
    let mut scan = make_scan_result(80, "2025-01-01T00:00:00Z");
    scan.issues = vec![make_snapshot_issue("security.csp")];
    let scan_id = db.save_scan(site_id, &scan).unwrap();
    db.execute(move |conn| {
        conn.execute_batch("PRAGMA ignore_check_constraints = ON")?;
        conn.execute(
            "UPDATE scan_findings SET severity = 'urgent' WHERE run_id = ?1",
            rusqlite::params![scan_id],
        )
    })
    .unwrap()
    .unwrap();

    assert!(
        db.get_scan_detail(scan_id).is_err(),
        "an unknown severity must not silently become Medium"
    );
}

#[test]
fn get_scan_detail_reports_corrupt_detected_stack_json() {
    let db = temp_db();
    let site_id = db.get_or_create_site("https://example.com").unwrap();
    let scan_id = db
        .save_scan(site_id, &make_scan_result(80, "2025-01-01T00:00:00Z"))
        .unwrap();
    db.execute(move |conn| {
        conn.execute(
            "UPDATE scan_runs SET detected_stack = '{not-json' WHERE id = ?1",
            rusqlite::params![scan_id],
        )
    })
    .unwrap()
    .unwrap();

    assert!(
        db.get_scan_detail(scan_id).is_err(),
        "corrupt stack evidence must not silently disappear"
    );
}

#[test]
fn get_prior_scan_check_severities_returns_prior_scan_items() {
    let db = temp_db();
    let project_id = db
        .upsert_project("test", "/tmp/test-prior-sevs", None)
        .unwrap();
    let site_id = db.get_or_create_site("https://example.com").unwrap();

    // Prior scan (older timestamp).
    let mut lcp = make_snapshot_issue("performance.lcp");
    lcp.category = crate::checks::ScanCategory::Performance;
    lcp.severity = crate::checks::Severity::Medium;
    let mut prior_result = make_scan_result(80, "2025-01-01T00:00:00Z");
    prior_result.issues = vec![lcp];
    let prior_id = db.save_scan(site_id, &prior_result).unwrap();
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://example.com",
        vec![make_work_item(
            "performance.lcp",
            "medium",
            prior_id,
            project_id,
        )],
        1_000,
    )
    .unwrap();

    // Current scan (newer timestamp).
    let mut current_result = make_scan_result(70, "2025-01-02T00:00:00Z");
    let mut current_lcp = make_snapshot_issue("performance.lcp");
    current_lcp.category = crate::checks::ScanCategory::Performance;
    current_lcp.severity = crate::checks::Severity::High;
    current_result.issues = vec![current_lcp];
    let current_id = db.save_scan(site_id, &current_result).unwrap();
    // The active work item moves to the current scan. Historical severity
    // must still come from the prior immutable snapshot.
    db.upsert_work_items_diff(
        "web_scan",
        project_id,
        "https://example.com",
        vec![make_work_item(
            "performance.lcp",
            "high",
            current_id,
            project_id,
        )],
        2_000,
    )
    .unwrap();

    let prior_sevs = db
        .get_prior_scan_check_severities("https://example.com", current_id, "web_scan")
        .unwrap();

    assert_eq!(
        prior_sevs.get("performance.lcp").map(|s| s.as_str()),
        Some("medium"),
        "prior scan should have lcp at medium"
    );
    assert!(
        !prior_sevs.contains_key("performance.ttfb"),
        "ttfb was not in prior scan"
    );
}

#[test]
fn get_prior_code_scan_severities_uses_same_project_environment_snapshot() {
    use crate::checks::{IssueConfidence, Severity};
    use crate::core::code_scan::{CodeIssue, CodeScanReport};

    let db = temp_db();
    let project_id = db
        .upsert_project("test", "/tmp/test-prior-code-sevs", None)
        .unwrap();
    let issue = |severity| CodeIssue {
        id: "ai-timeout".to_string(),
        check_id: String::new(),
        category: "ai-safety".to_string(),
        severity,
        title: "Missing timeout".to_string(),
        description: "AI request has no bounded timeout.".to_string(),
        relative_path: "app/api/chat/route.ts".to_string(),
        absolute_path: "/tmp/test-prior-code-sevs/app/api/chat/route.ts".to_string(),
        line: Some(42),
        source_excerpt: None,
        evidence: None,
        why_now: None,
        likely_fix: None,
        confidence: IssueConfidence::High,
        confidence_reason: None,
        verify_hint: None,
    };
    let report = |checked_at: &str, issue: CodeIssue| CodeScanReport {
        skipped_scopes: Default::default(),
        checked_at: checked_at.to_string(),
        framework: Some("Next.js".to_string()),
        issue_count: 1,
        critical_count: usize::from(issue.severity == Severity::Critical),
        high_count: usize::from(issue.severity == Severity::High),
        medium_count: usize::from(issue.severity == Severity::Medium),
        low_count: usize::from(issue.severity == Severity::Low),
        issues: vec![issue],
    };

    let prior = issue(Severity::Medium);
    db.save_code_scan(
        project_id,
        Some("https://example.com".to_string()),
        "/tmp/test-prior-code-sevs".to_string(),
        &report("2025-01-01T00:00:00Z", prior),
        100,
    )
    .unwrap();
    let current = issue(Severity::High);
    let current_id = db
        .save_code_scan(
            project_id,
            Some("https://example.com".to_string()),
            "/tmp/test-prior-code-sevs".to_string(),
            &report("2025-01-02T00:00:00Z", current.clone()),
            100,
        )
        .unwrap();
    let input = crate::commands::scan::work_items::code_issue_to_work_item_input(
        &current,
        project_id,
        "https://example.com",
        current_id,
        2_000,
        None,
    );
    db.upsert_work_items_diff(
        "code_scan",
        project_id,
        "https://example.com",
        vec![input],
        2_000,
    )
    .unwrap();

    let prior_severities = db
        .get_prior_scan_check_severities("https://example.com", current_id, "code_scan")
        .unwrap();
    let canonical = crate::core::correlation::resolve_check_id("code_scan", "ai-timeout");
    assert_eq!(
        prior_severities.get(&canonical).map(String::as_str),
        Some("medium")
    );
}

#[test]
fn get_prior_scan_check_severities_returns_empty_when_no_prior_scan() {
    let db = temp_db();
    db.upsert_project("test", "/tmp/test-no-prior", None)
        .unwrap();
    let site_id = db.get_or_create_site("https://example.com").unwrap();

    let result = make_scan_result(80, "2025-01-01T00:00:00Z");
    let scan_id = db.save_scan(site_id, &result).unwrap();

    let prior_sevs = db
        .get_prior_scan_check_severities("https://example.com", scan_id, "web_scan")
        .unwrap();

    assert!(
        prior_sevs.is_empty(),
        "no prior scan should yield empty map"
    );
}
