//! Completes sparse agent fix briefs from stored scan guidance.

use crate::db::Database;

/// Guidance fields that a sparse caller did not supply.
#[derive(Debug, Default)]
pub(super) struct StoredIssueGuidance {
    pub(super) why_it_matters: Option<String>,
    pub(super) evidence: Option<serde_json::Value>,
    pub(super) manual_fix: Option<String>,
}

impl StoredIssueGuidance {
    fn is_complete(&self) -> bool {
        self.why_it_matters.is_some() && self.evidence.is_some() && self.manual_fix.is_some()
    }
}

/// Complete sparse guidance from the newest retained health scan for this
/// environment.
fn stored_web_issue_guidance(
    db: &Database,
    project_id: i64,
    env_url: &str,
    check_id: &str,
) -> Result<Option<StoredIssueGuidance>, String> {
    // Search retained scan types newest-first for canonical or producer aliases.
    let history =
        db.get_scan_history_for_project(project_id, env_url, crate::db::MAX_SCAN_RETENTION)?;
    let mut seen_scan_types = std::collections::HashSet::new();
    for scan in history {
        if !seen_scan_types.insert(scan.scan_type) {
            continue;
        }
        let detail = db
            .get_scan_detail(scan.id)?
            .ok_or_else(|| format!("scan history references missing scan {}", scan.id))?;
        let issue = detail
            .issues
            .into_iter()
            .filter(|issue| {
                matches!(
                    issue.status,
                    crate::checks::CheckStatus::Fail | crate::checks::CheckStatus::Warn
                ) && crate::core::correlation::resolve_check_id("web_scan", &issue.check_id)
                    == check_id
            })
            .min_by_key(|issue| issue.severity.sort_rank());
        if let Some(issue) = issue {
            return Ok(Some(StoredIssueGuidance {
                why_it_matters: issue.why_it_matters,
                evidence: issue.raw_data,
                manual_fix: issue.manual_fix,
            }));
        }
    }
    Ok(None)
}

/// Guidance from the newest stored code scan. `get_code_scan_detail` returns
/// the full issue views; `get_code_scan_issue_views` is the lightweight
/// variant that strips exactly the guidance fields we need.
fn stored_code_issue_guidance(
    db: &Database,
    project_id: i64,
    env_url: &str,
    check_id: &str,
) -> Result<Option<StoredIssueGuidance>, String> {
    let target_env = crate::db::normalize_env_url(Some(env_url));
    let history = db.get_code_scan_history(project_id, crate::db::MAX_SCAN_RETENTION)?;
    let Some(newest) = history
        .into_iter()
        .find(|scan| crate::db::normalize_env_url(scan.environment_url.as_deref()) == target_env)
    else {
        return Ok(None);
    };
    let detail = db
        .get_code_scan_detail(newest.id)?
        .ok_or_else(|| format!("Code Scan history references missing scan {}", newest.id))?;
    let view = detail
        .issues
        .into_iter()
        .find(|view| view.check_id == check_id);
    Ok(view.map(|view| StoredIssueGuidance {
        why_it_matters: view.why_now,
        evidence: view.evidence.map(serde_json::Value::String),
        manual_fix: view.likely_fix,
    }))
}

/// Fills missing Web guidance fields from the matching Code finding.
pub(super) fn stored_issue_guidance(
    db: &Database,
    project_id: i64,
    env_url: &str,
    check_id: &str,
) -> Result<StoredIssueGuidance, String> {
    let web = stored_web_issue_guidance(db, project_id, env_url, check_id)?.unwrap_or_default();
    if web.is_complete() {
        return Ok(web);
    }
    let code = stored_code_issue_guidance(db, project_id, env_url, check_id)?.unwrap_or_default();
    Ok(StoredIssueGuidance {
        why_it_matters: web.why_it_matters.or(code.why_it_matters),
        evidence: web.evidence.or(code.evidence),
        manual_fix: web.manual_fix.or(code.manual_fix),
    })
}

#[cfg(test)]
mod tests {
    use crate::checks::Severity;
    use crate::commands::fix_attempts::{create_fix_attempt_inner, CreateFixAttemptArgs};
    use crate::db::test_helpers::temp_db;

    fn project_site(db: &crate::db::Database, project_id: i64) -> i64 {
        db.add_environment(
            project_id,
            "https://example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("environment");
        db.get_or_create_site_for_project(project_id, "https://example.com")
            .expect("project site")
    }
    use crate::db::work_items::WorkItemMetadata;

    fn dispatch_args(project_id: i64, check_id: &str) -> CreateFixAttemptArgs {
        CreateFixAttemptArgs {
            project_id,
            env_url: Some("https://example.com".to_string()),
            check_id: check_id.to_string(),
            agent_tool: crate::core::agent_tools::AgentTool::ClaudeCode,
            title: "Issue title".to_string(),
            severity: Severity::High,
            description: "Issue description".to_string(),
            why_it_matters: None,
            evidence: None,
            manual_fix: None,
            url: "https://example.com".to_string(),
            detected_stack: None,
            code_locations: None,
            previous_failure: None,
        }
    }

    /// The attempt id appears twice in a brief: the metadata line and the
    /// request_verification instruction. Mask both so briefs from two
    /// attempts can be compared structurally.
    fn mask_attempt_id(brief: &str, attempt_id: i64) -> String {
        brief
            .replace(&format!("Attempt: {attempt_id} |"), "Attempt: # |")
            .replace(&format!("attempt_id={attempt_id}"), "attempt_id=#")
    }

    /// A sparse web dispatch may omit evidence, manual_fix, and
    /// why_it_matters. The stored scan keeps all three, so hydration must
    /// produce the same brief as a complete dispatch.
    #[test]
    fn sparse_brief_matches_complete_brief_for_web_issues() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Fix Loop", "/tmp/fix-loop", Some("astro"))
            .expect("upsert");
        let site_id = project_site(&db, project_id);
        let evidence = serde_json::json!({ "header": "content-security-policy", "present": false });
        let why_text = "Without CSP, injected scripts run unrestricted.";
        let manual_fix_text = "Send a Content-Security-Policy header from your server config.";
        let scan = crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: "https://example.com".to_string(),
            mode: "full".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 70,
            categories: vec![],
            issues: vec![crate::checks::CheckResult {
                check_id: "security.csp".into(),
                category: crate::checks::ScanCategory::Security,
                title: "Missing Content-Security-Policy".into(),
                description: "The site does not send a Content-Security-Policy header.".into(),
                status: crate::checks::CheckStatus::Fail,
                severity: Severity::High,
                fix_prompt: Some("Add a Content-Security-Policy header.".into()),
                manual_fix: Some(manual_fix_text.into()),
                raw_data: Some(evidence.clone()),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: Some(why_text.into()),
            }],
            detected_stack: None,
            duration_ms: 1_000,
            timestamp: "2026-06-01T00:00:00Z".to_string(),
        };
        let scan_id = db.save_scan(site_id, &scan).expect("save scan");
        db.upsert_work_items_diff(
            "web_scan",
            project_id,
            "https://example.com",
            vec![crate::db::work_items::WorkItemInput {
                project_id,
                env_url: "https://example.com".to_string(),
                source: "web_scan".to_string(),
                signal_id: "web_scan:security.csp:https://example.com".to_string(),
                check_id: "security.csp".to_string(),
                category: "security".to_string(),
                severity: Severity::High,
                title: "Missing Content-Security-Policy".to_string(),
                description: "The site does not send a Content-Security-Policy header.".to_string(),
                detail_json: Some(evidence.to_string()),
                scan_ref: Some(scan_id),
                page_url: Some("https://example.com".to_string()),
                fix_prompt: Some("Add a Content-Security-Policy header.".to_string()),
                manual_fix: Some(manual_fix_text.to_string()),
                why_it_matters: Some(why_text.to_string()),
                observed_at: 1_000,
                metadata: WorkItemMetadata::default(),
            }],
            1_000,
        )
        .expect("work items");

        // Complete args: the live-scan dossier payload with all guidance intact.
        let mut complete_args = dispatch_args(project_id, "security.csp");
        complete_args.why_it_matters = Some(why_text.to_string());
        complete_args.evidence = Some(evidence.clone());
        complete_args.manual_fix = Some(manual_fix_text.to_string());
        let complete_dto =
            create_fix_attempt_inner(&db, complete_args, 2_000).expect("complete attempt");
        let brief_a = db
            .get_fix_attempt(complete_dto.id)
            .expect("get complete attempt")
            .expect("complete attempt row")
            .brief_md;
        // Sparse attempt B reuses the same check_id; cancel A so the one-active-
        // attempt unique index admits B.
        db.cancel_fix_attempt_if_active(complete_dto.id, 2_100)
            .expect("cancel complete attempt");

        // Sparse args: why_it_matters/evidence/manual_fix are all None.
        let sparse_dto =
            create_fix_attempt_inner(&db, dispatch_args(project_id, "security.csp"), 2_200)
                .expect("sparse attempt");
        let brief_b = db
            .get_fix_attempt(sparse_dto.id)
            .expect("get sparse attempt")
            .expect("sparse attempt row")
            .brief_md;

        assert_eq!(
            mask_attempt_id(&brief_a, complete_dto.id),
            mask_attempt_id(&brief_b, sparse_dto.id),
            "a sparse dispatch must produce the same brief as complete args"
        );
        // Guards against both briefs being equally hollow.
        assert!(
            brief_b.contains(why_text),
            "why_it_matters must be recovered from the stored scan, got:\n{brief_b}"
        );
        assert!(
            brief_b.contains(manual_fix_text),
            "manual_fix must be recovered from the stored scan, got:\n{brief_b}"
        );
        assert!(
            brief_b.contains("content-security-policy"),
            "evidence must be recovered from the stored scan, got:\n{brief_b}"
        );
    }

    /// Merge missing Web guidance from a canonical Code twin.
    #[test]
    fn merge_takes_web_evidence_and_code_fix_for_the_same_check_id() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Fix Loop", "/tmp/fix-loop", Some("astro"))
            .expect("upsert");
        let site_id = project_site(&db, project_id);
        let evidence = serde_json::json!({ "header": "content-security-policy", "present": false });

        // Web half: a stored health scan whose work_item carries evidence
        // only - manual_fix and why_it_matters stay NULL.
        let scan = crate::core::scanner::ScanResult {
            page_signals: None,
            site_facts: None,
            url: "https://example.com".to_string(),
            mode: "full".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 70,
            categories: vec![],
            issues: vec![crate::checks::CheckResult {
                // Producer alias intentionally differs from the canonical
                // fix-attempt key; stored guidance must resolve it.
                check_id: "security.headers.csp".into(),
                category: crate::checks::ScanCategory::Security,
                title: "Missing Content-Security-Policy".into(),
                description: "The site does not send a Content-Security-Policy header.".into(),
                status: crate::checks::CheckStatus::Fail,
                severity: Severity::High,
                fix_prompt: None,
                manual_fix: None,
                raw_data: Some(evidence.clone()),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }],
            detected_stack: None,
            duration_ms: 1_000,
            timestamp: "2026-06-01T00:00:00Z".to_string(),
        };
        let scan_id = db.save_scan(site_id, &scan).expect("save scan");
        db.upsert_work_items_diff(
            "web_scan",
            project_id,
            "https://example.com",
            vec![crate::db::work_items::WorkItemInput {
                project_id,
                env_url: "https://example.com".to_string(),
                source: "web_scan".to_string(),
                signal_id: "web_scan:security.csp:https://example.com".to_string(),
                check_id: "security.csp".to_string(),
                category: "security".to_string(),
                severity: Severity::High,
                title: "Missing Content-Security-Policy".to_string(),
                description: "The site does not send a Content-Security-Policy header.".to_string(),
                detail_json: Some(evidence.to_string()),
                scan_ref: Some(scan_id),
                page_url: Some("https://example.com".to_string()),
                fix_prompt: None,
                manual_fix: None,
                why_it_matters: None,
                observed_at: 1_000,
                metadata: WorkItemMetadata::default(),
            }],
            1_000,
        )
        .expect("work items");

        // Code half: security_headers resolves to the same canonical
        // security.csp check_id and carries the why/fix guidance.
        let check_id = crate::core::correlation::resolve_check_id("code_scan", "security_headers");
        assert_eq!(check_id, "security.csp", "mapped pair must stay canonical");
        let likely_fix_text = "Add a Content-Security-Policy header in next.config.js.";
        let why_now_text = "Responses ship without any CSP directives.";
        let issue = crate::core::code_scan::CodeIssue {
            id: "security_headers".to_string(),
            check_id: check_id.clone(),
            category: "security".to_string(),
            severity: crate::checks::Severity::High,
            title: "Missing security headers".to_string(),
            description: "No CSP configured for responses".to_string(),
            relative_path: "next.config.js".to_string(),
            absolute_path: "/tmp/fix-loop/next.config.js".to_string(),
            line: Some(3),
            source_excerpt: None,
            evidence: Some("headers() returns no Content-Security-Policy entry".to_string()),
            why_now: Some(why_now_text.to_string()),
            likely_fix: Some(likely_fix_text.to_string()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        };
        let report = crate::core::code_scan::CodeScanReport {
            checked_at: "2026-06-01T00:00:00Z".to_string(),
            framework: None,
            issue_count: 1,
            critical_count: 0,
            high_count: 1,
            medium_count: 0,
            low_count: 0,
            issues: vec![issue.clone()],
            skipped_scopes: Default::default(),
        };
        let code_scan_id = db
            .save_code_scan(
                project_id,
                Some("https://example.com".to_string()),
                "/tmp/fix-loop".to_string(),
                &report,
                1_000,
            )
            .expect("save code scan");
        // Code scan issues live in work_items (source = 'code_scan'); the
        // full CodeIssue rides along in detail_json.
        db.upsert_work_items_diff(
            "code_scan",
            project_id,
            "https://example.com",
            vec![crate::db::work_items::WorkItemInput {
                project_id,
                env_url: "https://example.com".to_string(),
                source: "code_scan".to_string(),
                signal_id: "code_scan:security_headers:next.config.js:3".to_string(),
                check_id: check_id.clone(),
                category: "code_quality".to_string(),
                severity: Severity::High,
                title: issue.title.clone(),
                description: issue.description.clone(),
                detail_json: serde_json::to_string(&issue).ok(),
                scan_ref: Some(code_scan_id),
                page_url: None,
                fix_prompt: None,
                manual_fix: None,
                why_it_matters: None,
                observed_at: 1_000,
                metadata: WorkItemMetadata::default(),
            }],
            1_000,
        )
        .expect("code work items");

        // Sparse args: why_it_matters/evidence/manual_fix are all None.
        let sparse_dto =
            create_fix_attempt_inner(&db, dispatch_args(project_id, "security.csp"), 2_000)
                .expect("sparse attempt");
        let brief = db
            .get_fix_attempt(sparse_dto.id)
            .expect("get attempt")
            .expect("attempt row")
            .brief_md;

        assert!(
            brief.contains("content-security-policy"),
            "merge must keep the web evidence, got:\n{brief}"
        );
        assert!(
            brief.contains(likely_fix_text),
            "merge must take the code twin's likely_fix, got:\n{brief}"
        );
        assert!(
            brief.contains(why_now_text),
            "merge must take the code twin's why_now, got:\n{brief}"
        );
    }

    /// Sparse code dispatches can omit evidence, why_now, and likely_fix. The
    /// stored code scan keeps the full issue, so hydration must restore all
    /// three and match the complete brief exactly.
    #[test]
    fn sparse_brief_matches_complete_brief_for_code_issues() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Fix Loop", "/tmp/fix-loop", Some("astro"))
            .expect("upsert");
        // Code issues address by the view's canonical check_id.
        let check_id = crate::core::correlation::resolve_check_id("code_scan", "sql-injection");
        let issue = crate::core::code_scan::CodeIssue {
            id: "sql-injection".to_string(),
            check_id: check_id.clone(),
            category: "security".to_string(),
            severity: crate::checks::Severity::High,
            title: "SQL injection risk".to_string(),
            description: "Unsafely concatenated query".to_string(),
            relative_path: "src/db.ts".to_string(),
            absolute_path: "/tmp/fix-loop/src/db.ts".to_string(),
            line: Some(12),
            source_excerpt: None,
            evidence: Some("Template string query built from user input".to_string()),
            why_now: Some("This query runs on a production route".to_string()),
            likely_fix: Some("Parameterize the query".to_string()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        };
        let report = crate::core::code_scan::CodeScanReport {
            checked_at: "2026-06-01T00:00:00Z".to_string(),
            framework: None,
            issue_count: 1,
            critical_count: 0,
            high_count: 1,
            medium_count: 0,
            low_count: 0,
            issues: vec![issue.clone()],
            skipped_scopes: Default::default(),
        };
        let scan_id = db
            .save_code_scan(
                project_id,
                Some("https://example.com".to_string()),
                "/tmp/fix-loop".to_string(),
                &report,
                1_000,
            )
            .expect("save code scan");
        db.upsert_work_items_diff(
            "code_scan",
            project_id,
            "https://example.com",
            vec![crate::db::work_items::WorkItemInput {
                project_id,
                env_url: "https://example.com".to_string(),
                source: "code_scan".to_string(),
                signal_id: "code_scan:sql-injection:src/db.ts:12".to_string(),
                check_id: check_id.clone(),
                category: "code_quality".to_string(),
                severity: Severity::High,
                title: issue.title.clone(),
                description: issue.description.clone(),
                detail_json: serde_json::to_string(&issue).ok(),
                scan_ref: Some(scan_id),
                page_url: None,
                fix_prompt: None,
                manual_fix: None,
                why_it_matters: None,
                observed_at: 1_000,
                metadata: WorkItemMetadata::default(),
            }],
            1_000,
        )
        .expect("work items");

        let code_args = |check_id: &str| {
            let mut args = dispatch_args(project_id, check_id);
            args.title = "SQL injection risk".to_string();
            args.description = "Unsafely concatenated query".to_string();
            args
        };

        // Complete args: the dossier passes why_now/evidence/likely_fix
        // through, with the evidence string as a JSON string value.
        let mut complete_args = code_args(&check_id);
        complete_args.why_it_matters = Some("This query runs on a production route".to_string());
        complete_args.evidence = Some(serde_json::Value::String(
            "Template string query built from user input".to_string(),
        ));
        complete_args.manual_fix = Some("Parameterize the query".to_string());
        let complete_dto =
            create_fix_attempt_inner(&db, complete_args, 2_000).expect("complete attempt");
        let brief_a = db
            .get_fix_attempt(complete_dto.id)
            .expect("get complete attempt")
            .expect("complete attempt row")
            .brief_md;
        db.cancel_fix_attempt_if_active(complete_dto.id, 2_100)
            .expect("cancel complete attempt");

        // Sparse args: why_it_matters/evidence/manual_fix are all None.
        let sparse_dto =
            create_fix_attempt_inner(&db, code_args(&check_id), 2_200).expect("sparse attempt");
        let brief_b = db
            .get_fix_attempt(sparse_dto.id)
            .expect("get sparse attempt")
            .expect("sparse attempt row")
            .brief_md;

        assert_eq!(
            mask_attempt_id(&brief_a, complete_dto.id),
            mask_attempt_id(&brief_b, sparse_dto.id),
            "a sparse dispatch must produce the same brief as complete args"
        );
        // Guards against both briefs being equally hollow.
        assert!(
            brief_b.contains("Parameterize the query"),
            "brief must carry the likely_fix text, got:\n{brief_b}"
        );
        assert!(
            brief_b.contains("This query runs on a production route"),
            "brief must carry the why_now text, got:\n{brief_b}"
        );
    }
}
