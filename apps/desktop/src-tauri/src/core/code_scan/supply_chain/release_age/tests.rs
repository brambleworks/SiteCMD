use super::{
    bunfig_configures_release_age, dependabot_configures_cooldown,
    manifest_content_configures_release_age, npmrc_configures_release_age, pinned_package_manager,
    pnpm_workspace_configures_release_age, yarnrc_configures_release_age, JsPackageManager,
    PackageManifest,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[test]
fn npm_native_min_release_age_counts_as_release_age_policy() {
    assert!(npmrc_configures_release_age("min-release-age=7\n"));
    assert!(npmrc_configures_release_age(
        "registry=https://registry.npmjs.org/\nmin-release-age = 7\n"
    ));
    assert!(!npmrc_configures_release_age("# min-release-age=7\n"));
    assert!(!npmrc_configures_release_age("min-release-age-notes=x\n"));
    assert!(!npmrc_configures_release_age("min-release-age=0\n"));
    assert!(!npmrc_configures_release_age("min-release-age=7days\n"));
}

#[test]
fn bunfig_minimum_release_age_counts_as_release_age_policy() {
    assert!(bunfig_configures_release_age(
        "[install]\nminimumReleaseAge = 259200\n"
    ));
    assert!(bunfig_configures_release_age(
        "[install]\nregistry = \"https://registry.npmjs.org\"\nminimumReleaseAge = 86400\nminimumReleaseAgeExcludes = [\"typescript\"]\n"
    ));
    assert!(bunfig_configures_release_age(
        "install.minimumReleaseAge = 259200\n"
    ));
    assert!(!bunfig_configures_release_age(
        "[test]\nminimumReleaseAge = 259200\n"
    ));
    assert!(!bunfig_configures_release_age(
        "[install]\nregistry = \"https://registry.npmjs.org\"\n"
    ));
    assert!(!bunfig_configures_release_age(
        "[install]\n# minimumReleaseAge = 259200\n"
    ));
    assert!(!bunfig_configures_release_age(
        "[install]\nminimumReleaseAge = 0\n"
    ));
    assert!(!bunfig_configures_release_age(
        "[install]\nminimumReleaseAge = 1day\n"
    ));
    assert!(!bunfig_configures_release_age(
        "[install]\nminimumReleaseAge = 0\nminimumReleaseAge = 259200\n"
    ));
}

#[test]
fn package_json_renovate_key_counts_as_release_age_policy() {
    assert!(manifest_content_configures_release_age(
        r#"{"name":"app","renovate":{"minimumReleaseAge":"3 days"}}"#
    ));
    assert!(manifest_content_configures_release_age(
        r#"{"name":"app","renovate":{"packageRules":[{"minimumReleaseAge":"5 days"}]}}"#
    ));
    assert!(!manifest_content_configures_release_age(
        r#"{"name":"app","renovate":{"packageRules":[{"matchPackageNames":["react"],"minimumReleaseAge":"5 days"}]}}"#
    ));
    assert!(!manifest_content_configures_release_age(
        r#"{"name":"app","pnpm":{"minimumReleaseAge":1440}}"#
    ));
    assert!(!manifest_content_configures_release_age(
        r#"{"name":"app","renovate":{"extends":["config:recommended"]}}"#
    ));
    assert!(!manifest_content_configures_release_age(
        r#"{"name":"app","renovate":{"minimumReleaseAge":"0 days"}}"#
    ));
    assert!(!manifest_content_configures_release_age(
        r#"{"name":"app","renovate":{"minimumReleaseAge":null}}"#
    ));
    assert!(!manifest_content_configures_release_age(
        r#"{"name":"app"}"#
    ));
}

#[test]
fn release_age_config_detection_matches_real_setting_shapes() {
    assert!(pnpm_workspace_configures_release_age(
        "packages:\n  - apps/*\nminimumReleaseAge: 1440\n"
    ));
    assert!(!pnpm_workspace_configures_release_age(
        "packages:\n  - apps/*\n"
    ));
    assert!(!pnpm_workspace_configures_release_age(
        "minimumReleaseAgeNotes: see docs\n"
    ));
    assert!(!pnpm_workspace_configures_release_age(
        "minimumReleaseAge: 0\n"
    ));
    assert!(!pnpm_workspace_configures_release_age(
        "minimumReleaseAge: 1day\n"
    ));

    assert!(npmrc_configures_release_age("minimum-release-age=1440\n"));
    assert!(npmrc_configures_release_age(
        "registry=https://registry.npmjs.org/\nminimum-release-age = 1440\n"
    ));
    assert!(!npmrc_configures_release_age(
        "# minimum-release-age=1440\nregistry=https://registry.npmjs.org/\n"
    ));

    assert!(dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n    cooldown:\n      default-days: 3\n"
    ));
    assert!(!dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n"
    ));
    assert!(!dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n    cooldown: {}\n"
    ));
    assert!(!dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n    cooldown:\n      default-days: 0\n"
    ));
    assert!(!dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n    cooldown:\n      default-days: 91\n"
    ));
    assert!(dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n    cooldown: { default-days: 3 }\n"
    ));
    assert!(dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: \"npm\"\n    cooldown:\n      \"default-days\": \"3\"\n"
    ));
    assert!(!dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n    cooldown:\n      default-days: 3\n      include:\n        - react\n"
    ));
    assert!(dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n    cooldown:\n      default-days: 3\n      include:\n        - '*'\n"
    ));
    assert!(!dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n    cooldown:\n      semver-patch-days: 3\n"
    ));
    assert!(dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: npm\n    cooldown:\n      semver-major-days: 30\n      semver-minor-days: 7\n      semver-patch-days: 3\n"
    ));
    assert!(!dependabot_configures_cooldown(
        "updates:\n  - package-ecosystem: pip\n    cooldown:\n      default-days: 3\n  - package-ecosystem: npm\n    directory: /\n"
    ));

    assert!(yarnrc_configures_release_age("npmMinimalAgeGate: 1d\n"));
    assert!(yarnrc_configures_release_age(
        "npmMinimalAgeGate: 3h # reviewed\n"
    ));
    assert!(!yarnrc_configures_release_age("# npmMinimalAgeGate: 1d\n"));
    assert!(!yarnrc_configures_release_age("npmMinimalAgeGate: 0d\n"));
}

fn manifest(relative_path: &str, package_manager: &str) -> PackageManifest {
    PackageManifest {
        absolute_path: PathBuf::from(relative_path),
        relative_path: relative_path.into(),
        content: format!(r#"{{"packageManager":"{package_manager}"}}"#),
        package_name: None,
        dependencies: HashSet::new(),
        local_dependencies: HashSet::new(),
        dependency_specs: HashMap::new(),
    }
}

#[test]
fn root_package_manager_pin_wins_over_nested_manifest_order() {
    let manifests = vec![
        manifest("apps/docs/package.json", "yarn@4.12.0"),
        manifest("package.json", "npm@11.10.0"),
    ];

    let pinned = pinned_package_manager(&manifests).expect("root manager pin");
    assert_eq!(pinned.manager, JsPackageManager::Npm);
    assert_eq!(pinned.version, (11, 10, 0));
}

#[test]
fn conflicting_peer_package_manager_pins_are_not_treated_as_project_wide() {
    let manifests = vec![
        manifest("apps/docs/package.json", "yarn@4.12.0"),
        manifest("apps/web/package.json", "pnpm@11.0.0"),
    ];

    assert!(pinned_package_manager(&manifests).is_none());
}
