mod baseline;
pub(crate) mod code_scan;
pub(crate) mod control;
mod domain_summary;
pub(crate) mod execution;
mod execution_events;
pub(crate) mod history;
mod issue_link_resolve;
pub(crate) mod multi_scan;
mod page_loop;
mod policy;
pub(crate) mod schedule;
pub(crate) mod tools;
pub(crate) mod verification;
pub(crate) mod web_scan;
#[cfg(test)]
pub(crate) mod work_items;

pub use control::{cancel_scan, ScanControlState};
pub(crate) use domain_summary::{
    describe_code_scan_domain_trend, top_code_scan_domain_from_summaries,
};
pub use execution::run_scan_execution;
pub use history::{
    get_resolved_issues, get_scan_execution_detail, get_scan_executions, get_score_trend,
};
pub(crate) use policy::configured_scan_retention;
pub(crate) use policy::webview_analysis_profile;
pub use schedule::{
    get_due_schedules, get_pagespeed_report, get_scan_schedule, mark_schedule_run,
    pagespeed_api_key_is_set, save_scan_schedule, set_pagespeed_api_key,
};
pub use tools::{
    build_prompt, export_scan_markdown, get_fix_document, run_webview_analysis, verify_scan_checks,
};

/// Send a best-effort deploy-regression notification after web or code scans.
pub(super) async fn notify_deploy_regression(
    app: &tauri::AppHandle,
    notice: &crate::core::regression_blame::RegressionNotice,
) {
    let request = crate::commands::ActionableDesktopNotificationRequest {
        id: Some(format!("deploy-regression-{}", notice.regression_id)),
        title: notice.title.clone(),
        body: notice.body.clone(),
        click_target: None,
        actions: Vec::new(),
    };
    if let Err(error) =
        crate::commands::send_actionable_desktop_notification(app.clone(), request).await
    {
        tracing::warn!("failed to send deploy-regression notification: {}", error);
    }
}

#[cfg(test)]
use domain_summary::{build_domain_summaries, select_relevant_previous_code_scan_summary};
#[cfg(test)]
use policy::{
    sanitize_history_limit, should_run_accessibility_webview_analysis, should_run_webview_analysis,
    validate_issue_link_provider, DEFAULT_HISTORY_QUERY_LIMIT, MAX_HISTORY_QUERY_LIMIT,
};
#[cfg(test)]
use work_items::{check_result_to_work_item_input, code_issue_to_work_item_input};

#[cfg(test)]
mod tests {
    use super::{
        build_domain_summaries, check_result_to_work_item_input, describe_code_scan_domain_trend,
        sanitize_history_limit, select_relevant_previous_code_scan_summary,
        should_run_accessibility_webview_analysis, should_run_webview_analysis,
        top_code_scan_domain_from_summaries, validate_issue_link_provider,
        DEFAULT_HISTORY_QUERY_LIMIT, MAX_HISTORY_QUERY_LIMIT,
    };
    use crate::checks::{CheckStatus, ScanCategory, Severity};
    use crate::core::code_scan::{CodeIssue, CodeScanDomain};
    use crate::core::scanner::ScanType;
    use crate::db::{CodeScanDomainSummary, CodeScanSummary};

    fn issue(id: &str, category: &str, severity: Severity) -> CodeIssue {
        CodeIssue {
            check_id: String::new(),
            id: id.into(),
            category: category.into(),
            severity,
            title: id.into(),
            description: "".into(),
            relative_path: format!("src/{}.rs", id),
            absolute_path: format!("/tmp/src/{}.rs", id),
            line: None,
            source_excerpt: None,
            evidence: None,
            why_now: None,
            likely_fix: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        }
    }

    #[test]
    fn build_domain_summaries_groups_issues_by_domain() {
        // Categories here match `code_issue_domain`'s direct-mapping rules:
        // "ai-safety" → AiSafety, "security" → Security, "supply-chain" →
        // SupplyChain. Database/Architecture depend on richer heuristics so
        // we stick to the deterministic mappings for this unit test.
        let issues = vec![
            issue("ai-timeout", "ai-safety", Severity::High),
            issue("ai-retries", "ai-safety", Severity::Medium),
            issue("exposed-env", "security", Severity::Critical),
            issue("open-cors", "security", Severity::High),
            issue("outdated-dep-1", "supply-chain", Severity::Low),
        ];

        let summaries = build_domain_summaries(&issues);
        assert_eq!(summaries.len(), 3);

        let ai = summaries
            .iter()
            .find(|s| s.domain == CodeScanDomain::AiSafety)
            .expect("ai-safety domain present");
        assert_eq!(ai.issue_count, 2);
        assert_eq!(ai.high_count, 1);
        assert_eq!(ai.medium_count, 1);

        let sec = summaries
            .iter()
            .find(|s| s.domain == CodeScanDomain::Security)
            .expect("security domain present");
        assert_eq!(sec.issue_count, 2);
        assert_eq!(sec.critical_count, 1);
        assert_eq!(sec.high_count, 1);

        let sc = summaries
            .iter()
            .find(|s| s.domain == CodeScanDomain::SupplyChain)
            .expect("supply-chain domain present");
        assert_eq!(sc.issue_count, 1);
        assert_eq!(sc.low_count, 1);
    }

    #[test]
    fn health_scans_still_run_webview_metrics_when_accessibility_toggle_is_off() {
        assert!(should_run_webview_analysis(ScanType::Health, false));
        assert!(!should_run_accessibility_webview_analysis(
            ScanType::Health,
            Some(false),
            false
        ));
    }

    #[test]
    fn accessibility_scans_always_run_accessibility_webview_analysis_off_localhost() {
        assert!(should_run_webview_analysis(ScanType::Accessibility, false));
        assert!(should_run_accessibility_webview_analysis(
            ScanType::Accessibility,
            Some(false),
            false
        ));
    }

    #[test]
    fn remote_production_targets_keep_webview_axe_and_cwv_coverage() {
        for scan_type in [ScanType::Health, ScanType::Accessibility] {
            assert!(
                should_run_webview_analysis(scan_type, false),
                "webview analysis must run for remote {scan_type} scans"
            );
        }
        assert!(should_run_accessibility_webview_analysis(
            ScanType::Accessibility,
            None,
            false
        ));
    }

    #[test]
    fn security_scans_skip_hidden_webview_analysis() {
        assert!(!should_run_webview_analysis(ScanType::Security, false));
        assert!(!should_run_accessibility_webview_analysis(
            ScanType::Security,
            Some(true),
            false
        ));
    }

    #[test]
    fn sanitize_history_limit_bounds_query_size_only() {
        assert_eq!(
            sanitize_history_limit(Some(MAX_HISTORY_QUERY_LIMIT + 250)),
            MAX_HISTORY_QUERY_LIMIT
        );
        assert_eq!(sanitize_history_limit(None), DEFAULT_HISTORY_QUERY_LIMIT);
    }

    #[test]
    fn issue_link_resolution_accepts_known_providers() {
        // Auto-resolve depends only on the provider being recognised; no
        // tier exists anywhere on the path.
        assert!(validate_issue_link_provider("github").is_ok());
        assert!(validate_issue_link_provider("jira").is_ok());
        assert!(
            validate_issue_link_provider("linear").is_err(),
            "unknown providers should still be rejected"
        );
    }

    #[test]
    fn build_domain_summaries_empty_when_no_issues() {
        assert_eq!(build_domain_summaries(&[]).len(), 0);
    }

    #[test]
    fn top_code_scan_domain_from_summaries_matches_issue_ranking() {
        let issues = vec![
            issue("ai-timeout", "ai-safety", Severity::High),
            issue("ai-retries", "ai-safety", Severity::Medium),
            issue("exposed-env", "security", Severity::Critical),
        ];
        let summaries = build_domain_summaries(&issues);
        assert_eq!(
            top_code_scan_domain_from_summaries(&summaries),
            crate::core::code_scan::summarize_code_scan_domain(&issues),
        );
        assert_eq!(
            top_code_scan_domain_from_summaries(&summaries),
            Some((CodeScanDomain::AiSafety, 2))
        );
    }

    #[test]
    fn top_code_scan_domain_from_summaries_breaks_ties_by_canonical_order() {
        let issues = vec![
            issue("exposed-env", "security", Severity::Critical),
            issue("outdated-dep-1", "supply-chain", Severity::Low),
        ];
        let summaries = build_domain_summaries(&issues);
        assert_eq!(
            top_code_scan_domain_from_summaries(&summaries).map(|(domain, _)| domain),
            Some(CodeScanDomain::Security)
        );
        assert!(top_code_scan_domain_from_summaries(&[]).is_none());
    }

    #[test]
    fn describe_code_scan_domain_trend_reports_strongest_lane_change() {
        let label = describe_code_scan_domain_trend(
            &[CodeScanDomainSummary {
                domain: CodeScanDomain::Database,
                issue_count: 5,
                critical_count: 1,
                high_count: 2,
                medium_count: 1,
                low_count: 1,
            }],
            &[CodeScanDomainSummary {
                domain: CodeScanDomain::Database,
                issue_count: 2,
                critical_count: 0,
                high_count: 1,
                medium_count: 1,
                low_count: 0,
            }],
            Some(CodeScanDomain::Database),
            Some(CodeScanDomain::Database),
        );

        assert_eq!(label.as_deref(), Some("Database Analysis grew by 3"));
    }

    #[test]
    fn select_relevant_previous_code_scan_summary_prefers_exact_environment() {
        let history = vec![
            CodeScanSummary {
                id: 3,
                project_id: 1,
                environment_url: Some("https://other.example".into()),
                overall_score: 74,
                issue_count: 4,
                grouped_issue_count: 4,
                critical_count: 0,
                high_count: 2,
                duration_ms: 1200,
                checked_at: "2026-04-10T12:00:00Z".into(),
                framework: None,
                top_domain: Some(CodeScanDomain::Security),
                top_domain_count: 2,
                domain_summaries: vec![],
            },
            CodeScanSummary {
                id: 2,
                project_id: 1,
                environment_url: None,
                overall_score: 78,
                issue_count: 3,
                grouped_issue_count: 3,
                critical_count: 0,
                high_count: 1,
                duration_ms: 1200,
                checked_at: "2026-04-09T12:00:00Z".into(),
                framework: None,
                top_domain: Some(CodeScanDomain::Architecture),
                top_domain_count: 2,
                domain_summaries: vec![],
            },
            CodeScanSummary {
                id: 1,
                project_id: 1,
                environment_url: Some("https://app.example".into()),
                overall_score: 81,
                issue_count: 2,
                grouped_issue_count: 2,
                critical_count: 0,
                high_count: 1,
                duration_ms: 1200,
                checked_at: "2026-04-08T12:00:00Z".into(),
                framework: None,
                top_domain: Some(CodeScanDomain::Database),
                top_domain_count: 1,
                domain_summaries: vec![],
            },
        ];

        let picked =
            select_relevant_previous_code_scan_summary(history, Some("https://app.example/"));

        assert_eq!(picked.map(|summary| summary.id), Some(1));
    }

    #[test]
    fn check_result_to_work_item_input_maps_fields_correctly() {
        let cr = crate::checks::CheckResult {
            check_id: "csp-missing".to_string(),
            category: ScanCategory::Security,
            title: "Missing CSP header".to_string(),
            description: "Content-Security-Policy is not set.".to_string(),
            status: CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        };

        let input =
            check_result_to_work_item_input(&cr, 42, "https://example.com", 99, 1_000_000, None);

        assert_eq!(input.source, "web_scan");
        assert_eq!(input.signal_id, "web_scan:csp-missing:https://example.com");
        assert_eq!(input.check_id, "csp-missing");
        assert_eq!(input.category, "security");
        assert_eq!(input.severity, Severity::High);
        assert_eq!(input.title, "Missing CSP header");
        assert_eq!(input.project_id, 42);
        assert_eq!(input.env_url, "https://example.com");
        assert_eq!(input.scan_ref, Some(99));
        assert_eq!(input.observed_at, 1_000_000);
        assert_eq!(input.detail_json, None);
    }

    #[test]
    fn check_result_to_work_item_input_serialises_raw_data() {
        let cr = crate::checks::CheckResult {
            check_id: "tls-version".to_string(),
            category: ScanCategory::Security,
            title: "Weak TLS".to_string(),
            description: "TLS 1.0 detected.".to_string(),
            status: CheckStatus::Warn,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: None,
            raw_data: Some(serde_json::json!({"version": "TLS1.0"})),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        };

        let input = check_result_to_work_item_input(&cr, 1, "https://example.com", 7, 2_000, None);

        assert!(input.detail_json.is_some());
        let parsed: serde_json::Value =
            serde_json::from_str(input.detail_json.as_ref().unwrap()).unwrap();
        assert_eq!(parsed["version"], "TLS1.0");
    }

    #[test]
    fn check_result_to_work_item_input_populates_fix_prompt_on_fail() {
        let cr = crate::checks::CheckResult {
            check_id: "security.csp".into(),
            category: ScanCategory::Security,
            title: "Missing CSP".into(),
            description: "desc".into(),
            status: CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        };
        let input = check_result_to_work_item_input(&cr, 1, "https://example.com", 7, 2_000, None);
        assert!(
            input.fix_prompt.is_some(),
            "failing check should have a fix_prompt"
        );
        let prompt = input.fix_prompt.as_deref().unwrap();
        assert!(
            prompt.contains("Missing CSP"),
            "fix_prompt should reference the issue title"
        );
    }

    #[test]
    fn code_issue_to_work_item_input_maps_fields() {
        use super::code_issue_to_work_item_input;

        let ci = CodeIssue {
            check_id: String::new(),
            id: "AUTH-001".into(),
            category: "security".into(),
            severity: Severity::High,
            title: "Missing auth middleware".into(),
            description: "No authentication guard found.".into(),
            relative_path: "src/auth.ts".into(),
            absolute_path: "/project/src/auth.ts".into(),
            line: Some(42),
            source_excerpt: None,
            evidence: None,
            why_now: None,
            likely_fix: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        };

        let input = code_issue_to_work_item_input(&ci, 1, "https://example.com", 99, 1_000, None);

        assert_eq!(input.source, "code_scan");
        assert_eq!(input.signal_id, "code_scan:AUTH-001:src/auth.ts:42");
        assert_eq!(input.check_id, "code_scan.AUTH-001");
        assert_eq!(input.category, "code_quality");
        assert_eq!(input.severity, Severity::High);
        assert_eq!(input.scan_ref, Some(99));
        assert_eq!(input.project_id, 1);
        assert_eq!(input.env_url, "https://example.com");
        assert!(input.detail_json.is_some());
    }

    #[test]
    fn code_issue_to_work_item_input_no_line() {
        use super::code_issue_to_work_item_input;

        let ci = CodeIssue {
            check_id: String::new(),
            id: "SC-002".into(),
            category: "supply-chain".into(),
            severity: Severity::Medium,
            title: "Outdated dependency".into(),
            description: "lodash is out of date.".into(),
            relative_path: "package.json".into(),
            absolute_path: "/project/package.json".into(),
            line: None,
            source_excerpt: None,
            evidence: None,
            why_now: None,
            likely_fix: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        };

        let input = code_issue_to_work_item_input(&ci, 2, "", 77, 2_000, None);

        // signal_id should end with an empty line segment, not panic
        assert_eq!(input.signal_id, "code_scan:SC-002:package.json:");
        assert_eq!(input.check_id, "code_scan.SC-002");
        assert_eq!(input.severity, Severity::Medium);
        assert_eq!(input.scan_ref, Some(77));
    }

    #[test]
    fn code_issue_with_mapped_signal_uses_canonical_check_id() {
        use super::code_issue_to_work_item_input;
        use crate::checks::Severity;
        use crate::core::code_scan::CodeIssue;

        let issue = CodeIssue {
            check_id: String::new(),
            id: "security_headers".to_string(),
            title: "Missing CSP".to_string(),
            description: "No Content-Security-Policy header".to_string(),
            severity: Severity::High,
            relative_path: "next.config.ts".to_string(),
            absolute_path: "".to_string(),
            line: Some(10),
            category: "security".to_string(),
            source_excerpt: None,
            evidence: None,
            why_now: None,
            likely_fix: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        };

        let input =
            code_issue_to_work_item_input(&issue, 1, "https://example.com", 99, 1_000, None);
        assert_eq!(input.check_id, "security.csp");
    }

    #[test]
    fn code_issue_with_unmapped_signal_falls_through() {
        use super::code_issue_to_work_item_input;
        use crate::checks::Severity;
        use crate::core::code_scan::CodeIssue;

        let issue = CodeIssue {
            check_id: String::new(),
            id: "some_new_issue_type".to_string(),
            title: "Unmapped".into(),
            description: "Unmapped issue".into(),
            severity: Severity::Low,
            relative_path: "x.ts".into(),
            absolute_path: "".into(),
            line: None,
            category: "operations".to_string(),
            source_excerpt: None,
            evidence: None,
            why_now: None,
            likely_fix: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: None,
        };

        let input = code_issue_to_work_item_input(&issue, 1, "https://example.com", 1, 0, None);
        assert_eq!(input.check_id, "code_scan.some_new_issue_type");
    }

    #[test]
    fn code_issue_work_item_carries_populated_fix_prompt() {
        // Fix prompts must retain issue-specific context.
        use super::code_issue_to_work_item_input;
        use crate::checks::Severity;
        use crate::core::code_scan::CodeIssue;

        let issue = CodeIssue {
            check_id: String::new(),
            id: "raw-sql-unsafe:src/api/users.ts".into(),
            category: "security".into(),
            severity: Severity::Critical,
            title: "Raw SQL with user-controlled interpolation".into(),
            description: "Template-literal SQL with ${id}.".into(),
            relative_path: "src/api/users.ts".into(),
            absolute_path: "/proj/src/api/users.ts".into(),
            line: Some(42),
            source_excerpt: Some(
                "  41 | const id = req.query.id;\n  42 | db.query(`SELECT * FROM users WHERE id=${id}`);"
                    .into(),
            ),
            evidence: Some("Matched template-literal SQL pattern".into()),
            why_now: None,
            likely_fix: Some("Parameterize the query.".into()),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("Heuristic pattern; verify before acting.".into()),
            verify_hint: Some("Send a malicious id and confirm escape.".into()),
        };

        let input = code_issue_to_work_item_input(&issue, 1, "", 99, 1_000, None);

        let prompt = input
            .fix_prompt
            .as_ref()
            .expect("code-scan work_item must carry a populated fix_prompt");
        assert!(
            prompt.contains("src/api/users.ts:42"),
            "fix_prompt must include file:line"
        );
        assert!(
            prompt.contains("Raw SQL with user-controlled interpolation"),
            "fix_prompt must include the issue title"
        );
        assert!(
            prompt.contains("CRITICAL"),
            "fix_prompt must convey severity"
        );
    }

    #[test]
    fn check_result_to_work_item_input_canonicalizes_check_id() {
        let cr = crate::checks::CheckResult {
            check_id: "security.headers.csp".into(),
            category: ScanCategory::Security,
            title: "Missing CSP".into(),
            description: "desc".into(),
            status: CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        };
        let input = check_result_to_work_item_input(&cr, 1, "https://example.com", 7, 2_000, None);
        assert_eq!(
            input.check_id, "security.csp",
            "check_id should canonicalize"
        );
        assert_eq!(
            input.signal_id, "web_scan:security.headers.csp:https://example.com",
            "signal_id should preserve the raw source signal while check_id canonicalizes",
        );
    }

    #[test]
    fn equivalent_web_checks_share_a_canonical_group_without_overwriting_each_other() {
        let make = |check_id: &str| crate::checks::CheckResult {
            check_id: check_id.into(),
            category: ScanCategory::Accessibility,
            title: check_id.into(),
            description: "desc".into(),
            status: CheckStatus::Warn,
            severity: Severity::Low,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("review".into()),
            why_it_matters: None,
        };

        let primary = check_result_to_work_item_input(
            &make("accessibility.lang"),
            1,
            "https://example.com",
            7,
            2_000,
            None,
        );
        let polish = check_result_to_work_item_input(
            &make("polish.missing-lang"),
            1,
            "https://example.com",
            7,
            2_000,
            None,
        );

        assert_eq!(primary.check_id, "accessibility.lang");
        assert_eq!(polish.check_id, "accessibility.lang");
        assert_ne!(primary.signal_id, polish.signal_id);
    }

    #[test]
    fn check_result_to_work_item_input_passes_through_already_canonical_id() {
        let cr = crate::checks::CheckResult {
            check_id: "accessibility.image_alt".into(),
            category: ScanCategory::Accessibility,
            title: "Missing alt".into(),
            description: "desc".into(),
            status: CheckStatus::Fail,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        };
        let input = check_result_to_work_item_input(&cr, 1, "https://example.com", 7, 2_000, None);
        assert_eq!(
            input.check_id, "accessibility.image_alt",
            "already-canonical id should pass through unchanged"
        );
    }
}
