use super::hook_install::{inspect_hook_install, HookInstallState};
use super::manifest_scripts::ScriptInventory;
use super::quality_markers::{contains_any_marker, QUALITY_GATE_MARKERS, TEST_GATE_MARKERS};
use super::*;

const JS_TEST_PACKAGES: &[&str] = &[
    "vitest",
    "jest",
    "@jest/core",
    "mocha",
    "ava",
    "tap",
    "playwright",
    "cypress",
    "@playwright/test",
    "@testing-library/react",
    "@testing-library/jest-dom",
    "@web/test-runner",
    "@wdio/cli",
    "karma",
    "pytest",
    "unittest",
];

pub(super) struct QualitySignals {
    pub(super) has_linter_config: bool,
    pub(super) has_build_script: bool,
    pub(super) has_test_script: bool,
    pub(super) has_lint_or_typecheck_script: bool,
    pub(super) has_test_infrastructure: bool,
    pub(super) has_runnable_tests: bool,
    pub(super) placeholder_test_script: bool,
    pub(super) has_ci_config: bool,
    pub(super) ci_has_quality_gate: bool,
    pub(super) ci_runs_tests: bool,
    pub(super) has_commit_hooks: bool,
    pub(super) hooks_have_quality_gate: bool,
    pub(super) hooks_run_tests: bool,
    pub(super) has_quality_scripts: bool,
    pub(super) install: HookInstallState,
}

impl QualitySignals {
    /// A CI workflow exists and runs at least one quality command.
    pub(super) fn remote_enforced(&self) -> bool {
        self.has_ci_config && self.ci_has_quality_gate
    }

    /// A project-managed hook exists and runs at least one quality command.
    pub(super) fn hooks_enforced(&self) -> bool {
        self.has_commit_hooks && self.hooks_have_quality_gate
    }
}

pub(super) fn inspect_quality_signals(
    context: &ProjectHygieneContext<'_>,
    scripts: &ScriptInventory,
) -> QualitySignals {
    let root = context.root;
    let manifests = context.manifests;
    let paths = context.project_paths_lower;

    let linter_configs = LINTER_CONFIG_FILES
        .iter()
        .map(|config| config.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let has_linter_config = paths.iter().any(|path| {
        let file_name = path.rsplit('/').next().unwrap_or(path);
        linter_configs.iter().any(|config| file_name == config)
    }) || manifests.iter().any(|manifest| {
        let lower = manifest.content.to_ascii_lowercase();
        lower.contains("\"eslintconfig\"") || lower.contains("\"prettier\"")
    }) || scripts.has_lint_or_typecheck_script()
        || scripts.has_composer_dev_tool(PHP_LINT_TOOL_PACKAGES);

    let has_build_script = scripts.has_build_script();
    let has_test_script = scripts.has_test_script();
    let has_lint_or_typecheck_script = scripts.has_lint_or_typecheck_script();
    // A test runner in devDependencies is enough to withhold the missing-tests
    // review, but only files, config, or a script prove there is a suite to run.
    let has_runnable_tests =
        has_test_files(paths) || has_test_config_file(paths) || has_test_script;
    let has_test_infrastructure = has_runnable_tests
        || has_named_dependency(context.declared_dependencies, JS_TEST_PACKAGES)
        || scripts.has_composer_dev_tool(PHP_TEST_TOOL_PACKAGES);

    let ci_paths = ci_workflow_paths(paths);
    let ci_sources = read_lowercased(root, &ci_paths);
    let ci_has_quality_gate = ci_sources
        .iter()
        .any(|content| content_has_quality_gate(content, scripts));
    let ci_runs_tests = ci_sources
        .iter()
        .any(|content| content_runs_tests(content, scripts));

    let hook_paths = commit_hook_paths(paths);
    let hook_sources = read_lowercased(root, &hook_paths);
    let has_commit_hooks = !hook_paths.is_empty()
        || manifest_declares_hooks(manifests)
        || scripts.has_composer_dev_tool(PHP_HOOK_TOOL_PACKAGES);
    let hooks_have_quality_gate = hook_sources
        .iter()
        .any(|content| content_has_quality_gate(content, scripts))
        || manifest_hook_values_match(manifests, |value| content_has_quality_gate(value, scripts));
    let hooks_run_tests = hook_sources
        .iter()
        .any(|content| content_runs_tests(content, scripts))
        || manifest_hook_values_match(manifests, |value| content_runs_tests(value, scripts));

    QualitySignals {
        has_linter_config,
        has_build_script,
        has_test_script,
        has_lint_or_typecheck_script,
        has_test_infrastructure,
        has_runnable_tests,
        placeholder_test_script: scripts.has_placeholder_test_script(),
        has_ci_config: !ci_paths.is_empty(),
        ci_has_quality_gate,
        ci_runs_tests,
        has_commit_hooks,
        hooks_have_quality_gate,
        hooks_run_tests,
        has_quality_scripts: has_build_script || has_test_script || has_lint_or_typecheck_script,
        install: inspect_hook_install(root),
    }
}

fn has_test_files(paths: &[String]) -> bool {
    paths.iter().any(|path| {
        is_test_artifact_path(path)
            || path.starts_with("tests/")
            || path.starts_with("test/")
            || path.starts_with("__tests__/")
            || path.contains("/tests/")
            || path.contains("/test/")
            || path.ends_with("_test.py")
            || path.ends_with("_test.go")
            || path.ends_with("_spec.rb")
            // pytest uses the test_*.py prefix convention.
            || path
                .rsplit('/')
                .next()
                .is_some_and(|name| name.starts_with("test_") && name.ends_with(".py"))
    })
}

fn has_test_config_file(paths: &[String]) -> bool {
    paths.iter().any(|path| {
        TEST_CONFIG_FILES
            .iter()
            .any(|config| path.ends_with(config))
    })
}

fn read_lowercased(root: &Path, paths: &[&str]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| {
            crate::core::code_scan::filesystem::read_text_under_root(root, &root.join(path))
                .map(|content| content.to_ascii_lowercase())
        })
        .collect()
}

pub(super) fn ci_workflow_paths(project_paths_lower: &[String]) -> Vec<&str> {
    project_paths_lower
        .iter()
        .filter_map(|path| {
            let is_ci = (path.starts_with(".github/workflows/")
                && (path.ends_with(".yml") || path.ends_with(".yaml")))
                || path == ".gitlab-ci.yml"
                || path == ".circleci/config.yml"
                || path.starts_with(".buildkite/")
                || path == "azure-pipelines.yml"
                || path == "bitbucket-pipelines.yml"
                || path == "jenkinsfile"
                || path.ends_with("/jenkinsfile");
            is_ci.then_some(path.as_str())
        })
        .collect()
}

pub(super) fn commit_hook_paths(project_paths_lower: &[String]) -> Vec<&str> {
    project_paths_lower
        .iter()
        .filter_map(|path| {
            let is_hook = path == ".pre-commit-config.yaml"
                || path == ".pre-commit-config.yml"
                || path == "lefthook.yml"
                || path == "lefthook.yaml"
                || path == ".lefthook.yml"
                || path == ".lefthook.yaml"
                || path == ".husky/pre-commit"
                || path == ".husky/pre-push"
                || path.ends_with("/.husky/pre-commit")
                || path.ends_with("/.husky/pre-push")
                || PHP_HOOK_CONFIG_FILES.contains(&path.as_str());
            is_hook.then_some(path.as_str())
        })
        .collect()
}

fn content_has_quality_gate(lower: &str, scripts: &ScriptInventory) -> bool {
    contains_any_marker(lower, QUALITY_GATE_MARKERS) || scripts.content_calls_quality_script(lower)
}

fn content_runs_tests(lower: &str, scripts: &ScriptInventory) -> bool {
    contains_any_marker(lower, TEST_GATE_MARKERS) || scripts.content_calls_test_script(lower)
}

fn is_hook_script_name(name: &str) -> bool {
    name.contains("precommit")
        || name.contains("pre-commit")
        || name.contains("prepush")
        || name.contains("pre-push")
}

fn manifest_declares_hooks(manifests: &[PackageManifest]) -> bool {
    manifests.iter().any(|manifest| {
        let Ok(json) = serde_json::from_str::<Value>(&manifest.content) else {
            return false;
        };
        if json.get("simple-git-hooks").is_some() || json.get("pre-commit").is_some() {
            return true;
        }
        let Some(scripts) = json.get("scripts").and_then(Value::as_object) else {
            return false;
        };
        scripts.iter().any(|(name, value)| {
            let name = name.to_ascii_lowercase();
            let command = value.as_str().unwrap_or_default().to_ascii_lowercase();
            (name == "prepare"
                && (command.contains("husky")
                    || command.contains("lefthook")
                    || command.contains("simple-git-hooks")))
                || is_hook_script_name(&name)
        })
    })
}

/// Apply `predicate` to every hook command declared inside package.json:
/// lint-staged, simple-git-hooks, and pre-commit tables plus hook-named scripts.
fn manifest_hook_values_match(
    manifests: &[PackageManifest],
    predicate: impl Fn(&str) -> bool,
) -> bool {
    manifests.iter().any(|manifest| {
        let Ok(json) = serde_json::from_str::<Value>(&manifest.content) else {
            return false;
        };
        let mut values = Vec::new();
        for key in ["lint-staged", "simple-git-hooks", "pre-commit"] {
            if let Some(value) = json.get(key) {
                collect_string_values(value, &mut values);
            }
        }
        if let Some(scripts) = json.get("scripts").and_then(Value::as_object) {
            for (name, value) in scripts {
                if is_hook_script_name(&name.to_ascii_lowercase()) {
                    if let Some(command) = value.as_str() {
                        values.push(command.to_ascii_lowercase());
                    }
                }
            }
        }
        values.iter().any(|value| predicate(value))
    })
}

fn collect_string_values(value: &Value, target: &mut Vec<String>) {
    match value {
        Value::String(value) => target.push(value.to_ascii_lowercase()),
        Value::Array(values) => values
            .iter()
            .for_each(|value| collect_string_values(value, target)),
        Value::Object(map) => map
            .values()
            .for_each(|value| collect_string_values(value, target)),
        _ => {}
    }
}
