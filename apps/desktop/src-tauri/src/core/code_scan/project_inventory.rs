use super::*;

pub(super) fn collect_ai_config_files(
    project_files: &[ProjectFile],
    text_budget: &mut ScanTextBudget<'_>,
) -> Result<Vec<TextArtifact>, CodeScanError> {
    collect_text_artifacts(project_files, looks_like_ai_config, text_budget)
}

pub(super) fn collect_project_paths(project_files: &[ProjectFile]) -> Vec<String> {
    project_files
        .iter()
        .map(|file| file.relative_path.clone())
        .collect()
}

pub(super) fn looks_like_ai_config(relative_path: &str, file_name: &str) -> bool {
    let relative_path = relative_path.to_ascii_lowercase();
    let file_name = file_name.to_ascii_lowercase();
    let has_config_extension = file_name.ends_with(".json")
        || file_name.ends_with(".jsonc")
        || file_name.ends_with(".toml")
        || file_name.ends_with(".yaml")
        || file_name.ends_with(".yml");

    if !has_config_extension {
        return false;
    }

    // Another check owns root-level MCP config secrets.
    if matches!(
        relative_path.as_str(),
        ".mcp.json" | ".cursor/mcp.json" | ".vscode/mcp.json"
    ) {
        return false;
    }

    // Walker-relative root paths have no leading slash.
    file_name == "claude_desktop_config.json"
        || file_name == "cline_mcp_settings.json"
        || file_name == "mcp.json"
        || file_name == ".mcp.json"
        || file_name == "mcp.jsonc"
        || relative_path.contains("/.cursor/mcp.")
        || relative_path.starts_with(".cursor/mcp.")
        || relative_path.contains("/.claude/")
        || relative_path.starts_with(".claude/")
        || (relative_path.contains("mcp") && relative_path.contains(".json"))
}

#[cfg(test)]
mod tests {
    use super::{find_hardcoded_config_secret, looks_like_ai_config, ConfigSecretKind};

    #[test]
    fn config_secret_matcher_reports_the_pattern_class() {
        // Value-shaped: the matched text is itself a provider token format.
        let value_shaped = r#"{ "apiKey": "sk-ant-abcdefghijklmnopqrstuvwxyz123456" }"#; // gitleaks:allow
        assert_eq!(
            find_hardcoded_config_secret(value_shaped),
            Some((1, ConfigSecretKind::ValueShaped))
        );

        let heuristic = r#"{ "api_key": "your-key-goes-here" }"#;
        assert_eq!(
            find_hardcoded_config_secret(heuristic),
            Some((1, ConfigSecretKind::NameValueHeuristic))
        );

        // A value-shaped match later in the file outranks an earlier
        // heuristic match: the strongest evidence anchors the finding.
        let both = "{\n  \"api_key\": \"maybe-a-placeholder\",\n  \"token\": \"ghp_abcdefghijklmnopqrstuv\"\n}"; // gitleaks:allow
        assert_eq!(
            find_hardcoded_config_secret(both),
            Some((3, ConfigSecretKind::ValueShaped))
        );

        // Env substitution stays exempt.
        assert_eq!(
            find_hardcoded_config_secret(r#"{ "api_key": "${OPENAI_API_KEY}" }"#),
            None
        );
    }

    #[test]
    fn root_level_claude_configs_are_scanned_and_root_mcp_files_are_deduped() {
        assert!(looks_like_ai_config(
            ".claude/settings.json",
            "settings.json"
        ));
        assert!(looks_like_ai_config(
            "apps/web/.claude/settings.json",
            "settings.json"
        ));
        assert!(!looks_like_ai_config(".mcp.json", ".mcp.json"));
        assert!(!looks_like_ai_config(".cursor/mcp.json", "mcp.json"));
        assert!(!looks_like_ai_config(".vscode/mcp.json", "mcp.json"));
        // Nested MCP configs are NOT read by ai_scaffolding (root-only),
        // so they stay in scope here.
        assert!(looks_like_ai_config(
            "packages/app/.cursor/mcp.json",
            "mcp.json"
        ));
        assert!(looks_like_ai_config("config/mcp.json", "mcp.json"));
    }
}

/// Which credential-pattern class a config-secret line matched. Value-shaped
/// matches are provider token formats (a direct structural fact); the
/// name-value heuristic also matches placeholders and public keys, so its
/// findings ship as NeedsReview at the emit site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfigSecretKind {
    ValueShaped,
    NameValueHeuristic,
}

/// Return the strongest credential-pattern class and its 1-based line number.
/// Provider-shaped values take precedence, and matched values are never returned.
pub(super) fn find_hardcoded_config_secret(content: &str) -> Option<(u32, ConfigSecretKind)> {
    let mut heuristic_line: Option<u32> = None;
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

        if MCP_SECRET_VALUE_PATTERNS
            .iter()
            .any(|pattern| pattern.is_match(trimmed))
        {
            return Some((index as u32 + 1, ConfigSecretKind::ValueShaped));
        }
        if heuristic_line.is_none() && MCP_SECRET_NAME_VALUE_PATTERN.is_match(trimmed) {
            heuristic_line = Some(index as u32 + 1);
        }
    }

    heuristic_line.map(|line| (line, ConfigSecretKind::NameValueHeuristic))
}
