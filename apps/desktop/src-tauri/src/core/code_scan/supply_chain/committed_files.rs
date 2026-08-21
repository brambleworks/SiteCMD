//! Supply-chain checks over project configuration and build artifacts.

use super::*;
use std::sync::LazyLock;

const COMMITTED_FILE_MAX_BYTES: u64 = 250_000;
const MAX_LISTED_LINES: usize = 8;

/// Literal npm authentication values, excluding environment substitutions.
static NPMRC_AUTH_LINE: LazyLock<regex::Regex> = LazyLock::new(|| {
    // Match scoped and legacy npm auth forms without crossing lines or comments.
    regex::Regex::new(r"(?im)^[ \t]*(?:[^#;\r\n]*:)?_(?:authToken|auth|password)[ \t]*=[ \t]*(\S+)")
        .expect("static npmrc auth regex") // allow-expect: compile-time literal regex
});

/// Remote scripts piped directly into a shell without integrity verification.
static REMOTE_PIPE_TO_SHELL: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\b(?:curl|wget)\b[^|\r\n]{0,200}\|\s*(?:sudo\s+)?(?:ba|z|da)?sh\b")
        .expect("static pipe-to-shell regex") // allow-expect: compile-time literal regex
});

fn is_npmrc_path(relative_path: &str) -> bool {
    let lower = relative_path.to_ascii_lowercase();
    lower == ".npmrc" || lower.ends_with("/.npmrc")
}

fn is_dockerfile_path(relative_path: &str) -> bool {
    let lower = relative_path.to_ascii_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    base == "dockerfile" || base.starts_with("dockerfile.") || base.ends_with(".dockerfile")
}

fn read_utf8_project_file(file: &ProjectFile) -> Option<String> {
    let bytes = read_project_file(file, COMMITTED_FILE_MAX_BYTES)?;
    String::from_utf8(bytes).ok()
}

pub(super) fn collect_npmrc_token_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
) {
    for file in project_files {
        if !is_npmrc_path(&file.relative_path) {
            continue;
        }
        let Some(content) = read_utf8_project_file(file) else {
            continue;
        };

        let mut first_line: Option<u32> = None;
        for captures in NPMRC_AUTH_LINE.captures_iter(&content) {
            let value = captures.get(1).map(|m| m.as_str()).unwrap_or("");
            // Environment substitution is the safe, standard form.
            if value.starts_with("${") {
                continue;
            }
            let offset = captures.get(0).map(|m| m.start()).unwrap_or(0);
            first_line = Some(line_number(&content, offset));
            break;
        }
        let Some(line) = first_line else {
            continue;
        };

        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("npmrc-committed-token:{}", file.relative_path),
            category: "supply-chain".into(),
            severity: Severity::High,
            title: "Project .npmrc contains a literal registry auth value".into(),
            description: "This .npmrc sets `_authToken`, `_auth`, or `_password` to a literal instead of an environment-variable substitution. The scan does not establish whether the file is tracked, the value is a real live credential, or which registry privileges it grants; an untracked local file, placeholder, encoded credential, and publish-capable token have different exposure and impact.".into(),
            relative_path: file.relative_path.clone(),
            absolute_path: file.absolute_path.to_string_lossy().to_string(),
            line: Some(line),
            // The excerpt would echo the credential itself, so it is omitted.
            source_excerpt: None,
            evidence: Some(redact_evidence(format!(
                "A literal _authToken/_auth/_password value is set in {} (line {}). The value is not shown here.",
                file.relative_path, line
            ))),
            why_now: Some("If the value is real and the file is tracked, shared, backed up, or copied into CI, the registry credential can spread to every reader or runner and may permit package download or publication according to its scope.".into()),
            likely_fix: Some("Determine whether the file is tracked/shared and whether the value is a real credential without printing it. If the credential was shared, committed, logged, or otherwise exposed, revoke or rotate it first. Replace the literal with a registry-specific environment substitution such as `//registry.npmjs.org/:_authToken=${NPM_TOKEN}` and supply the variable through the local or CI secret store.".into()),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("A literal registry-auth assignment is directly present, but the scan cannot determine repository tracking, credential validity, registry scope, or prior sharing.".into()),
            verify_hint: Some("Confirm the file now contains only environment substitution, inspect version-control history and CI artifacts under the incident policy, and, when the old value was real and exposed, verify that it no longer authenticates. Test install/publish behavior with a least-privilege replacement in an isolated package or dry-run workflow.".into()),
        });
    }
}

pub(super) fn collect_dockerfile_pinning_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
) {
    for file in project_files {
        if !is_dockerfile_path(&file.relative_path) {
            continue;
        }
        let Some(content) = read_utf8_project_file(file) else {
            continue;
        };

        let mut stage_names: Vec<String> = Vec::new();
        let mut unpinned: Vec<(u32, String)> = Vec::new();
        for (index, line) in content.lines().enumerate() {
            let mut parts = line.split_whitespace();
            if !parts
                .next()
                .is_some_and(|token| token.eq_ignore_ascii_case("FROM"))
            {
                continue;
            }
            let mut image = match parts.next() {
                Some(part) => part,
                None => continue,
            };
            if image.starts_with("--platform") {
                image = match parts.next() {
                    Some(part) => part,
                    None => continue,
                };
            }
            // Record build-stage aliases so `FROM <stage>` never counts.
            let rest: Vec<&str> = parts.collect();
            if rest.len() == 2 && rest[0].eq_ignore_ascii_case("as") {
                stage_names.push(rest[1].to_ascii_lowercase());
            }

            if image.contains("@sha256:") {
                continue; // digest-pinned: immutable
            }
            if image.contains('$') {
                continue; // build-arg base: cannot be judged statically
            }
            let lower_image = image.to_ascii_lowercase();
            if lower_image == "scratch" || stage_names.contains(&lower_image) {
                continue;
            }
            // A `:` only counts as a tag separator when the part after it has
            // no `/` (myregistry:5000/app is a registry port, still tagless).
            let tag = image
                .rsplit_once(':')
                .map(|(_, tag)| tag)
                .filter(|tag| !tag.contains('/'));
            match tag {
                // Flag only the moving `:latest` tag and the implicit default.
                Some(tag) if !tag.eq_ignore_ascii_case("latest") => {}
                _ => unpinned.push((index as u32 + 1, image.to_string())),
            }
        }
        if unpinned.is_empty() {
            continue;
        }

        let first_line = unpinned.first().map(|(line, _)| *line);
        let listed = unpinned
            .iter()
            .take(MAX_LISTED_LINES)
            .map(|(line, image)| format!("`{}` (line {})", image, line))
            .collect::<Vec<_>>()
            .join(", ");

        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("dockerfile-unpinned-base:{}", file.relative_path),
            category: "supply-chain".into(),
            severity: Severity::Medium,
            title: "Dockerfile base image floats on :latest or has no tag".into(),
            description: "This Dockerfile uses a base image with no tag or with `:latest`, so each build resolves the registry's current value for that moving reference. The resulting bytes can change without a Dockerfile edit; whether that image is deployed and what trust or signing controls apply were not inspected.".into(),
            relative_path: file.relative_path.clone(),
            absolute_path: file.absolute_path.to_string_lossy().to_string(),
            line: first_line,
            source_excerpt: excerpt_for_line(&content, first_line),
            evidence: Some(redact_evidence(format!(
                "Unpinned base image {}: {}.",
                if unpinned.len() == 1 { "reference" } else { "references" },
                listed
            ))),
            why_now: Some("A moving base reference weakens build reproducibility and can introduce upstream regressions or compromised bytes into the next build that consumes it without a reviewable reference change.".into()),
            likely_fix: Some("Choose a maintained, minimal base image. Use a specific version tag to avoid implicit `latest`; for reproducible bytes, pin the reviewed digest as well and use update automation to propose digest and version changes. Apply the registry's signature or provenance policy instead of treating a tag as immutable.".into()),
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            verify_hint: Some("Build twice from a clean cache, inspect the resolved image digests and provenance, confirm each external `FROM` has the intended version/digest policy, and run the image's security and behavior checks.".into()),
        });
    }
}

pub(super) fn collect_pipe_to_shell_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
    manifests: &[PackageManifest],
) {
    // Executable build surfaces where a fetched script may run: active-looking
    // Dockerfile/CI lines and parsed package.json script values. Comments and
    // non-script manifest metadata are deliberately excluded.
    for file in project_files {
        let lower = file.relative_path.to_ascii_lowercase();
        let is_workflow = (lower.ends_with(".yml") || lower.ends_with(".yaml"))
            && (lower.starts_with(".github/workflows/") || lower.contains("/.github/workflows/"));
        if !is_dockerfile_path(&file.relative_path) && !is_workflow {
            continue;
        }
        if let Some(content) = read_utf8_project_file(file) {
            let matched = content.lines().enumerate().find_map(|(index, line)| {
                let trimmed = line.trim_start();
                (!trimmed.starts_with('#') && REMOTE_PIPE_TO_SHELL.is_match(line))
                    .then_some(index as u32 + 1)
            });
            if let Some(line) = matched {
                push_pipe_to_shell_issue(
                    issues,
                    &file.relative_path,
                    &file.absolute_path.to_string_lossy(),
                    line,
                    "an active-looking Dockerfile or workflow line",
                );
            }
        }
    }
    for manifest in manifests {
        let Ok(json) = serde_json::from_str::<Value>(&manifest.content) else {
            continue;
        };
        let Some((script_name, command)) =
            json.get("scripts")
                .and_then(Value::as_object)
                .and_then(|scripts| {
                    scripts.iter().find_map(|(name, value)| {
                        let command = value.as_str()?;
                        REMOTE_PIPE_TO_SHELL
                            .is_match(command)
                            .then_some((name.as_str(), command))
                    })
                })
        else {
            continue;
        };
        let line = find_line(&manifest.content, &format!("\"{script_name}\""))
            .or_else(|| find_line(&manifest.content, command))
            .unwrap_or(1);
        push_pipe_to_shell_issue(
            issues,
            &manifest.relative_path,
            &manifest.absolute_path.to_string_lossy(),
            line,
            &format!("package script `{script_name}`"),
        );
    }
}

fn push_pipe_to_shell_issue(
    issues: &mut Vec<CodeIssue>,
    relative_path: &str,
    absolute_path: &str,
    line: u32,
    surface: &str,
) {
    issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("remote-pipe-to-shell:{}", relative_path),
            category: "supply-chain".into(),
            severity: Severity::High,
            title: "Build command appears to pipe a remote download into a shell".into(),
            description: "SiteCMD found a curl/wget-to-shell text pattern in an executable build surface. If the shell reaches this command, downloaded bytes execute without an independently pinned integrity check in the pipeline. Static matching does not evaluate shell control flow, quoting, functions, aliases, runtime reachability, URL ownership, transport policy, or effective build privileges.".into(),
            relative_path: relative_path.to_string(),
            absolute_path: absolute_path.to_string(),
            line: Some(line),
            source_excerpt: None,
            evidence: Some(redact_evidence(format!(
                "A curl/wget-to-shell pattern appears in {} in {} (line {}); the URL and command text are omitted from issue evidence.",
                surface, relative_path, line
            ))),
            why_now: Some("If this path runs, a mutable or compromised installer origin can change executed build code without a corresponding project diff and may inherit the job's filesystem, network, token, or secret access. The surfaced text must be reviewed as executable shell before that impact is assumed.".into()),
            likely_fix: Some("Prefer a trusted package or pinned container when available. Otherwise download to a file, verify an independently sourced and review-pinned digest or signature before execution, inspect the script during updates, and run it with minimal privileges and no unnecessary secrets. A checksum downloaded from the same mutable location is not an independent control.".into()),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("The pattern is inside a recognized executable surface, but text matching does not parse shell control flow or prove that the command is reachable and executed.".into()),
            verify_hint: Some("In an isolated build, substitute altered bytes and confirm execution is refused before the shell starts. Then verify the approved artifact succeeds with the pinned digest/signature and the step has only the permissions and secrets it requires.".into()),
        });
}
