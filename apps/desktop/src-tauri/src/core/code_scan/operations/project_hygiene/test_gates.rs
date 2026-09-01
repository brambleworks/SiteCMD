//! Test-infrastructure findings: none at all, or present but never run.

use super::project_kind::ProjectKind;
use super::quality_gates::{ci_workflow_paths, commit_hook_paths, QualitySignals};
use super::*;

pub(super) fn collect_test_infrastructure_issues(
    issues: &mut Vec<CodeIssue>,
    context: &ProjectHygieneContext<'_>,
    kind: &ProjectKind,
    signals: &QualitySignals,
) {
    if !kind.hygiene_eligible() {
        return;
    }

    if !signals.has_test_infrastructure {
        let Some((relative_path, absolute_path)) = kind.anchor.clone() else {
            return;
        };
        // Routes and data paths make missing coverage a Medium; a content site
        // without either can reasonably ship on a build and lint gate alone.
        let (severity, description, why_now) = if kind.app_like {
            (
                Severity::Medium,
                "The scanned project has routes, database access, or other app logic, but no recognized test files, runner configuration, test scripts, or test dependencies were found. Tests may exist outside the scanned project, use an unrecognized convention, or run through organization tooling, so this is not proof that all validation is manual.",
                "Automated coverage for a high-risk route or data path reduces the chance that a refactor, dependency change, or urgent fix silently breaks behavior.",
            )
        } else {
            (
                Severity::Low,
                "The scanned project builds a site but has no server routes or data access, and no recognized test files, runner configuration, test scripts, or test dependencies were found. Tests may exist outside the scanned project, use an unrecognized convention, or run through organization tooling. A content or marketing site can reasonably ship without a test suite as long as its build and lint gates run automatically.",
                "A small smoke test that builds the site and renders its key pages is the cheapest way to notice when an automated edit breaks a page nobody opens by hand.",
            )
        };
        let mut evidence = String::from("Within the scanned tree, no recognized *.test.*, *.spec.*, __tests__/, tests/, or test_*.py artifact, common runner config/dependency, or package test script was found.");
        if signals.placeholder_test_script {
            evidence.push_str(" The package.json test script is the npm placeholder (`echo ... && exit 1`), which exits without running anything.");
        }
        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("no-automated-tests:{}", relative_path),
            category: "architecture".into(),
            severity,
            title: "No recognized automated test infrastructure was found".into(),
            description: description.into(),
            relative_path,
            absolute_path,
            line: None,
            source_excerpt: None,
            evidence: Some(redact_evidence(evidence)),
            why_now: Some(why_now.into()),
            likely_fix: Some("First confirm whether tests run through a parent workspace, external repository, or unrecognized command. If meaningful coverage is absent, add the stack-appropriate runner and one observable test around the route, workflow, or data path with the highest failure cost, then expose a documented command for local and CI use.".into()),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("Test discovery is convention-based and limited to the scanned tree; external, generated, dynamically composed, or unusually named test infrastructure may exist.".into()),
            verify_hint: Some("From a clean environment, run the documented test command and confirm at least one real application behavior is exercised; if coverage lives elsewhere, record the owning project and command before marking this not applicable.".into()),
        });
        return;
    }

    // Only when a suite exists and some automation already runs: with neither
    // CI nor a hook, the ci-workflow-missing and pre-commit-hooks-missing
    // findings own the gap.
    let remote_enforced = signals.remote_enforced();
    if !signals.has_runnable_tests
        || !(remote_enforced || signals.hooks_enforced())
        || signals.ci_runs_tests
        || signals.hooks_run_tests
    {
        return;
    }
    let paths = context.project_paths_lower;
    let gate_path = if remote_enforced {
        ci_workflow_paths(paths).first().copied()
    } else {
        commit_hook_paths(paths).first().copied()
    };
    let anchor = gate_path
        .map(|path| {
            (
                path.to_string(),
                context.root.join(path).to_string_lossy().to_string(),
            )
        })
        .or_else(|| kind.manifest_anchor.clone());
    let Some((relative_path, absolute_path)) = anchor else {
        return;
    };
    let gate_summary = match (remote_enforced, signals.hooks_enforced()) {
        (true, true) => "the CI workflow and the checked-in hook run other quality commands",
        (true, false) => "the CI workflow runs other quality commands",
        _ => "the checked-in hook runs other quality commands",
    };
    issues.push(CodeIssue {
        check_id: String::new(),
        id: format!("tests-not-enforced:{}", relative_path),
        category: "operations".into(),
        severity: Severity::Medium,
        title: "Tests exist but nothing runs them automatically".into(),
        description: "The scanned project has recognized test files, configuration, or a test script, and it has CI or a commit hook that runs other quality commands, but none of those automated steps appear to run the tests. Reusable workflows, composite actions, task runners, or a differently named command may still run them outside this static match.".into(),
        relative_path,
        absolute_path,
        line: None,
        source_excerpt: None,
        evidence: Some(redact_evidence(format!("Recognized test infrastructure exists and {gate_summary}, but no recognized test command or call to the project's test script appears in those files."))),
        why_now: Some("A test suite only protects the site when it runs on every change. Tests that run only when someone remembers to start them stop catching regressions as soon as the code is edited by a tool or a teammate who does not run them.".into()),
        likely_fix: Some("Add the project's test command to the CI job that already runs, for example `npm test` after the build step, so a failing test blocks the merge, and consider a fast subset in the pre-push hook. If a reusable workflow or task runner already runs the tests, mark this finding not applicable.".into()),
        confidence: crate::checks::IssueConfidence::NeedsReview,
        confidence_reason: Some("Static command matching cannot resolve every reusable workflow, composite action, task runner, or script indirection that might run the tests.".into()),
        verify_hint: Some("Introduce a deliberately failing test on a branch and confirm CI or the hook fails on it, then remove it and confirm the run passes again.".into()),
    });
}
