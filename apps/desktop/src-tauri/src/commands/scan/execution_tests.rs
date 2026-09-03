use super::*;

#[test]
fn fingerprint_is_stable_after_category_normalization() {
    let plan = ValidatedExecutionPlan {
        project_id: Some(1),
        environment_id: Some(2),
        environment_url: Some("https://example.com".into()),
        environment_scope_key: "https://example.com".into(),
        requested_mode: ScanExecutionMode::Full,
        web_focus: Some(ScanType::Health),
        urls: vec!["https://example.com".into()],
        enabled_categories: vec!["seo".into(), "security".into()],
        timeout_secs: Some(30),
        axe_enabled: true,
        inspect_local_databases: false,
        project_path: Some("/tmp/project".into()),
        retention: 10,
        trigger: ScanTrigger::Manual,
        idempotency_key: "action".into(),
        web_status: Some(ScanComponentStatus::Planned),
        web_detail: None,
        code_status: Some(ScanComponentStatus::Planned),
        code_detail: None,
    };
    let first = plan.fingerprint().unwrap();
    let second = plan.fingerprint().unwrap();
    assert_eq!(first, second);
    assert!(first.starts_with("v1:"));
    assert_eq!(first.len(), 67);

    let mut opted_in = plan;
    opted_in.inspect_local_databases = true;
    assert_ne!(first, opted_in.fingerprint().unwrap());
}

#[test]
fn component_failure_classification_is_typed_at_the_execution_boundary() {
    assert_eq!(
        component_failure_status("Code scan cancelled."),
        ScanComponentStatus::Cancelled
    );
    assert_eq!(
        component_failure_status("DNS failed"),
        ScanComponentStatus::Failed
    );
}

#[test]
fn incomplete_page_scope_produces_a_partial_execution_detail() {
    assert_eq!(
        incomplete_page_scope_detail(2, 3).as_deref(),
        Some("2 of 3 selected pages completed.")
    );
    assert_eq!(incomplete_page_scope_detail(3, 3), None);
}

#[test]
fn issue_changes_reconcile_the_open_total_across_an_execution() {
    let before = std::collections::HashSet::from([
        "unchanged".to_string(),
        "resolved-a".to_string(),
        "resolved-b".to_string(),
        "resolved-c".to_string(),
        "resolved-d".to_string(),
    ]);
    let mut after = std::collections::HashSet::from(["unchanged".to_string()]);
    after.extend((0..10).map(|index| format!("new-{index}")));

    let changes = build_scan_issue_changes(&before, &after);

    assert_eq!(changes.previous_open_issues, 5);
    assert_eq!(changes.open_issues, 11);
    assert_eq!(changes.new_issues, 10);
    assert_eq!(changes.resolved_issues, 4);
    assert_eq!(
        changes.previous_open_issues + changes.new_issues - changes.resolved_issues,
        changes.open_issues
    );
}

// `validate_plan` runs on the async runtime, so every database read inside it
// must go through the async interface; the blocking sibling parks a runtime
// worker on the SQLite thread for the whole read.
#[test]
fn plan_validation_resolves_the_project_folder_off_the_async_worker() {
    const SOURCE: &str = include_str!("execution.rs");
    // The part of the module that ships, with its test declaration stripped.
    let production = SOURCE
        .split_once("\n#[cfg(test)]")
        .map_or(SOURCE, |(production, _)| production);
    assert!(
        production.contains("resolve_registered_project_dir_async("),
        "validate_plan must resolve the registered project folder through the async interface"
    );
    assert!(
        !production.contains("crate::project_paths::resolve_registered_project_dir("),
        "the blocking project-folder resolver must not run on the async worker"
    );
}
