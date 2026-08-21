use super::*;

const MANIFEST_WITH_DEPENDENCIES: &str = r#"
{
  "name": "demo-app",
  "dependencies": {
    "react": "^19.0.0"
  }
}
"#;

fn release_age_issue(report: &CodeScanReport) -> Option<&CodeIssue> {
    report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("release-age-policy-missing:"))
}

#[test]
fn release_age_policy_missing_flags_js_project_with_lockfile() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);
    write_file(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

    let report = audit_project(temp.path()).unwrap();
    let issue = release_age_issue(&report).expect("release-age issue should fire");
    assert_eq!(issue.id, "release-age-policy-missing:package.json");
    assert_eq!(issue.severity, Severity::Low);
    assert_eq!(
        issue.confidence,
        crate::checks::IssueConfidence::NeedsReview
    );
    assert!(issue
        .confidence_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("user-level")));
}

#[test]
fn release_age_policy_fires_for_yarn_only_lockfile_too() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);
    write_file(temp.path(), "yarn.lock", "# yarn lockfile v1\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&report).is_some(),
        "yarn projects still lack a cooldown and must flag"
    );
}

#[test]
fn release_age_policy_yarn_native_setting_passes() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);
    write_file(temp.path(), "yarn.lock", "# yarn lockfile\n");
    write_file(temp.path(), ".yarnrc.yml", "npmMinimalAgeGate: 1d\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&report).is_none(),
        "Yarn 4.12+ npmMinimalAgeGate must satisfy the check"
    );
}

#[test]
fn yarn_4_12_builtin_age_gate_satisfies_the_check_unless_disabled() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "demo-app",
  "packageManager": "yarn@4.12.0",
  "dependencies": { "react": "^19.0.0" }
}"#,
    );
    write_file(temp.path(), "yarn.lock", "# yarn lockfile\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(release_age_issue(&report).is_none());

    write_file(temp.path(), ".yarnrc.yml", "npmMinimalAgeGate: 0d\n");
    let disabled_report = audit_project(temp.path()).unwrap();
    assert!(release_age_issue(&disabled_report).is_some());
}

#[test]
fn pnpm_11_builtin_age_gate_satisfies_the_check_unless_disabled() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "demo-app",
  "packageManager": "pnpm@11.0.0",
  "dependencies": { "react": "^19.0.0" }
}"#,
    );
    write_file(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '10.0'\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(release_age_issue(&report).is_none());

    write_file(temp.path(), "pnpm-workspace.yaml", "minimumReleaseAge: 0\n");
    let disabled_report = audit_project(temp.path()).unwrap();
    assert!(release_age_issue(&disabled_report).is_some());
}

#[test]
fn native_setting_does_not_clear_the_issue_for_an_unsupported_pinned_client() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "demo-app",
  "packageManager": "npm@11.9.0",
  "dependencies": { "react": "^19.0.0" }
}"#,
    );
    write_file(
        temp.path(),
        "package-lock.json",
        r#"{ "lockfileVersion": 3 }"#,
    );
    write_file(temp.path(), ".npmrc", "min-release-age=7\n");

    let npm_report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&npm_report).is_some(),
        "npm added min-release-age in 11.10"
    );

    write_file(
        temp.path(),
        "package.json",
        r#"{
  "name": "demo-app",
  "packageManager": "npm@11.10.0",
  "dependencies": { "react": "^19.0.0" }
}"#,
    );
    let supported_report = audit_project(temp.path()).unwrap();
    assert!(release_age_issue(&supported_report).is_none());
}

#[test]
fn release_age_policy_ignores_nonstandard_yarnrc_yaml_filename() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);
    write_file(temp.path(), "yarn.lock", "# yarn lockfile\n");
    write_file(temp.path(), ".yarnrc.yaml", "npmMinimalAgeGate: 1d\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&report).is_some(),
        "Yarn reads .yarnrc.yml; a similarly named .yarnrc.yaml must not satisfy the check"
    );
}

#[test]
fn release_age_policy_pnpm_workspace_setting_passes() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);
    write_file(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
    write_file(
        temp.path(),
        "pnpm-workspace.yaml",
        "packages:\n  - apps/*\nminimumReleaseAge: 1440\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&report).is_none(),
        "pnpm-workspace.yaml minimumReleaseAge must satisfy the check"
    );
}

#[test]
fn package_json_pnpm_field_does_not_masquerade_as_manager_configuration() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "package.json",
        r#"
{
  "name": "demo-app",
  "dependencies": { "react": "^19.0.0" },
  "pnpm": { "minimumReleaseAge": 1440 }
}
"#,
    );
    write_file(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(release_age_issue(&report).is_some(),
        "pnpm documents minimumReleaseAge as a manager setting in pnpm-workspace.yaml/.npmrc, not a package.json pnpm field");
}

#[test]
fn release_age_policy_npmrc_setting_passes() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);
    write_file(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
    write_file(temp.path(), ".npmrc", "minimum-release-age=1440\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&report).is_none(),
        ".npmrc minimum-release-age must satisfy the check"
    );
}

#[test]
fn release_age_policy_npm_native_npmrc_setting_passes() {
    // npm 11.10+ native cooldown: `min-release-age` (days) in `.npmrc`. The
    // official npm fix must clear the finding on re-scan.
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);
    write_file(
        temp.path(),
        "package-lock.json",
        r#"{ "lockfileVersion": 3 }"#,
    );
    write_file(temp.path(), ".npmrc", "min-release-age=7\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&report).is_none(),
        ".npmrc min-release-age (npm-native) must satisfy the check"
    );
}

#[test]
fn release_age_policy_bunfig_setting_passes() {
    // Bun 1.3+ native cooldown: [install] minimumReleaseAge (seconds) in
    // bunfig.toml. The official Bun fix must clear the finding on re-scan.
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);
    write_file(temp.path(), "bun.lock", "{}\n");
    write_file(
        temp.path(),
        "bunfig.toml",
        "[install]\nminimumReleaseAge = 259200\n",
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&report).is_none(),
        "bunfig.toml [install] minimumReleaseAge must satisfy the check"
    );
}

#[test]
fn release_age_policy_bun_project_without_cooldown_still_flags() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);
    write_file(temp.path(), "bun.lock", "{}\n");
    // A bunfig.toml without the cooldown key must not satisfy the check.
    write_file(
        temp.path(),
        "bunfig.toml",
        "[install]\nregistry = \"https://registry.npmjs.org\"\n",
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = release_age_issue(&report).expect("bun project without cooldown should flag");
    let fix = issue.likely_fix.as_deref().unwrap_or_default();
    assert!(
        fix.contains("min-release-age") && fix.contains("bunfig.toml"),
        "fix should name the npm and Bun native cooldowns: {fix}"
    );
    assert!(
        !fix.contains("pnpm-native"),
        "the false pnpm-native claim must be gone: {fix}"
    );
}

#[test]
fn release_age_policy_skips_projects_without_third_party_dependencies() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", r#"{ "name": "demo-app" }"#);
    write_file(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

    let report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&report).is_none(),
        "no third-party dependencies means no cooldown to require"
    );
}

#[test]
fn release_age_policy_skips_projects_without_a_lockfile() {
    let temp = TempDir::new().unwrap();
    write_file(temp.path(), "package.json", MANIFEST_WITH_DEPENDENCIES);

    let report = audit_project(temp.path()).unwrap();
    assert!(
        release_age_issue(&report).is_none(),
        "lockfile-missing owns the no-lockfile case"
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.id.starts_with("lockfile-missing:")),
        "the missing lockfile is still reported by its own rule"
    );
}
