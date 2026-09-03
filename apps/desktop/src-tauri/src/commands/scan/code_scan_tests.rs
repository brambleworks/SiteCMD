//! Blame helpers must compare only scans from the same environment;
//! cross-environment fallback is valid for trends, not regression attribution.

use super::*;

fn summary(id: i64, environment_url: Option<&str>) -> CodeScanSummary {
    CodeScanSummary {
        id,
        project_id: 1,
        environment_url: environment_url.map(str::to_string),
        overall_score: 88,
        issue_count: 3,
        grouped_issue_count: 3,
        critical_count: 0,
        high_count: 1,
        duration_ms: 1200,
        checked_at: "2026-06-09T12:00:00Z".to_string(),
        framework: None,
        top_domain: None,
        top_domain_count: 0,
        domain_summaries: Vec::new(),
    }
}

#[test]
fn blame_previous_scan_same_env_normalized_variants_match() {
    // Trailing slash and host case differences must normalize equal,
    // exactly like the work_items env key (normalize_env_url).
    let previous = summary(7, Some("https://Example.COM/app/"));
    let result = blame_previous_scan(Some(&previous), Some("https://example.com/app"))
        .expect("normalized-equal envs must produce a blame PreviousScan");
    assert_eq!(result.scan_id, 7);
    assert_eq!(result.overall_score, 88);
    assert_eq!(result.timestamp, "2026-06-09T12:00:00Z");
}

#[test]
fn blame_previous_scan_differing_envs_returns_none() {
    let previous = summary(7, Some("https://staging.example.com"));
    assert!(
        blame_previous_scan(Some(&previous), Some("https://example.com")).is_none(),
        "cross-env history must not feed blame"
    );
}

#[test]
fn blame_previous_scan_env_less_history_with_env_scan_returns_none() {
    // First scan under a new env key: the only history is project-wide
    // (NULL env). Blaming against it would mark every finding "new".
    let previous = summary(7, None);
    assert!(blame_previous_scan(Some(&previous), Some("https://example.com")).is_none());
}

#[test]
fn blame_previous_scan_both_env_less_matches() {
    // Env-less project history is consistent with an env-less current
    // scan; blame may diff against it.
    let previous = summary(9, None);
    let result =
        blame_previous_scan(Some(&previous), None).expect("both-None envs are the same key space");
    assert_eq!(result.scan_id, 9);
}

#[test]
fn blame_previous_scan_without_history_returns_none() {
    assert!(blame_previous_scan(None, Some("https://example.com")).is_none());
    assert!(blame_previous_scan(None, None).is_none());
}

fn code_scan_result_fixture() -> CodeScanResult {
    CodeScanResult {
        id: 1,
        project_id: 1,
        environment_url: None,
        overall_score: 100,
        issue_count: 0,
        critical_count: 0,
        high_count: 0,
        medium_count: 0,
        low_count: 0,
        duration_ms: 10,
        checked_at: "2026-06-09T12:00:00Z".to_string(),
        framework: None,
        domain_summaries: Vec::new(),
        skipped_scopes: Default::default(),
        issues: Vec::new(),
    }
}

/// The part of this file that ships, with the test module stripped.
fn production_half(source: &str) -> &str {
    source
        .split_once("\n#[cfg(test)]")
        .map_or(source, |(production, _)| production)
}

/// First line of every statement in an `async fn` body of this file that
/// names a `Database` handle, sits outside every `spawn_blocking` closure,
/// and is never awaited. Such a statement parks an async runtime worker on
/// the SQLite thread; an awaited statement reaches the database through the
/// async interface, and a `spawn_blocking` closure has its own thread.
///
/// Both handle shapes count: a method call on a handle
/// (`db.get_x(..)`, `history_db.as_ref()`) and a handle passed to a helper
/// that does the read itself (`resolve_registered_project_dir(&db, ..)`).
/// `db.clone()` is exempt because cloning a handle touches no connection.
///
/// Lines are joined into statements (ending at `;`, a brace, or a blank
/// line) before matching, so a call rustfmt split across lines is judged
/// whole. Only `async fn` bodies are scanned, so a synchronous helper in
/// this file is not flagged.
///
/// What it cannot see: a database read behind a helper that takes no
/// visible handle (a global, or a handle held in a struct the statement
/// passes), and an awaited statement that also does blocking database work
/// on the side.
fn database_uses_on_the_async_worker(source: &str) -> Vec<usize> {
    // Both delimiters sit on one line so this file stays brace-balanced for
    // the repository's production-half scanners.
    const BLOCK_DELIMITERS: [char; 2] = ['{', '}'];

    let handle_use = regex::Regex::new(r"(?:\b[a-z_]*db\s*\.\s*[a-z_]+\s*\(|&[a-z_]*db\b)")
        .expect("database handle pattern");
    let async_signature = regex::Regex::new(r"^\s*(?:pub(?:\([a-z()]+\))?\s+)?async\s+fn\s")
        .expect("async fn signature pattern");

    let mut findings = Vec::new();
    let mut depth: i64 = 0;
    let mut offloads: Vec<i64> = Vec::new();
    let mut async_body: Option<i64> = None;
    let mut pending_async_depth: Option<i64> = None;
    let mut statement = String::new();
    let mut statement_line = 0usize;
    let mut statement_on_async_worker = false;

    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if async_body.is_none() && pending_async_depth.is_none() && async_signature.is_match(line) {
            pending_async_depth = Some(depth);
        }
        let opens_offload = line.contains("spawn_blocking(");

        if !trimmed.is_empty() && !trimmed.starts_with("//") {
            if statement.is_empty() {
                statement_line = index + 1;
                statement_on_async_worker =
                    async_body.is_some() && offloads.is_empty() && !opens_offload;
            }
            statement.push('\n');
            statement.push_str(line);
        }

        if opens_offload {
            offloads.push(depth);
        }
        depth += line.matches('{').count() as i64 - line.matches('}').count() as i64;
        while offloads.last().is_some_and(|opened| depth <= *opened) {
            offloads.pop();
        }
        if let Some(signature_depth) = pending_async_depth {
            if depth > signature_depth {
                pending_async_depth = None;
                async_body = Some(signature_depth);
            }
        }
        if async_body.is_some_and(|opened| depth <= opened) {
            async_body = None;
        }

        let ends_statement = trimmed.is_empty()
            || trimmed.ends_with(';')
            || trimmed.trim_end_matches(',').ends_with(BLOCK_DELIMITERS);
        if ends_statement && !statement.is_empty() {
            if statement_on_async_worker
                && handle_use.is_match(&statement)
                && !statement.contains(".await")
                && !statement.contains("db.clone()")
            {
                findings.push(statement_line);
            }
            statement.clear();
        }
    }
    findings
}

#[test]
fn post_audit_database_work_never_blocks_the_async_worker() {
    let production = production_half(include_str!("code_scan.rs"));
    assert!(
        production.contains(".persist_normalized_scan_run_async(batch)"),
        "the Code Scan report must be written through the async database interface"
    );
    assert!(
        !production.contains(".persist_normalized_scan_run(batch)"),
        "the blocking scan-run write must not run on the async worker"
    );
    let blocking = database_uses_on_the_async_worker(production);
    assert!(
        blocking.is_empty(),
        "inside an async fn, a database handle may only be cloned into a spawn_blocking closure or used in an awaited statement; found unawaited uses at lines {blocking:?}"
    );
}

#[test]
fn the_offload_scanner_sees_a_call_that_escapes_its_closure() {
    let escaped = "async fn a() {\n    spawn_blocking(move || {\n        history_db.read(1)\n    });\n    db.write(2);\n}\n";
    assert_eq!(database_uses_on_the_async_worker(escaped), vec![5]);
    let wrapped =
        "async fn a() {\n    spawn_blocking(move || {\n        history_db.read(1)\n    });\n}\n";
    assert!(database_uses_on_the_async_worker(wrapped).is_empty());
}

#[test]
fn the_offload_scanner_sees_a_handle_handed_to_a_blocking_helper() {
    // The shape review finding 1 caught: the read happens inside a helper,
    // so no method call on the handle appears at the call site.
    let helper = "async fn a() {\n    let path = resolve_dir(&db, id)?;\n}\n";
    assert_eq!(database_uses_on_the_async_worker(helper), vec![2]);
    let awaited =
        "async fn a() {\n    let path = resolve_dir_async(&db, id)\n        .await?;\n}\n";
    assert!(database_uses_on_the_async_worker(awaited).is_empty());
}

#[test]
fn the_offload_scanner_reads_a_call_rustfmt_split_across_lines() {
    let split = "async fn a() {\n    let id = db\n        .write_blocking(batch)\n        .map_err(sanitize)?;\n}\n";
    assert_eq!(database_uses_on_the_async_worker(split), vec![2]);
    let split_async = "async fn a() {\n    let id = db\n        .write_async(batch)\n        .await\n        .map_err(sanitize)?;\n}\n";
    assert!(database_uses_on_the_async_worker(split_async).is_empty());
}

#[test]
fn the_offload_scanner_ignores_synchronous_helpers_and_handle_clones() {
    let sync_helper = "fn a(db: &Database) -> i64 {\n    db.read(1)\n}\n";
    assert!(database_uses_on_the_async_worker(sync_helper).is_empty());
    let cloned = "async fn a() {\n    let history_db = db.clone();\n}\n";
    assert!(database_uses_on_the_async_worker(cloned).is_empty());
}

#[test]
fn a_cancelled_audit_returns_before_the_persistence_step() {
    let production = production_half(include_str!("code_scan.rs"));
    assert!(
        production.contains("CodeScanError::Cancelled => CodeScanError::Cancelled,"),
        "engine cancellation must stay a cancellation instead of becoming a failure"
    );
    let audit_result = production
        .find("let source_controlled = report.map_err")
        .expect("the audit result must be mapped into the command error");
    let persist = production
        .find(".persist_normalized_scan_run_async(batch)")
        .expect("the audit path must persist its report");
    assert!(
        audit_result < persist,
        "the cancelled audit result must propagate before anything is written"
    );
    assert!(
        production[audit_result..persist].contains("?;"),
        "the audit result must propagate with `?`, so a cancelled audit never reaches persistence"
    );
}

#[test]
fn cancellation_is_checked_immediately_before_the_report_is_written() {
    let production = production_half(include_str!("code_scan.rs"));
    let persist = production
        .find(".persist_normalized_scan_run_async(batch)")
        .expect("the audit path must persist its report");
    let gate = production[..persist]
        .rfind("if is_cancelled() {")
        .expect("a cancellation check must sit before the report write");
    let between = &production[gate..persist];
    assert!(
        between.contains("return Err(CodeScanError::Cancelled);"),
        "the check before the write must abandon the run"
    );
    assert!(
        !between.contains("spawn_blocking(") && !between.contains(".await"),
        "nothing may run between the last cancellation check and the write, so a cancel arriving during the history read or blame baseline still leaves no run behind"
    );
    let running_save = production
        .find(r#""code-scan.save", "running""#)
        .expect("the audit path must announce the save stage it is about to run");
    assert!(
        gate < running_save,
        "the gate must sit above the code-scan.save running emit, or a cancel landing between \
         the two leaves the progress feed with a save stage stuck on running"
    );
}

#[test]
fn failure_alert_skips_user_cancellation() {
    let result: Result<CodeScanResult, CodeScanError> = Err(CodeScanError::Cancelled);
    let error = result.as_ref().expect_err("fixture is a cancellation");
    assert!(
        crate::core::native_alerts::is_user_cancelled_code_scan(error),
        "the typed predicate must match the Cancelled variant"
    );
    assert!(
        code_scan_failure_alert_error(&result).is_none(),
        "a user cancellation must not record a scan-failure alert"
    );
}

#[test]
fn failure_alert_fires_for_real_errors() {
    let result: Result<CodeScanResult, CodeScanError> = Err(CodeScanError::Failed(
        "Code scan task failed: boom".to_string(),
    ));
    let error = code_scan_failure_alert_error(&result)
        .expect("engine/infra failures must record a scan-failure alert");
    assert!(matches!(error, CodeScanError::Failed(_)));
}

#[test]
fn failure_alert_skips_success() {
    let result: Result<CodeScanResult, CodeScanError> = Ok(code_scan_result_fixture());
    assert!(code_scan_failure_alert_error(&result).is_none());
}

#[test]
fn source_control_suppressions_filter_the_persisted_scan_report() {
    let project = tempfile::tempdir().expect("project");
    let sitecmd_dir = project.path().join(".sitecmd");
    std::fs::create_dir_all(&sitecmd_dir).expect("sitecmd directory");
    std::fs::write(
        sitecmd_dir.join("config.json"),
        r#"{
  "version": 1,
  "url": "https://example.com",
  "name": "Suppressed project",
  "code_scan": {
"suppressions": [{
  "match": {
    "rule": "code_scan.cors-origin-reflection",
    "path": "content/security.ts"
  },
  "reason": "This file contains inert security guidance."
}]
  }
}"#,
    )
    .expect("suppression config");
    let issue = crate::core::code_scan::CodeIssue {
        id: "cors-origin-reflection:content/security.ts:371".to_string(),
        check_id: "code_scan.cors-origin-reflection".to_string(),
        category: "security".to_string(),
        severity: crate::checks::Severity::High,
        title: "CORS reflects the request origin while allowing credentials".to_string(),
        description: "The source appears to reflect credentialed origins.".to_string(),
        relative_path: "content/security.ts".to_string(),
        absolute_path: project
            .path()
            .join("content/security.ts")
            .to_string_lossy()
            .to_string(),
        line: Some(371),
        source_excerpt: Some("replace origin: true with an exact allowlist".to_string()),
        evidence: None,
        why_now: None,
        likely_fix: Some("Use an exact allowlist.".to_string()),
        confidence: crate::checks::IssueConfidence::High,
        confidence_reason: None,
        verify_hint: None,
    };
    let report = crate::core::code_scan::CodeScanReport {
        checked_at: "2026-08-31T12:00:00Z".to_string(),
        framework: Some("typescript".to_string()),
        issue_count: 1,
        critical_count: 0,
        high_count: 1,
        medium_count: 0,
        low_count: 0,
        issues: vec![issue],
        skipped_scopes: Default::default(),
    };

    let filtered = apply_source_control_suppressions(
        project.path(),
        report,
        chrono::NaiveDate::from_ymd_opt(2026, 8, 31).expect("date"),
    )
    .expect("valid suppression");

    assert_eq!(filtered.report.issue_count, 0);
    assert!(filtered.report.issues.is_empty());
    assert_eq!(filtered.ignored_count, 1);
    assert_eq!(filtered.evidence_report.issue_count, 1);

    let mut batch = crate::core::normalized_scan::normalize_code_scan(
        &filtered.evidence_report,
        1,
        1,
        Some("https://example.com".to_string()),
        "https://example.com".to_string(),
        project.path().to_string_lossy().to_string(),
        100,
        10,
        1_000,
    )
    .expect("normalize evidence");
    mark_suppressed_findings(&mut batch, &filtered.suppressed_occurrence_ids)
        .expect("mark suppressed evidence");

    assert_eq!(batch.findings.len(), 1);
    assert_eq!(
        batch.findings[0].verdict,
        crate::checks::CheckStatus::Skipped
    );
}
