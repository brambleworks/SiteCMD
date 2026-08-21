use super::*;
use std::sync::LazyLock;

const WORKFLOW_FILE_MAX_BYTES: u64 = 250_000;
const MAX_LISTED_REFS: usize = 8;

/// First-party owners contribute evidence but never trigger alone.
const FIRST_PARTY_ACTION_OWNERS: &[&str] = &["actions", "github"];

pub(super) fn collect_workflow_pinning_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
) {
    for file in project_files {
        if !is_workflow_path(&file.relative_path) {
            continue;
        }
        let Some(bytes) = read_project_file(file, WORKFLOW_FILE_MAX_BYTES) else {
            continue;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };

        // Lines inside `run:` block scalars are shell, not workflow YAML.
        let script_lines = run_script_line_numbers(&content);
        let lines: Vec<&str> = content.lines().collect();
        let mut third_party_unpinned: Vec<(u32, String)> = Vec::new();
        let mut first_party_unpinned = 0usize;
        for (index, line) in lines.iter().enumerate() {
            if script_lines.contains(&(index as u32 + 1)) {
                continue;
            }
            let spec = uses_value_from_line(line)
                .or_else(|| uses_continuation_value(line, lines.get(index + 1).copied()));
            let Some(spec) = spec else {
                continue;
            };
            match classify_uses_ref(&spec) {
                UsesPinning::Local | UsesPinning::Pinned => {}
                UsesPinning::UnpinnedFirstParty => first_party_unpinned += 1,
                UsesPinning::UnpinnedThirdParty => {
                    third_party_unpinned.push((index as u32 + 1, spec));
                }
            }
        }
        if third_party_unpinned.is_empty() {
            continue;
        }

        let first_line = third_party_unpinned.first().map(|(line, _)| *line);
        let mut listed = third_party_unpinned
            .iter()
            .take(MAX_LISTED_REFS)
            .map(|(line, spec)| format!("'{}' (line {})", spec, line))
            .collect::<Vec<_>>()
            .join(", ");
        if third_party_unpinned.len() > MAX_LISTED_REFS {
            listed.push_str(&format!(
                ", and {} more",
                third_party_unpinned.len() - MAX_LISTED_REFS
            ));
        }
        let mut evidence = format!(
            "Third-party actions referenced by tag or branch instead of a full commit SHA: {}.",
            listed
        );
        if first_party_unpinned > 0 {
            evidence.push_str(&format!(
                " {} first-party actions/* or github/* {} also {} tags; pinning those to commit SHAs is recommended as well, but they did not trigger this issue.",
                first_party_unpinned,
                if first_party_unpinned == 1 { "reference" } else { "references" },
                if first_party_unpinned == 1 { "uses" } else { "use" },
            ));
        }

        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("unpinned-github-action:{}", file.relative_path),
            category: "supply-chain".into(),
            severity: Severity::Medium,
            title: "Workflow uses third-party GitHub Actions without commit pinning".into(),
            description: "This workflow references third-party actions by tag or branch instead of an immutable reference (a full 40-character commit SHA, or an @sha256: digest for docker:// images). Tags and branches are mutable: whoever controls the action can move them, and the changed code runs with this workflow's permissions and secrets on the next trigger. Pinning first-party actions/* and github/* references is also recommended, but third-party actions are the exposure that matters most.".into(),
            relative_path: file.relative_path.clone(),
            absolute_path: file.absolute_path.to_string_lossy().to_string(),
            line: first_line,
            source_excerpt: excerpt_for_line(&content, first_line),
            evidence: Some(redact_evidence(evidence)),
            why_now: Some("A mutable tag or branch lets a later upstream change run under this workflow's effective permissions without a reviewable reference change in this repository.".into()),
            likely_fix: Some("Pin each third-party action to a full 40-character commit SHA with a version comment, for example `uses: owner/action@<commit-sha> # v4`. For `docker://` references, pin to an image digest instead: `uses: docker://image@sha256:<digest>` (commit SHAs do not apply to container images). Then let Dependabot or Renovate raise pull requests that bump the pins.".into()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: Some("Confirm each third-party `uses:` reference resolves to the reviewed full commit SHA, or each `docker://` image to the reviewed digest, then run the workflow and configure update automation to propose future pin changes for review.".into()),
        });
    }
}

/// Flag workflows that grant the `GITHUB_TOKEN` the blanket `write-all` scope.
/// One finding per workflow file, anchored at the first offending line.
pub(super) fn collect_workflow_permission_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
) {
    for file in project_files {
        if !is_workflow_path(&file.relative_path) {
            continue;
        }
        let Some(bytes) = read_project_file(file, WORKFLOW_FILE_MAX_BYTES) else {
            continue;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };
        let script_lines = run_script_line_numbers(&content);
        let lines: Vec<&str> = content.lines().collect();
        let mut found: Option<usize> = None;
        for (index, line) in lines.iter().enumerate() {
            if script_lines.contains(&(index as u32 + 1)) {
                continue;
            }
            if line_grants_write_all(line) {
                found = Some(index);
                break;
            }
            // YAML also allows the scalar on the following line
            // (`permissions:` then `write-all`).
            let key_only = line.split('#').next().unwrap_or("").trim() == "permissions:";
            if key_only {
                if let Some(next) = lines.get(index + 1) {
                    let value = next
                        .split('#')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches(|c| c == '"' || c == '\'');
                    if value.eq_ignore_ascii_case("write-all") {
                        found = Some(index + 1);
                        break;
                    }
                }
            }
        }
        let Some(index) = found else {
            continue;
        };
        let line_no = index as u32 + 1;

        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("workflow-write-all-permissions:{}", file.relative_path),
            category: "supply-chain".into(),
            severity: Severity::Medium,
            title: "Workflow grants the GITHUB_TOKEN blanket write-all permissions".into(),
            description: "This workflow sets `permissions: write-all`, requesting write access for every available GITHUB_TOKEN permission at the declared workflow or job scope. Effective permissions can still be reduced by the event, fork policy, and repository or organization settings, but any step in the affected scope receives a broader token than most jobs require.".into(),
            relative_path: file.relative_path.clone(),
            absolute_path: file.absolute_path.to_string_lossy().to_string(),
            line: Some(line_no),
            source_excerpt: excerpt_for_line(&content, Some(line_no)),
            evidence: Some(redact_evidence(format!(
                "`permissions: write-all` is declared in {} (line {}); its YAML scope and event/repository permission reductions must be reviewed.",
                file.relative_path, line_no
            ))),
            why_now: Some("If a step in the affected scope is compromised or executes untrusted input, an over-scoped token can increase the repository, package, issue, deployment, or release actions available to it.".into()),
            likely_fix: Some("Replace `write-all` with the minimal permissions each job needs. Use a restrictive workflow-level default such as `contents: read` or `{}`, then grant a specific write only on the job that performs it. Preserve `id-token: write` only where an OIDC exchange is required, and review permissions of called reusable workflows too.".into()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: Some("Inspect the effective permissions in a real run for each job, confirm no `write-all` remains, and test that the intended release/deploy operation still succeeds while an unneeded write operation is denied.".into()),
        });
    }
}

/// Untrusted GitHub Actions expression paths that must not be interpolated into
/// `run:` scripts. Values routed through `env:` do not match this pattern.
static WORKFLOW_INJECTION_EXPRESSION: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"(?x)\$\{\{\s*(?:
              github\.head_ref\b
            | github\.event\.issue\.(?:title|body)\b
            | github\.event\.pull_request\.(?:title|body|head\.ref|head\.label|head\.repo\.default_branch)\b
            | github\.event\.(?:comment|review|review_comment)\.body\b
            | github\.event\.discussion\.(?:title|body)\b
            | github\.event\.pages(?:\[[^\]]*\]|\.\*)*\.page_name\b
            | github\.event\.commits(?:\[[^\]]*\]|\.\*)*\.message\b
            | github\.event\.head_commit\.(?:message\b|(?:author|committer)\.(?:name|email)\b)
            | github\.event\.workflow_run\.(?:head_branch\b|head_commit\.message\b|head_commit\.author\.(?:name|email)\b)
            )",
    )
    .expect("static workflow injection regex") // allow-expect: compile-time literal regex
});

/// Flag attacker-controllable `${{... }}` expressions interpolated inside
/// `run:` scripts. One finding per workflow file, anchored at the first
/// offending line.
pub(super) fn collect_workflow_injection_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
) {
    for file in project_files {
        if !is_workflow_path(&file.relative_path) {
            continue;
        }
        let Some(bytes) = read_project_file(file, WORKFLOW_FILE_MAX_BYTES) else {
            continue;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };

        let mut hits: Vec<(u32, String)> = Vec::new();
        for (line_no, line) in run_script_lines(&content) {
            for m in WORKFLOW_INJECTION_EXPRESSION.find_iter(line) {
                // Report the full `${{... }}` expression for evidence; the
                // regex match stops at the path for precision.
                let expression = line[m.start()..]
                    .split_inclusive("}}")
                    .next()
                    .unwrap_or(m.as_str())
                    .trim()
                    .to_string();
                hits.push((line_no, expression));
            }
        }
        if hits.is_empty() {
            continue;
        }

        let first_line = hits.first().map(|(line, _)| *line);
        let mut listed = hits
            .iter()
            .take(MAX_LISTED_REFS)
            .map(|(line, expression)| format!("`{}` (line {})", expression, line))
            .collect::<Vec<_>>()
            .join(", ");
        if hits.len() > MAX_LISTED_REFS {
            listed.push_str(&format!(", and {} more", hits.len() - MAX_LISTED_REFS));
        }

        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("workflow-script-injection:{}", file.relative_path),
            category: "supply-chain".into(),
            severity: Severity::High,
            title: "Workflow interpolates attacker-controllable input into a shell script".into(),
            description: "This workflow expands an event-derived GitHub Actions expression directly into `run:` script text before the shell parses it. Whether an attacker can control the value depends on the workflow trigger and actor: issue, comment, discussion, or pull-request text may be externally supplied, while push commit metadata generally requires repository write access. If an untrusted actor can trigger the path, shell metacharacters in the value can alter the command.".into(),
            relative_path: file.relative_path.clone(),
            absolute_path: file.absolute_path.to_string_lossy().to_string(),
            line: first_line,
            source_excerpt: excerpt_for_line(&content, first_line),
            evidence: Some(redact_evidence(format!(
                "Attacker-controllable {} interpolated inside run: scripts: {}.",
                if hits.len() == 1 { "expression" } else { "expressions" },
                listed
            ))),
            why_now: Some("Direct expression substitution crosses GitHub's template boundary into shell source. Impact depends on who can supply the event value and what token, secrets, network access, and commands the job has.".into()),
            likely_fix: Some("Move the expression into a step-level environment variable and reference that variable with shell-appropriate quoting, for example `env: { TITLE: ${{ github.event.issue.title }} }` and `printf '%s\\n' \"$TITLE\"`. Do not pass it to `eval` or rebuild shell source, and validate it against command-specific rules before giving it to a tool that interprets options, paths, templates, or code.".into()),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("Direct template-to-shell interpolation is present, but exploitability depends on the workflow trigger, actor permissions, event field, shell, quoting context, and effective job privileges.".into()),
            verify_hint: Some("Confirm no flagged event expression remains inside `run:` source, review the trigger and effective job permissions, and exercise the workflow in a safe test repository with a metacharacter-bearing event value to verify it is handled only as data.".into()),
        });
    }
}

/// Contributor-controlled refs that are unsafe to check out under privileged
/// `pull_request_target` or `workflow_run` triggers.
static PR_CONTROLLED_CHECKOUT_REF: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"(?x)
          \$\{\{\s*github\.(?:
                head_ref\b
              | event\.pull_request\.head\.(?:sha|ref)\b
              | event\.workflow_run\.head_(?:sha|branch)\b
            )
        | ref\s*:\s*refs/pull/",
    )
    .expect("static pr-target checkout ref regex") // allow-expect: compile-time literal regex
});

/// True when the workflow declares the `pull_request_target` trigger (the
/// substring is unambiguous - it never appears inside an expression, which
/// uses `github.event.pull_request`) or a `workflow_run:` trigger key.
fn workflow_has_privileged_pr_trigger(content: &str, script_lines: &HashSet<u32>) -> bool {
    for (index, line) in content.lines().enumerate() {
        if script_lines.contains(&(index as u32 + 1)) {
            continue;
        }
        let code = line.split('#').next().unwrap_or("");
        if code.contains("pull_request_target") {
            return true;
        }
        // workflow_run as a trigger is a mapping key; the expression form
        // (github.event.workflow_run...) is excluded so it never counts here.
        let key = code
            .trim_start()
            .trim_start_matches(['-', '[', ' '])
            .trim_start();
        if key.starts_with("workflow_run:") && !code.contains("github.event") {
            return true;
        }
    }
    false
}

/// Find a PR-controlled `ref:` only inside its `actions/checkout` step.
fn pr_controlled_checkout_ref_line(content: &str, script_lines: &HashSet<u32>) -> Option<u32> {
    let mut checkout_step_indent: Option<usize> = None;
    for (index, line) in content.lines().enumerate() {
        let line_no = index as u32 + 1;
        if script_lines.contains(&line_no) {
            continue;
        }
        let code = line.split('#').next().unwrap_or("");
        let trimmed = code.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let indent = code.len() - trimmed.len();
        if let Some(step_indent) = checkout_step_indent {
            // A new sequence item at or above the step's own indent ends the
            // checkout step (deeper-indented dashes are list values inside
            // `with:`, e.g. sparse-checkout paths).
            if trimmed.starts_with('-') && indent <= step_indent {
                checkout_step_indent = None;
            } else if trimmed.starts_with("ref:") && PR_CONTROLLED_CHECKOUT_REF.is_match(code) {
                return Some(line_no);
            }
        }
        if let Some(spec) = uses_value_from_line(code) {
            checkout_step_indent = if spec.starts_with("actions/checkout") {
                Some(indent)
            } else {
                None
            };
        }
    }
    None
}

/// Flag privileged-trigger workflows only when an actions/checkout step
/// directly consumes a pull-request-controlled `ref:`. A matching expression
/// elsewhere (for example safe `env:` indirection) is not checkout evidence.
pub(super) fn collect_workflow_pr_target_checkout_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
) {
    for file in project_files {
        if !is_workflow_path(&file.relative_path) {
            continue;
        }
        let Some(bytes) = read_project_file(file, WORKFLOW_FILE_MAX_BYTES) else {
            continue;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };

        let script_lines = run_script_line_numbers(&content);
        if !workflow_has_privileged_pr_trigger(&content, &script_lines) {
            continue;
        }
        // The dangerous ref only matters when an actual checkout consumes it.
        if !content.contains("actions/checkout") {
            continue;
        }
        let Some(line_no) = pr_controlled_checkout_ref_line(&content, &script_lines) else {
            continue;
        };

        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("workflow-pr-target-checkout:{}", file.relative_path),
            category: "supply-chain".into(),
            severity: Severity::High,
            title: "Privileged workflow checks out untrusted pull-request code".into(),
            description: "This workflow uses a privileged trigger (`pull_request_target` or `workflow_run`) and passes a pull-request-controlled ref to `actions/checkout`. If a later step executes files from that checkout, including package lifecycle scripts, builds, tests, local actions, or scripts, the untrusted code can run with any secrets, token permissions, network access, and other privileges available to that job. Checkout alone does not execute the code, and effective privileges depend on repository and workflow settings.".into(),
            relative_path: file.relative_path.clone(),
            absolute_path: file.absolute_path.to_string_lossy().to_string(),
            line: Some(line_no),
            source_excerpt: excerpt_for_line(&content, Some(line_no)),
            evidence: Some(redact_evidence(format!(
                "A pull_request_target/workflow_run workflow passes a PR-controlled ref to actions/checkout at line {}.",
                line_no
            ))),
            why_now: Some("The dangerous boundary is the combination of a privileged job, an untrusted checkout, and a later execution path; reviewing all three prevents pull-request content from inheriting base-repository authority.".into()),
            likely_fix: Some("Use the plain `pull_request` trigger for jobs that build or execute pull-request code, with no write token or secrets. Keep `pull_request_target` only for base-branch automation that does not check out or execute the PR head. If a multi-stage design is required, treat artifacts from the untrusted stage as hostile data: validate formats and paths, avoid executable artifacts, and grant the privileged stage only the minimum permissions it needs.".into()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: Some("Confirm no privileged-trigger job checks out a PR-controlled ref before executing workspace content. Inspect the real run's token permissions and secrets, and verify an untrusted PR build runs only in an unprivileged job.".into()),
        });
    }
}

/// The lines of `content` that belong to `run:` scripts, as
/// (1-based line number, line) pairs. Handles inline scalars
/// (`run: echo hi`) and block scalars (`run: |` / `run: >` with the script
/// on the following, deeper-indented lines).
fn run_script_lines(content: &str) -> Vec<(u32, &str)> {
    let mut out = Vec::new();
    let mut block_indent: Option<usize> = None;
    for (index, line) in content.lines().enumerate() {
        let line_no = index as u32 + 1;
        let trimmed_start = line.trim_start();
        if let Some(indent) = block_indent {
            if trimmed_start.is_empty() {
                continue; // blank lines stay inside a block scalar
            }
            if line.len() - trimmed_start.len() > indent {
                out.push((line_no, line));
                continue;
            }
            block_indent = None;
        }
        // `- run:` or `run:`; the key column governs the block's indent.
        let after_dash = trimmed_start
            .strip_prefix('-')
            .map(str::trim_start)
            .unwrap_or(trimmed_start);
        let Some(value) = after_dash.strip_prefix("run:") else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || value.starts_with('|') || value.starts_with('>') {
            let key_column = line.len() - after_dash.len();
            block_indent = Some(key_column);
        } else {
            out.push((line_no, line));
        }
    }
    out
}

/// 1-based line numbers of every `run:` block-scalar/inline script line.
fn run_script_line_numbers(content: &str) -> std::collections::HashSet<u32> {
    run_script_lines(content)
        .iter()
        .map(|(line_no, _)| *line_no)
        .collect()
}

/// Read a `uses:` value continued onto the next YAML line.
fn uses_continuation_value(line: &str, next_line: Option<&str>) -> Option<String> {
    let trimmed = line.trim_start();
    let trimmed = trimmed
        .strip_prefix('-')
        .map(str::trim_start)
        .unwrap_or(trimmed);
    let rest = trimmed.strip_prefix("uses:")?.trim();
    if !rest.is_empty() {
        return None;
    }
    let next = next_line?.trim_start();
    let value = if let Some(inner) = next.strip_prefix('"') {
        inner.split('"').next()?
    } else if let Some(inner) = next.strip_prefix('\'') {
        inner.split('\'').next()?
    } else {
        next.split_whitespace().next()?
    };
    let value = value.trim();
    if value.is_empty() || value.ends_with(':') || value.starts_with('#') || value.starts_with('-')
    {
        return None;
    }
    Some(value.to_string())
}

fn is_workflow_path(relative_path: &str) -> bool {
    let normalized = relative_path.to_ascii_lowercase();
    (normalized.ends_with(".yml") || normalized.ends_with(".yaml"))
        && (normalized.starts_with(".github/workflows/")
            || normalized.contains("/.github/workflows/"))
}

/// Extract the value of a `uses:` line (`- uses: owner/action@ref`,
/// quoted or unquoted, with optional trailing comment). Returns `None`
/// for lines that are not `uses:` entries.
fn uses_value_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let trimmed = trimmed
        .strip_prefix('-')
        .map(str::trim_start)
        .unwrap_or(trimmed);
    let rest = trimmed.strip_prefix("uses:")?.trim_start();
    let value = if let Some(inner) = rest.strip_prefix('"') {
        inner.split('"').next()?
    } else if let Some(inner) = rest.strip_prefix('\'') {
        inner.split('\'').next()?
    } else {
        rest.split_whitespace().next()?
    };
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

enum UsesPinning {
    Local,
    Pinned,
    UnpinnedFirstParty,
    UnpinnedThirdParty,
}

fn classify_uses_ref(spec: &str) -> UsesPinning {
    if spec.starts_with("./") || spec.starts_with(".\\") {
        return UsesPinning::Local;
    }
    if let Some(image) = spec.strip_prefix("docker://") {
        // A digest-addressed image is immutable; a tag-addressed one is not.
        return if image.contains("@sha256:") {
            UsesPinning::Pinned
        } else {
            UsesPinning::UnpinnedThirdParty
        };
    }

    let (action, reference) = match spec.split_once('@') {
        Some((action, reference)) => (action, Some(reference)),
        None => (spec, None),
    };
    let owner = action.split('/').next().unwrap_or(action);
    let first_party = FIRST_PARTY_ACTION_OWNERS
        .iter()
        .any(|candidate| owner.eq_ignore_ascii_case(candidate));
    if reference.is_some_and(is_full_commit_sha) {
        return UsesPinning::Pinned;
    }
    if first_party {
        UsesPinning::UnpinnedFirstParty
    } else {
        UsesPinning::UnpinnedThirdParty
    }
}

fn is_full_commit_sha(reference: &str) -> bool {
    reference.len() == 40 && reference.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Match only blanket `write-all`; specific write scopes remain valid.
fn line_grants_write_all(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix("permissions:") else {
        return false;
    };
    let value = rest.split('#').next().unwrap_or(rest).trim();
    let value = value.trim_matches(|c| c == '"' || c == '\'').trim();
    value.eq_ignore_ascii_case("write-all")
}

#[cfg(test)]
mod tests {
    use super::{
        classify_uses_ref, is_workflow_path, line_grants_write_all, run_script_line_numbers,
        uses_continuation_value, uses_value_from_line, UsesPinning,
    };

    #[test]
    fn run_block_scalar_lines_are_not_workflow_yaml() {
        let content = "jobs:\n  gen:\n    steps:\n      - run: |\n          cat > w.yml <<'EOF'\n          uses: owner/action@v4\n          permissions: write-all\n          EOF\n      - uses: real/action@v1\n";
        let script = run_script_line_numbers(content);
        assert!(script.contains(&6), "generated uses: line is script");
        assert!(script.contains(&7), "generated permissions: line is script");
        assert!(!script.contains(&9), "the real uses: step is workflow YAML");
    }

    #[test]
    fn uses_value_on_the_next_line_is_parsed() {
        assert_eq!(
            uses_continuation_value("      - uses:", Some("          owner/action@v4")),
            Some("owner/action@v4".to_string())
        );
        // A following mapping key is not a value.
        assert_eq!(
            uses_continuation_value("      - uses:", Some("        with:")),
            None
        );
        // Inline values are handled by the single-line parser instead.
        assert_eq!(
            uses_continuation_value("      - uses: owner/action@v4", Some("        with:")),
            None
        );
    }

    #[test]
    fn write_all_permission_detection_is_precise() {
        // The blanket over-grant, in its common shapes, fires.
        assert!(line_grants_write_all("permissions: write-all"));
        assert!(line_grants_write_all("  permissions: write-all"));
        assert!(line_grants_write_all(
            "permissions: write-all # loosened for release"
        ));
        assert!(line_grants_write_all("  permissions: 'write-all'"));
        assert!(line_grants_write_all("permissions: \"write-all\""));

        // read-all and least-privilege block forms are fine.
        assert!(!line_grants_write_all("permissions: read-all"));
        // Block form: the value follows on later, per-scope lines.
        assert!(!line_grants_write_all("permissions:"));
        assert!(!line_grants_write_all("permissions: {}"));
        // A specific write scope is legitimately needed by deploy workflows.
        assert!(!line_grants_write_all("  contents: write"));

        // Not a permissions declaration at all.
        assert!(!line_grants_write_all(
            "      run: echo \"permissions: write-all\""
        ));
        assert!(!line_grants_write_all("# permissions: write-all"));
    }

    fn is_third_party_unpinned(spec: &str) -> bool {
        matches!(classify_uses_ref(spec), UsesPinning::UnpinnedThirdParty)
    }

    #[test]
    fn unpinned_uses_line_parsing_handles_common_shapes() {
        assert_eq!(
            uses_value_from_line("      - uses: owner/action@v4"),
            Some("owner/action@v4".to_string())
        );
        assert_eq!(
            uses_value_from_line("        uses: \"owner/action@main\" # comment"),
            Some("owner/action@main".to_string())
        );
        assert_eq!(
            uses_value_from_line("    uses: owner/action@v4 # pinned later"),
            Some("owner/action@v4".to_string())
        );
        // Not a uses: line at all (including commented-out entries).
        assert_eq!(uses_value_from_line("      # uses: owner/action@v4"), None);
        assert_eq!(uses_value_from_line("      run: echo uses"), None);
    }

    #[test]
    fn unpinned_ref_classification_matches_the_pinning_policy() {
        // Tag / branch / missing ref on a third-party action triggers.
        assert!(is_third_party_unpinned("owner/action@v4"));
        assert!(is_third_party_unpinned("owner/action@main"));
        assert!(is_third_party_unpinned("owner/action"));
        // Reusable workflow refs count as third-party too.
        assert!(is_third_party_unpinned(
            "owner/repo/.github/workflows/ci.yml@master"
        ));

        // A full 40-char commit SHA is pinned.
        assert!(matches!(
            classify_uses_ref("owner/action@8f4b7f84864484a7bf31766abe9204da3cbe65b3"),
            UsesPinning::Pinned
        ));
        // A short SHA is not a full pin.
        assert!(is_third_party_unpinned("owner/action@8f4b7f8"));

        // First-party unpinned refs are the low-signal bucket, not a trigger.
        assert!(matches!(
            classify_uses_ref("actions/checkout@v4"),
            UsesPinning::UnpinnedFirstParty
        ));
        assert!(matches!(
            classify_uses_ref("github/codeql-action/analyze@v3"),
            UsesPinning::UnpinnedFirstParty
        ));

        // Local composite actions and digest-addressed docker images are skipped.
        assert!(matches!(
            classify_uses_ref("./.github/actions/setup"),
            UsesPinning::Local
        ));
        assert!(matches!(
            classify_uses_ref(
                "docker://alpine@sha256:c5b1261d6d3e43071626931fc004f70149baeba2c8ec672bd4f27761f8e1ad6b"
            ),
            UsesPinning::Pinned
        ));
        // A tag-addressed docker image is mutable and counts as unpinned.
        assert!(is_third_party_unpinned("docker://alpine:3.19"));
    }

    #[test]
    fn unpinned_workflow_path_matching_covers_yml_and_yaml() {
        assert!(is_workflow_path(".github/workflows/deploy.yml"));
        assert!(is_workflow_path(".github/workflows/release.yaml"));
        assert!(is_workflow_path("apps/web/.github/workflows/ci.yml"));
        assert!(!is_workflow_path(".github/dependabot.yml"));
        assert!(!is_workflow_path(".github/workflows/README.md"));
        assert!(!is_workflow_path("workflows/deploy.yml"));
    }
}
