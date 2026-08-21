use super::quality_markers::QUALITY_GATE_MARKERS;
use super::*;

pub(super) fn inspect_quality_signals(context: &ProjectHygieneContext<'_>) -> QualitySignals {
    let manifests = context.manifests;
    let project_paths_lower = context.project_paths_lower;
    let has_linter_config = project_paths_lower.iter().any(|path| {
        LINTER_CONFIG_FILES.iter().any(|config| {
            let config_lower = config.to_ascii_lowercase();
            path == &config_lower || path.ends_with(&format!("/{}", config_lower))
        })
    }) || manifests.iter().any(|manifest| {
        let lower = manifest.content.to_ascii_lowercase();
        lower.contains("\"eslintconfig\"")
            || lower.contains("\"prettier\"")
            || lower.contains("\"lint\"")
    });
    let has_build_script = manifests
        .iter()
        .any(|manifest| manifest_has_script(manifest, |name, _| name == "build"));
    let has_test_script = manifests.iter().any(|manifest| {
        manifest_has_script(manifest, |name, command| {
            name == "test"
                || name.starts_with("test:")
                || command.contains("vitest")
                || command.contains("jest")
                || command.contains("playwright")
                || command.contains("cypress")
        })
    });
    let has_lint_or_typecheck_script = manifests.iter().any(|manifest| {
        manifest_has_script(manifest, |name, command| {
            name == "lint"
                || name.starts_with("lint:")
                || name == "typecheck"
                || name == "type-check"
                || name.starts_with("typecheck:")
                || name.starts_with("type-check:")
                || command.contains("eslint")
                || command.contains("biome")
                || command.contains("tsc")
        })
    });
    let has_ci_config = has_ci_workflow_config(project_paths_lower);
    let has_commit_hooks = has_commit_hook_config(project_paths_lower, manifests);

    QualitySignals {
        has_linter_config,
        has_build_script,
        has_test_script,
        has_lint_or_typecheck_script,
        has_ci_config,
        has_commit_hooks,
        has_quality_scripts: has_build_script || has_test_script || has_lint_or_typecheck_script,
    }
}

fn manifest_has_script<F>(manifest: &PackageManifest, predicate: F) -> bool
where
    F: Fn(&str, &str) -> bool,
{
    let Ok(json) = serde_json::from_str::<Value>(&manifest.content) else {
        return false;
    };
    let Some(scripts) = json.get("scripts").and_then(Value::as_object) else {
        return false;
    };

    scripts.iter().any(|(name, value)| {
        value
            .as_str()
            .is_some_and(|command| predicate(name.as_str(), &command.to_ascii_lowercase()))
    })
}

fn has_ci_workflow_config(project_paths_lower: &[String]) -> bool {
    !ci_workflow_paths(project_paths_lower).is_empty()
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

fn has_commit_hook_config(project_paths_lower: &[String], manifests: &[PackageManifest]) -> bool {
    if !commit_hook_paths(project_paths_lower).is_empty() {
        return true;
    }

    manifests.iter().any(|manifest| {
        let Ok(json) = serde_json::from_str::<Value>(&manifest.content) else {
            return false;
        };

        json.get("simple-git-hooks").is_some()
            || json.get("pre-commit").is_some()
            || manifest_has_script(manifest, |name, command| {
                name == "prepare"
                    && (command.contains("husky")
                        || command.contains("lefthook")
                        || command.contains("simple-git-hooks"))
                    || name.contains("precommit")
                    || name.contains("pre-commit")
                    || name.contains("prepush")
                    || name.contains("pre-push")
            })
    })
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
                || path.ends_with("/.husky/pre-push");
            is_hook.then_some(path.as_str())
        })
        .collect()
}

pub(super) fn has_ci_quality_gate(
    root: &Path,
    project_paths_lower: &[String],
    manifests: &[PackageManifest],
) -> bool {
    ci_workflow_paths(project_paths_lower).iter().any(|path| {
        crate::core::code_scan::filesystem::read_text_under_root(root, &root.join(*path))
            .map(|content| content_has_quality_gate(&content, manifests))
            .unwrap_or(false)
    })
}

pub(super) fn has_commit_hook_quality_gate(
    root: &Path,
    project_paths_lower: &[String],
    manifests: &[PackageManifest],
) -> bool {
    if commit_hook_paths(project_paths_lower).iter().any(|path| {
        crate::core::code_scan::filesystem::read_text_under_root(root, &root.join(*path))
            .map(|content| content_has_quality_gate(&content, manifests))
            .unwrap_or(false)
    }) {
        return true;
    }

    manifests.iter().any(|manifest| {
        let Ok(json) = serde_json::from_str::<Value>(&manifest.content) else {
            return false;
        };

        json.get("lint-staged")
            .is_some_and(value_contains_quality_gate)
            || json
                .get("simple-git-hooks")
                .is_some_and(value_contains_quality_gate)
            || json
                .get("pre-commit")
                .is_some_and(value_contains_quality_gate)
            || manifest_has_script(manifest, |name, command| {
                (name.contains("precommit")
                    || name.contains("pre-commit")
                    || name.contains("prepush")
                    || name.contains("pre-push"))
                    && content_has_direct_quality_gate(command)
            })
    })
}

fn content_has_quality_gate(content: &str, manifests: &[PackageManifest]) -> bool {
    content_has_direct_quality_gate(content)
        || content_calls_manifest_quality_script(content, manifests)
}

fn content_has_direct_quality_gate(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    QUALITY_GATE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

fn value_contains_quality_gate(value: &Value) -> bool {
    match value {
        Value::String(value) => content_has_direct_quality_gate(value),
        Value::Array(values) => values.iter().any(value_contains_quality_gate),
        Value::Object(map) => map.values().any(value_contains_quality_gate),
        _ => false,
    }
}

fn content_calls_manifest_quality_script(content: &str, manifests: &[PackageManifest]) -> bool {
    let lower = content.to_ascii_lowercase();
    manifest_quality_script_names(manifests).iter().any(|name| {
        let script = name.to_ascii_lowercase();
        [
            format!("npm run {}", script),
            format!("npm run -s {}", script),
            format!("npm run --if-present {}", script),
            format!("pnpm {}", script),
            format!("pnpm run {}", script),
            format!("yarn {}", script),
            format!("yarn run {}", script),
            format!("bun run {}", script),
        ]
        .iter()
        .any(|needle| lower.contains(needle))
    })
}

fn manifest_quality_script_names(manifests: &[PackageManifest]) -> Vec<String> {
    let mut names = HashSet::new();
    for manifest in manifests {
        let Ok(json) = serde_json::from_str::<Value>(&manifest.content) else {
            continue;
        };
        let Some(scripts) = json.get("scripts").and_then(Value::as_object) else {
            continue;
        };
        for (name, value) in scripts {
            let Some(command) = value.as_str() else {
                continue;
            };
            let name_lower = name.to_ascii_lowercase();
            if matches!(
                name_lower.as_str(),
                "build" | "test" | "lint" | "typecheck" | "type-check" | "check" | "ci"
            ) || name_lower.starts_with("test:")
                || name_lower.starts_with("lint:")
                || name_lower.starts_with("typecheck:")
                || name_lower.starts_with("type-check:")
                || content_has_direct_quality_gate(command)
            {
                names.insert(name.clone());
            }
        }
    }
    names.into_iter().collect()
}
