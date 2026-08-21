//! Detect missing or legacy SHA-1 integrity hashes in npm registry lock entries.

use super::*;

const LOCKFILE_MAX_BYTES: u64 = 4_000_000;
const MAX_LISTED_PACKAGES: usize = 8;

fn is_npm_lockfile_path(relative_path: &str) -> bool {
    let base = relative_path
        .rsplit('/')
        .next()
        .unwrap_or(relative_path)
        .to_ascii_lowercase();
    base == "package-lock.json" || base == "npm-shrinkwrap.json"
}

/// A resolved source that npm fetches from a registry as an integrity-verified
/// tarball. Git, file:, link:, and workspace resolutions legitimately carry no
/// integrity hash, so they are never flagged.
fn is_registry_tarball(resolved: &str) -> bool {
    resolved.starts_with("https://") && resolved.ends_with(".tgz")
}

/// Classify one lockfile package entry. Returns the offending package label
/// when it resolves from the registry but has a missing or SHA-1 integrity.
fn weak_integrity_label(name: &str, entry: &serde_json::Value) -> Option<String> {
    let resolved = entry.get("resolved").and_then(|v| v.as_str())?;
    if !is_registry_tarball(resolved) {
        return None;
    }
    match entry.get("integrity").and_then(|v| v.as_str()) {
        None => Some(format!("{} (no integrity hash)", name)),
        Some(hash) => {
            let algorithms = hash
                .split_ascii_whitespace()
                .filter_map(|digest| digest.split_once('-').map(|(algorithm, _)| algorithm))
                .collect::<Vec<_>>();
            if algorithms
                .iter()
                .any(|algorithm| matches!(*algorithm, "sha256" | "sha384" | "sha512"))
            {
                None
            } else if algorithms.contains(&"sha1") {
                Some(format!("{} (SHA-1 integrity)", name))
            } else {
                None
            }
        }
    }
}

/// The package name for a lockfileVersion 2/3 `packages` key
/// (`node_modules/foo`, `node_modules/@scope/bar/node_modules/baz`). The
/// segment after the LAST `node_modules/` is the installed package.
fn package_name_from_packages_key(key: &str) -> Option<&str> {
    let after = key.rsplit_once("node_modules/").map(|(_, tail)| tail)?;
    if after.is_empty() {
        None
    } else {
        Some(after)
    }
}

fn collect_weak_entries(json: &serde_json::Value) -> Vec<String> {
    let mut weak = Vec::new();
    // lockfileVersion 2/3: flat `packages` map keyed by install path.
    if let Some(packages) = json.get("packages").and_then(|v| v.as_object()) {
        for (key, entry) in packages {
            let Some(name) = package_name_from_packages_key(key) else {
                continue; // the root package (key "") has no registry tarball
            };
            if let Some(label) = weak_integrity_label(name, entry) {
                weak.push(label);
            }
        }
    }
    // lockfileVersion 1: nested `dependencies` tree.
    if let Some(dependencies) = json.get("dependencies").and_then(|v| v.as_object()) {
        collect_weak_dependencies_tree(dependencies, &mut weak);
    }
    weak
}

fn collect_weak_dependencies_tree(
    dependencies: &serde_json::Map<String, serde_json::Value>,
    weak: &mut Vec<String>,
) {
    for (name, entry) in dependencies {
        if let Some(label) = weak_integrity_label(name, entry) {
            weak.push(label);
        }
        if let Some(nested) = entry.get("dependencies").and_then(|v| v.as_object()) {
            collect_weak_dependencies_tree(nested, weak);
        }
    }
}

pub(super) fn collect_lockfile_integrity_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
) {
    for file in project_files {
        if !is_npm_lockfile_path(&file.relative_path) {
            continue;
        }
        let Some(bytes) = read_project_file(file, LOCKFILE_MAX_BYTES) else {
            continue;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };

        let mut weak = collect_weak_entries(&json);
        if weak.is_empty() {
            continue;
        }
        weak.sort();
        weak.dedup();

        let total = weak.len();
        let mut listed = weak
            .iter()
            .take(MAX_LISTED_PACKAGES)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if total > MAX_LISTED_PACKAGES {
            listed.push_str(&format!(", and {} more", total - MAX_LISTED_PACKAGES));
        }

        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("lockfile-integrity-weak:{}", file.relative_path),
            category: "supply-chain".into(),
            severity: Severity::Medium,
            title: "Lockfile has registry dependencies with missing or weak integrity metadata".into(),
            description: "This npm lockfile contains registry tarballs with a missing integrity value or a legacy SHA-1 value. Missing entries provide no digest check against the bytes recorded at resolution time. SHA-1 entries still perform a digest check, but use a collision-weakened algorithm rather than the SHA-512 metadata current npm registries normally provide. Integrity metadata is one supply-chain layer; it does not establish publisher trust or provenance.".into(),
            relative_path: file.relative_path.clone(),
            absolute_path: file.absolute_path.to_string_lossy().to_string(),
            line: None,
            source_excerpt: None,
            evidence: Some(redact_evidence(format!(
                "{} registry dependency entr{} in {} lack a strong integrity hash: {}.",
                total,
                if total == 1 { "y" } else { "ies" },
                file.relative_path,
                listed
            ))),
            why_now: Some("Recorded digests let npm detect unexpected tarball bytes during install. Missing metadata removes that check, while SHA-1 provides a weaker legacy check; TLS, registry controls, review, and provenance remain separate layers.".into()),
            likely_fix: Some("On a disposable branch, use the project's package-manager version to refresh the affected lockfile entries or run its normal lockfile-only update with lifecycle scripts disabled during inspection. Do not delete the lockfile as a first step; review the lockfile diff for unintended version, resolved-host, integrity, or dependency-graph changes before accepting it.".into()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: Some("In a disposable clean checkout using the pinned npm version, review the changed entries, run `npm ci --ignore-scripts`, and confirm affected registry tarballs now have a strong supported integrity digest without unexpected version, resolved-host, or dependency-graph changes.".into()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{is_registry_tarball, package_name_from_packages_key, weak_integrity_label};

    #[test]
    fn only_registry_tarballs_are_candidates() {
        assert!(is_registry_tarball(
            "https://registry.npmjs.org/left-pad/-/left-pad-1.3.0.tgz"
        ));
        assert!(!is_registry_tarball("git+https://github.com/a/b.git#abc"));
        assert!(!is_registry_tarball("file:../local-pkg"));
        assert!(!is_registry_tarball(""));
    }

    #[test]
    fn packages_key_resolves_to_the_installed_package() {
        assert_eq!(
            package_name_from_packages_key("node_modules/left-pad"),
            Some("left-pad")
        );
        assert_eq!(
            package_name_from_packages_key("node_modules/@scope/a/node_modules/b"),
            Some("b")
        );
        assert_eq!(package_name_from_packages_key(""), None);
    }

    #[test]
    fn missing_and_sha1_integrity_are_weak_strong_is_not() {
        let missing = serde_json::json!({
            "resolved": "https://registry.npmjs.org/a/-/a-1.0.0.tgz"
        });
        assert!(weak_integrity_label("a", &missing)
            .unwrap()
            .contains("no integrity"));

        let sha1 = serde_json::json!({
            "resolved": "https://registry.npmjs.org/b/-/b-1.0.0.tgz",
            "integrity": "sha1-abcdef"
        });
        assert!(weak_integrity_label("b", &sha1).unwrap().contains("SHA-1"));

        let strong = serde_json::json!({
            "resolved": "https://registry.npmjs.org/c/-/c-1.0.0.tgz",
            "integrity": "sha512-abcdef"
        });
        assert_eq!(weak_integrity_label("c", &strong), None);

        let strongest_available_wins = serde_json::json!({
            "resolved": "https://registry.npmjs.org/c/-/c-1.0.0.tgz",
            "integrity": "sha1-legacy sha512-current"
        });
        assert_eq!(
            weak_integrity_label("c", &strongest_available_wins),
            None,
            "a strong SRI digest must prevent a legacy companion digest from being misreported"
        );

        // A git/link entry with no integrity is legitimate, never flagged.
        let git = serde_json::json!({ "resolved": "git+https://x/y.git#abc" });
        assert_eq!(weak_integrity_label("d", &git), None);
    }
}
