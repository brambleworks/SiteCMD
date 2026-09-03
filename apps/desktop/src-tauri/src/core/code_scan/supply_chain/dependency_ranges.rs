use super::super::{is_example_like_path, is_test_like_path};
use super::*;
use std::path::Path;

/// How many offending dependencies to name in the evidence before summarizing
/// the remainder as a count.
const MAX_LISTED_DEPS: usize = 8;

/// Match only fully unbounded specs: `*`, `x`, or `latest`.
fn spec_is_unbounded(spec: &str) -> bool {
    let normalized = spec.trim();
    ["*", "x", "latest"]
        .iter()
        .any(|token| normalized.eq_ignore_ascii_case(token))
}

/// Flag unbounded runtime dependency ranges once per manifest.
///
/// `local_package_names` holds the workspace packages the scan found: npm
/// workspaces conventionally reference a sibling package with a bare `"*"`,
/// which resolves to that local directory and never to a registry release.
pub(super) fn collect_unbounded_dependency_issues(
    issues: &mut Vec<CodeIssue>,
    manifests: &[PackageManifest],
    local_package_names: &HashSet<String>,
) {
    for manifest in manifests {
        // Fixture / example / playground manifests use loose specs on purpose and
        // are not the user's shipped dependencies.
        let path = Path::new(&manifest.relative_path);
        if is_example_like_path(path) || is_test_like_path(path) {
            continue;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&manifest.content) else {
            continue;
        };
        let Some(deps) = json.get("dependencies").and_then(|value| value.as_object()) else {
            continue;
        };
        let mut offenders: Vec<String> = deps
            .iter()
            .filter_map(|(name, value)| {
                let spec = value.as_str()?;
                if local_package_names.contains(&name.to_ascii_lowercase()) {
                    return None;
                }
                spec_is_unbounded(spec).then(|| format!("{name} (\"{spec}\")"))
            })
            .collect();
        if offenders.is_empty() {
            continue;
        }
        offenders.sort();

        let total = offenders.len();
        let mut listed = offenders
            .iter()
            .take(MAX_LISTED_DEPS)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if total > MAX_LISTED_DEPS {
            listed.push_str(&format!(", and {} more", total - MAX_LISTED_DEPS));
        }

        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("unbounded-dependency-range:{}", manifest.relative_path),
            category: "supply-chain".into(),
            severity: Severity::Low,
            title: "Runtime dependency uses an unbounded version spec".into(),
            description: "One or more runtime `dependencies` use `*`, `x`, or `latest`. Resolution without an authoritative lockfile, and updates that honor the manifest range, can select a different major or dist-tag target without a manifest edit. A valid frozen lockfile still pins the current install, and a bounded range is a compatibility policy rather than a guarantee of safe package contents.".into(),
            relative_path: manifest.relative_path.clone(),
            absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
            line: None,
            source_excerpt: None,
            evidence: Some(redact_evidence(format!(
                "Unbounded runtime dependency {} in {}: {}.",
                if total == 1 { "spec" } else { "specs" },
                manifest.relative_path,
                listed
            ))),
            why_now: Some("An unbounded runtime range makes compatibility changes less predictable during an update or any install that does not enforce the current lockfile; provenance and vulnerability controls remain separate concerns.".into()),
            likely_fix: Some("Choose the update policy the project intends: an exact version, tilde, caret, or another bounded range based on the currently tested release. Update the lockfile with the pinned package-manager version, review the dependency diff, and require frozen installs in CI and release builds.".into()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: Some("Confirm each flagged runtime dependency now has the intended bounded policy, a frozen clean install leaves the lockfile unchanged, and the build/tests pass against the resolved versions.".into()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::spec_is_unbounded;

    #[test]
    fn unbounded_spec_detection_ignores_bounded_and_protocol_specs() {
        // Open-ended specs fire.
        assert!(spec_is_unbounded("*"));
        assert!(spec_is_unbounded("x"));
        assert!(spec_is_unbounded("X"));
        assert!(spec_is_unbounded("latest"));
        assert!(spec_is_unbounded("  latest  "));

        // Bounded ranges are the npm norm and must never fire.
        assert!(!spec_is_unbounded("^1.2.3"));
        assert!(!spec_is_unbounded("~1.2.3"));
        assert!(!spec_is_unbounded("1.2.3"));
        assert!(!spec_is_unbounded("1.x"));
        assert!(!spec_is_unbounded(">=1.2.0 <2.0.0"));
        assert!(!spec_is_unbounded(""));

        // Protocol / workspace / alias specs are not wildcard ranges.
        assert!(!spec_is_unbounded("workspace:*"));
        assert!(!spec_is_unbounded("npm:left-pad@*"));
        assert!(!spec_is_unbounded("file:../local"));
        assert!(!spec_is_unbounded("github:user/repo"));
    }
}
