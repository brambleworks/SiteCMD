//! Detects hardcoded credentials in AI and MCP configuration files.

use super::*;

pub(super) fn collect_config_secret_issues(
    issues: &mut Vec<CodeIssue>,
    seen_ids: &mut HashSet<String>,
    project_files: &[ProjectFile],
    text_budget: &mut ScanTextBudget<'_>,
) -> Result<(), CodeScanError> {
    for artifact in collect_ai_config_files(project_files, text_budget)? {
        if let Some((line, kind)) = find_hardcoded_config_secret(&artifact.content) {
            let id = format!("config-secret:{}", artifact.relative_path);
            if seen_ids.insert(id.clone()) {
                // Provider-shaped literals still need review because the scan cannot
                // prove usability, tracking, sharing, or deployment.
                let (confidence, confidence_reason) = match kind {
                    ConfigSecretKind::ValueShaped => (
                        crate::checks::IssueConfidence::NeedsReview,
                        Some(
                            "A provider-shaped literal is directly present, but the scan cannot establish whether it is genuine, live, privileged, tracked, shared, or deployed."
                                .to_string(),
                        ),
                    ),
                    ConfigSecretKind::NameValueHeuristic => (
                        crate::checks::IssueConfidence::NeedsReview,
                        Some(
                            "Matched a name-value credential heuristic; placeholder and public values match too, so confirm the value is a real secret."
                                .to_string(),
                        ),
                    ),
                };
                let evidence = match kind {
                    ConfigSecretKind::ValueShaped => format!(
                        "Line {} matches a known credential value format (a provider token shape such as sk-, ghp_, or AIza). The literal value is not shown here.",
                        line
                    ),
                    ConfigSecretKind::NameValueHeuristic => format!(
                        "Line {} assigns a secret-named config key (api key, access token, or secret) a literal value. The pattern also matches placeholders, and the literal value is not shown here.",
                        line
                    ),
                };
                issues.push(CodeIssue {
                    check_id: String::new(),
                    id,
                    category: "supply-chain".into(),
                    severity: Severity::High,
                    title: "AI or MCP config may contain a hardcoded credential".into(),
                    description: "This config file contains a provider-shaped value or a secret-named literal. It may be a plaintext credential, but static source does not verify that the value is genuine, live, privileged, tracked, shared, or deployed; placeholders, public identifiers, revoked values, and local-only credentials can also match.".into(),
                    relative_path: artifact.relative_path.clone(),
                    absolute_path: artifact.absolute_path.to_string_lossy().to_string(),
                    line: Some(line),
                    source_excerpt: excerpt_for_line(&artifact.content, Some(line)),
                    // Never echo the matched value: report the location only. The
                    // redacted source excerpt above carries enough context.
                    evidence: Some(redact_evidence(evidence)),
                    why_now: Some("Plaintext config values are commonly copied, synchronized, screenshared, or committed. A real credential can therefore spread beyond its intended machine even when the file began as local configuration.".into()),
                    likely_fix: Some("Classify the matched value without exposing it further. If it is real and may have been tracked, shared, logged, or deployed, revoke or rotate it first; then remove the literal and load the replacement from an environment variable, secret manager, or keychain-backed flow appropriate to the tool. Mark an unmistakably fake fixture or documented public identifier as reviewed instead of rotating it.".into()),
                    confidence,
                    confidence_reason,
                    verify_hint: Some("For a real credential, confirm the old value no longer authenticates and the replacement works with least privilege, then search tracked history, local config, logs, and shared artifacts under the incident policy. For a fixture or public value, verify it cannot grant the suspected privilege and document why it is safe.".into()),
                });
            }
        }
    }
    Ok(())
}
