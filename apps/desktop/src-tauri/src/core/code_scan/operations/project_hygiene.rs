use super::*;

mod quality_gates;
mod quality_markers;

use quality_gates::{
    ci_workflow_paths, commit_hook_paths, has_ci_quality_gate, has_commit_hook_quality_gate,
    inspect_quality_signals,
};

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

struct QualitySignals {
    has_linter_config: bool,
    has_build_script: bool,
    has_test_script: bool,
    has_lint_or_typecheck_script: bool,
    has_ci_config: bool,
    has_commit_hooks: bool,
    has_quality_scripts: bool,
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
    let quality_signals = inspect_quality_signals(&context);

    collect_test_infrastructure_issues(issues, &context);
    collect_ignore_policy_issues(issues, &context);
    collect_lint_and_ci_issues(issues, &context, &quality_signals);
    collect_hook_issues(issues, &context, &quality_signals);
    collect_source_readiness_issues(issues, &context);
}

fn collect_test_infrastructure_issues(
    issues: &mut Vec<CodeIssue>,
    context: &ProjectHygieneContext<'_>,
) {
    let manifests = context.manifests;
    let project_paths_lower = context.project_paths_lower;
    let declared_dependencies = context.declared_dependencies;
    let route_files = context.route_files;
    let app_like = context.app_like;
    let has_any_test_file = project_paths_lower.iter().any(|path| {
        is_test_artifact_path(path)
            || path.contains("/tests/")
            || path.contains("/test/")
            || path.ends_with("_test.py")
            || path.ends_with("_test.go")
            // pytest uses the test_*.py prefix convention.
            || path
                .rsplit('/')
                .next()
                .is_some_and(|name| name.starts_with("test_") && name.ends_with(".py"))
    });
    let has_test_config = project_paths_lower.iter().any(|path| {
        TEST_CONFIG_FILES
            .iter()
            .any(|config| path.ends_with(config))
    }) || manifests.iter().any(|manifest| {
        let lower = manifest.content.to_ascii_lowercase();
        lower.contains("\"test\"")
            || lower.contains("\"test:")
            || lower.contains("\"vitest\"")
            || lower.contains("\"jest\"")
    });
    let has_test_infrastructure = has_any_test_file
        || has_test_config
        || has_named_dependency(
            declared_dependencies,
            &[
                "vitest",
                "jest",
                "@jest/core",
                "mocha",
                "ava",
                "tap",
                "playwright",
                "cypress",
                "@playwright/test",
                "@testing-library/react",
                "@testing-library/jest-dom",
                "pytest",
                "unittest",
            ],
        );

    if app_like && !has_test_infrastructure {
        let anchor = manifests
            .first()
            .map(|manifest| {
                (
                    manifest.relative_path.clone(),
                    manifest.absolute_path.to_string_lossy().to_string(),
                )
            })
            .or_else(|| {
                route_files.first().map(|file| {
                    (
                        file.relative_path.clone(),
                        file.absolute_path.to_string_lossy().to_string(),
                    )
                })
            });

        if let Some((relative_path, absolute_path)) = anchor {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("no-automated-tests:{}", relative_path),
                category: "architecture".into(),
                severity: Severity::Medium,
                title: "No recognized automated test infrastructure was found".into(),
                description: "The scanned project has routes, database access, or other app logic, but no recognized test files, runner configuration, test scripts, or test dependencies were found. Tests may exist outside the scanned project, use an unrecognized convention, or run through organization tooling, so this is not proof that all validation is manual.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("Within the scanned tree, no recognized *.test.*, *.spec.*, __tests__/, tests/, or test_*.py artifact, common runner config/dependency, or package test script was found.")),
                why_now: Some("Automated coverage for a high-risk route or data path reduces the chance that a refactor, dependency change, or urgent fix silently breaks behavior.".into()),
                likely_fix: Some("First confirm whether tests run through a parent workspace, external repository, or unrecognized command. If meaningful coverage is absent, add the stack-appropriate runner and one observable test around the route, workflow, or data path with the highest failure cost, then expose a documented command for local and CI use.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Test discovery is convention-based and limited to the scanned tree; external, generated, dynamically composed, or unusually named test infrastructure may exist.".into()),
                verify_hint: Some("From a clean environment, run the documented test command and confirm at least one real application behavior is exercised; if coverage lives elsewhere, record the owning project and command before marking this not applicable.".into()),
            });
        }
    }
}

fn collect_ignore_policy_issues(issues: &mut Vec<CodeIssue>, context: &ProjectHygieneContext<'_>) {
    let root = context.root;
    let manifests = context.manifests;
    let project_paths_lower = context.project_paths_lower;
    let route_files = context.route_files;
    let env_usage_file = context.env_usage_file;
    let app_like = context.app_like;

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

    if !has_gitignore && app_like && root_is_git_repo {
        let anchor = manifests
            .first()
            .map(|manifest| {
                (
                    manifest.relative_path.clone(),
                    manifest.absolute_path.to_string_lossy().to_string(),
                )
            })
            .or_else(|| {
                route_files.first().map(|file| {
                    (
                        file.relative_path.clone(),
                        file.absolute_path.to_string_lossy().to_string(),
                    )
                })
            });

        if let Some((relative_path, absolute_path)) = anchor {
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
    signals: &QualitySignals,
) {
    let root = context.root;
    let manifests = context.manifests;
    let project_paths_lower = context.project_paths_lower;
    let route_files = context.route_files;
    let app_like = context.app_like;
    let root_is_git_repo = root.join(".git").exists();
    let has_linter_config = signals.has_linter_config;
    let has_build_script = signals.has_build_script;
    let has_test_script = signals.has_test_script;
    let has_lint_or_typecheck_script = signals.has_lint_or_typecheck_script;
    let has_ci_config = signals.has_ci_config;
    let has_quality_scripts = signals.has_quality_scripts;

    if app_like && !has_linter_config && !manifests.is_empty() {
        if let Some(manifest) = manifests.first() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("linter-missing:{}", manifest.relative_path),
                category: "architecture".into(),
                severity: Severity::Medium,
                title: "No recognized linter or formatter configuration was found".into(),
                description: "The scanned project has app-like source and a package manifest, but no recognized lint or format configuration or package script was found. A parent workspace, editor, language-native default, external CI command, or unrecognized tool may still provide equivalent checks.".into(),
                relative_path: manifest.relative_path.clone(),
                absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("No recognized ESLint, Biome, Prettier, Ruff, or comparable config, package lint script, or manifest marker was found within the scanned project.")),
                why_now: Some("A repeatable static-quality command can catch stack-specific correctness issues and keep formatting consistent before review or CI.".into()),
                likely_fix: Some("First check parent-workspace and CI configuration for an existing command. If none covers this project, add the stack-appropriate linter and/or formatter with a documented script. Start from maintained defaults, enable rules that fit the codebase, and avoid a bulk rewrite without reviewing the diff.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Tool discovery is name- and pattern-based within the scanned project; inherited, language-native, external, or custom quality commands may not be recognized.".into()),
                verify_hint: Some("Run the documented command from the project root and in CI, confirm it checks real source files, and intentionally introduce one representative violation to prove the gate fails.".into()),
            });
        }
    }

    if app_like && !has_ci_config && root_is_git_repo {
        let anchor = manifests
            .first()
            .map(|manifest| {
                (
                    manifest.relative_path.clone(),
                    manifest.absolute_path.to_string_lossy().to_string(),
                )
            })
            .or_else(|| {
                route_files.first().map(|file| {
                    (
                        file.relative_path.clone(),
                        file.absolute_path.to_string_lossy().to_string(),
                    )
                })
            });

        if let Some((relative_path, absolute_path)) = anchor {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("ci-workflow-missing:{}", relative_path),
                category: "operations".into(),
                // Medium: process-hygiene gap, graded below no-automated-tests
                // (matches the registry pin).
                severity: Severity::Medium,
                title: "No recognized CI workflow was found in the scanned repository".into(),
                description: "The scanned Git repository looks app-like, but no recognized GitHub Actions, GitLab CI, CircleCI, Buildkite, Azure Pipelines, Bitbucket Pipelines, or Jenkins workflow file was found. CI may be configured in a parent repository, organization, deploy platform, or unsupported system, so manual-only validation is not established.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("No workflow file for the CI providers recognized by SiteCMD was found within the scanned Git repository; external and organization-level configuration was not inspected.")),
                why_now: Some("A clean, repeatable remote check can catch environment-dependent build and test failures before deployment, provided it runs the same supported commands and its result is enforced.".into()),
                likely_fix: Some("Confirm whether an external, parent-workspace, deploy-platform, or organization-level pipeline already covers this project. If not, add a CI workflow that installs from the lockfile and runs the documented build, lint/typecheck, and risk-based test commands with least-privilege permissions.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("CI discovery is limited to recognized files in the scanned repository; external, inherited, organization-level, or unsupported providers may supply equivalent checks.".into()),
                verify_hint: Some("Open or trigger the authoritative pipeline from a clean revision and confirm it runs on the intended branch or pull-request events, executes the expected commands, and blocks a deliberately failing check.".into()),
            });
        }
    }

    if app_like
        && has_ci_config
        && has_quality_scripts
        && !has_ci_quality_gate(root, project_paths_lower, manifests)
    {
        let anchor = ci_workflow_paths(project_paths_lower)
            .first()
            .map(|path| {
                (
                    (*path).to_string(),
                    root.join(*path).to_string_lossy().to_string(),
                )
            })
            .or_else(|| {
                manifests.first().map(|manifest| {
                    (
                        manifest.relative_path.clone(),
                        manifest.absolute_path.to_string_lossy().to_string(),
                    )
                })
            });

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

    if app_like && !manifests.is_empty() && !has_build_script {
        if let Some(manifest) = manifests.first() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("build-script-missing:{}", manifest.relative_path),
                category: "operations".into(),
                // Medium: process-hygiene gap, graded below no-automated-tests
                // (matches the registry pin).
                severity: Severity::Medium,
                title: "package.json has no obvious production build script".into(),
                description: "This project has app-like source, but the scanned package.json does not define `scripts.build`. A static or interpreted application, parent-workspace task, deploy-provider command, or differently named script may make that intentional.".into(),
                relative_path: manifest.relative_path.clone(),
                absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("No scripts.build entry was found in package.json.")),
                why_now: Some("When a build step is required, one documented command makes local, CI, and deploy behavior easier to reproduce and compare.".into()),
                likely_fix: Some("Confirm the actual deploy process first. If this package requires compilation or bundling, expose the framework-native production build under `scripts.build` or clearly document the parent task that owns it, then make CI and deployment use that same path. Do not add a no-op build to silence the finding.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("The missing `scripts.build` entry is factual, but static, interpreted, parent-managed, provider-managed, or differently named build paths may be valid.".into()),
                verify_hint: Some("From a clean frozen install, run the documented production build or parent task and confirm it creates the artifact or server entry the deploy target actually uses.".into()),
            });
        }
    }

    if app_like
        && has_ci_config
        && has_build_script
        && !has_test_script
        && !has_lint_or_typecheck_script
    {
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

fn collect_hook_issues(
    issues: &mut Vec<CodeIssue>,
    context: &ProjectHygieneContext<'_>,
    signals: &QualitySignals,
) {
    let root = context.root;
    let manifests = context.manifests;
    let project_paths_lower = context.project_paths_lower;
    let app_like = context.app_like;
    let has_linter_config = signals.has_linter_config;
    let has_build_script = signals.has_build_script;
    let has_test_script = signals.has_test_script;
    let has_lint_or_typecheck_script = signals.has_lint_or_typecheck_script;
    let has_commit_hooks = signals.has_commit_hooks;
    let has_quality_scripts = signals.has_quality_scripts;

    if app_like
        && !has_commit_hooks
        && (has_build_script
            || has_lint_or_typecheck_script
            || has_test_script
            || has_linter_config)
    {
        if let Some(manifest) = manifests.first() {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("pre-commit-hooks-missing:{}", manifest.relative_path),
                category: "operations".into(),
                severity: Severity::Low,
                title: "No recognized project-managed pre-commit or pre-push hook was found".into(),
                description: "The project has local quality commands, but the scanned tree contains no recognized Husky, Lefthook, pre-commit, lint-staged, or package-manager hook configuration. Hooks are optional and can be bypassed; CI or organization tooling may enforce the same checks outside the project.".into(),
                relative_path: manifest.relative_path.clone(),
                absolute_path: manifest.absolute_path.to_string_lossy().to_string(),
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("No .husky hook, lefthook config, pre-commit config, lint-staged config, or package.json prepare/simple-git-hooks hook was found.")),
                why_now: Some("An optional fast local hook can shorten feedback time, but it is bypassable and is not a substitute for required CI checks.".into()),
                likely_fix: Some("First confirm that required CI already protects the branch. If the team wants faster local feedback, add a lightweight project-managed hook for touched-file linting or focused checks, document the bypass behavior, and keep authoritative gates in CI. Mark this not applicable when hooks are intentionally avoided.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Hook discovery is limited to recognized project files; global, organization-managed, custom, or intentionally absent hooks cannot be evaluated.".into()),
                verify_hint: Some("If a hook is adopted, make a harmless staged change and a controlled failing change to confirm it runs locally, then confirm CI still enforces the authoritative check when hooks are bypassed.".into()),
            });
        }
    }

    if app_like
        && has_commit_hooks
        && has_quality_scripts
        && !has_commit_hook_quality_gate(root, project_paths_lower, manifests)
    {
        let anchor = commit_hook_paths(project_paths_lower)
            .first()
            .map(|path| {
                (
                    (*path).to_string(),
                    root.join(*path).to_string_lossy().to_string(),
                )
            })
            .or_else(|| {
                manifests.first().map(|manifest| {
                    (
                        manifest.relative_path.clone(),
                        manifest.absolute_path.to_string_lossy().to_string(),
                    )
                })
            });

        if let Some((relative_path, absolute_path)) = anchor {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("pre-commit-hooks-weak:{}", relative_path),
                category: "operations".into(),
                severity: Severity::Low,
                title: "Recognized project hook does not reference a recognized quality command".into(),
                description: "The scanned project has a recognized hook configuration, but its text does not reference a recognized build, lint, typecheck, test, or staged-file quality command. The hook may intentionally serve another purpose or call a wrapper the static matcher cannot resolve; it is not proof of a broken safeguard.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("A project hook config was found, but its commands do not mention build, lint, typecheck, test, lint-staged, or another recognizable quality gate.")),
                why_now: Some("If the team expects this hook to provide quality feedback, an unresolved or bookkeeping-only command may not deliver that feedback. Required CI remains the reliable enforcement point.".into()),
                likely_fix: Some("Confirm the hook's intended purpose and resolve any wrapper it calls. If it is meant to run quality checks, connect it to the fastest relevant command and leave slower authoritative checks in CI; otherwise document the purpose and mark this finding not applicable.".into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Static marker matching cannot resolve arbitrary shell wrappers, package indirection, global hooks, or a hook whose purpose is unrelated to code quality.".into()),
                verify_hint: Some("Execute the hook with a controlled passing and failing change, observe the effective command, and confirm required CI still catches the same failure if the local hook is skipped.".into()),
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
