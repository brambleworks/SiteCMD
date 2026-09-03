use super::*;

mod hook_install;
mod hooks;
mod manifest_scripts;
mod project_kind;
mod quality_gates;
mod quality_markers;
mod test_gates;

use hooks::collect_hook_issues;
use manifest_scripts::collect_script_inventory;
use project_kind::{classify_project, ProjectKind};
use quality_gates::{ci_workflow_paths, inspect_quality_signals, QualitySignals};
use test_gates::collect_test_infrastructure_issues;

/// Environment files that may carry machine-specific values. Templates and
/// examples are documentation and should remain trackable.
fn is_nonexample_env_file_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let env_shaped = name == ".env" || name.starts_with(".env.") || name.ends_with(".env");
    env_shaped
        && !["example", "sample", "template", "dist", "defaults"]
            .iter()
            .any(|marker| name.contains(marker))
}

struct ProjectHygieneContext<'a> {
    root: &'a Path,
    files: &'a [SourceFile],
    summaries: &'a [FileSignalSummary],
    manifests: &'a [PackageManifest],
    project_paths_lower: &'a [String],
    declared_dependencies: &'a HashSet<String>,
    route_files: &'a [&'a SourceFile],
    env_usage_file: Option<&'a SourceFile>,
    app_like: bool,
}

pub(super) fn collect_project_hygiene_issues(
    issues: &mut Vec<CodeIssue>,
    root: &Path,
    files: &[SourceFile],
    summaries: &[FileSignalSummary],
    manifests: &[PackageManifest],
    project_paths_lower: &[String],
    declared_dependencies: &HashSet<String>,
    route_files: &[&SourceFile],
    env_usage_file: Option<&SourceFile>,
    app_like: bool,
) {
    let context = ProjectHygieneContext {
        root,
        files,
        summaries,
        manifests,
        project_paths_lower,
        declared_dependencies,
        route_files,
        env_usage_file,
        app_like,
    };
    let scripts = collect_script_inventory(root, manifests, project_paths_lower);
    let kind = classify_project(&context, &scripts);
    let quality_signals = inspect_quality_signals(&context, &scripts);

    collect_test_infrastructure_issues(issues, &context, &kind, &quality_signals);
    collect_ignore_policy_issues(issues, &context, &kind);
    collect_lint_and_ci_issues(issues, &context, &kind, &quality_signals);
    collect_hook_issues(issues, &context, &kind, &quality_signals);
    collect_source_readiness_issues(issues, &context);
}

fn collect_ignore_policy_issues(
    issues: &mut Vec<CodeIssue>,
    context: &ProjectHygieneContext<'_>,
    kind: &ProjectKind,
) {
    let root = context.root;
    let project_paths_lower = context.project_paths_lower;
    let env_usage_file = context.env_usage_file;

    // Only a repository root owns repository-level hygiene such as CI and .gitignore.
    let root_is_git_repo = root.join(".git").exists();

    // Any discovered .gitignore may cover nested environment files.
    let gitignore_paths = project_paths_lower
        .iter()
        .filter(|path| path.as_str() == ".gitignore" || path.ends_with("/.gitignore"))
        .cloned()
        .collect::<Vec<_>>();
    let has_gitignore = !gitignore_paths.is_empty();
    let gitignore_covers_env = gitignore_paths.iter().any(|path| {
        crate::core::code_scan::filesystem::read_text_under_root(root, &root.join(path))
            .map(|content| {
                let lower = content.to_ascii_lowercase();
                lower.contains(".env") || lower.contains("*.env")
            })
            .unwrap_or(false)
    });

    if !has_gitignore && kind.hygiene_eligible() && root_is_git_repo {
        if let Some((relative_path, absolute_path)) = kind.anchor.clone() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("gitignore-missing:{}", relative_path),
                category: "operations".into(),
                // Medium: process-hygiene gap, graded below no-automated-tests
                // (matches the registry pin).
                severity: Severity::Medium,
                title: "Project has no .gitignore file".into(),
                description: "The scanned Git repository root has no project-managed `.gitignore` file. Global excludes or `.git/info/exclude` may still hide local files, but those rules are not shared with other clones, so generated or machine-specific files can appear as trackable unless another workflow controls them.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("No project-managed .gitignore file was found within the scanned Git repository root.")),
                why_now: Some("A shared ignore policy keeps reproducible generated, dependency, and machine-local artifacts out of ordinary staging, while secret scanning and review remain necessary safeguards.".into()),
                likely_fix: Some("Add a root `.gitignore` containing only paths that this project actually generates or keeps machine-local, using the framework's maintained template as a starting point. Do not ignore files that should be reviewed or versioned, and do not treat ignore rules as secret protection for files already tracked.".into()),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                verify_hint: Some("Use `git check-ignore -v <path>` for representative generated files and review `git status --ignored`; confirm intended source and configuration files remain visible.".into()),
            });
        }
    } else if has_gitignore
        && !gitignore_covers_env
        && env_usage_file.is_some()
        && project_paths_lower
            .iter()
            .any(|path| is_nonexample_env_file_path(path))
        && root_is_git_repo
    {
        // Anchor at the root.gitignore when it exists, otherwise at the
        // first nested one actually found.
        let gitignore_path_str = gitignore_paths
            .iter()
            .find(|path| path.as_str() == ".gitignore")
            .cloned()
            .unwrap_or_else(|| gitignore_paths[0].clone());
        let abs = root.join(&gitignore_path_str).to_string_lossy().to_string();
        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("gitignore-missing-env:{}", gitignore_path_str),
            category: "security".into(),
            severity: Severity::Medium,
            title: ".gitignore does not cover environment files present in the project".into(),
            description: "A non-example environment file is present in the scanned project and source code reads environment variables, but no scanned `.gitignore` contains a recognized `.env` rule. The file may contain only non-sensitive local settings, may already be tracked, or may be excluded globally; this check does not inspect Git index state or prove secret exposure.".into(),
            relative_path: gitignore_path_str,
            absolute_path: abs,
            line: None,
            source_excerpt: None,
            evidence: Some(redact_evidence("At least one non-example env file is present and env-variable usage was detected, while no scanned .gitignore contains a recognized .env or *.env pattern. Git tracking and global excludes were not inspected.")),
            why_now: Some("Environment files often mix harmless local settings with credentials. A shared ignore rule reduces accidental staging, but it does not remove an already tracked file or remediate a credential that was exposed.".into()),
            likely_fix: Some("Classify the environment file first. Add only the actual machine-local or secret-bearing patterns to `.gitignore`, keep a scrubbed `.env.example` for required keys, and check whether the file is already tracked before relying on the rule. If a real credential was previously shared, logged, or committed, revoke or rotate that confirmed credential and remove the tracked copy without deleting a teammate's local data.".into()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: Some("Use `git check-ignore -v` on each intended local env file and `git ls-files` to confirm it is not already tracked. Verify the example file remains visible and contains no live values.".into()),
        });
    }
}

fn collect_lint_and_ci_issues(
    issues: &mut Vec<CodeIssue>,
    context: &ProjectHygieneContext<'_>,
    kind: &ProjectKind,
    signals: &QualitySignals,
) {
    if !kind.hygiene_eligible() {
        return;
    }
    let root = context.root;
    let manifests = context.manifests;
    let project_paths_lower = context.project_paths_lower;
    let root_is_git_repo = root.join(".git").exists();
    let has_build_script = signals.has_build_script;
    let has_test_script = signals.has_test_script;
    let has_lint_or_typecheck_script = signals.has_lint_or_typecheck_script;
    let has_ci_config = signals.has_ci_config;

    if !signals.has_linter_config {
        if let Some((relative_path, absolute_path)) = kind.manifest_anchor.clone() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("linter-missing:{}", relative_path),
                category: "architecture".into(),
                severity: Severity::Medium,
                title: "No recognized linter or formatter configuration was found".into(),
                description: "The scanned project has app-like source and a package manifest, but no recognized lint or format configuration or package script was found. A parent workspace, editor, language-native default, external CI command, or unrecognized tool may still provide equivalent checks.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("No recognized ESLint, Biome, Prettier, Ruff, Pint, PHPStan, or comparable config, package lint script, or manifest marker was found within the scanned project.")),
                why_now: Some("A repeatable static-quality command can catch stack-specific correctness issues and keep formatting consistent before review or CI.".into()),
                likely_fix: Some("First check parent-workspace and CI configuration for an existing command. If none covers this project, add the stack-appropriate linter and/or formatter with a documented script. Start from maintained defaults, enable rules that fit the codebase, and avoid a bulk rewrite without reviewing the diff.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Tool discovery is name- and pattern-based within the scanned project; inherited, language-native, external, or custom quality commands may not be recognized.".into()),
                verify_hint: Some("Run the documented command from the project root and in CI, confirm it checks real source files, and intentionally introduce one representative violation to prove the gate fails.".into()),
            });
        }
    }

    if !has_ci_config && root_is_git_repo {
        if let Some((relative_path, absolute_path)) = kind.anchor.clone() {
            let mut evidence = String::from("No workflow file for the CI providers recognized by SiteCMD was found within the scanned Git repository; external and organization-level configuration was not inspected.");
            if !kind.hosting_configs.is_empty() {
                evidence.push_str(&format!(
                    " A hosting config ({}) was found; a host build on push compiles the site but does not run its lint or tests.",
                    kind.hosting_configs.join(", ")
                ));
            }
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("ci-workflow-missing:{}", relative_path),
                category: "operations".into(),
                // Medium: process-hygiene gap, graded below no-automated-tests
                // (matches the registry pin).
                severity: Severity::Medium,
                title: "No recognized CI workflow was found in the scanned repository".into(),
                description: "The scanned Git repository looks like a deployable project, but no recognized GitHub Actions, GitLab CI, CircleCI, Buildkite, Azure Pipelines, Bitbucket Pipelines, or Jenkins workflow file was found. CI may be configured in a parent repository, organization, deploy platform, or unsupported system, so manual-only validation is not established.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence(evidence)),
                why_now: Some("A clean, repeatable remote check can catch environment-dependent build and test failures before deployment, provided it runs the same supported commands and its result is enforced.".into()),
                likely_fix: Some("Confirm whether an external, parent-workspace, deploy-platform, or organization-level pipeline already covers this project. If not, add a CI workflow that installs from the lockfile and runs the documented build, lint/typecheck, and risk-based test commands with least-privilege permissions.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("CI discovery is limited to recognized files in the scanned repository; external, inherited, organization-level, or unsupported providers may supply equivalent checks.".into()),
                verify_hint: Some("Open or trigger the authoritative pipeline from a clean revision and confirm it runs on the intended branch or pull-request events, executes the expected commands, and blocks a deliberately failing check.".into()),
            });
        }
    }

    if has_ci_config && signals.has_quality_scripts && !signals.ci_has_quality_gate {
        let anchor = ci_workflow_paths(project_paths_lower)
            .first()
            .map(|path| {
                (
                    (*path).to_string(),
                    root.join(*path).to_string_lossy().to_string(),
                )
            })
            .or_else(|| kind.manifest_anchor.clone());

        if let Some((relative_path, absolute_path)) = anchor {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("ci-quality-gate-missing:{}", relative_path),
                category: "operations".into(),
                // Medium: process-hygiene gap, graded below no-automated-tests
                // (matches the registry pin).
                severity: Severity::Medium,
                title: "CI workflow does not appear to run build, lint, or tests".into(),
                description: "The scanned project has CI and package scripts that can exercise launch-readiness checks, but the workflow file does not appear to run those quality gates. Reusable workflows, organization-level required checks, or dynamically composed commands may still provide coverage outside this static match.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("A CI workflow file was found, but its commands do not mention build, lint, typecheck, test, or an equivalent quality script.")),
                why_now: Some("A workflow that omits the project's protective commands may report success without checking whether the application builds or its critical behavior still works.".into()),
                likely_fix: Some("Inspect reusable and dynamically composed jobs first. If they do not provide coverage, update the workflow to install dependencies reproducibly and run this project's documented build, lint/typecheck, and risk-based test commands.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Static command matching cannot resolve every reusable workflow, composite action, task runner, script indirection, or organization-required check.".into()),
                verify_hint: Some("Inspect a real CI run and confirm the effective job executes the intended quality commands; introduce a controlled failure and verify the required check blocks success.".into()),
            });
        }
    }

    // A tooling-only package.json beside a Composer or route-less project has
    // nothing to build; only a JavaScript-driven web project or app owes one.
    let owes_build_script = kind.js_site_like || (kind.app_like && !kind.composer_root);
    if owes_build_script && !manifests.is_empty() && !has_build_script {
        if let Some(manifest) = manifests.first() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("build-script-missing:{}", manifest.relative_path),
                category: "operations".into(),
                // Medium: process-hygiene gap, graded below no-automated-tests
                // (matches the registry pin).
                severity: Severity::Medium,
                title: "package.json has no obvious production build script".into(),
                description: "This project has app-like source, but the scanned package.json does not define `scripts.build` or a `production`/`prod` equivalent. A static or interpreted application, parent-workspace task, deploy-provider command, or differently named script may make that intentional.".into(),
                relative_path: manifest.relative_path.clone(),
                absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("No build, production, or prod script entry was found in package.json.")),
                why_now: Some("When a build step is required, one documented command makes local, CI, and deploy behavior easier to reproduce and compare.".into()),
                likely_fix: Some("Confirm the actual deploy process first. If this package requires compilation or bundling, expose the framework-native production build under `scripts.build` or clearly document the parent task that owns it, then make CI and deployment use that same path. Do not add a no-op build to silence the finding.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The missing `scripts.build` entry is factual, but static, interpreted, parent-managed, provider-managed, or differently named build paths may be valid.".into()),
                verify_hint: Some("From a clean frozen install, run the documented production build or parent task and confirm it creates the artifact or server entry the deploy target actually uses.".into()),
            });
        }
    }

    if has_ci_config && has_build_script && !has_test_script && !has_lint_or_typecheck_script {
        if let Some(manifest) = manifests.first() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("ci-only-builds:{}", manifest.relative_path),
                category: "operations".into(),
                severity: Severity::Medium,
                title: "Package exposes a build script but no recognized lint, typecheck, or test script".into(),
                description: "The scanned project contains a recognized CI file and package build script, but package.json exposes no recognized lint, typecheck, or test script. The CI workflow was not proven to call the build script, and reusable jobs, parent tasks, or commands outside package.json may provide additional checks.".into(),
                relative_path: manifest.relative_path.clone(),
                absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("CI config and a build script were found, but package.json does not expose clear lint, typecheck, or test scripts.")),
                why_now: Some("A successful production build does not exercise every runtime behavior or necessarily run lint and type checks, so the effective CI job should make its validation coverage explicit.".into()),
                likely_fix: Some("Inspect the effective CI job and parent task graph. If lint, type, and behavior checks are genuinely absent, add the smallest useful documented commands for this stack and invoke them from CI; do not add empty scripts only to satisfy detection.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Package-script inspection cannot resolve reusable workflows, parent task runners, inline CI commands, or external required checks.".into()),
                verify_hint: Some("Inspect a real CI run, confirm which build, lint, typecheck, and test commands actually execute, and prove each required gate fails on a controlled representative error.".into()),
            });
        }
    }
}

fn collect_source_readiness_issues(
    issues: &mut Vec<CodeIssue>,
    context: &ProjectHygieneContext<'_>,
) {
    let files = context.files;
    let summaries = context.summaries;
    let project_paths_lower = context.project_paths_lower;

    let placeholder_source = |file: &SourceFile, summary: &FileSignalSummary| {
        !is_config_file(&file.relative_path)
            && !summary.pattern_registry
            && !summary.scanner_rule_impl
    };
    let total_placeholder_count: usize = files
        .iter()
        .zip(summaries)
        .filter(|(file, summary)| placeholder_source(file, summary))
        .map(|(file, _)| {
            PLACEHOLDER_PATTERNS
                .iter()
                .map(|pat| pat.find_iter(&file.content).count())
                .sum::<usize>()
        })
        .sum();
    let source_file_count = files
        .iter()
        .zip(summaries)
        .filter(|(file, summary)| placeholder_source(file, summary))
        .count();
    let placeholder_density = total_placeholder_count as f64 / source_file_count.max(1) as f64;
    let high_placeholder_density =
        source_file_count >= 5 && total_placeholder_count >= 25 && placeholder_density >= 1.0;
    let moderate_placeholder_density =
        source_file_count >= 3 && total_placeholder_count >= 8 && placeholder_density >= 1.5;

    if high_placeholder_density || moderate_placeholder_density {
        let worst_file = files
            .iter()
            .zip(summaries)
            .filter(|(file, summary)| placeholder_source(file, summary))
            .map(|(file, _)| file)
            .max_by_key(|file| {
                PLACEHOLDER_PATTERNS
                    .iter()
                    .map(|pat| pat.find_iter(&file.content).count())
                    .sum::<usize>()
            });

        if let Some(file) = worst_file {
            issues.push(build_issue(
                "placeholder-density",
                "architecture",
                if high_placeholder_density { Severity::Medium } else { Severity::Low },
                "High density of placeholder-style markers needs review",
                "The scanned source contains a high density of TODO, FIXME, HACK, XXX, CHANGEME, or PLACEHOLDER tokens. Some may represent real unfinished work, while others may be stale comments, test text, compatibility notes, or intentionally deferred tasks; marker count does not establish authorship or production impact.",
                file,
                None,
                Some(format!("Found {} placeholder-style tokens across {} scanned source files; token meaning and reachability were not evaluated.", total_placeholder_count, source_file_count)),
                Some("Review the surfaced markers by risk and context. Implement or track genuine launch blockers, remove only comments that are truly stale, and keep useful explanations or explicitly deferred work with an owner and rationale. Prioritize authentication, authorization, destructive writes, billing, and failure handling.".into()),
                Some("Review every surfaced marker and confirm each is resolved, linked to an owned task with an intentional deferral, or retained as accurate documentation. Re-run Code Scan to verify the unexplained density has fallen.".into()),
            ));
        }
    }

    // Pre-filter test paths once for all risky files.
    let test_paths_lower = project_paths_lower
        .iter()
        .filter(|path| is_test_artifact_path(path))
        .map(String::as_str)
        .collect::<Vec<_>>();
    for (file, summary) in files.iter().zip(summaries) {
        if !(summary.route_like
            && (summary.uses_llm
                || summary.sensitive_handler
                || (summary.touches_db && summary.write_handler)))
        {
            continue;
        }
        if has_nearby_test(&file.relative_path, &test_paths_lower) || summary.inline_rust_tests {
            continue;
        }

        issues.push(build_issue(
            "critical-path-no-test",
            "architecture",
            Severity::Medium,
            "No recognized nearby test for a high-risk server path",
            "This file appears to handle AI, sensitive writes, or database-backed route logic, but no recognized colocated or nearby test artifact was found. Coverage may exist in a distant integration suite, generated test plan, parent project, or naming convention the scanner does not recognize.",
            file,
            None,
            Some("A high-risk route heuristic matched this file, and no nearby recognized *.test.*, *.spec.*, __tests__/, or language-specific test artifact was found; distant or external coverage was not resolved.".into()),
            Some("Locate the authoritative test coverage first. If the risky behavior is not exercised, add a focused integration or route-level test for the highest-cost failure and its authorization boundary; the test does not need to be colocated if the project has a clear suite structure.".into()),
            Some("Run the authoritative suite and confirm a controlled regression in the risky branch makes the relevant test fail, then restore the code and confirm it passes.".into()),
        ));
    }
}
