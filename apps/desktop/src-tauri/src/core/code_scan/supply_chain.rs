use super::*;

mod committed_files;
use committed_files::{
    collect_dockerfile_pinning_issues, collect_npmrc_token_issues, collect_pipe_to_shell_issues,
};
mod config_secrets;
use config_secrets::collect_config_secret_issues;
mod github_actions;
use github_actions::{
    collect_workflow_injection_issues, collect_workflow_permission_issues,
    collect_workflow_pinning_issues, collect_workflow_pr_target_checkout_issues,
};
mod release_age;
use release_age::collect_release_age_issues;
mod dependency_ranges;
use dependency_ranges::collect_unbounded_dependency_issues;
mod lockfile_integrity;
use lockfile_integrity::collect_lockfile_integrity_issues;
mod usage_evidence;
use usage_evidence::collect_extra_usage_evidence;

pub(super) fn analyze_supply_chain(
    root: &Path,
    files: &[SourceFile],
    project_files: &[ProjectFile],
    manifests: &[PackageManifest],
) -> Vec<CodeIssue> {
    let registry_config = collect_registry_config(root, manifests);
    let declared_dependencies = manifests
        .iter()
        .flat_map(|manifest| manifest.dependencies.iter().cloned())
        .collect::<HashSet<_>>();
    let local_package_names = manifests
        .iter()
        .filter_map(|manifest| manifest.package_name.clone())
        .collect::<HashSet<_>>();
    // Treat tsconfig path aliases as internal imports, not undeclared npm
    // dependencies.
    let path_alias_prefixes = collect_path_alias_prefixes(project_files);
    let package_refs = collect_js_package_refs(files);
    let imported_packages = package_refs
        .iter()
        .map(|package_ref| package_ref.package_name.clone())
        .collect::<HashSet<_>>();

    let mut issues = Vec::new();
    let mut seen_ids = HashSet::new();
    // Usage evidence the source walk cannot see, read lazily only when an
    // unused-dependency candidate appears (bounded file reads).
    let mut extra_usage: Option<HashSet<String>> = None;
    // Excerpt lookups below were a linear scan of all files per emitted issue;
    // a repo with hundreds of supply-chain findings paid O(issues x files).
    let content_by_path: HashMap<&str, &str> = files
        .iter()
        .map(|file| (file.relative_path.as_str(), file.content.as_str()))
        .collect();
    let content_for =
        |relative_path: &str| -> &str { content_by_path.get(relative_path).copied().unwrap_or("") };

    for package_ref in package_refs {
        let framework_provided =
            is_framework_provided(&package_ref.package_name, &declared_dependencies);
        let alias_internal = path_alias_prefixes.iter().any(|prefix| {
            package_ref.package_name == *prefix
                || package_ref
                    .package_name
                    .starts_with(&format!("{}/", prefix))
        });
        let declared = declared_dependencies.contains(&package_ref.package_name)
            || local_package_names.contains(&package_ref.package_name)
            || framework_provided
            || alias_internal;

        if !declared {
            let id = format!(
                "undeclared-package:{}:{}",
                package_ref.relative_path,
                sanitize_identifier(&package_ref.package_name),
            );
            if seen_ids.insert(id.clone()) {
                // Graded by the shared confidence policy (NeedsReview): import
                // resolution is a heuristic with a documented false-positive
                // history (path aliases, framework-provided namespaces).
                let (confidence, confidence_reason) =
                    crate::core::confidence_policy::code_issue_confidence("undeclared-package");
                issues.push(CodeIssue {
                    check_id: String::new(),
                    id,
                    category: "supply-chain".into(),
                    severity: Severity::Medium,
                    title: "Import is not matched to a declared or recognized local dependency".into(),
                    description: "This source import was not matched to a dependency declaration in any scanned package manifest, a scanned workspace package name, a recognized framework-provided namespace, or a parsed TypeScript path alias. It may be undeclared or misspelled, but an unsupported resolver, generated alias, parent workspace, or runtime-provided module can also explain it.".into(),
                    relative_path: package_ref.relative_path.clone(),
                    absolute_path: package_ref.absolute_path.clone(),
                    line: package_ref.line,
                    source_excerpt: excerpt_for_line(content_for(&package_ref.relative_path), package_ref.line),
                    evidence: Some(redact_evidence(format!(
                        "Import specifier '{}' was detected in source, but it was not matched to scanned dependencies, devDependencies, peerDependencies, optionalDependencies, workspace package names, recognized framework namespaces, or parsed TypeScript path aliases.",
                        package_ref.package_name
                    ))),
                    why_now: Some("A genuinely undeclared external dependency can fail in a clean install or rely on accidental local state, while an intentional alias should be explicit enough for the project resolver and CI to reproduce.".into()),
                    likely_fix: Some("Resolve the import with the project's actual package manager and build tool first. If it is an external package, confirm its identity and add it to the owning package manifest; if it is local, configure or document the workspace/path alias; if it is stale, remove the import. Do not install a similarly named package solely to clear the finding.".into()),
                    confidence,
                    confidence_reason: confidence_reason.map(|reason| reason.to_string()),
                    verify_hint: Some("Perform a frozen clean install and run the owning package's resolver, typecheck, or build. Confirm the import resolves through the intended declared package, workspace, or documented alias without pre-existing node_modules state.".into()),
                });
            }

            if let Some(expected_package) = suspicious_package_match(&package_ref.package_name) {
                let id = format!(
                    "suspicious-package:{}:{}",
                    package_ref.relative_path,
                    sanitize_identifier(&package_ref.package_name),
                );
                if seen_ids.insert(id.clone()) {
                    let (confidence, confidence_reason) = policy_confidence("suspicious-package");
                    issues.push(CodeIssue {
                        check_id: String::new(),
                        id,
                        category: "supply-chain".into(),
                        severity: Severity::Medium,
                        title: "Undeclared import resembles a popular package name".into(),
                        description: "This undeclared import is one edit or adjacent-character swap away from a curated popular package name. That string similarity is useful review evidence, but it does not prove the package exists, is installed, or is malicious. Review whether it is a typo, a workspace alias, or a module supplied by framework tooling.".into(),
                        relative_path: package_ref.relative_path.clone(),
                        absolute_path: package_ref.absolute_path.clone(),
                        line: package_ref.line,
                        source_excerpt: excerpt_for_line(content_for(&package_ref.relative_path), package_ref.line),
                        evidence: Some(redact_evidence(format!(
                            "Imported package '{}' looks very close to '{}'.",
                            package_ref.package_name, expected_package
                        ))),
                        why_now: Some("The nearest manifest does not declare this package. A clean install may fail unless a workspace alias, framework, or build tool supplies it, so confirming the resolution path avoids silently relying on local-only state.".into()),
                        likely_fix: Some(format!(
                            "Double-check the intended module. If this was meant to be the external package '{}', replace the import and pin the official package; otherwise document or configure the workspace/framework alias that resolves it.",
                            expected_package
                        )),
                        confidence,
                        confidence_reason,
                        verify_hint: Some("After fixing the import, reinstall from scratch and confirm package.json and the lockfile now point at the library you actually intended to use.".into()),
                    });
                }
            }
        }
    }

    for manifest in manifests {
        let manifest_dir = manifest.absolute_path.parent().unwrap_or(root);
        let resolved_registry_hosts = collect_lockfile_registry_hosts(manifest_dir);
        let has_supported_lockfile = has_lockfile_in_scope(manifest_dir, root);
        let locked_packages = crate::updates::npm::parse(manifest_dir)
            .into_iter()
            .map(|package| package.name.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let external_declared_dependencies = manifest
            .dependencies
            .iter()
            .filter(|dependency| !manifest.local_dependencies.contains(*dependency))
            .cloned()
            .collect::<Vec<_>>();

        for dependency in &external_declared_dependencies {
            let Some(spec) = manifest.dependency_specs.get(dependency) else {
                continue;
            };
            if !dependency_spec_uses_remote_url(spec) {
                continue;
            }

            let id = format!(
                "direct-url-dependency:{}:{}",
                manifest.relative_path,
                sanitize_identifier(dependency),
            );
            if seen_ids.insert(id.clone()) {
                issues.push(CodeIssue {
                    check_id: String::new(),
                    id,
                    category: "supply-chain".into(),
                    severity: Severity::Medium,
                    title: "Dependency uses a direct URL or Git source".into(),
                    description: "This dependency uses a Git or direct URL spec instead of a package-registry version. That can be intentional, but reproducibility and review depend on whether the spec resolves immutably, the lockfile captures it, credentials are kept out of the URL, and the source remains available.".into(),
                    relative_path: manifest.relative_path.clone(),
                    absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                    line: find_line(&manifest.content, dependency),
                    source_excerpt: excerpt_for_line(&manifest.content, find_line(&manifest.content, dependency)),
                    evidence: Some(redact_evidence(format!(
                        "Dependency '{}' uses a Git or direct URL source; the credential-bearing or otherwise sensitive spec is intentionally omitted from issue evidence.",
                        dependency
                    ))),
                    why_now: Some("A mutable branch, tag, or replaceable URL can change dependency bytes without a manifest edit, while embedded URL credentials can leak through source, logs, or reports.".into()),
                    likely_fix: Some("Prefer a trusted registry release when it meets the requirement. Otherwise remove credentials from the URL, pin Git sources to a full commit or direct artifacts to an independently verified digest/signature where supported, retain lockfile integrity metadata, and document the source and update owner.".into()),
                    confidence: crate::checks::IssueConfidence::High,
                    confidence_reason: None,
                    verify_hint: Some("From an isolated clean environment with approved credentials supplied out of band, run a frozen install twice and confirm the same immutable revision or digest resolves, the lockfile is unchanged, and no credential appears in the manifest, lockfile, logs, or scan output.".into()),
                });
            }
        }

        if !external_declared_dependencies.is_empty() && !has_supported_lockfile {
            let id = format!("lockfile-missing:{}", manifest.relative_path);
            if seen_ids.insert(id.clone()) {
                issues.push(CodeIssue {
                    check_id: String::new(),
                    id,
                    category: "supply-chain".into(),
                    severity: Severity::Medium,
                    title: "Declared npm dependencies have no recognized lockfile in scope".into(),
                    description: "This package.json declares external dependencies, but SiteCMD found no supported npm, Yarn, pnpm, or Bun lockfile in the package directory or applicable scanned workspace root. An unsupported package manager, generated lockfile, or intentionally library-only workflow may explain the absence; otherwise dependency resolution is not reproducibly captured.".into(),
                    relative_path: manifest.relative_path.clone(),
                    absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                    line: None,
                    source_excerpt: None,
                    evidence: Some(redact_evidence("External dependencies were found, but no lockfile format supported by SiteCMD was found in the package directory or applicable scanned workspace root.")),
                    why_now: Some("Without an authoritative lockfile or equivalent immutable resolution, clean installs can select different transitive versions at different times.".into()),
                    likely_fix: Some("Confirm the package manager and workspace ownership first. For an application or otherwise reproducible install, generate the native lockfile with the pinned package-manager version, review it, and make frozen installs authoritative in CI. If this is a published library that intentionally omits a lockfile, document that policy and mark the finding not applicable.".into()),
                    confidence: crate::checks::IssueConfidence::NeedsReview,
                    confidence_reason: Some("The absence of a SiteCMD-supported in-scope lockfile is factual, but unsupported, generated, parent-managed, or intentionally omitted resolution workflows may exist.".into()),
                    verify_hint: Some("In a disposable clean environment, run the package manager's frozen install and the build/tests twice. Confirm the lockfile remains unchanged and the owning package resolves through the intended workspace policy.".into()),
                });
            }
        }

        for dependency in &manifest.dependencies {
            if let Some(expected_package) = suspicious_package_match(dependency) {
                let id = format!(
                    "suspicious-manifest-package:{}:{}",
                    manifest.relative_path,
                    sanitize_identifier(dependency),
                );
                if seen_ids.insert(id.clone()) {
                    let (confidence, confidence_reason) =
                        policy_confidence("suspicious-manifest-package");
                    issues.push(CodeIssue {
                        check_id: String::new(),
                        id,
                        category: "supply-chain".into(),
                        severity: Severity::High,
                        title: "Declared dependency resembles a popular package name".into(),
                        description: "This declared dependency is one edit or adjacent-character swap away from a curated popular package name. Because it is in package.json, an install may fetch it, but name similarity alone does not prove the publisher, package contents, or intent are malicious. Verify provenance before changing or installing it.".into(),
                        relative_path: manifest.relative_path.clone(),
                        absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                        line: find_line(&manifest.content, dependency),
                        source_excerpt: excerpt_for_line(&manifest.content, find_line(&manifest.content, dependency)),
                        evidence: Some(redact_evidence(format!(
                            "Declared dependency '{}' looks very close to '{}'.",
                            dependency, expected_package
                        ))),
                        why_now: Some("The near-match name is part of the install manifest, so it can enter the dependency tree on a clean install if it resolves. This warrants prompt provenance review, not an automatic malware verdict.".into()),
                        likely_fix: Some(format!(
                            "Verify the declared package's provenance, publisher, registry, and intended use before installing it. If it is a typo for '{}', replace the name and regenerate the affected lockfile entries intentionally; if it is a legitimate distinct package, document the decision and mark the finding reviewed.",
                            expected_package
                        )),
                        confidence,
                        confidence_reason,
                        verify_hint: Some("Reinstall dependencies from a clean state and verify the package manager now resolves the intended official package.".into()),
                    });
                }
            }
        }

        if has_supported_lockfile && !locked_packages.is_empty() {
            for dependency in &external_declared_dependencies {
                if locked_packages.contains(dependency) {
                    continue;
                }
                let id = format!(
                    "lockfile-mismatch:{}:{}",
                    manifest.relative_path,
                    sanitize_identifier(dependency),
                );
                if seen_ids.insert(id.clone()) {
                    issues.push(CodeIssue {
                        check_id: String::new(),
                        id,
                        category: "supply-chain".into(),
                        severity: Severity::Medium,
                        title: "Declared dependency was not matched by the local lockfile parser".into(),
                        description: "A dependency declared in package.json was not returned as a resolved direct package by SiteCMD's parser for the in-scope lockfile. The manifest and lockfile may be out of sync, but workspace indirection, aliases, optional/platform-specific entries, unsupported lockfile details, or parser limitations can also cause the mismatch.".into(),
                        relative_path: manifest.relative_path.clone(),
                        absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                        line: find_line(&manifest.content, dependency),
                        source_excerpt: excerpt_for_line(&manifest.content, find_line(&manifest.content, dependency)),
                        evidence: Some(redact_evidence(format!(
                            "Declared dependency '{}' was found in package.json, but the local lockfile parser did not resolve it as an installed direct dependency.",
                            dependency
                        ))),
                        why_now: Some("A real manifest/lockfile mismatch can break frozen installs, while blindly regenerating a valid lockfile can create a large and risky dependency diff.".into()),
                        likely_fix: Some("Run the package manager's frozen install in a disposable clean environment first. If it succeeds, inspect workspace, alias, platform, and parser support before changing files. If it fails because the lockfile is stale, update it with the project's pinned package-manager version and review only the intended dependency-graph changes.".into()),
                        confidence: crate::checks::IssueConfidence::NeedsReview,
                        confidence_reason: Some("The parser did not match the declaration, but static lockfile parsing may not model every workspace, alias, platform, optional-dependency, or lockfile-version behavior.".into()),
                        verify_hint: Some("A frozen clean install must succeed without changing the lockfile. If an update was necessary, confirm the declared package resolves as intended, review the full diff and registry/integrity metadata, and run the build and tests.".into()),
                    });
                }
            }
        }

        if !resolved_registry_hosts.is_empty() {
            for dependency in &external_declared_dependencies {
                let Some(resolved_host) = resolved_registry_hosts.get(dependency) else {
                    continue;
                };
                let allowed_hosts =
                    allowed_registry_hosts_for_dependency(dependency, &registry_config);
                if allowed_hosts.contains(resolved_host) {
                    continue;
                }
                let id = format!(
                    "registry-host-mismatch:{}:{}",
                    manifest.relative_path,
                    sanitize_identifier(dependency),
                );
                if seen_ids.insert(id.clone()) {
                    issues.push(CodeIssue {
                        check_id: String::new(),
                        id,
                        category: "supply-chain".into(),
                        severity: Severity::High,
                        title: "Dependency resolves from an unexpected registry host".into(),
                        description: "This dependency resolves from a registry host that does not match the npm registries this project appears to expect. The observed mismatch is real, but it does not prove tampering or a malicious package: an intentional private registry, caching proxy, mirror, or registry migration can produce it too.".into(),
                        relative_path: manifest.relative_path.clone(),
                        absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                        line: find_line(&manifest.content, dependency),
                        source_excerpt: excerpt_for_line(&manifest.content, find_line(&manifest.content, dependency)),
                        evidence: Some(redact_evidence(format!(
                            "Dependency '{}' resolved from '{}', but the expected registry hosts were {}.",
                            dependency,
                            resolved_host,
                            format_registry_host_list(&allowed_hosts)
                        ))),
                        why_now: Some("A lockfile can keep using an obsolete or unapproved registry after configuration changes. If the host is not intentional, clean installs can fetch code or send registry credentials across the wrong trust boundary.".into()),
                        likely_fix: Some("Build the approved default/scoped registry map from project configuration, CI, and organization policy. If the host is intentional, document it and mark the finding reviewed. If not, investigate the host and package before installing, pin the intended registry without credentials, regenerate only the affected lockfile entries with the project's package-manager version, and inspect the diff plus integrity metadata.".into()),
                        confidence: crate::checks::IssueConfidence::NeedsReview,
                        confidence_reason: Some("The resolved-host mismatch is directly observed, but the scanner cannot know the organization's approved mirrors, private registries, or migration history.".into()),
                        verify_hint: Some("Run an isolated frozen install using the approved configuration and confirm the dependency resolves from the intended host with expected integrity and contents. Review that unrelated lockfile entries and lifecycle scripts did not change or run unexpectedly.".into()),
                    });
                }
            }
        }

        if has_supported_lockfile {
            // Count package.json script binaries as dependency usage even when
            // source files never import them.
            let script_referenced = collect_script_referenced_packages(
                &manifest.content,
                &external_declared_dependencies,
            );

            for dependency in &external_declared_dependencies {
                if imported_packages.contains(dependency)
                    || script_referenced.contains(dependency)
                    || should_ignore_unused_dependency(dependency)
                    || suspicious_package_match(dependency).is_some()
                    || extra_usage
                        .get_or_insert_with(|| collect_extra_usage_evidence(project_files))
                        .contains(dependency)
                {
                    continue;
                }
                let id = format!(
                    "unused-dependency:{}:{}",
                    manifest.relative_path,
                    sanitize_identifier(dependency),
                );
                if seen_ids.insert(id.clone()) {
                    // Graded by the shared confidence policy. The expanded
                    // source/config/script inventory makes this a strong lead,
                    // while the confidence reason preserves dynamic-tooling
                    // caveats.
                    let (confidence, confidence_reason) =
                        crate::core::confidence_policy::code_issue_confidence("unused-dependency");
                    issues.push(CodeIssue {
                        check_id: String::new(),
                        id,
                        category: "supply-chain".into(),
                        severity: Severity::Low,
                        title: "Declared dependency has no clear source usage".into(),
                        description: "This dependency is declared and locked, but there is no clear import or require usage in the scanned source files. Sometimes that is intentional, but it can also mean leftover packages, abandoned experiments, or install commands that were never cleaned up.".into(),
                        relative_path: manifest.relative_path.clone(),
                        absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                        line: find_line(&manifest.content, dependency),
                        source_excerpt: excerpt_for_line(&manifest.content, find_line(&manifest.content, dependency)),
                        evidence: Some(redact_evidence(format!(
                            "Dependency '{}' was declared, but no matching import was found in JavaScript or TypeScript source files.",
                            dependency
                        ))),
                        why_now: Some("A truly unused installed package adds maintenance and install-time supply-chain surface, but configuration-only, plugin, CLI, stylesheet, generated, or runtime-loaded usage may not appear as a source import.".into()),
                        likely_fix: Some("Search package scripts, configuration, plugins, stylesheets, generated code, and runtime loading before deciding. If no supported path uses the package, remove it in a branch and update the lockfile intentionally; otherwise document the non-import usage and mark the finding reviewed.".into()),
                        confidence,
                        confidence_reason: confidence_reason.map(|reason| reason.to_string()),
                        verify_hint: Some("Remove the dependency in a branch, reinstall, and confirm the app still builds and tests cleanly.".into()),
                    });
                }
            }
        }
    }

    collect_config_secret_issues(&mut issues, &mut seen_ids, project_files);

    // Evaluate alternatives per manifest; different workspace members may
    // legitimately choose different libraries.
    for manifest in manifests {
        for (group, label) in DUPLICATE_UTILITY_GROUPS {
            let matches: Vec<&&str> = group
                .iter()
                .filter(|pkg| manifest.dependencies.contains(**pkg))
                .collect();
            if matches.len() >= 2 {
                let names: Vec<String> = matches.iter().map(|s| format!("`{}`", s)).collect();
                let line = find_line(&manifest.content, matches[0]);
                issues.push(CodeIssue {
                    check_id: String::new(),
                    id: format!("duplicate-utility-deps:{}:{}", label.replace(' ', "-"), manifest.relative_path),
                    category: "supply-chain".into(),
                    severity: Severity::Low,
                    title: format!("Multiple {} libraries declared", label),
                    description: format!(
                        "This package declares {} {} libraries: {}. Multiple libraries may be intentional when they serve distinct APIs, migration stages, sub-bundles, or transitive integration requirements; declarations alone do not prove redundant runtime code.",
                        matches.len(), label, names.join(", ")
                    ),
                    relative_path: manifest.relative_path.clone(),
                    absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                    line,
                    source_excerpt: excerpt_for_line(&manifest.content, line),
                    evidence: Some(redact_evidence(format!("Found {} {} libraries in this package.json's dependencies: {}.", matches.len(), label, names.join(", ")))),
                    // The merged dependency set includes devDependencies, so the
                    // bundle-size claim is scoped to runtime dependencies.
                    why_now: Some("Overlapping runtime use can increase maintenance and bundle cost, while forced consolidation can remove capabilities or destabilize a migration.".into()),
                    likely_fix: Some(format!("Map where each {} library is used and compare its required capabilities. If their roles overlap materially, choose a target deliberately, migrate and measure one slice at a time, then remove only the library with no remaining direct or configuration usage. If the roles differ, document them and keep both.", label)),
                    confidence: crate::checks::IssueConfidence::NeedsReview,
                    confidence_reason: Some("Multiple declarations are factual, but static manifest inspection cannot determine whether their responsibilities, bundles, or migration roles overlap.".into()),
                    verify_hint: Some("Search imports, package scripts, configuration, generated code, and bundle output for both libraries. After any removal, run the relevant behavior tests and compare bundle artifacts before marking the review complete.".into()),
                });
            }
        }
    }

    collect_workflow_pinning_issues(&mut issues, project_files);
    collect_workflow_permission_issues(&mut issues, project_files);
    collect_workflow_injection_issues(&mut issues, project_files);
    collect_workflow_pr_target_checkout_issues(&mut issues, project_files);
    collect_npmrc_token_issues(&mut issues, project_files);
    collect_dockerfile_pinning_issues(&mut issues, project_files);
    collect_pipe_to_shell_issues(&mut issues, project_files, manifests);
    collect_unbounded_dependency_issues(&mut issues, manifests);
    collect_release_age_issues(&mut issues, project_files, manifests);
    collect_lockfile_integrity_issues(&mut issues, project_files);

    issues
}

/// Whether a declared framework provides an imported module namespace.
///
/// Framework-owned namespaces are not declared as separate app dependencies.
fn is_framework_provided(package: &str, declared: &HashSet<String>) -> bool {
    let declares = |name: &str| declared.contains(name);
    if (declares("ember-source") || declares("ember-cli"))
        && (package.starts_with("@ember/") || package.starts_with("@glimmer/"))
    {
        return true;
    }
    if declares("vue") && package.starts_with("@vue/") {
        return true;
    }
    if declares("nuxt") && (package == "vue" || package.starts_with("@vue/")) {
        return true;
    }
    false
}

/// Find a supported lockfile between a manifest and the scanned project root.
/// This recognizes workspace-root lockfiles without escaping scan scope.
fn has_lockfile_in_scope(manifest_dir: &std::path::Path, root: &std::path::Path) -> bool {
    const MAX_DEPTH: usize = 8;
    let mut current = manifest_dir.to_path_buf();
    for _ in 0..MAX_DEPTH {
        if SUPPORTED_NPM_LOCKFILES
            .iter()
            .any(|name| current.join(name).exists())
        {
            return true;
        }
        // Stop after checking the repo root or the scan root; do not look higher.
        if current.join(".git").exists() || current == root {
            return false;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return false,
        }
    }
    false
}

/// Collect import prefixes provided by path aliases, Deno import maps, and
/// bundler aliases rather than `package.json` dependencies.
fn collect_path_alias_prefixes(project_files: &[ProjectFile]) -> std::collections::HashSet<String> {
    const MAX_CONFIG_BYTES: u64 = 256 * 1024;
    let mut prefixes = std::collections::HashSet::new();
    for file in project_files {
        let name = file
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(file.relative_path.as_str());
        let is_ts_config = (name.starts_with("tsconfig") || name.starts_with("jsconfig"))
            && name.ends_with(".json");
        let is_deno_config = name == "deno.json" || name == "deno.jsonc";
        // Bundler configs declare `resolve.alias` mapping specifiers to local
        // files (e.g. Vite/Webpack/Rollup/Vitest/Next).
        let is_bundler_config = matches!(
            name.split('.').next(),
            Some("vite" | "vitest" | "webpack" | "rollup" | "next" | "rspack" | "rsbuild")
        ) && name.contains(".config.");
        if !is_ts_config && !is_deno_config && !is_bundler_config {
            continue;
        }
        let Some(bytes) = read_project_file(file, MAX_CONFIG_BYTES) else {
            continue;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };
        for line in content.lines() {
            let prefix = if is_deno_config {
                deno_import_prefix_from_line(line)
            } else if is_bundler_config {
                bundler_alias_prefix_from_line(line)
            } else {
                path_alias_prefix_from_line(line)
            };
            if let Some(prefix) = prefix {
                prefixes.insert(prefix);
            }
        }
    }
    prefixes
}

/// Extract the first import segment from a `resolve.alias` entry whose value
/// references a local filesystem path.
fn bundler_alias_prefix_from_line(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line.strip_prefix('"').or_else(|| line.strip_prefix('\''))?;
    let quote = if line.starts_with('"') { '"' } else { '\'' };
    let end = rest.find(quote)?;
    let key = &rest[..end];
    let after = rest[end + 1..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let lower = after.to_ascii_lowercase();
    let references_local_path = lower.starts_with("path.resolve")
        || lower.starts_with("path.join")
        || lower.starts_with("__dirname")
        || lower.starts_with("fileurltopath")
        || lower.starts_with("new url(")
        || lower.starts_with("resolve(")
        || lower.starts_with("\"./")
        || lower.starts_with("\"../")
        || lower.starts_with("'./")
        || lower.starts_with("'../");
    if !references_local_path {
        return None;
    }
    // Reduce the key to its first segment, mirroring import normalization:
    // `@scope/name` keeps two segments, otherwise just the first path segment.
    let prefix = if let Some(scoped) = key.strip_prefix('@') {
        let mut segments = scoped.split('/');
        let scope = segments.next()?;
        match segments.next() {
            Some(pkg) if !pkg.is_empty() => format!("@{}/{}", scope, pkg),
            _ => format!("@{}", scope),
        }
    } else {
        key.split('/').next()?.to_string()
    };
    let prefix = prefix.trim_end_matches('*').trim_end_matches('/');
    if prefix.is_empty() {
        return None;
    }
    Some(prefix.to_ascii_lowercase())
}

/// Extract a Deno import-map alias from a module-specifier entry.
fn deno_import_prefix_from_line(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line.strip_prefix('"')?;
    let end = rest.find('"')?;
    let key = &rest[..end];
    let after = rest[end + 1..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let value = after.strip_prefix('"')?;
    let value = &value[..value.find('"')?];
    let is_specifier = ["jsr:", "npm:", "https://", "http://", "./", "../", "node:"]
        .iter()
        .any(|scheme| value.starts_with(scheme));
    if !is_specifier {
        return None;
    }
    let prefix = key.trim_end_matches('/');
    if prefix.is_empty() {
        return None;
    }
    Some(prefix.to_ascii_lowercase())
}

/// Extract an alias-marked `compilerOptions.paths` key without its trailing glob.
fn path_alias_prefix_from_line(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line.strip_prefix('"')?;
    let end = rest.find('"')?;
    let key = &rest[..end];
    let after = rest[end + 1..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    if !after.starts_with('[') {
        return None;
    }
    let looks_like_alias =
        key.contains('*') || key.contains('/') || key.starts_with('@') || key.starts_with('~');
    if !looks_like_alias {
        return None;
    }
    let prefix = key.trim_end_matches('*').trim_end_matches('/');
    if prefix.is_empty() {
        return None;
    }
    Some(prefix.to_ascii_lowercase())
}

#[cfg(test)]
mod path_alias_tests {
    use super::{is_framework_provided, path_alias_prefix_from_line};
    use std::collections::HashSet;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn framework_provided_namespaces() {
        let ember = set(&["ember-source"]);
        assert!(is_framework_provided("@ember/component", &ember));
        assert!(is_framework_provided("@glimmer/tracking", &ember));
        assert!(!is_framework_provided("@vue/reactivity", &ember));

        // Vue provides its own @vue/* internals; Nuxt bundles vue itself.
        let vue = set(&["vue"]);
        assert!(is_framework_provided("@vue/reactivity", &vue));
        assert!(!is_framework_provided("vue", &vue)); // declared directly anyway
        let nuxt = set(&["nuxt"]);
        assert!(is_framework_provided("vue", &nuxt));
        assert!(is_framework_provided("@vue/runtime-core", &nuxt));

        // A real third-party package is never framework-provided.
        assert!(!is_framework_provided("lodash", &nuxt));
        assert!(!is_framework_provided("@ember/component", &set(&[])));
    }

    #[test]
    fn extracts_alias_prefixes_and_ignores_plain_keys() {
        assert_eq!(
            path_alias_prefix_from_line(r#"      "@ui/*": ["./src/*"],"#),
            Some("@ui".to_string())
        );
        assert_eq!(
            path_alias_prefix_from_line(r#""@utils/*": ["./utils/*"]"#),
            Some("@utils".to_string())
        );
        assert_eq!(
            path_alias_prefix_from_line(r#""components/*": ["src/components/*"]"#),
            Some("components".to_string())
        );
        // Non-alias tsconfig keys (arrays, but no alias marker) are ignored.
        assert_eq!(path_alias_prefix_from_line(r#""types": ["node"]"#), None);
        assert_eq!(path_alias_prefix_from_line(r#""lib": ["ESNext"]"#), None);
        // Non-paths lines are ignored.
        assert_eq!(path_alias_prefix_from_line(r#""strict": true,"#), None);
        assert_eq!(path_alias_prefix_from_line("// a comment"), None);
    }

    #[test]
    fn extracts_deno_import_map_prefixes() {
        use super::deno_import_prefix_from_line;
        assert_eq!(
            deno_import_prefix_from_line(r#"    "@std/path": "jsr:@std/path@^1.0.0","#),
            Some("@std/path".to_string())
        );
        assert_eq!(
            deno_import_prefix_from_line(r#""preact": "npm:preact@^10.0.0""#),
            Some("preact".to_string())
        );
        assert_eq!(
            deno_import_prefix_from_line(r#""$fresh/": "./fresh/""#),
            Some("$fresh".to_string())
        );
        // Tasks / config entries (value is not a module specifier) are ignored.
        assert_eq!(
            deno_import_prefix_from_line(r#""dev": "deno run -A main.ts""#),
            None
        );
        assert_eq!(deno_import_prefix_from_line(r#""name": "my-app""#), None);
    }

    #[test]
    fn extracts_bundler_alias_prefixes() {
        use super::bundler_alias_prefix_from_line;
        // Vite/Webpack alias to a local shim -- reduced to first segment so it
        // matches an import that normalizes to that segment (plane: next/link).
        assert_eq!(
            bundler_alias_prefix_from_line(
                r#"      "next/link": path.resolve(__dirname, "app/compat/next/link.tsx"),"#
            ),
            Some("next".to_string())
        );
        assert_eq!(
            bundler_alias_prefix_from_line(
                r#""@/components": path.join(__dirname, "src/components")"#
            ),
            Some("@/components".to_string())
        );
        assert_eq!(
            bundler_alias_prefix_from_line(r#"'~': './src'"#),
            Some("~".to_string())
        );
        // Non-alias config (value is not a local path) is ignored.
        assert_eq!(
            bundler_alias_prefix_from_line(
                r#""process.env.NODE_ENV": JSON.stringify("production")"#
            ),
            None
        );
        assert_eq!(bundler_alias_prefix_from_line(r#""port": 3000"#), None);
    }
}
