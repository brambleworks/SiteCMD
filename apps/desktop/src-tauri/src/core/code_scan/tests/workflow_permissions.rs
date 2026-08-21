use super::*;

#[test]
fn write_all_permissions_flag_once_per_workflow_file() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".github/workflows/deploy.yml",
        r#"
name: Deploy
on: push
permissions: write-all
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@8f4b7f84864484a7bf31766abe9204da3cbe65b3
      - run: npm run deploy
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let matches = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("workflow-write-all-permissions:"))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one write-all issue per workflow file, got {:?}",
        matches.iter().map(|issue| &issue.id).collect::<Vec<_>>()
    );

    let issue = matches[0];
    assert_eq!(
        issue.id,
        "workflow-write-all-permissions:.github/workflows/deploy.yml"
    );
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(issue.confidence, crate::checks::IssueConfidence::High);
    assert_eq!(issue.line, Some(4), "should point at the permissions line");
    assert!(issue.description.contains("declared workflow or job scope"));
    assert!(!issue.description.contains("for every job"));
}

#[test]
fn least_privilege_and_specific_write_scopes_do_not_flag() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    // A deploy workflow that legitimately needs `contents: write` but scopes it,
    // plus a read-all workflow. Neither is the blanket over-grant.
    write_file(
        temp.path(),
        ".github/workflows/release.yml",
        r#"
name: Release
on: push
permissions:
  contents: write
  id-token: write
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - run: npm publish
"#,
    );
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        r#"
name: CI
on: push
permissions: read-all
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - run: npm test
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("workflow-write-all-permissions:")),
        "scoped write permissions and read-all must not trigger the blanket over-grant issue"
    );
}
