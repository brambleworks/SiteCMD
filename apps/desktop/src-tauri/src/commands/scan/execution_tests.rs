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
