use super::*;

#[test]
fn unpinned_third_party_actions_flag_once_per_workflow_file() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".github/workflows/deploy.yml",
        r#"
name: Deploy
on: push
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: some-vendor/deploy-action@v2
      - uses: another-vendor/notify@main
      - uses: pinned-vendor/build@8f4b7f84864484a7bf31766abe9204da3cbe65b3
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let matches = report
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("unpinned-github-action:"))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one issue per workflow file, got {:?}",
        matches.iter().map(|issue| &issue.id).collect::<Vec<_>>()
    );

    let issue = matches[0];
    assert_eq!(
        issue.id,
        "unpinned-github-action:.github/workflows/deploy.yml"
    );
    assert_eq!(issue.severity, Severity::Medium);
    assert_eq!(issue.confidence, crate::checks::IssueConfidence::High);
    let evidence = issue.evidence.as_deref().unwrap_or_default();
    assert!(
        evidence.contains("some-vendor/deploy-action@v2"),
        "evidence should list the tag-pinned action: {evidence}"
    );
    assert!(
        evidence.contains("another-vendor/notify@main"),
        "evidence should list the branch-pinned action: {evidence}"
    );
    assert!(
        !evidence.contains("pinned-vendor/build"),
        "SHA-pinned actions must not appear as offenders: {evidence}"
    );
    assert!(
        evidence.contains("first-party"),
        "evidence should mention the unpinned first-party reference: {evidence}"
    );
}

#[test]
fn unpinned_first_party_only_workflow_does_not_flag() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".github/workflows/ci.yml",
        r#"
name: CI
on: push
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
      - uses: github/codeql-action/analyze@v3
      - run: npm test
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("unpinned-github-action:")),
        "first-party unpinned refs alone must not trigger the issue"
    );
}

#[test]
fn unpinned_check_skips_sha_pins_local_paths_and_docker_digests() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".github/workflows/release.yaml",
        r#"
name: Release
on: push
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: some-vendor/release@8f4b7f84864484a7bf31766abe9204da3cbe65b3 # v3
      - uses: ./.github/actions/setup
      - uses: docker://alpine@sha256:c5b1261d6d3e43071626931fc004f70149baeba2c8ec672bd4f27761f8e1ad6b
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("unpinned-github-action:")),
        "SHA pins, local actions, and digest-addressed docker images must all pass"
    );
}

#[test]
fn unpinned_reusable_workflow_reference_counts_as_third_party() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".github/workflows/shared.yml",
        r#"
name: Shared
on: push
jobs:
  call:
    uses: some-org/shared-workflows/.github/workflows/build.yml@main
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.id == "unpinned-github-action:.github/workflows/shared.yml"),
        "a reusable workflow pinned to a branch must flag"
    );
}

#[test]
fn pull_request_target_checkout_of_pr_head_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".github/workflows/pr.yml",
        r#"
name: PR build
on:
  pull_request_target:
    types: [opened, synchronize]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event.pull_request.head.sha }}
      - run: npm ci && npm run build
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("workflow-pr-target-checkout:"))
        .expect("expected workflow-pr-target-checkout issue");
    assert_eq!(issue.severity, Severity::High);
    assert_eq!(issue.confidence, crate::checks::IssueConfidence::High);
    // The ref is in the checkout step itself, so the definitive claim holds.
    assert_eq!(
        issue.title,
        "Privileged workflow checks out untrusted pull-request code"
    );
    assert!(issue.description.contains("If a later step executes"));
    assert!(!issue.description.contains("then runs with those secrets"));
}

#[test]
fn pr_expression_outside_checkout_ref_is_not_a_checkout_finding() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        temp.path(),
        ".github/workflows/pr-label.yml",
        r#"
name: PR label
on:
  pull_request_target:
    types: [opened]
jobs:
  label:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Report branch
        env:
          BRANCH: ${{ github.head_ref }}
        run: echo "$BRANCH"
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(!report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with("workflow-pr-target-checkout:")));
}

#[test]
fn pull_request_target_without_pr_head_checkout_is_not_flagged() {
    let safe = TempDir::new().unwrap();
    write_file(safe.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        safe.path(),
        ".github/workflows/label.yml",
        r#"
name: Label
on:
  pull_request_target:
    types: [opened]
jobs:
  label:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/labeler@v5
"#,
    );
    let report = audit_project(safe.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("workflow-pr-target-checkout:")),
        "base-branch checkout under pull_request_target must stay quiet: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );

    let plain = TempDir::new().unwrap();
    write_file(plain.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(
        plain.path(),
        ".github/workflows/ci.yml",
        r#"
name: CI
on: pull_request
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event.pull_request.head.sha }}
      - run: npm test
"#,
    );
    let report = audit_project(plain.path()).unwrap();
    assert!(
        !report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("workflow-pr-target-checkout:")),
        "the unprivileged pull_request trigger must stay quiet: {:?}",
        report.issues.iter().map(|i| &i.id).collect::<Vec<_>>()
    );
}
