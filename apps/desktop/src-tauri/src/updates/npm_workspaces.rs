//! Workspace-member discovery for npm, Yarn, and pnpm monorepos.
//! Resolves member manifests so dependency scans do not stop at a sparse root package.

use std::path::{Path, PathBuf};

/// Bound traversal of hostile or pathological workspace globs.
const MAX_MEMBERS: usize = 250;

/// Maximum recursive workspace-glob depth.
const MAX_DEPTH: usize = 8;

/// Ignore dependency trees, VCS metadata, and dotted directories.
fn is_ignored_dir(name: &str) -> bool {
    name == "node_modules" || name.starts_with('.')
}

/// Find declared workspace directories containing `package.json`, excluding root.
pub(super) fn member_dirs(root: &Path, root_pkg: &serde_json::Value) -> Vec<PathBuf> {
    let patterns = collect_patterns(root, root_pkg);
    if patterns.is_empty() {
        return Vec::new();
    }

    let (positive, negative): (Vec<&String>, Vec<&String>) =
        patterns.iter().partition(|p| !p.starts_with('!'));

    let mut members = Vec::new();
    for pattern in positive {
        let segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
        expand(root, root, &segments, 0, 0, &mut members);
        if members.len() >= MAX_MEMBERS {
            break;
        }
    }
    members.sort();
    members.dedup();

    if !negative.is_empty() {
        members.retain(|dir| {
            let Some(rel) = relative_key(root, dir) else {
                return true;
            };
            !negative
                .iter()
                .any(|pattern| matches_path(pattern.trim_start_matches('!'), &rel))
        });
    }
    members
}

/// The member globs declared by this project. `pnpm-workspace.yaml` wins when
/// present (a pnpm workspace's `package.json` often has no `workspaces` field
/// at all); otherwise the npm/yarn `workspaces` field is used.
fn collect_patterns(root: &Path, root_pkg: &serde_json::Value) -> Vec<String> {
    let patterns = pnpm_workspace_patterns(root);
    if patterns.is_empty() {
        package_json_workspace_patterns(root_pkg)
    } else {
        patterns
    }
}

/// Parse the `packages:` sequence out of `pnpm-workspace.yaml`. Hand-rolled
/// rather than a YAML dependency: the file's shape is a fixed top-level key
/// holding a flat list of strings, and the parser only has to survive it.
fn pnpm_workspace_patterns(root: &Path) -> Vec<String> {
    let content = super::read_dependency_file(&root.join("pnpm-workspace.yaml"))
        .or_else(|| super::read_dependency_file(&root.join("pnpm-workspace.yml")));
    let Some(content) = content else {
        return Vec::new();
    };

    let mut patterns = Vec::new();
    let mut in_packages = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            in_packages = trimmed == "packages:";
            continue;
        }
        if !in_packages {
            continue;
        }
        let Some(item) = trimmed.strip_prefix('-') else {
            continue;
        };
        let item = item.trim().trim_matches('"').trim_matches('\'');
        if !item.is_empty() {
            patterns.push(item.to_string());
        }
    }
    patterns
}

/// npm/yarn declare members in `package.json`, either as a bare array or as
/// `{"packages": [...]}` (yarn's nohoist shape).
fn package_json_workspace_patterns(pkg: &serde_json::Value) -> Vec<String> {
    let items = match pkg.get("workspaces") {
        Some(serde_json::Value::Array(items)) => items,
        Some(serde_json::Value::Object(obj)) => match obj.get("packages") {
            Some(serde_json::Value::Array(items)) => items,
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    items
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Walk `current` against `pattern[idx..]`, pushing directories that hold a
/// `package.json` once the whole pattern is consumed.
fn expand(
    root: &Path,
    current: &Path,
    pattern: &[&str],
    idx: usize,
    depth: usize,
    out: &mut Vec<PathBuf>,
) {
    if out.len() >= MAX_MEMBERS || depth > MAX_DEPTH {
        return;
    }

    if idx == pattern.len() {
        if current != root && current.join("package.json").is_file() {
            out.push(current.to_path_buf());
        }
        return;
    }

    let segment = pattern[idx];

    // `**` matches zero or more directories.
    if segment == "**" {
        expand(root, current, pattern, idx + 1, depth, out);
        for child in child_dirs(current) {
            expand(root, &child, pattern, idx, depth + 1, out);
        }
        return;
    }

    if !segment.contains('*') {
        let next = current.join(segment);
        if next.is_dir() {
            expand(root, &next, pattern, idx + 1, depth + 1, out);
        }
        return;
    }

    for child in child_dirs(current) {
        let Some(name) = child.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if segment_matches(segment, name) {
            expand(root, &child, pattern, idx + 1, depth + 1, out);
        }
    }
}

/// Return sorted immediate directories without following symlinks.
/// Enumeration failure marks dependency detection partial.
fn child_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        super::set_present_but_unreadable();
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| !is_ignored_dir(name))
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs
}

/// `dir` as a `/`-separated path relative to `root`.
fn relative_key(root: &Path, dir: &Path) -> Option<String> {
    let rel = dir.strip_prefix(root).ok()?;
    Some(rel.to_str()?.replace('\\', "/"))
}

/// Whether a whole `/`-separated relative path matches a glob pattern.
fn matches_path(pattern: &str, rel: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let rel: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
    match_segments(&pattern, &rel)
}

fn match_segments(pattern: &[&str], rel: &[&str]) -> bool {
    let Some(segment) = pattern.first() else {
        return rel.is_empty();
    };
    if *segment == "**" {
        return (0..=rel.len()).any(|i| match_segments(&pattern[1..], &rel[i..]));
    }
    match rel.first() {
        Some(name) if segment_matches(segment, name) => match_segments(&pattern[1..], &rel[1..]),
        _ => false,
    }
}

/// Wildcard match for one path segment: `*` matches any run of characters
/// within the segment (never across `/`).
fn segment_matches(pattern: &str, name: &str) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == name;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let last = parts.len() - 1;
    let mut rest = name;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            let Some(stripped) = rest.strip_prefix(part) else {
                return false;
            };
            rest = stripped;
        } else if i == last {
            // The trailing literal must not re-consume characters an earlier
            // part already matched, so compare against what is left.
            return rest.len() >= part.len() && rest.ends_with(part);
        } else {
            let Some(pos) = rest.find(part) else {
                return false;
            };
            rest = &rest[pos + part.len()..];
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_member(root: &Path, rel: &str) {
        let dir = root.join(rel);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), r#"{"name":"m"}"#).unwrap();
    }

    fn empty_pkg() -> serde_json::Value {
        serde_json::json!({})
    }

    fn keys(root: &Path, dirs: &[PathBuf]) -> Vec<String> {
        dirs.iter().filter_map(|d| relative_key(root, d)).collect()
    }

    #[test]
    fn finds_members_from_pnpm_workspace_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - \"apps/*\"\n  - \"packages/*\"\n",
        )
        .unwrap();
        write_member(root, "apps/desktop");
        write_member(root, "apps/mcp-server");
        write_member(root, "packages/pricing");

        let found = member_dirs(root, &empty_pkg());
        assert_eq!(
            keys(root, &found),
            vec!["apps/desktop", "apps/mcp-server", "packages/pricing"]
        );
    }

    #[test]
    fn finds_members_from_package_json_workspaces_array() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_member(root, "packages/api");
        write_member(root, "packages/web");

        let pkg = serde_json::json!({ "workspaces": ["packages/*"] });
        let found = member_dirs(root, &pkg);
        assert_eq!(keys(root, &found), vec!["packages/api", "packages/web"]);
    }

    #[test]
    fn finds_members_from_yarn_workspaces_object() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_member(root, "packages/api");

        let pkg = serde_json::json!({ "workspaces": { "packages": ["packages/*"] } });
        assert_eq!(keys(root, &member_dirs(root, &pkg)), vec!["packages/api"]);
    }

    #[test]
    fn pnpm_workspace_yaml_wins_over_package_json_workspaces() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*'\n",
        )
        .unwrap();
        write_member(root, "apps/desktop");
        write_member(root, "packages/ignored");

        let pkg = serde_json::json!({ "workspaces": ["packages/*"] });
        assert_eq!(keys(root, &member_dirs(root, &pkg)), vec!["apps/desktop"]);
    }

    #[test]
    fn double_star_matches_nested_members() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_member(root, "packages/group/inner");
        write_member(root, "packages/flat");

        let pkg = serde_json::json!({ "workspaces": ["packages/**"] });
        assert_eq!(
            keys(root, &member_dirs(root, &pkg)),
            vec!["packages/flat", "packages/group/inner"]
        );
    }

    #[test]
    fn negated_pattern_excludes_members() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_member(root, "packages/keep");
        write_member(root, "packages/examples");

        let pkg = serde_json::json!({ "workspaces": ["packages/*", "!packages/examples"] });
        assert_eq!(keys(root, &member_dirs(root, &pkg)), vec!["packages/keep"]);
    }

    #[test]
    fn node_modules_is_never_a_member() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_member(root, "node_modules/left-pad");
        write_member(root, "packages/real");

        let pkg = serde_json::json!({ "workspaces": ["*/*"] });
        assert_eq!(keys(root, &member_dirs(root, &pkg)), vec!["packages/real"]);
    }

    #[test]
    fn directory_without_package_json_is_not_a_member() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("packages/docs")).unwrap();
        write_member(root, "packages/real");

        let pkg = serde_json::json!({ "workspaces": ["packages/*"] });
        assert_eq!(keys(root, &member_dirs(root, &pkg)), vec!["packages/real"]);
    }

    #[test]
    fn no_workspace_config_yields_no_members() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_member(root, "packages/real");

        assert!(member_dirs(root, &empty_pkg()).is_empty());
    }

    #[test]
    fn segment_wildcards_match_partial_names() {
        assert!(segment_matches("*", "anything"));
        assert!(segment_matches("app-*", "app-web"));
        assert!(segment_matches("*-lib", "core-lib"));
        assert!(segment_matches("a*c", "abc"));
        assert!(!segment_matches("app-*", "web-app"));
        assert!(!segment_matches("*-lib", "lib-core"));
        assert!(segment_matches("exact", "exact"));
        assert!(!segment_matches("exact", "other"));
    }

    #[test]
    fn path_matching_handles_double_star() {
        assert!(matches_path("packages/**", "packages/a/b"));
        assert!(matches_path("packages/**", "packages"));
        assert!(matches_path("**/examples", "packages/nested/examples"));
        assert!(!matches_path("packages/*", "packages/a/b"));
    }
}
