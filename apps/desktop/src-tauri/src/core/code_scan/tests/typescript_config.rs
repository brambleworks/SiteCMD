use super::*;

#[test]
fn tsconfig_with_strict_disabled_flags_once() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        "tsconfig.json",
        r#"{
  "compilerOptions": {
    "target": "ES2020",
    "strict": false
  }
}"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let matches = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("tsconfig-strict-off:"))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected one strict-off issue, got {:?}",
        matches.iter().map(|issue| &issue.id).collect::<Vec<_>>()
    );

    let issue = matches[0];
    assert_eq!(issue.id, "tsconfig-strict-off:tsconfig.json");
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert_eq!(issue.line, Some(4));
    assert!(issue.title.contains("strict mode"));
    assert!(issue.description.contains("strict-family checks"));
    assert!(
        issue
            .evidence
            .as_deref()
            .unwrap_or_default()
            .contains("strict"),
        "evidence should name the disabled setting: {:?}",
        issue.evidence
    );
}

#[test]
fn no_implicit_any_override_describes_only_that_option() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        "tsconfig.json",
        r#"{
  "compilerOptions": {
    "strict": true,
    "noImplicitAny": false,
  }
}"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("tsconfig-strict-off:"))
        .expect("explicit noImplicitAny override review");
    assert_eq!(issue.severity, Severity::Low);
    assert!(issue.title.contains("noImplicitAny"));
    assert!(issue.description.contains("implicit `any`"));
    assert!(!issue.description.contains("null-safety"));
    assert!(!issue.description.contains("function-variance"));
}

#[test]
fn playground_and_fixture_tsconfigs_do_not_flag() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    // Demo and fixture tsconfigs are not production configuration.
    write_file(
        temp.path(),
        "playground/tsconfig.json",
        r#"{ "compilerOptions": { "strict": false } }"#,
    );
    write_file(
        temp.path(),
        "test/fixtures/loose/tsconfig.json",
        r#"{ "compilerOptions": { "strict": false } }"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("tsconfig-strict-off:")),
        "playground and fixture tsconfigs must not trigger the strict-off issue"
    );
}

#[test]
fn strict_tsconfig_does_not_flag() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "clean-app" }"#);
    write_file(
        temp.path(),
        "tsconfig.json",
        r#"{
  "compilerOptions": {
    "target": "ES2020",
    "strict": true
  }
}"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("tsconfig-strict-off:")),
        "strict:true must not trigger the issue"
    );
}
