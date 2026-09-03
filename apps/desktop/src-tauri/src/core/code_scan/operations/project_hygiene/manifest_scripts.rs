//! Package scripts from package.json and composer.json, with npm's placeholder
//! scripts filtered out so an `npm init` default never counts as a test suite.

use super::quality_markers::{
    contains_any_marker, contains_marker, LINT_COMMAND_MARKERS, QUALITY_GATE_MARKERS,
    TEST_GATE_MARKERS,
};
use super::*;

/// Composer manifests read per scan; the root manifest sorts first.
const COMPOSER_MANIFEST_LIMIT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScriptRunner {
    Npm,
    Composer,
}

#[derive(Debug, Clone)]
pub(super) struct ProjectScript {
    pub(super) runner: ScriptRunner,
    pub(super) name: String,
    pub(super) command: String,
    pub(super) placeholder: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ComposerManifest {
    pub(super) relative_path: String,
    pub(super) dev_packages: HashSet<String>,
}

#[derive(Debug, Default)]
pub(super) struct ScriptInventory {
    pub(super) scripts: Vec<ProjectScript>,
    pub(super) composer: Vec<ComposerManifest>,
}

pub(super) fn collect_script_inventory(
    root: &Path,
    manifests: &[PackageManifest],
    project_paths_lower: &[String],
) -> ScriptInventory {
    let mut inventory = ScriptInventory::default();
    for manifest in manifests {
        let Ok(json) = serde_json::from_str::<Value>(&manifest.content) else {
            continue;
        };
        push_scripts(
            &mut inventory.scripts,
            ScriptRunner::Npm,
            json.get("scripts"),
        );
    }

    let mut composer_paths = project_paths_lower
        .iter()
        .filter(|path| path.as_str() == "composer.json" || path.ends_with("/composer.json"))
        .collect::<Vec<_>>();
    composer_paths.sort_by_key(|path| path.len());
    for path in composer_paths.into_iter().take(COMPOSER_MANIFEST_LIMIT) {
        let Some(content) =
            crate::core::code_scan::filesystem::read_text_under_root(root, &root.join(path))
        else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<Value>(&content) else {
            continue;
        };
        push_scripts(
            &mut inventory.scripts,
            ScriptRunner::Composer,
            json.get("scripts"),
        );
        let dev_packages = json
            .get("require-dev")
            .and_then(Value::as_object)
            .map(|table| table.keys().map(|key| key.to_ascii_lowercase()).collect())
            .unwrap_or_default();
        inventory.composer.push(ComposerManifest {
            relative_path: path.clone(),
            dev_packages,
        });
    }
    inventory
}

fn push_scripts(target: &mut Vec<ProjectScript>, runner: ScriptRunner, scripts: Option<&Value>) {
    let Some(scripts) = scripts.and_then(Value::as_object) else {
        return;
    };
    for (name, value) in scripts {
        let name = name.to_ascii_lowercase();
        // Composer event hooks (pre-/post-install) are lifecycle glue, not
        // quality commands.
        if runner == ScriptRunner::Composer
            && (name.starts_with("pre-") || name.starts_with("post-"))
        {
            continue;
        }
        let command = match value {
            Value::String(command) => command.clone(),
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" && "),
            _ => continue,
        }
        .to_ascii_lowercase();
        let placeholder = is_placeholder_script(&command);
        target.push(ProjectScript {
            runner,
            name,
            command,
            placeholder,
        });
    }
}

/// `npm init` writes `echo "Error: no test specified" && exit 1`; a script
/// that only echoes or exits proves nothing about the project.
pub(super) fn is_placeholder_script(command_lower: &str) -> bool {
    if command_lower.contains("no test specified") {
        return true;
    }
    command_lower
        .split(';')
        .flat_map(|segment| segment.split("&&"))
        .flat_map(|segment| segment.split("||"))
        .map(str::trim)
        .all(|segment| {
            segment.is_empty()
                || segment == "true"
                || segment == ":"
                || segment == "echo"
                || segment.starts_with("echo ")
                || segment.starts_with("printf ")
                || segment == "exit"
                || segment.starts_with("exit ")
        })
}

fn is_test_script(script: &ProjectScript) -> bool {
    script.name == "test"
        || script.name.starts_with("test:")
        || script.name == "e2e"
        || script.name.starts_with("e2e:")
        || contains_any_marker(&script.command, TEST_GATE_MARKERS)
}

fn is_lint_script(script: &ProjectScript) -> bool {
    matches!(
        script.name.as_str(),
        "lint"
            | "typecheck"
            | "type-check"
            | "check"
            | "cs"
            | "analyse"
            | "analyze"
            | "phpstan"
            | "psalm"
            | "pint"
            | "stan"
    ) || script.name.starts_with("lint:")
        || script.name.starts_with("typecheck:")
        || script.name.starts_with("type-check:")
        || script.name.starts_with("check:")
        || script.name.starts_with("cs:")
        || contains_any_marker(&script.command, LINT_COMMAND_MARKERS)
}

/// `build`, plus Laravel Mix's documented `production` and `prod` entry points.
fn is_build_script(script: &ProjectScript) -> bool {
    matches!(script.name.as_str(), "build" | "production" | "prod")
}

fn is_quality_script(script: &ProjectScript) -> bool {
    is_build_script(script)
        || script.name == "ci"
        || is_test_script(script)
        || is_lint_script(script)
        || contains_any_marker(&script.command, QUALITY_GATE_MARKERS)
}

impl ScriptInventory {
    fn live(&self) -> impl Iterator<Item = &ProjectScript> {
        self.scripts.iter().filter(|script| !script.placeholder)
    }

    pub(super) fn has_build_script(&self) -> bool {
        self.live()
            .any(|script| script.runner == ScriptRunner::Npm && is_build_script(script))
    }

    pub(super) fn has_test_script(&self) -> bool {
        self.live().any(is_test_script)
    }

    pub(super) fn has_lint_or_typecheck_script(&self) -> bool {
        self.live().any(is_lint_script)
    }

    /// The npm `test` script when it is only a placeholder.
    pub(super) fn has_placeholder_test_script(&self) -> bool {
        self.scripts
            .iter()
            .any(|script| script.placeholder && script.name == "test")
    }

    pub(super) fn quality_script_names(&self) -> Vec<(ScriptRunner, &str)> {
        self.live()
            .filter(|script| is_quality_script(script))
            .map(|script| (script.runner, script.name.as_str()))
            .collect()
    }

    pub(super) fn test_script_names(&self) -> Vec<(ScriptRunner, &str)> {
        self.live()
            .filter(|script| is_test_script(script))
            .map(|script| (script.runner, script.name.as_str()))
            .collect()
    }

    pub(super) fn has_composer_dev_tool(&self, candidates: &[&str]) -> bool {
        self.composer.iter().any(|manifest| {
            candidates
                .iter()
                .any(|candidate| manifest.dev_packages.contains(*candidate))
        })
    }

    /// Whether CI or hook text invokes a recognized quality script by name.
    pub(super) fn content_calls_quality_script(&self, lower: &str) -> bool {
        self.quality_script_names()
            .iter()
            .any(|(runner, name)| content_calls_script(lower, *runner, name))
    }

    /// Whether CI or hook text invokes a recognized test script by name.
    pub(super) fn content_calls_test_script(&self, lower: &str) -> bool {
        self.test_script_names()
            .iter()
            .any(|(runner, name)| content_calls_script(lower, *runner, name))
    }
}

fn content_calls_script(lower: &str, runner: ScriptRunner, name: &str) -> bool {
    let invocations = match runner {
        ScriptRunner::Npm => vec![
            format!("npm run {name}"),
            format!("npm run -s {name}"),
            format!("npm run --silent {name}"),
            format!("npm run --if-present {name}"),
            format!("pnpm {name}"),
            format!("pnpm run {name}"),
            format!("pnpm run --if-present {name}"),
            format!("pnpm -r {name}"),
            format!("pnpm -r run {name}"),
            format!("yarn {name}"),
            format!("yarn run {name}"),
            format!("bun run {name}"),
            format!("turbo run {name}"),
            format!("turbo {name}"),
        ],
        ScriptRunner::Composer => vec![
            format!("composer {name}"),
            format!("composer run {name}"),
            format!("composer run-script {name}"),
        ],
    };
    if invocations
        .iter()
        .any(|needle| contains_marker(lower, needle))
    {
        return true;
    }
    // Task runners accept several tasks on one line (`turbo run build lint
    // test`), so a task name anywhere on a runner line counts.
    runner == ScriptRunner::Npm
        && lower.lines().any(|line| {
            (line.contains("turbo") || line.contains("nx run-many") || line.contains("nx affected"))
                && contains_marker(line, name)
        })
}
