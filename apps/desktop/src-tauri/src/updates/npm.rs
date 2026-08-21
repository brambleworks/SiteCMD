//! npm, yarn, and pnpm dependency census across root and workspace manifests.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::npm_lockfiles::LockVersions;
use super::types::{Ecosystem, InstalledPackage};

/// One manifest's declared dependency names, tagged with the lockfile importer
/// key that manifest corresponds to.
struct DeclaredDeps {
    importer: Option<String>,
    /// Display label for the manifest's location, relative to the scanned
    /// project root (`.` for the root manifest). `None` outside a workspace,
    /// where "which package declares this" has only one answer and naming it
    /// would be noise.
    member: Option<String>,
    deps: Vec<String>,
    dev_deps: Vec<String>,
}

/// Parse npm/yarn/pnpm project to get installed packages.
/// Strategy: read the root package.json plus every workspace member manifest
/// for dependency names, then resolve versions from lockfiles.
pub fn parse(dir: &Path) -> Vec<InstalledPackage> {
    let pkg_json = match read_package_json(dir) {
        Some(v) => v,
        None => return Vec::new(),
    };

    // The scanned directory first, then the workspace members it declares.
    let mut manifests: Vec<(PathBuf, serde_json::Value)> = vec![(dir.to_path_buf(), pkg_json)];
    let root_index = 0;
    for member in super::npm_workspaces::member_dirs(dir, &manifests[root_index].1) {
        if let Some(json) = read_package_json(&member) {
            manifests.push((member, json));
        }
    }
    // A single-package project has exactly one manifest, so labelling every
    // row with its location would add noise and no information.
    let is_workspace = manifests.len() > 1;

    // Look for a lockfile at this directory, then walk up. pnpm workspaces
    // (and npm/yarn workspaces) keep a single lockfile at the workspace root,
    // which is several levels above any individual package.json.
    let lockfile_dir = find_lockfile_dir(dir);

    let parsed = lockfile_dir
        .as_ref()
        .and_then(|lockdir| super::npm_lockfiles::read(lockdir));

    let Some((versions, source)) = parsed else {
        // No lockfile - can't determine installed versions from package.json ranges alone.
        // Lockfiles are the source of truth for pinned versions.
        tracing::warn!(
            "No npm/yarn/pnpm lockfile found in {} or any parent up to the repo root - npm updates skipped",
            dir.display()
        );
        return Vec::new();
    };

    // Safe to unwrap: `parsed` is Some only when `lockfile_dir` was.
    let lockdir = lockfile_dir.expect("lockfile dir present when a lockfile parsed");
    let declared: Vec<DeclaredDeps> = manifests
        .iter()
        .map(|(manifest_dir, json)| {
            let (deps, dev_deps) = extract_declared_deps(json);
            DeclaredDeps {
                importer: importer_key(&lockdir, manifest_dir),
                member: is_workspace
                    .then(|| relative_label(dir, manifest_dir))
                    .flatten(),
                deps,
                dev_deps,
            }
        })
        .collect();

    resolve(&declared, &versions, source)
}

/// A manifest directory as a lockfile importer key: `/`-separated and relative
/// to the lockfile, with the lockfile's own directory keyed `.` the way pnpm
/// writes it.
fn importer_key(lock_dir: &Path, manifest_dir: &Path) -> Option<String> {
    relative_label(lock_dir, manifest_dir)
}

/// `manifest_dir` as a `/`-separated path relative to `base`, with `base`
/// itself rendered as `.`.
fn relative_label(base: &Path, manifest_dir: &Path) -> Option<String> {
    let rel = manifest_dir.strip_prefix(base).ok()?;
    let key = rel.to_str()?.replace('\\', "/");
    Some(if key.is_empty() { ".".to_string() } else { key })
}

/// Walk up from `start` looking for an npm/yarn/pnpm lockfile.
/// Stops at the first directory containing one, or at the git repo root,
/// or after a bounded depth. Workspaces typically need 1-3 levels.
fn find_lockfile_dir(start: &Path) -> Option<std::path::PathBuf> {
    const MAX_DEPTH: usize = 8;
    let mut current = start.to_path_buf();
    for _ in 0..MAX_DEPTH {
        if current.join("package-lock.json").exists()
            || current.join("yarn.lock").exists()
            || current.join("pnpm-lock.yaml").exists()
        {
            return Some(current);
        }
        // Stop at the repo root - if we hit `.git` without finding a lockfile,
        // there isn't one in this project.
        if current.join(".git").exists() {
            return None;
        }
        current = current.parent()?.to_path_buf();
    }
    None
}

fn read_package_json(dir: &Path) -> Option<serde_json::Value> {
    let content = super::read_dependency_file(&dir.join("package.json"))?;
    serde_json::from_str(&content).ok()
}

fn extract_declared_deps(pkg: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let deps = pkg
        .get("dependencies")
        .and_then(|d| d.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();

    let dev_deps = pkg
        .get("devDependencies")
        .and_then(|d| d.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();

    (deps, dev_deps)
}

/// Resolve declared dependencies into one row per package across the workspace.
/// Runtime declarations take precedence over development-only declarations.
fn resolve(
    declared: &[DeclaredDeps],
    versions: &LockVersions,
    source: &str,
) -> Vec<InstalledPackage> {
    // Every member declaring a given package, in manifest order, so a shared
    // dependency's single row still names all the places to upgrade it.
    let mut members: HashMap<&str, Vec<String>> = HashMap::new();
    for manifest in declared {
        let Some(member) = manifest.member.as_ref() else {
            continue;
        };
        for name in manifest.deps.iter().chain(manifest.dev_deps.iter()) {
            let entry = members.entry(name.as_str()).or_default();
            if !entry.iter().any(|existing| existing == member) {
                entry.push(member.clone());
            }
        }
    }

    let mut result = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();

    for is_dev in [false, true] {
        for manifest in declared {
            let names = if is_dev {
                &manifest.dev_deps
            } else {
                &manifest.deps
            };
            for name in names {
                if seen.contains(name.as_str()) {
                    continue;
                }
                let Some(version) = versions.lookup(manifest.importer.as_ref(), name) else {
                    continue;
                };
                seen.insert(name.as_str());
                result.push(InstalledPackage {
                    name: name.clone(),
                    version: version.clone(),
                    ecosystem: Ecosystem::Npm,
                    source: source.into(),
                    is_dev,
                    workspace_members: members.get(name.as_str()).cloned().unwrap_or_default(),
                });
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_package_lock_v2() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(
            dir.join("package.json"),
            r#"{"dependencies":{"react":"^18.0.0","next":"^14.0.0"},"devDependencies":{"typescript":"^5.0.0"}}"#,
        ).unwrap();
        fs::write(
            dir.join("package-lock.json"),
            r#"{
                "lockfileVersion": 3,
                "packages": {
                    "": {"name": "myapp"},
                    "node_modules/react": {"version": "18.2.0"},
                    "node_modules/next": {"version": "14.1.0"},
                    "node_modules/typescript": {"version": "5.3.3"},
                    "node_modules/react/node_modules/loose-envify": {"version": "1.4.0"}
                }
            }"#,
        )
        .unwrap();

        let result = parse(dir);
        assert_eq!(result.len(), 3);

        let react = result.iter().find(|p| p.name == "react").unwrap();
        assert_eq!(react.version, "18.2.0");
        assert!(!react.is_dev);

        let ts = result.iter().find(|p| p.name == "typescript").unwrap();
        assert_eq!(ts.version, "5.3.3");
        assert!(ts.is_dev);

        // Nested dep should NOT be included
        assert!(result.iter().find(|p| p.name == "loose-envify").is_none());
    }

    #[test]
    fn test_package_lock_v1() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(
            dir.join("package.json"),
            r#"{"dependencies":{"express":"^4.18.0"}}"#,
        )
        .unwrap();
        fs::write(
            dir.join("package-lock.json"),
            r#"{"lockfileVersion": 1, "dependencies": {"express": {"version": "4.18.2"}}}"#,
        )
        .unwrap();

        let result = parse(dir);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "express");
        assert_eq!(result[0].version, "4.18.2");
    }

    #[test]
    fn test_yarn_lock() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(
            dir.join("package.json"),
            r#"{"dependencies":{"lodash":"^4.17.0","@babel/core":"^7.0.0"}}"#,
        )
        .unwrap();
        fs::write(
            dir.join("yarn.lock"),
            r#"# yarn lockfile v1

lodash@^4.17.0:
  version "4.17.21"
  resolved "https://registry.yarnpkg.com/lodash/-/lodash-4.17.21.tgz"

"@babel/core@^7.0.0":
  version "7.23.7"
  resolved "https://registry.yarnpkg.com/@babel/core/-/core-7.23.7.tgz"
"#,
        )
        .unwrap();

        let result = parse(dir);
        assert_eq!(result.len(), 2);

        let lodash = result.iter().find(|p| p.name == "lodash").unwrap();
        assert_eq!(lodash.version, "4.17.21");

        let babel = result.iter().find(|p| p.name == "@babel/core").unwrap();
        assert_eq!(babel.version, "7.23.7");
    }

    #[test]
    fn test_pnpm_lock() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(
            dir.join("package.json"),
            r#"{"dependencies":{"vue":"^3.0.0"},"devDependencies":{"vite":"^5.0.0"}}"#,
        )
        .unwrap();
        fs::write(
            dir.join("pnpm-lock.yaml"),
            r#"lockfileVersion: '9.0'

dependencies:
  vue:
    specifier: ^3.0.0
    version: 3.4.15

devDependencies:
  vite:
    specifier: ^5.0.0
    version: 5.0.12
"#,
        )
        .unwrap();

        let result = parse(dir);
        assert_eq!(result.len(), 2);

        let vue = result.iter().find(|p| p.name == "vue").unwrap();
        assert_eq!(vue.version, "3.4.15");
        assert!(!vue.is_dev);

        let vite = result.iter().find(|p| p.name == "vite").unwrap();
        assert_eq!(vite.version, "5.0.12");
        assert!(vite.is_dev);
    }

    #[test]
    fn test_no_lockfile_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(
            dir.join("package.json"),
            r#"{"dependencies":{"react":"^18.0.0"}}"#,
        )
        .unwrap();

        let result = parse(dir);
        assert!(result.is_empty(), "Should return empty without a lockfile");
    }

    #[test]
    fn test_pnpm_workspace_lockfile_at_repo_root() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let pkg_dir = root.join("apps/sitecmd.com");
        fs::create_dir_all(&pkg_dir).unwrap();

        fs::write(
            pkg_dir.join("package.json"),
            r#"{"dependencies":{"vite":"^7.3.3"}}"#,
        )
        .unwrap();

        // pnpm-lock.yaml sits at the workspace root, two levels up
        fs::write(
            root.join("pnpm-lock.yaml"),
            "importers:\n  apps/sitecmd.com:\n    dependencies:\n      vite:\n        version: 7.3.3\n",
        )
        .unwrap();

        let result = parse(&pkg_dir);
        assert_eq!(result.len(), 1, "Should walk up to find the lockfile");
        assert_eq!(result[0].name, "vite");
        assert_eq!(result[0].version, "7.3.3");
    }

    #[test]
    fn test_pnpm_workspace_members_are_scanned() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();

        fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"typescript":"~6.0.3"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - \"apps/*\"\n",
        )
        .unwrap();

        let desktop = root.join("apps/desktop");
        fs::create_dir_all(&desktop).unwrap();
        fs::write(
            desktop.join("package.json"),
            r#"{"dependencies":{"react":"^19.2.8"},"devDependencies":{"vite":"^8.1.5"}}"#,
        )
        .unwrap();

        fs::write(
            root.join("pnpm-lock.yaml"),
            "importers:\n  .:\n    devDependencies:\n      typescript:\n        version: 6.0.3\n  apps/desktop:\n    dependencies:\n      react:\n        version: 19.2.8\n    devDependencies:\n      vite:\n        version: 8.1.5\n",
        )
        .unwrap();

        let result = parse(root);
        let names: Vec<&str> = result.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"typescript"), "root dep missing: {names:?}");
        assert!(
            names.contains(&"react"),
            "workspace member dep missing: {names:?}"
        );
        assert!(
            names.contains(&"vite"),
            "workspace member devDep missing: {names:?}"
        );

        let react = result.iter().find(|p| p.name == "react").unwrap();
        assert_eq!(react.version, "19.2.8");
        assert!(!react.is_dev);
        assert!(result.iter().find(|p| p.name == "vite").unwrap().is_dev);
    }

    /// npm/yarn declare members in package.json rather than a pnpm file.
    #[test]
    fn test_npm_workspaces_field_members_are_scanned() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();

        fs::write(
            root.join("package.json"),
            r#"{"workspaces":["packages/*"],"devDependencies":{"typescript":"^5.0.0"}}"#,
        )
        .unwrap();
        let api = root.join("packages/api");
        fs::create_dir_all(&api).unwrap();
        fs::write(
            api.join("package.json"),
            r#"{"dependencies":{"express":"^4.18.0"}}"#,
        )
        .unwrap();

        fs::write(
            root.join("package-lock.json"),
            r#"{
                "lockfileVersion": 3,
                "packages": {
                    "": {"name": "root"},
                    "node_modules/typescript": {"version": "5.3.3"},
                    "node_modules/express": {"version": "4.18.2"}
                }
            }"#,
        )
        .unwrap();

        let result = parse(root);
        let express = result
            .iter()
            .find(|p| p.name == "express")
            .expect("workspace member dep should be scanned");
        assert_eq!(express.version, "4.18.2");
        assert!(!express.is_dev);
    }

    /// A package declared by two members must produce one row, not one per
    /// member, so the Updates page does not repeat the same upgrade.
    #[test]
    fn test_shared_workspace_dependency_is_reported_once() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();

        fs::write(root.join("package.json"), r#"{"workspaces":["apps/*"]}"#).unwrap();
        for member in ["apps/one", "apps/two"] {
            let dir = root.join(member);
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("package.json"),
                r#"{"devDependencies":{"typescript":"~6.0.3"}}"#,
            )
            .unwrap();
        }
        fs::write(
            root.join("pnpm-lock.yaml"),
            "importers:\n  apps/one:\n    devDependencies:\n      typescript:\n        version: 6.0.3\n  apps/two:\n    devDependencies:\n      typescript:\n        version: 6.0.3\n",
        )
        .unwrap();

        let result = parse(root);
        assert_eq!(
            result.iter().filter(|p| p.name == "typescript").count(),
            1,
            "shared dependency should collapse to one row: {result:?}"
        );
        // The one row still has to name every place the upgrade applies.
        let typescript = result.iter().find(|p| p.name == "typescript").unwrap();
        assert_eq!(typescript.workspace_members, vec!["apps/one", "apps/two"]);
    }

    /// The Updates page has to say *where* an upgrade applies: in a monorepo
    /// "better-sqlite3 12 -> 13" is ambiguous without the declaring member.
    #[test]
    fn test_workspace_members_are_attributed() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();

        fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"typescript":"~6.0.3"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - \"apps/*\"\n",
        )
        .unwrap();

        let mcp = root.join("apps/mcp-server");
        fs::create_dir_all(&mcp).unwrap();
        fs::write(
            mcp.join("package.json"),
            r#"{"devDependencies":{"better-sqlite3":"^12.11.1"}}"#,
        )
        .unwrap();

        fs::write(
            root.join("pnpm-lock.yaml"),
            "importers:\n  .:\n    devDependencies:\n      typescript:\n        version: 6.0.3\n  apps/mcp-server:\n    devDependencies:\n      better-sqlite3:\n        version: 12.11.1\n",
        )
        .unwrap();

        let result = parse(root);
        let sqlite = result.iter().find(|p| p.name == "better-sqlite3").unwrap();
        assert_eq!(sqlite.workspace_members, vec!["apps/mcp-server"]);
        // The root manifest is a location too, and pnpm already calls it ".".
        let typescript = result.iter().find(|p| p.name == "typescript").unwrap();
        assert_eq!(typescript.workspace_members, vec!["."]);
    }

    /// A single-package project has one possible answer, so labelling every
    /// row with its location would be noise. Guards the UI against rendering
    /// a pointless "." on every non-monorepo scan.
    #[test]
    fn test_single_package_project_has_no_member_labels() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(
            dir.join("package.json"),
            r#"{"dependencies":{"express":"^4.18.0"}}"#,
        )
        .unwrap();
        fs::write(
            dir.join("package-lock.json"),
            r#"{"lockfileVersion": 3, "packages": {"node_modules/express": {"version": "4.18.2"}}}"#,
        )
        .unwrap();

        let result = parse(dir);
        assert!(result[0].workspace_members.is_empty());
    }

    /// pnpm records a block per member, so a member pinning a different major
    /// than its siblings must resolve to its own version instead of whichever
    /// block the flat merge wrote last.
    #[test]
    fn test_member_version_beats_flat_merge_order() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();

        fs::write(root.join("package.json"), r#"{"workspaces":["apps/*"]}"#).unwrap();
        let legacy = root.join("apps/legacy");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(
            legacy.join("package.json"),
            r#"{"dependencies":{"react":"^17.0.0"}}"#,
        )
        .unwrap();

        // apps/modern is listed last, so a flat merge would report react 19
        // for every member including apps/legacy.
        fs::write(
            root.join("pnpm-lock.yaml"),
            "importers:\n  apps/legacy:\n    dependencies:\n      react:\n        version: 17.0.2\n  apps/modern:\n    dependencies:\n      react:\n        version: 19.2.8\n",
        )
        .unwrap();

        let result = parse(root);
        let react = result.iter().find(|p| p.name == "react").unwrap();
        assert_eq!(react.version, "17.0.2");
    }

    /// Workspace-internal `link:` entries are local packages with no
    /// published release, so they must not become update rows.
    #[test]
    fn test_workspace_link_dependencies_are_skipped() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();

        fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"@local/pricing":"workspace:*","react":"^19.2.8"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("pnpm-lock.yaml"),
            "importers:\n  .:\n    dependencies:\n      '@local/pricing':\n        version: link:../../packages/pricing\n      react:\n        version: 19.2.8\n",
        )
        .unwrap();

        let result = parse(root);
        let names: Vec<&str> = result.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["react"]);
    }

    /// The `snapshots:` section also nests `dependencies:` maps. Reading them
    /// as project dependencies would inject the entire transitive graph.
    #[test]
    fn test_snapshots_section_is_not_read_as_project_deps() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();

        fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"react":"^19.2.8"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("pnpm-lock.yaml"),
            "importers:\n  .:\n    dependencies:\n      react:\n        version: 19.2.8\n\npackages:\n\n  react@19.2.8:\n    resolution: {integrity: sha512-abc}\n\nsnapshots:\n\n  react@19.2.8:\n    dependencies:\n      scheduler:\n        version: 0.23.0\n",
        )
        .unwrap();

        let result = parse(root);
        let names: Vec<&str> = result.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["react"]);
    }

    #[test]
    fn test_lockfile_walk_stops_at_git_repo_boundary() {
        let outer = tempfile::tempdir().unwrap();
        let outer = outer.path();
        // Outer dir has a lockfile (an unrelated project up the tree)
        fs::write(
            outer.join("pnpm-lock.yaml"),
            "dependencies:\n  react:\n    version: 1.0.0\n",
        )
        .unwrap();

        // Inner dir is its own git repo with no lockfile
        let inner = outer.join("inner-repo");
        fs::create_dir_all(inner.join(".git")).unwrap();
        fs::write(
            inner.join("package.json"),
            r#"{"dependencies":{"react":"^18.0.0"}}"#,
        )
        .unwrap();

        let result = parse(&inner);
        assert!(
            result.is_empty(),
            "Walk should not escape past .git into unrelated projects",
        );
    }
}
