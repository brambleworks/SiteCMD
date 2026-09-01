//! Hook findings: no project-managed hook, a hook that runs nothing useful, or
//! a checked-in hook that this clone never installed.

use super::project_kind::ProjectKind;
use super::quality_gates::{commit_hook_paths, QualitySignals};
use super::*;

pub(super) fn collect_hook_issues(
    issues: &mut Vec<CodeIssue>,
    context: &ProjectHygieneContext<'_>,
    kind: &ProjectKind,
    signals: &QualitySignals,
) {
    if !kind.hygiene_eligible() {
        return;
    }
    // With CI running quality commands, a hook is a convenience; without it,
    // the hook is the only automated gate, so its absence grades Medium.
    let remote_enforced = signals.remote_enforced();
    let graded = |extra_relief: bool| {
        if remote_enforced || extra_relief {
            Severity::Low
        } else {
            Severity::Medium
        }
    };

    if !signals.has_commit_hooks
        && (signals.has_build_script
            || signals.has_lint_or_typecheck_script
            || signals.has_test_script
            || signals.has_linter_config)
    {
        if let Some((relative_path, absolute_path)) = kind.manifest_anchor.clone() {
            let local_hooks = &signals.install.native_hooks;
            let mut evidence = String::from("No .husky hook, lefthook config, pre-commit config, lint-staged config, or package.json prepare/simple-git-hooks hook was found.");
            if !local_hooks.is_empty() {
                evidence.push_str(&format!(
                    " A hook is installed in this clone only (.git/hooks/{}); other clones and CI do not receive it.",
                    local_hooks.join(", .git/hooks/")
                ));
            }
            let (description_tail, why_now, likely_fix) = if remote_enforced {
                (
                    "Hooks are optional and can be bypassed; the CI quality gate remains the authoritative check.",
                    "An optional fast local hook can shorten feedback time, but it is bypassable and is not a substitute for required CI checks.",
                    "First confirm that required CI already protects the branch. If the team wants faster local feedback, add a lightweight project-managed hook for touched-file linting or focused checks, document the bypass behavior, and keep authoritative gates in CI. Mark this not applicable when hooks are intentionally avoided.",
                )
            } else {
                (
                    "No CI quality gate was found either, so nothing runs the project's build, lint, or test commands automatically before a change is committed or pushed.",
                    "With no CI gate, a project-managed hook is the only thing standing between an automated edit and a broken push. Without it, every commit depends on someone remembering to run the checks by hand.",
                    "Add a lightweight project-managed hook (Lefthook, Husky with lint-staged, pre-commit, or simple-git-hooks) that runs the fastest useful check, such as lint on touched files before commit and the build or tests before push, and commit its config so every clone shares it. Then add a CI workflow so the same checks also run away from the machine that made the change.",
                )
            };
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("pre-commit-hooks-missing:{}", relative_path),
                category: "operations".into(),
                severity: graded(!local_hooks.is_empty()),
                title: "No recognized project-managed pre-commit or pre-push hook was found".into(),
                description: format!("The project has local quality commands, but the scanned tree contains no recognized Husky, Lefthook, pre-commit, lint-staged, or package-manager hook configuration. {description_tail}"),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence(evidence)),
                why_now: Some(why_now.into()),
                likely_fix: Some(likely_fix.into()),
                confidence: crate::checks::IssueConfidence::NeedsReview,
                confidence_reason: Some("Hook discovery is limited to recognized project files; global, organization-managed, custom, or intentionally absent hooks cannot be evaluated.".into()),
                verify_hint: Some("If a hook is adopted, make a harmless staged change and a controlled failing change to confirm it runs locally, then confirm CI still enforces the authoritative check when hooks are bypassed.".into()),
            });
        }
    }

    if signals.has_commit_hooks && signals.has_quality_scripts && !signals.hooks_have_quality_gate {
        if let Some((relative_path, absolute_path)) = hook_anchor(context, kind) {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("pre-commit-hooks-weak:{}", relative_path),
                category: "operations".into(),
                severity: graded(false),
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

    if signals.has_commit_hooks && signals.install.git_dir && !signals.install.active() {
        if let Some((relative_path, absolute_path)) = hook_anchor(context, kind) {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("pre-commit-hooks-not-installed:{}", relative_path),
                category: "operations".into(),
                severity: graded(false),
                title: "The checked-in git hook is not installed in this clone".into(),
                description: "The project has a hook configuration, but this clone has no active hook: `.git/hooks` holds no hook script, no Husky runtime directory exists, and `.git/config` sets no `core.hooksPath`. The hook therefore does not run here even though its config is committed. A hooks path configured in global git settings outside the project would not be visible to this check.".into(),
                relative_path: relative_path.clone(),
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence(format!("Hook config at {relative_path} was found, but .git/hooks contains no pre-commit, pre-push, or commit-msg script, .husky/_ does not exist, and .git/config does not set core.hooksPath."))),
                why_now: Some("An uninstalled hook gives the impression of a guardrail without providing one. Commits and pushes from this clone skip the checks the config promises, which is exactly the gap an automated editing session needs closed.".into()),
                likely_fix: Some("Run the tool's install step in this clone: `npm install` (which runs the `prepare` script) or `npx husky` for Husky, `npx lefthook install` for Lefthook, `pre-commit install` for pre-commit, or `npx simple-git-hooks`. Then make a harmless staged change and confirm the hook runs.".into()),
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: Some("The absence of native hook scripts, a Husky runtime directory, and a configured hooks path is directly observed in this clone; a hooks path set in global git configuration is not visible from the project.".into()),
                verify_hint: Some("After installing, confirm `.git/hooks/pre-commit` or `.git/hooks/pre-push` exists or `git config core.hooksPath` prints the hook directory, then confirm a deliberately failing change is blocked.".into()),
            });
        }
    }
}

fn hook_anchor(
    context: &ProjectHygieneContext<'_>,
    kind: &ProjectKind,
) -> Option<(String, String)> {
    commit_hook_paths(context.project_paths_lower)
        .first()
        .map(|path| {
            (
                (*path).to_string(),
                context.root.join(*path).to_string_lossy().to_string(),
            )
        })
        .or_else(|| kind.manifest_anchor.clone())
}
