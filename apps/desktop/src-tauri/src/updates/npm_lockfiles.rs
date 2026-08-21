//! Reads installed versions from npm ecosystem lockfiles.
//! Priority: `package-lock.json`, `yarn.lock`, then `pnpm-lock.yaml`.

use std::collections::HashMap;
use std::path::Path;

/// Installed lockfile versions. `flat` covers hoisted trees; `by_importer`
/// preserves pnpm workspace-specific versions.
#[derive(Default)]
pub(super) struct LockVersions {
    pub(super) flat: HashMap<String, String>,
    pub(super) by_importer: HashMap<String, HashMap<String, String>>,
}

impl LockVersions {
    fn from_flat(flat: HashMap<String, String>) -> Self {
        Self {
            flat,
            by_importer: HashMap::new(),
        }
    }

    /// The version of `name` as seen by the member at `importer`, falling back
    /// to the repo-wide map when that member has no block of its own.
    pub(super) fn lookup(&self, importer: Option<&String>, name: &str) -> Option<&String> {
        importer
            .and_then(|key| self.by_importer.get(key))
            .and_then(|scoped| scoped.get(name))
            .or_else(|| self.flat.get(name))
    }
}

/// Read the highest-priority lockfile present in `dir`, with the filename it
/// came from for `InstalledPackage::source`.
pub(super) fn read(dir: &Path) -> Option<(LockVersions, &'static str)> {
    parse_package_lock(dir)
        .map(|versions| (LockVersions::from_flat(versions), "package-lock.json"))
        .or_else(|| {
            parse_yarn_lock(dir).map(|versions| (LockVersions::from_flat(versions), "yarn.lock"))
        })
        .or_else(|| parse_pnpm_lock(dir).map(|versions| (versions, "pnpm-lock.yaml")))
}

/// Parse package-lock.json to extract exact installed versions.
/// Fallback: use version ranges from package.json (strip range prefixes).
fn parse_package_lock(dir: &Path) -> Option<HashMap<String, String>> {
    let content = super::read_dependency_file(&dir.join("package-lock.json"))?;
    let lock: serde_json::Value = serde_json::from_str(&content).ok()?;

    let version = lock
        .get("lockfileVersion")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);

    if version >= 2 {
        // v2/v3: "packages" object with "" (root) + "node_modules/pkg" keys
        parse_package_lock_v2(&lock)
    } else {
        // v1: "dependencies" object with nested structure
        parse_package_lock_v1(&lock)
    }
}

fn parse_package_lock_v1(lock: &serde_json::Value) -> Option<HashMap<String, String>> {
    let deps = lock.get("dependencies")?.as_object()?;
    let mut versions = HashMap::new();

    for (name, info) in deps {
        if let Some(ver) = info.get("version").and_then(|v| v.as_str()) {
            versions.insert(name.clone(), ver.to_string());
        }
    }

    Some(versions)
}

fn parse_package_lock_v2(lock: &serde_json::Value) -> Option<HashMap<String, String>> {
    let packages = lock.get("packages")?.as_object()?;
    let mut versions = HashMap::new();

    for (key, info) in packages {
        // Keys are "node_modules/package-name" or "" for root
        let name = if key.starts_with("node_modules/") {
            // Handle scoped packages: "node_modules/@scope/name"
            key.strip_prefix("node_modules/").unwrap_or(key)
        } else if key.is_empty() {
            continue; // root package
        } else {
            continue; // unknown key format
        };

        // Skip nested dependencies (node_modules/a/node_modules/b)
        let slash_count = name.matches('/').count();
        let is_scoped = name.starts_with('@');
        if (is_scoped && slash_count > 1) || (!is_scoped && slash_count > 0) {
            continue;
        }

        if let Some(ver) = info.get("version").and_then(|v| v.as_str()) {
            versions.insert(name.to_string(), ver.to_string());
        }
    }

    Some(versions)
}

fn parse_yarn_lock(dir: &Path) -> Option<HashMap<String, String>> {
    let content = super::read_dependency_file(&dir.join("yarn.lock"))?;
    let mut versions = HashMap::new();

    // Yarn v1 headers may be quoted; the following indented version line holds
    // the resolved version.
    let mut current_names: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            current_names.clear();
            continue;
        }

        // Header line: "name@version", "name@version":, or name@version:
        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.contains('@') {
            current_names.clear();
            let header = trimmed.trim_end_matches(':').trim_matches('"');

            // Can have multiple specs: "name@^1.0.0, name@^1.2.0":
            for spec in header.split(", ") {
                let spec = spec.trim().trim_matches('"');
                if let Some(name) = extract_package_name_from_spec(spec) {
                    current_names.push(name);
                }
            }
        }

        // version line
        if (line.starts_with("  ") || line.starts_with('\t')) && trimmed.starts_with("version ") {
            let ver = trimmed
                .trim_start_matches("version ")
                .trim_matches('"')
                .trim_matches('\'');

            for name in &current_names {
                versions
                    .entry(name.clone())
                    .or_insert_with(|| ver.to_string());
            }
            current_names.clear();
        }
    }

    if versions.is_empty() {
        None
    } else {
        Some(versions)
    }
}

/// Extract package name from "name@version" spec
fn extract_package_name_from_spec(spec: &str) -> Option<String> {
    if let Some(after_scope) = spec.strip_prefix('@') {
        // Scoped: @scope/name@version
        if let Some(slash_pos) = after_scope.find('/') {
            let after_slash = &after_scope[slash_pos + 1..];
            if let Some(at_pos) = after_slash.find('@') {
                Some(format!("@{}", &after_scope[..slash_pos + 1 + at_pos]))
            } else {
                Some(format!("@{}", after_scope))
            }
        } else {
            None
        }
    } else {
        // Unscoped: name@version
        spec.find('@').map(|pos| spec[..pos].to_string())
    }
}
fn parse_pnpm_lock(dir: &Path) -> Option<LockVersions> {
    let content = super::read_dependency_file(&dir.join("pnpm-lock.yaml"))?;
    let mut versions = LockVersions::default();

    // pnpm v9 dependency blocks are nested four spaces below `importers`;
    // pnpm v6 blocks are top-level.
    let uses_importers = content.lines().any(|l| l.trim() == "importers:");

    let section_indent: usize = if uses_importers { 4 } else { 0 };
    let pkg_indent: usize = if uses_importers { 6 } else { 2 };
    let prop_indent: usize = pkg_indent + 2;

    let dep_markers = ["dependencies:", "devDependencies:", "optionalDependencies:"];

    // Without importers every dependency belongs to the single root project.
    let mut in_importers = !uses_importers;
    let mut current_importer = ".".to_string();
    let mut in_dep_section = false;
    let mut current_pkg: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        // Top-level key: only `importers:` holds per-project dependency blocks.
        // `packages:` and `snapshots:` describe the resolved graph, and their
        // nested `dependencies:` maps must not be read as project deps.
        if uses_importers && indent == 0 {
            in_importers = trimmed == "importers:";
            in_dep_section = false;
            current_pkg = None;
            continue;
        }
        if !in_importers {
            continue;
        }

        // Importer key line: `.:`, `apps/desktop:`
        if uses_importers && indent == 2 && trimmed.ends_with(':') {
            current_importer = trimmed
                .trim_end_matches(':')
                .trim_matches('\'')
                .trim_matches('"')
                .to_string();
            in_dep_section = false;
            current_pkg = None;
            continue;
        }

        // Dependency section header, or any other key at that level closing one.
        if indent == section_indent {
            in_dep_section = dep_markers.contains(&trimmed);
            current_pkg = None;
            continue;
        }
        if !in_dep_section {
            continue;
        }

        // Package name line
        if indent == pkg_indent && trimmed.ends_with(':') {
            let name = trimmed
                .trim_end_matches(':')
                .trim_matches('\'')
                .trim_matches('"');
            current_pkg = Some(name.to_string());
        } else if indent == prop_indent {
            // Property line: "version: X.Y.Z" or "specifier: ^X.Y.Z"
            let (Some(name), Some(rest)) = (current_pkg.as_ref(), trimmed.strip_prefix("version:"))
            else {
                continue;
            };
            let ver = rest.trim().trim_matches('\'').trim_matches('"');
            // pnpm adds suffixes like "18.2.0(react@18.2.0)" - strip them
            let clean_ver = ver.split('(').next().unwrap_or(ver).trim();
            // Workspace-internal links ("link:../../packages/pricing") are
            // local packages, not registry releases - there is no published
            // version to compare against.
            if clean_ver.is_empty() || clean_ver.starts_with("link:") {
                continue;
            }
            versions.flat.insert(name.clone(), clean_ver.to_string());
            versions
                .by_importer
                .entry(current_importer.clone())
                .or_default()
                .insert(name.clone(), clean_ver.to_string());
        }
    }

    if versions.flat.is_empty() {
        None
    } else {
        Some(versions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_and_unscoped_yarn_specs_yield_package_names() {
        assert_eq!(
            extract_package_name_from_spec("@babel/core@^7.0.0"),
            Some("@babel/core".into())
        );
        assert_eq!(
            extract_package_name_from_spec("lodash@^4.17.0"),
            Some("lodash".into())
        );
    }
}
