use super::*;

#[test]
fn unbounded_runtime_dependencies_flag_once_per_manifest() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "demo-app",
  "dependencies": {
    "stripe": "*",
    "next": "latest",
    "react": "^18.2.0",
    "zod": "3.22.4"
  },
  "devDependencies": {
    "typescript": "*"
  },
  "peerDependencies": {
    "react": "*"
  }
}"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let matches = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("unbounded-dependency-range:"))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one manifest-level issue, got {:?}",
        matches.iter().map(|issue| &issue.id).collect::<Vec<_>>()
    );

    let issue = matches[0];
    assert_eq!(issue.id, "unbounded-dependency-range:package.json");
    assert_eq!(issue.severity, Severity::Low);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    // Only the unbounded RUNTIME dependencies are offenders.
    assert!(evidence.contains("stripe"), "stripe (*): {evidence}");
    assert!(evidence.contains("next"), "next (latest): {evidence}");
    // Bounded runtime deps, the dev dependency, and the peer dependency are not.
    assert!(
        !evidence.contains("react"),
        "react is bounded / peer: {evidence}"
    );
    assert!(!evidence.contains("zod"), "zod is exact-pinned: {evidence}");
    assert!(
        !evidence.contains("typescript"),
        "devDependencies are out of scope: {evidence}"
    );
}

#[test]
fn fixture_and_example_manifests_do_not_flag() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    // Fixture and example manifests are not shipped dependencies.
    write_file(
        temp.path(),
        "test/fixtures/react-19/package.json",
        r#"{ "name": "fx", "dependencies": { "react": "latest", "astro": "*" } }"#,
    );
    write_file(
        temp.path(),
        "examples/basic/package.json",
        r#"{ "name": "ex", "dependencies": { "vite": "*" } }"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("unbounded-dependency-range:")),
        "fixture and example manifests must not trigger the range issue"
    );
}

#[test]
fn bounded_dependency_ranges_do_not_flag() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "clean-app",
  "dependencies": {
    "react": "^18.2.0",
    "next": "~14.1.0",
    "zod": "3.22.4",
    "lodash": "4.x"
  }
}"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("unbounded-dependency-range:")),
        "caret, tilde, exact, and major-pinned (1.x) ranges must all pass"
    );
}
