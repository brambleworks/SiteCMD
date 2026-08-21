use super::issue_utils::redact_evidence;
use super::CodeIssue;
use super::MCP_SECRET_PATTERNS;
use crate::checks::{IssueConfidence, Severity};
use std::path::Path;

/// Known AI instruction paths paired with their consuming tool.
const INSTRUCTION_FILES: &[(&str, &str)] = &[
    ("CLAUDE.md", "Claude"),
    ("AGENTS.md", "Codex / AGENTS"),
    ("GEMINI.md", "Gemini"),
    (".cursorrules", "Cursor"),
    (".windsurfrules", "Windsurf"),
    (".github/copilot-instructions.md", "GitHub Copilot"),
];

/// Config files that AI tools read as MCP server definitions. These commonly
/// hold credentials in an `env` block, so they are the highest-risk place for a
/// pasted API key to end up tracked in git.
const MCP_CONFIG_FILES: &[(&str, &str)] = &[
    (".mcp.json", "Claude Code / MCP"),
    (".cursor/mcp.json", "Cursor MCP"),
    (".vscode/mcp.json", "VS Code MCP"),
];

/// Legacy single-file rule formats and the modern directory-based layout each
/// editor moved to. The single file still works, so this is a recommendation,
/// not a defect.
const LEGACY_RULE_FILES: &[(&str, &str, &str)] = &[
    (".cursorrules", "Cursor", ".cursor/rules/"),
    (".windsurfrules", "Windsurf", ".windsurf/rules/"),
];

/// Substrings that mark a matched value as an obvious placeholder rather than a
/// live credential, so the secret rule does not flag documentation examples.
const PLACEHOLDER_MARKERS: &[&str] = &[
    "your",
    "example",
    "changeme",
    "placeholder",
    "redacted",
    "dummy",
    "sample",
    "xxxx",
    "<",
    "...",
];

const STUB_THRESHOLD: usize = 40;
const SUBSTANTIVE_THRESHOLD: usize = 120;

/// Length of the lines that actually carry guidance: non-empty lines that are
/// not Markdown headings (`#`) and not HTML comments (`<!--`). A file made of
/// only a title and blank lines has a meaningful length of 0.
fn meaningful_content_len(content: &str) -> usize {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("<!--"))
        .map(str::len)
        .sum()
}

fn basename_lower(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_ascii_lowercase()
}

/// Finds the first likely credential without returning its value.
fn hardcoded_secret_line(content: &str) -> Option<u32> {
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.contains("${")
            || trimmed.contains("process.env")
            || trimmed.contains("import.meta.env")
            || trimmed.contains("std::env")
            || trimmed.contains("os.getenv")
            || trimmed.contains("System.getenv")
            || trimmed.contains("env.")
        {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if PLACEHOLDER_MARKERS
            .iter()
            .any(|marker| lower.contains(marker))
        {
            continue;
        }

        if MCP_SECRET_PATTERNS
            .iter()
            .any(|pattern| pattern.is_match(trimmed))
        {
            return Some(index as u32 + 1);
        }
    }

    None
}

/// Inspect a project's AI agent instruction files and surface quality problems.
/// Reads only the known instruction-file paths directly (no filesystem walk),
/// mirroring how `project_hygiene` reads `.gitignore`.
pub fn analyze_ai_scaffolding(root: &Path) -> Vec<CodeIssue> {
    let mut issues = Vec::new();

    // (relative_path, tool, content, meaningful_len) for every instruction file present.
    let present: Vec<(String, &str, String, usize)> = INSTRUCTION_FILES
        .iter()
        .filter_map(|(rel, tool)| {
            let content = super::filesystem::read_text_under_root(root, &root.join(rel))?;
            let meaningful = meaningful_content_len(&content);
            Some(((*rel).to_string(), *tool, content, meaningful))
        })
        .collect();

    // Rule 1: an instruction file exists but carries almost no guidance.
    for (rel, tool, _content, meaningful) in &present {
        if *meaningful < STUB_THRESHOLD {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("agent-instructions-stub:{}", rel),
                category: "ai-scaffolding".into(),
                severity: Severity::Low,
                title: format!("{} has almost no guidance", rel),
                description: format!(
                    "`{}` is associated with {} but contains less than {} characters of non-heading content. It may be an unfinished placeholder, an intentional pointer, or a minimal tool-specific file; the scanner does not evaluate external guidance or semantic completeness.",
                    rel, tool, STUB_THRESHOLD
                ),
                relative_path: rel.clone(),
                absolute_path: root.join(rel).to_string_lossy().to_string(),
                line: Some(1),
                source_excerpt: None,
                evidence: Some(redact_evidence(format!(
                    "{} carries {} characters of non-heading content; SiteCMD treats anything under {} as a placeholder.",
                    rel, meaningful, STUB_THRESHOLD
                ))),
                why_now: Some(
                    "When a tool treats this file as authoritative and no referenced guidance is loaded, missing commands and boundaries can make automated changes less consistent or harder to verify."
                        .into(),
                ),
                likely_fix: Some(
                    "If the file is authoritative, add the project-specific build/test/lint commands, relevant architecture/conventions, and safety boundaries. If it intentionally delegates, add an explicit resolvable pointer to the canonical guidance instead of padding it to cross the threshold."
                        .into(),
                ),
                confidence: IssueConfidence::NeedsReview,
                confidence_reason: Some(
                    "Character count is a heuristic; a short file can intentionally delegate to guidance outside the scanned content."
                        .into(),
                ),
                verify_hint: Some(
                    "Open the project through the target tool and confirm it loads the intended canonical guidance, then verify the documented commands and boundaries are current."
                        .into(),
                ),
            });
        }
    }

    // Rule 2: multiple substantive instruction files with no shared source of truth.
    let substantive: Vec<&(String, &str, String, usize)> = present
        .iter()
        .filter(|(_, _, _, meaningful)| *meaningful >= SUBSTANTIVE_THRESHOLD)
        .collect();

    if substantive.len() >= 2 {
        let basenames: Vec<String> = substantive
            .iter()
            .map(|(rel, _, _, _)| basename_lower(rel))
            .collect();
        let any_cross_reference = substantive
            .iter()
            .enumerate()
            .any(|(i, (_, _, content, _))| {
                let lower = content.to_ascii_lowercase();
                basenames
                    .iter()
                    .enumerate()
                    .any(|(j, name)| i != j && lower.contains(name))
            });

        if !any_cross_reference {
            let (anchor_rel, _, _, _) = substantive[0];
            let names = substantive
                .iter()
                .map(|(rel, _, _, _)| rel.clone())
                .collect::<Vec<_>>()
                .join(", ");
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("agent-instructions-fragmented:{}", anchor_rel),
                category: "ai-scaffolding".into(),
                severity: Severity::Low,
                title: "AI instruction files can drift out of sync".into(),
                description: format!(
                    "This project has multiple substantive AI instruction files ({}) and none of them points at the others. Each tool reads its own file, so the guidance drifts apart and different assistants follow different rules.",
                    names
                ),
                relative_path: anchor_rel.clone(),
                absolute_path: root.join(anchor_rel).to_string_lossy().to_string(),
                line: Some(1),
                source_excerpt: None,
                evidence: Some(redact_evidence(format!("Substantive instruction files with no cross-reference: {}.", names))),
                why_now: Some(
                    "As soon as one file is updated and the others are not, different AI tools start enforcing different, stale rules."
                        .into(),
                ),
                likely_fix: Some(
                    "Pick one canonical file (AGENTS.md is the cross-tool convention) and have the others point at it, so every assistant reads one source of truth."
                        .into(),
                ),
                confidence: IssueConfidence::NeedsReview,
                confidence_reason: Some(
                    "Based on filename cross-references between the files, not a semantic comparison of their guidance."
                        .into(),
                ),
                verify_hint: Some(
                    "Confirm each instruction file either defers to the canonical file or is intentionally tool-specific."
                        .into(),
                ),
            });
        }
    }

    // Check instruction and MCP config files for embedded credentials.
    let secret_candidates = present
        .iter()
        .map(|(rel, tool, content, _)| (rel.clone(), *tool, content.clone()))
        .chain(MCP_CONFIG_FILES.iter().filter_map(|(rel, tool)| {
            let content = super::filesystem::read_text_under_root(root, &root.join(rel))?;
            Some(((*rel).to_string(), *tool, content))
        }));

    for (rel, tool, content) in secret_candidates {
        if let Some(line) = hardcoded_secret_line(&content) {
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("agent-instructions-secret:{}", rel),
                category: "ai-scaffolding".into(),
                severity: Severity::High,
                title: format!("Credential-shaped literal in {}", rel),
                description: format!(
                    "`{}` (read by {}) has a line that matches an API key or credential pattern. The scan did not inspect or test the value, query an issuing provider, or determine whether the file is tracked, shared, logged, or deployed, so this match does not prove a valid credential or exposure.",
                    rel, tool
                ),
                relative_path: rel.clone(),
                absolute_path: root.join(&rel).to_string_lossy().to_string(),
                line: Some(line),
                source_excerpt: None,
                evidence: Some(redact_evidence(format!(
                    "Line {} of {} matches a known credential pattern; the value is redacted here.",
                    line, rel
                ))),
                why_now: Some(
                    "If a live privileged credential is stored in a tracked or shared instruction/config file, tools and collaborators may load it and copies can persist in history, artifacts, logs, or transcripts."
                        .into(),
                ),
                likely_fix: Some(
                    "Classify the value without printing it. If it is real, determine whether the file or value was tracked, shared, logged, or deployed; revoke or rotate first when exposure is confirmed. Replace the literal with an environment reference or the tool's supported credential store, keeping the replacement out of public-prefixed variables and shared instructions."
                        .into(),
                ),
                confidence: IssueConfidence::NeedsReview,
                confidence_reason: Some(
                    "Matched by credential pattern; confirm the value is a real secret and not an internal identifier."
                        .into(),
                ),
                verify_hint: Some(
                    "Confirm the literal is absent from current files and emitted configs. For a confirmed exposure, verify the old credential fails, the replacement is least-privilege, and relevant history/artifacts/logs are handled under incident policy; for a public identifier or fixture, document why it cannot authenticate."
                        .into(),
                ),
            });
        }
    }

    // Flag meaningful legacy rule files when a directory format is available.
    for (rel, tool, modern) in LEGACY_RULE_FILES {
        let Some((_, _, _, meaningful)) = present.iter().find(|(p, _, _, _)| p == rel) else {
            continue;
        };
        if *meaningful < STUB_THRESHOLD {
            continue;
        }
        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("agent-instructions-legacy-format:{}", rel),
            category: "ai-scaffolding".into(),
            severity: Severity::Low,
            title: format!("{} uses {}'s legacy rule format", rel, tool),
            description: format!(
                "`{}` is {}'s original single-file rule format. Recent versions read project rules from `{}` instead, which lets you split guidance into focused, scoped files. The single file still works, so this is a recommendation rather than a break.",
                rel, tool, modern
            ),
            relative_path: (*rel).to_string(),
            absolute_path: root.join(rel).to_string_lossy().to_string(),
            line: Some(1),
            source_excerpt: None,
            evidence: Some(redact_evidence(format!(
                "{} carries {} characters of guidance in the legacy single-file format; {} now prefers `{}`.",
                rel, meaningful, tool, modern
            ))),
            why_now: Some(
                "Moving to the rules directory now lets you scope guidance per area before the single file grows unwieldy."
                    .into(),
            ),
            likely_fix: Some(format!(
                "Move the guidance into `{}` as one or more focused rule files, then remove the legacy file once the new layout is in place.",
                modern
            )),
            confidence: IssueConfidence::NeedsReview,
            confidence_reason: Some(
                "Based on the filename; the legacy format is still supported, so migration is optional."
                    .into(),
            ),
            verify_hint: Some(format!(
                "Confirm the guidance now lives under `{}` and the legacy file was removed or left as a deliberate fallback.",
                modern
            )),
        });
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn flags_a_stub_instruction_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# CLAUDE.md\n\n").unwrap();
        let issues = analyze_ai_scaffolding(dir.path());
        assert_eq!(issues.len(), 1);
        assert!(issues[0]
            .id
            .starts_with("agent-instructions-stub:CLAUDE.md"));
        assert_eq!(issues[0].category, "ai-scaffolding");
        assert_eq!(issues[0].severity, Severity::Low);
        assert_eq!(issues[0].confidence, IssueConfidence::NeedsReview);
        assert!(issues[0].description.contains("may be"));
        assert!(!issues[0].description.contains("cannot follow"));
    }

    #[test]
    fn ignores_a_substantive_single_instruction_file() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            "This project is a Tauri desktop app. Run `npm test` and `cargo test` before every commit. Never edit generated permission files by hand; they are produced by the build.",
        )
        .unwrap();
        assert!(analyze_ai_scaffolding(dir.path()).is_empty());
    }

    #[test]
    fn flags_fragmented_instruction_files_with_no_cross_reference() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("CLAUDE.md"),
            "Build with npm run build. The frontend lives in apps/desktop/src and uses React with strict TypeScript and the project theme tokens.",
        )
        .unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            "Run cargo test for the Rust backend. The Tauri commands live under src-tauri/src/commands and every command needs an ACL permission entry.",
        )
        .unwrap();
        let issues = analyze_ai_scaffolding(dir.path());
        assert_eq!(issues.len(), 1);
        assert!(issues[0].id.starts_with("agent-instructions-fragmented:"));
    }

    #[test]
    fn flags_a_hardcoded_secret_in_an_mcp_config() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(".mcp.json"),
            "{\n  \"mcpServers\": {\n    \"x\": { \"env\": { \"API_KEY\": \"sk-ant-abc123def456ghi789jkl\" } }\n  }\n}",
        )
        .unwrap();
        let issues = analyze_ai_scaffolding(dir.path());
        assert_eq!(issues.len(), 1);
        assert!(issues[0]
            .id
            .starts_with("agent-instructions-secret:.mcp.json"));
        assert_eq!(issues[0].severity, Severity::High);
        assert_eq!(issues[0].confidence, IssueConfidence::NeedsReview);
        assert!(issues[0].title.contains("Credential-shaped"));
        assert!(issues[0].description.contains("does not prove"));
        assert!(issues[0].description.contains("tracked"));
        assert!(issues[0]
            .likely_fix
            .as_deref()
            .unwrap_or_default()
            .contains("If it is real"));
        assert!(!issues[0].description.contains("effectively published"));
        assert!(!issues[0]
            .likely_fix
            .as_deref()
            .unwrap_or_default()
            .contains("already been committed"));
        // The matched secret value must never be echoed back into the issue.
        assert!(!issues[0]
            .evidence
            .as_deref()
            .unwrap_or_default()
            .contains("sk-ant-"));
    }

    #[test]
    fn flags_a_hardcoded_secret_in_an_instruction_file() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("CLAUDE.md"),
            "Use the deploy API. The token is ghp_abcdefghijklmnopqrstuvwxyz0123 for the release bot. Run npm run deploy to ship a build to production.",
        )
        .unwrap();
        let issues = analyze_ai_scaffolding(dir.path());
        assert!(issues
            .iter()
            .any(|issue| issue.id.starts_with("agent-instructions-secret:CLAUDE.md")));
    }

    #[test]
    fn ignores_env_references_and_placeholders() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(".mcp.json"),
            "{\n  \"mcpServers\": {\n    \"a\": { \"env\": { \"API_KEY\": \"${ANTHROPIC_API_KEY}\" } },\n    \"b\": { \"env\": { \"TOKEN\": \"your-token-here\" } }\n  }\n}",
        )
        .unwrap();
        assert!(analyze_ai_scaffolding(dir.path()).is_empty());
    }

    #[test]
    fn flags_a_legacy_cursorrules_file_with_real_content() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(".cursorrules"),
            "Always use the project theme tokens, never hardcode hex colors. Run the test suite before every commit and keep functions small and focused.",
        )
        .unwrap();
        let issues = analyze_ai_scaffolding(dir.path());
        assert!(issues.iter().any(|issue| issue
            .id
            .starts_with("agent-instructions-legacy-format:.cursorrules")));
    }

    #[test]
    fn does_not_flag_legacy_format_on_a_stub() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".cursorrules"), "# rules\n").unwrap();
        let issues = analyze_ai_scaffolding(dir.path());
        // A near-empty legacy file should only raise the stub issue, not a
        // migration nudge on top of it.
        assert!(issues
            .iter()
            .all(|issue| !issue.id.starts_with("agent-instructions-legacy-format")));
    }

    #[test]
    fn does_not_flag_fragmentation_when_files_cross_reference() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("CLAUDE.md"),
            "Canonical agent guidance lives in AGENTS.md. Read AGENTS.md first; this file stays as a compatibility pointer so every assistant shares one source of truth.",
        )
        .unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            "Run cargo test for the Rust backend. The Tauri commands live under src-tauri/src/commands and every command needs an ACL permission entry.",
        )
        .unwrap();
        assert!(analyze_ai_scaffolding(dir.path()).is_empty());
    }
}
