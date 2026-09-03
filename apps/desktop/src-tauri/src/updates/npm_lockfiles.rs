//! Reads installed versions from npm ecosystem lockfiles.
//! Priority: `package-lock.json`, `yarn.lock`, then `pnpm-lock.yaml`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Installed lockfile versions. `flat` covers hoisted trees; `by_importer`
/// preserves workspace-member-specific versions; `linked` holds workspace
/// links, which resolve to a local directory and so have no released version.
#[derive(Default)]
pub(super) struct LockVersions {
    pub(super) flat: HashMap<String, String>,
    pub(super) by_importer: HashMap<String, HashMap<String, String>>,
    pub(super) linked: HashSet<String>,
}

impl LockVersions {
    fn from_flat(flat: HashMap<String, String>) -> Self {
        Self {
            flat,
            ..Self::default()
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

    /// Every package name the lockfile resolves for `importer`: hoisted
    /// entries, that member's own installs, and workspace links. Version
    /// comparison has no answer for a link, but "is it resolved" does.
    pub(super) fn resolved_names(&self, importer: Option<&String>) -> HashSet<String> {
        let mut names = self.flat.keys().cloned().collect::<HashSet<_>>();
        names.extend(self.linked.iter().cloned());
        if let Some(scoped) = importer.and_then(|key| self.by_importer.get(key)) {
            names.extend(scoped.keys().cloned());
        }
        names
    }
}

/// Read the highest-priority lockfile present in `dir`, with the filename it
/// came from for `InstalledPackage::source`.
pub(super) fn read(dir: &Path) -> Option<(LockVersions, &'static str)> {
    parse_package_lock(dir)
        .map(|versions| (versions, "package-lock.json"))
        .or_else(|| {
            parse_yarn_lock(dir).map(|versions| (LockVersions::from_flat(versions), "yarn.lock"))
        })
        .or_else(|| parse_pnpm_lock(dir).map(|versions| (versions, "pnpm-lock.yaml")))
}

/// Parse package-lock.json to extract exact installed versions.
/// Fallback: use version ranges from package.json (strip range prefixes).
fn parse_package_lock(dir: &Path) -> Option<LockVersions> {
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
        parse_package_lock_v1(&lock).map(LockVersions::from_flat)
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

fn parse_package_lock_v2(lock: &serde_json::Value) -> Option<LockVersions> {
    let packages = lock.get("packages")?.as_object()?;
    let mut versions = LockVersions::default();

    for (key, info) in packages {
        // Keys are "node_modules/pkg" (hoisted), "<member>/node_modules/pkg"
        // (installed inside one workspace member), "" for the root package, or
        // "<member>" for a workspace member's own manifest.
        let Some((importer, name)) = split_node_modules_key(key) else {
            continue;
        };
        // A nested copy ("node_modules/a/node_modules/b") belongs to another
        // package rather than to any workspace member.
        if importer.is_some_and(is_nested_package_dir) {
            continue;
        }
        if !is_top_level_package_name(name) {
            continue;
        }

        // A workspace link resolves to a local directory, so it has no
        // published version to compare, but it is installed all the same.
        if info.get("link").and_then(|v| v.as_bool()).unwrap_or(false) {
            versions.linked.insert(name.to_string());
            continue;
        }

        let Some(ver) = info.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        match importer {
            Some(importer) => {
                versions
                    .by_importer
                    .entry(importer.to_string())
                    .or_default()
                    .insert(name.to_string(), ver.to_string());
            }
            None => {
                versions.flat.insert(name.to_string(), ver.to_string());
            }
        }
    }

    Some(versions)
}

/// Split a v2/v3 `packages` key into the importer directory that owns the
/// install (`None` when hoisted at the lockfile root) and the package name.
fn split_node_modules_key(key: &str) -> Option<(Option<&str>, &str)> {
    const MARKER: &str = "node_modules/";
    let index = key.rfind(MARKER)?;
    let name = &key[index + MARKER.len()..];
    if name.is_empty() {
        return None;
    }
    if index == 0 {
        return Some((None, name));
    }
    let importer = key[..index].trim_end_matches('/');
    (!importer.is_empty()).then_some((Some(importer), name))
}

/// Whether an importer path is itself inside a dependency tree.
fn is_nested_package_dir(importer: &str) -> bool {
    importer == "node_modules"
        || importer.starts_with("node_modules/")
        || importer.contains("/node_modules/")
        || importer.ends_with("/node_modules")
}

/// Whether a package name is a whole package rather than a subpath.
fn is_top_level_package_name(name: &str) -> bool {
    let slash_count = name.matches('/').count();
    if name.starts_with('@') {
        slash_count == 1
    } else {
        slash_count == 0
    }
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
            // version to compare against, though they are still installed.
            if clean_ver.starts_with("link:") {
                versions.linked.insert(name.clone());
                continue;
            }
            if clean_ver.is_empty() {
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

    // A lockfile that resolved no versioned package is not one this parser
    // understood. Keeping that judgement on `flat` alone leaves the Updates
    // page's "no lockfile I can read" path exactly where it was; a workspace
    // whose only entries are links has no drift to measure either.
    if versions.flat.is_empty() {
        None
    } else {
        Some(versions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// npm v2/v3 records a workspace link with no version and a member-local
    /// install under the member's own directory. Both are resolved packages,
    /// so a manifest declaring them is not out of sync with the lockfile.
    #[test]
    fn v3_links_and_member_local_installs_resolve() {
        let lock: serde_json::Value = serde_json::from_str(
            r#"{
              "lockfileVersion": 3,
              "packages": {
                "": { "name": "root" },
                "packages/lib": { "name": "@acme/lib", "version": "0.0.0" },
                "node_modules/@acme/lib": { "resolved": "packages/lib", "link": true },
                "node_modules/react": { "version": "19.0.0" },
                "apps/docs/node_modules/next": { "version": "15.1.0" },
                "node_modules/react/node_modules/loose-envify": { "version": "1.4.0" }
              }
            }"#,
        )
        .unwrap();

        let versions = parse_package_lock_v2(&lock).expect("v3 packages map parses");
        assert!(versions.linked.contains("@acme/lib"));
        assert_eq!(
            versions.flat.get("react").map(String::as_str),
            Some("19.0.0")
        );
        // A member-local install belongs to that member, not to the root.
        assert!(!versions.flat.contains_key("next"));
        assert_eq!(
            versions.by_importer["apps/docs"]
                .get("next")
                .map(String::as_str),
            Some("15.1.0")
        );
        // Nested transitive copies still belong to their parent package.
        assert!(!versions.flat.contains_key("loose-envify"));
        assert!(!versions.by_importer.contains_key("node_modules/react"));

        let docs = "apps/docs".to_string();
        let resolved = versions.resolved_names(Some(&docs));
        assert!(resolved.contains("@acme/lib"), "{resolved:?}");
        assert!(resolved.contains("next"), "{resolved:?}");
        assert!(resolved.contains("react"), "{resolved:?}");
        // Another member does not inherit apps/docs' own install.
        let other = "apps/web".to_string();
        assert!(!versions.resolved_names(Some(&other)).contains("next"));
    }

    #[test]
    fn node_modules_keys_split_into_importer_and_package() {
        assert_eq!(
            split_node_modules_key("node_modules/@scope/name"),
            Some((None, "@scope/name"))
        );
        assert_eq!(
            split_node_modules_key("apps/docs/node_modules/next"),
            Some((Some("apps/docs"), "next"))
        );
        assert_eq!(
            split_node_modules_key("node_modules/a/node_modules/b"),
            Some((Some("node_modules/a"), "b"))
        );
        assert_eq!(split_node_modules_key("packages/lib"), None);
        assert_eq!(split_node_modules_key(""), None);
    }

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
