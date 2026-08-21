use super::*;

const CONFIG_FILE_MAX_BYTES: u64 = 250_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReleaseAgeSetting {
    Absent,
    Enabled,
    DisabledOrInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum JsPackageManager {
    Pnpm,
    Npm,
    Bun,
    Yarn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PinnedPackageManager {
    manager: JsPackageManager,
    version: (u64, u64, u64),
}

/// Flag third-party npm installs without resolution-age or update-bot cooldown controls.
pub(super) fn collect_release_age_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
    manifests: &[PackageManifest],
) {
    let Some(anchor) = manifests
        .iter()
        .filter(|manifest| {
            manifest
                .dependencies
                .iter()
                .any(|dependency| !manifest.local_dependencies.contains(dependency))
        })
        .min_by(|left, right| manifest_path_order(left, right))
    else {
        return;
    };

    let has_js_lockfile = project_files.iter().any(|file| {
        let name = file
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(file.relative_path.as_str());
        SUPPORTED_NPM_LOCKFILES.contains(&name)
    });
    if !has_js_lockfile {
        // Without any lockfile the project has a bigger reproducibility
        // problem first; lockfile-missing covers it.
        return;
    }

    if project_has_effective_release_age_policy(project_files, manifests) {
        return;
    }

    issues.push(CodeIssue {
        check_id: String::new(),
        id: format!("release-age-policy-missing:{}", anchor.relative_path),
        category: "supply-chain".into(),
        severity: Severity::Low,
        title: "No detected project-level dependency cooldown".into(),
        description: "This source tree has third-party npm-family dependencies and a lockfile, but SiteCMD found no positive manager-native resolution age setting, pinned manager with a built-in default, or project-wide Renovate/Dependabot update delay. These are different controls: native settings filter newly resolved versions, while update-bot settings defer bot-generated dependency updates. None proves a release is safe, and user-level, CI, inherited-preset, or centrally enforced controls may exist outside the scanned tree.".into(),
        relative_path: anchor.relative_path.clone(),
        absolute_path: anchor.absolute_path.to_string_lossy().to_string(),
        line: Some(1),
        source_excerpt: excerpt_for_line(&anchor.content, Some(1)),
        evidence: Some(redact_evidence("Third-party dependencies and a lockfile were found, but the scanned files and pinned manager version established neither a manager-native resolution age setting/default nor a project-wide Renovate or Dependabot update delay.")),
        why_now: Some("A fresh resolution or automated update can introduce a just-published version before compromise reports, takedowns, or maintainer warnings appear. A documented delay narrows that timing window without establishing that an older release is trustworthy.".into()),
        likely_fix: Some("Choose the control that covers the project's dependency-change path. For new resolutions, pin a supporting manager and configure pnpm 10.16+ minimumReleaseAge (minutes) in pnpm-workspace.yaml, npm 11.10+ min-release-age (days) in .npmrc, Bun 1.3+ [install] minimumReleaseAge (seconds) in bunfig.toml, or Yarn 4.12+ npmMinimalAgeGate in .yarnrc.yml. pnpm 11 defaults to a one-day non-strict gate, Yarn 4.12 defaults to one day, and an explicit pnpm 11 setting is strict by default. Renovate minimumReleaseAge and Dependabot cooldown instead delay bot-generated version updates; they do not enforce manual or CI package-manager resolution.".into()),
        confidence: crate::checks::IssueConfidence::NeedsReview,
        confidence_reason: Some("SiteCMD can establish only the scanned project files and pinned manager version. CI flags, environment variables, user-level configuration, inherited Renovate presets, or a centrally enforced resolver may provide coverage outside the source tree; detected configuration also does not prove the deployed tool version applies it.".into()),
        verify_hint: Some("For a native gate, use the pinned manager in a disposable branch to perform a fresh resolution and confirm a release younger than the window is filtered; existing locked versions may remain unchanged. For Renovate or Dependabot, validate the resolved bot configuration and confirm an ordinary version update is deferred. Dependabot cooldown does not delay security updates.".into()),
    });
}

fn project_has_effective_release_age_policy(
    project_files: &[ProjectFile],
    manifests: &[PackageManifest],
) -> bool {
    if project_has_update_tool_policy(project_files, manifests) {
        return true;
    }

    if let Some(pinned) = pinned_package_manager(manifests) {
        return manager_has_effective_release_age_policy(
            pinned.manager,
            Some(pinned.version),
            project_files,
        );
    }

    inferred_package_manager(project_files).is_some_and(|manager| {
        manager_has_effective_release_age_policy(manager, None, project_files)
    })
}

fn project_has_update_tool_policy(
    project_files: &[ProjectFile],
    manifests: &[PackageManifest],
) -> bool {
    if manifests.iter().any(|manifest| {
        serde_json::from_str::<Value>(&manifest.content)
            .ok()
            .and_then(|json| json.get("renovate").cloned())
            .is_some_and(|renovate| renovate_has_project_wide_release_age(&renovate))
    }) {
        return true;
    }

    project_files.iter().any(|file| {
        let name = file
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(file.relative_path.as_str())
            .to_ascii_lowercase();
        let checker: fn(&str) -> bool = match name.as_str() {
            "renovate.json" | "renovate.jsonc" | "renovate.json5" | ".renovaterc"
            | ".renovaterc.json" | ".renovaterc.jsonc" | ".renovaterc.json5" => {
                renovate_configures_release_age
            }
            "dependabot.yml" | "dependabot.yaml" => dependabot_configures_cooldown,
            _ => return false,
        };
        let Some(bytes) = read_project_file(file, CONFIG_FILE_MAX_BYTES) else {
            return false;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            return false;
        };
        checker(&content)
    })
}

fn manager_has_effective_release_age_policy(
    manager: JsPackageManager,
    version: Option<(u64, u64, u64)>,
    project_files: &[ProjectFile],
) -> bool {
    let setting = manager_release_age_setting(manager, version, project_files);
    match setting {
        ReleaseAgeSetting::Enabled => version
            .map(|version| manager_supports_release_age(manager, version))
            .unwrap_or(true),
        ReleaseAgeSetting::DisabledOrInvalid => false,
        ReleaseAgeSetting::Absent => version
            .map(|version| manager_has_builtin_release_age(manager, version))
            .unwrap_or(false),
    }
}

fn manager_release_age_setting(
    manager: JsPackageManager,
    version: Option<(u64, u64, u64)>,
    project_files: &[ProjectFile],
) -> ReleaseAgeSetting {
    let mut states = Vec::new();

    for file in project_files {
        let name = file
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(file.relative_path.as_str())
            .to_ascii_lowercase();
        let applicable: Option<fn(&str) -> ReleaseAgeSetting> = match (manager, name.as_str()) {
            (JsPackageManager::Pnpm, "pnpm-workspace.yaml") => {
                Some(pnpm_workspace_release_age_setting as fn(&str) -> ReleaseAgeSetting)
            }
            (JsPackageManager::Pnpm, ".npmrc")
                if version.is_none_or(|version| version < (11, 0, 0)) =>
            {
                Some(pnpm_legacy_npmrc_release_age_setting)
            }
            (JsPackageManager::Npm, ".npmrc") => Some(npm_release_age_setting),
            (JsPackageManager::Bun, "bunfig.toml") => Some(bunfig_release_age_setting),
            (JsPackageManager::Yarn, ".yarnrc.yml") => Some(yarnrc_release_age_setting),
            _ => None,
        };
        let Some(checker) = applicable else {
            continue;
        };
        let depth = path_depth(&file.relative_path);
        let Some(bytes) = read_project_file(file, CONFIG_FILE_MAX_BYTES) else {
            states.push((depth, ReleaseAgeSetting::DisabledOrInvalid));
            continue;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            states.push((depth, ReleaseAgeSetting::DisabledOrInvalid));
            continue;
        };
        states.push((depth, checker(&content)));
    }

    let Some(min_depth) = states.iter().map(|(depth, _)| *depth).min() else {
        return ReleaseAgeSetting::Absent;
    };
    combine_setting_states(
        states
            .into_iter()
            .filter(|(depth, _)| *depth == min_depth)
            .map(|(_, state)| state),
    )
}

fn combine_setting_states(
    states: impl IntoIterator<Item = ReleaseAgeSetting>,
) -> ReleaseAgeSetting {
    let mut enabled = false;
    let mut disabled_or_invalid = false;
    for state in states {
        match state {
            ReleaseAgeSetting::Absent => {}
            ReleaseAgeSetting::Enabled => enabled = true,
            ReleaseAgeSetting::DisabledOrInvalid => disabled_or_invalid = true,
        }
    }
    match (enabled, disabled_or_invalid) {
        (true, false) => ReleaseAgeSetting::Enabled,
        _ => ReleaseAgeSetting::DisabledOrInvalid,
    }
}

fn pinned_package_manager(manifests: &[PackageManifest]) -> Option<PinnedPackageManager> {
    let candidates = manifests
        .iter()
        .filter_map(|manifest| {
            let json = serde_json::from_str::<Value>(&manifest.content).ok()?;
            let package_manager = json.get("packageManager")?.as_str()?;
            let (name, version) = package_manager.split_once('@')?;
            let manager = match name.to_ascii_lowercase().as_str() {
                "pnpm" => JsPackageManager::Pnpm,
                "npm" => JsPackageManager::Npm,
                "bun" => JsPackageManager::Bun,
                "yarn" => JsPackageManager::Yarn,
                _ => return None,
            };
            Some((
                path_depth(&manifest.relative_path),
                PinnedPackageManager {
                    manager,
                    version: parse_package_manager_version(version)?,
                },
            ))
        })
        .collect::<Vec<_>>();
    let min_depth = candidates.iter().map(|(depth, _)| *depth).min()?;
    let mut pins = candidates
        .into_iter()
        .filter(|(depth, _)| *depth == min_depth)
        .map(|(_, pinned)| pinned);
    let first = pins.next()?;
    pins.all(|pinned| pinned == first).then_some(first)
}

fn parse_package_manager_version(value: &str) -> Option<(u64, u64, u64)> {
    let version = value
        .trim_start_matches('v')
        .split(['+', '-'])
        .next()
        .unwrap_or("");
    let mut components = version.split('.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next().unwrap_or("0").parse().ok()?;
    let patch = components.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn inferred_package_manager(project_files: &[ProjectFile]) -> Option<JsPackageManager> {
    let mut candidates = Vec::new();
    for file in project_files {
        let name = file
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(file.relative_path.as_str())
            .to_ascii_lowercase();
        let manager = match name.as_str() {
            "pnpm-lock.yaml" => JsPackageManager::Pnpm,
            "package-lock.json" | "npm-shrinkwrap.json" => JsPackageManager::Npm,
            "bun.lock" | "bun.lockb" => JsPackageManager::Bun,
            "yarn.lock" => JsPackageManager::Yarn,
            _ => continue,
        };
        candidates.push((path_depth(&file.relative_path), manager));
    }
    let min_depth = candidates.iter().map(|(depth, _)| *depth).min()?;
    let managers = candidates
        .into_iter()
        .filter(|(depth, _)| *depth == min_depth)
        .map(|(_, manager)| manager)
        .collect::<std::collections::HashSet<_>>();
    if managers.len() == 1 {
        managers.into_iter().next()
    } else {
        None
    }
}

fn path_depth(relative_path: &str) -> usize {
    relative_path.bytes().filter(|byte| *byte == b'/').count()
}

fn manifest_path_order(left: &PackageManifest, right: &PackageManifest) -> std::cmp::Ordering {
    path_depth(&left.relative_path)
        .cmp(&path_depth(&right.relative_path))
        .then_with(|| left.relative_path.cmp(&right.relative_path))
}

fn manager_supports_release_age(manager: JsPackageManager, version: (u64, u64, u64)) -> bool {
    version
        >= match manager {
            JsPackageManager::Pnpm => (10, 16, 0),
            JsPackageManager::Npm => (11, 10, 0),
            JsPackageManager::Bun => (1, 3, 0),
            JsPackageManager::Yarn => (4, 12, 0),
        }
}

fn manager_has_builtin_release_age(manager: JsPackageManager, version: (u64, u64, u64)) -> bool {
    match manager {
        JsPackageManager::Pnpm => version >= (11, 0, 0),
        JsPackageManager::Yarn => version >= (4, 12, 0),
        JsPackageManager::Npm | JsPackageManager::Bun => false,
    }
}

#[cfg(test)]
fn manifest_content_configures_release_age(content: &str) -> bool {
    let Ok(json) = serde_json::from_str::<Value>(content) else {
        return false;
    };
    json.get("renovate")
        .is_some_and(renovate_has_project_wide_release_age)
}

#[cfg(test)]
fn pnpm_workspace_configures_release_age(content: &str) -> bool {
    pnpm_workspace_release_age_setting(content) == ReleaseAgeSetting::Enabled
}

fn pnpm_workspace_release_age_setting(content: &str) -> ReleaseAgeSetting {
    unique_line_setting(content.lines().filter_map(|line| {
        if line.len() != line.trim_start().len() {
            return None;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            return None;
        }
        trimmed
            .strip_prefix("minimumReleaseAge")
            .and_then(|rest| rest.trim_start().strip_prefix(':'))
            .map(positive_number_scalar)
    }))
}

#[cfg(test)]
fn npmrc_configures_release_age(content: &str) -> bool {
    combine_setting_states([
        npm_release_age_setting(content),
        pnpm_legacy_npmrc_release_age_setting(content),
    ]) == ReleaseAgeSetting::Enabled
}

fn npm_release_age_setting(content: &str) -> ReleaseAgeSetting {
    npmrc_named_number_setting(content, "min-release-age")
}

fn pnpm_legacy_npmrc_release_age_setting(content: &str) -> ReleaseAgeSetting {
    npmrc_named_number_setting(content, "minimum-release-age")
}

fn npmrc_named_number_setting(content: &str, expected_key: &str) -> ReleaseAgeSetting {
    let mut result = ReleaseAgeSetting::Absent;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() == expected_key {
            // npm-style rc files use last-definition-wins semantics.
            result = if positive_number_scalar(value) {
                ReleaseAgeSetting::Enabled
            } else {
                ReleaseAgeSetting::DisabledOrInvalid
            };
        }
    }
    result
}

/// Read Bun's release-age setting from `[install]` or its dotted form.
#[cfg(test)]
fn bunfig_configures_release_age(content: &str) -> bool {
    bunfig_release_age_setting(content) == ReleaseAgeSetting::Enabled
}

fn bunfig_release_age_setting(content: &str) -> ReleaseAgeSetting {
    let mut in_install_section = false;
    let mut values = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(section) = trimmed.strip_prefix('[') {
            let section = section.split(']').next().unwrap_or("").trim();
            in_install_section = section == "install";
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if (in_install_section && key == "minimumReleaseAge") || key == "install.minimumReleaseAge"
        {
            values.push(positive_number_scalar(value));
        }
    }
    unique_line_setting(values)
}

#[cfg(test)]
fn yarnrc_configures_release_age(content: &str) -> bool {
    yarnrc_release_age_setting(content) == ReleaseAgeSetting::Enabled
}

fn yarnrc_release_age_setting(content: &str) -> ReleaseAgeSetting {
    unique_line_setting(content.lines().filter_map(|line| {
        if line.len() != line.trim_start().len() {
            return None;
        }
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            return None;
        }
        trimmed.split_once(':').and_then(|(key, value)| {
            (key.trim() == "npmMinimalAgeGate").then(|| positive_duration_scalar(value))
        })
    }))
}

fn unique_line_setting(values: impl IntoIterator<Item = bool>) -> ReleaseAgeSetting {
    let values = values.into_iter().collect::<Vec<_>>();
    match values.as_slice() {
        [] => ReleaseAgeSetting::Absent,
        [true] => ReleaseAgeSetting::Enabled,
        [false] => ReleaseAgeSetting::DisabledOrInvalid,
        _ => ReleaseAgeSetting::DisabledOrInvalid,
    }
}

fn clean_scalar(raw: &str) -> &str {
    raw.split('#')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches(',')
        .trim()
        .trim_matches(['\'', '"'])
}

fn positive_number_scalar(raw: &str) -> bool {
    clean_scalar(raw)
        .parse::<f64>()
        .is_ok_and(|number| number.is_finite() && number > 0.0)
}

fn positive_duration_scalar(raw: &str) -> bool {
    let value = clean_scalar(raw);
    let number_end = value
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_digit() || *character == '.')
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    if number_end == 0 {
        return false;
    }
    let Ok(number) = value[..number_end].parse::<f64>() else {
        return false;
    };
    if !number.is_finite() || number <= 0.0 {
        return false;
    }
    matches!(
        value[number_end..].trim().to_ascii_lowercase().as_str(),
        "" | "ms"
            | "s"
            | "m"
            | "h"
            | "d"
            | "w"
            | "second"
            | "seconds"
            | "minute"
            | "minutes"
            | "hour"
            | "hours"
            | "day"
            | "days"
            | "week"
            | "weeks"
    )
}

fn renovate_has_project_wide_release_age(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .get("minimumReleaseAge")
        .is_some_and(value_is_positive_duration)
    {
        return true;
    }

    object
        .get("packageRules")
        .and_then(Value::as_array)
        .is_some_and(|rules| {
            rules.iter().any(|rule| {
                let Some(rule) = rule.as_object() else {
                    return false;
                };
                let applies_to_subset = rule
                    .keys()
                    .any(|key| key.starts_with("match") || key.starts_with("exclude"));
                !applies_to_subset
                    && rule
                        .get("minimumReleaseAge")
                        .is_some_and(value_is_positive_duration)
            })
        })
}

fn value_is_positive_duration(value: &Value) -> bool {
    value
        .as_f64()
        .is_some_and(|number| number.is_finite() && number > 0.0)
        || value.as_str().is_some_and(positive_duration_scalar)
}

fn renovate_configures_release_age(content: &str) -> bool {
    if let Ok(value) = serde_json::from_str::<Value>(content) {
        return renovate_has_project_wide_release_age(&value);
    }

    content.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            return false;
        }
        // Conservative JSON5 fallback: accept only a near-root property.
        // A nested packageRules override may cover one dependency subset and
        // must not clear the repository-wide finding.
        if line.len() - line.trim_start().len() > 2 {
            return false;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            return false;
        };
        key.trim().trim_matches(['\'', '"']) == "minimumReleaseAge"
            && positive_duration_scalar(value)
    })
}

fn dependabot_configures_cooldown(content: &str) -> bool {
    let lines = content.lines().collect::<Vec<_>>();
    let Some((updates_index, updates_indent)) =
        lines.iter().enumerate().find_map(|(index, line)| {
            let trimmed = line.trim();
            (trimmed.trim_matches(['\'', '"']) == "updates:")
                .then_some((index, line.len() - line.trim_start().len()))
        })
    else {
        return false;
    };
    let updates_end = lines
        .iter()
        .enumerate()
        .skip(updates_index + 1)
        .find_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            let indent = line.len() - line.trim_start().len();
            (indent <= updates_indent).then_some(index)
        })
        .unwrap_or(lines.len());
    let Some(entry_indent) = lines[updates_index + 1..updates_end]
        .iter()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            trimmed
                .starts_with('-')
                .then_some(line.len() - trimmed.len())
        })
        .min()
    else {
        return false;
    };

    let entry_starts = lines[updates_index + 1..updates_end]
        .iter()
        .enumerate()
        .filter_map(|(offset, line)| {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            (indent == entry_indent && trimmed.starts_with('-'))
                .then_some(updates_index + 1 + offset)
        })
        .collect::<Vec<_>>();

    for (position, start) in entry_starts.iter().copied().enumerate() {
        let end = entry_starts
            .get(position + 1)
            .copied()
            .unwrap_or(updates_end);
        let entry = &lines[start..end];
        let npm_entry = entry.iter().any(|line| {
            let candidate = line.trim().trim_start_matches('-').trim();
            candidate
                .split_once(':')
                .filter(|(key, _)| key.trim().trim_matches(['\'', '"']) == "package-ecosystem")
                .is_some_and(|(_, value)| clean_scalar(value).eq_ignore_ascii_case("npm"))
        });
        if !npm_entry {
            continue;
        }

        if dependabot_entry_has_project_wide_cooldown(entry) {
            return true;
        }
    }
    false
}

fn dependabot_entry_has_project_wide_cooldown(lines: &[&str]) -> bool {
    fn setting<'a>(candidate: &'a str, expected_key: &str) -> Option<&'a str> {
        candidate
            .split_once(':')
            .filter(|(key, _)| key.trim().trim_matches(['\'', '"']) == expected_key)
            .map(|(_, value)| value)
    }

    fn positive_days(candidate: &str, expected_key: &str) -> bool {
        setting(candidate, expected_key)
            .and_then(|value| clean_scalar(value).parse::<u64>().ok())
            .is_some_and(|days| (1..=90).contains(&days))
    }

    fn includes_everything(value: &str) -> bool {
        value
            .trim_matches(|character| matches!(character, '[' | ']' | '{' | '}' | ' '))
            .split(',')
            .map(|item| clean_scalar(item.trim().trim_start_matches('-')))
            .any(|item| item == "*")
    }

    let mut cooldown_indent = None;
    let mut include_indent = None;
    let mut include_declared = false;
    let mut include_all = false;
    let mut default_days = false;
    let mut major_days = false;
    let mut minor_days = false;
    let mut patch_days = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if cooldown_indent.is_some_and(|base| indent <= base) {
            break;
        }
        if include_indent.is_some_and(|base| indent <= base) {
            include_indent = None;
        }
        let cooldown_value = trimmed.split_once(':').and_then(|(key, value)| {
            (key.trim().trim_matches(['\'', '"']) == "cooldown").then_some(value)
        });
        if let Some(rest) = cooldown_value {
            cooldown_indent = Some(indent);
            for candidate in rest
                .trim_matches(|character| matches!(character, '{' | '}' | ' '))
                .split(',')
            {
                default_days |= positive_days(candidate, "default-days");
                major_days |= positive_days(candidate, "semver-major-days");
                minor_days |= positive_days(candidate, "semver-minor-days");
                patch_days |= positive_days(candidate, "semver-patch-days");
                if let Some(value) = setting(candidate, "include") {
                    include_declared = true;
                    include_all |= includes_everything(value);
                }
            }
            continue;
        }
        if cooldown_indent.is_none_or(|base| indent <= base) {
            continue;
        }

        if let Some(value) = setting(trimmed, "include") {
            include_declared = true;
            include_indent = Some(indent);
            include_all |= includes_everything(value);
            continue;
        }
        if include_indent.is_some_and(|base| indent > base) {
            include_all |= includes_everything(trimmed);
            continue;
        }

        default_days |= positive_days(trimmed, "default-days");
        major_days |= positive_days(trimmed, "semver-major-days");
        minor_days |= positive_days(trimmed, "semver-minor-days");
        patch_days |= positive_days(trimmed, "semver-patch-days");
    }

    let covers_update_types = default_days || (major_days && minor_days && patch_days);
    let covers_dependencies = !include_declared || include_all;
    covers_update_types && covers_dependencies
}

#[cfg(test)]
mod tests;
