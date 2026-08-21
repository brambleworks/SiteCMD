//! Tests for `agent_tools`.

use super::*;

fn test_spec() -> McpServerSpec {
    build_server_spec(
        Path::new("/usr/bin/node"),
        Path::new("/home/tester/.local/share/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs"),
        Path::new("/home/tester/.local/share/com.sitecmd.app/sitecmd.db"),
    )
}

#[test]
fn display_name_maps_tokens_and_falls_back() {
    assert_eq!(AgentTool::ClaudeCode.display_name(), "Claude Code");
    assert_eq!(AgentTool::Codex.display_name(), "Codex");
    assert_eq!(AgentTool::Cursor.display_name(), "Cursor");

    assert_eq!(agent_tool_display_name("claude-code"), "Claude Code");
    assert_eq!(agent_tool_display_name("codex"), "Codex");
    assert_eq!(agent_tool_display_name("cursor"), "Cursor");
    // Unknown tokens pass through unchanged rather than getting dropped.
    assert_eq!(agent_tool_display_name("mystery"), "mystery");

    // The token->display mapping stays in lockstep with as_str.
    for tool in [AgentTool::ClaudeCode, AgentTool::Codex, AgentTool::Cursor] {
        assert_eq!(agent_tool_display_name(tool.as_str()), tool.display_name());
    }
}

#[test]
fn cursor_registration_merges_into_existing_json() {
    let existing = r#"{
  "mcpServers": {
"other": { "command": "other-mcp", "args": ["--stdio"] }
  },
  "telemetry": false
}"#;
    let updated = upsert_cursor_config(existing, &test_spec()).expect("upsert succeeds");
    let root: serde_json::Value = serde_json::from_str(&updated).expect("output parses");

    assert_eq!(
        root.pointer("/mcpServers/other/command")
            .and_then(|v| v.as_str()),
        Some("other-mcp"),
        "existing servers must be preserved"
    );
    assert_eq!(
        root.pointer("/telemetry").and_then(|v| v.as_bool()),
        Some(false),
        "unrelated top-level keys must be preserved"
    );
    assert_eq!(
        root.pointer("/mcpServers/sitecmd/command")
            .and_then(|v| v.as_str()),
        Some("/usr/bin/node")
    );
    let args = root
        .pointer("/mcpServers/sitecmd/args")
        .and_then(|v| v.as_array())
        .expect("args array");
    assert_eq!(args.len(), 2);
    assert_eq!(
        args[0].as_str(),
        Some("--disable-warning=ExperimentalWarning")
    );
    assert_eq!(
        args[1].as_str(),
        Some("/home/tester/.local/share/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs")
    );
    assert_eq!(
        root.pointer("/mcpServers/sitecmd/env/SITECMD_DB_PATH")
            .and_then(|v| v.as_str()),
        Some("/home/tester/.local/share/com.sitecmd.app/sitecmd.db")
    );
}

#[test]
fn cursor_registration_treats_empty_input_as_fresh_config() {
    let updated = upsert_cursor_config("  \n", &test_spec()).expect("upsert");
    assert!(cursor_config_has_sitecmd(&updated));
}

#[test]
fn cursor_unregister_removes_only_sitecmd() {
    let existing = upsert_cursor_config(
        r#"{ "mcpServers": { "other": { "command": "other-mcp" } }, "theme": "dark" }"#,
        &test_spec(),
    )
    .expect("seed config");
    assert!(cursor_config_has_sitecmd(&existing));

    let updated = remove_cursor_config(&existing).expect("remove succeeds");
    let root: serde_json::Value = serde_json::from_str(&updated).expect("output parses");
    assert!(!cursor_config_has_sitecmd(&updated));
    assert!(
        root.pointer("/mcpServers/other").is_some(),
        "other servers must survive unregister"
    );
    assert_eq!(
        root.pointer("/theme").and_then(|v| v.as_str()),
        Some("dark")
    );
}

#[test]
fn codex_registration_adds_a_toml_table_and_preserves_content() {
    let existing = "# my codex config\nmodel = \"o4\"\n";
    let updated = upsert_codex_config(existing, &test_spec()).expect("upsert succeeds");

    assert!(
        updated.contains("model = \"o4\""),
        "existing keys must survive: {updated}"
    );
    assert!(
        updated.contains("# my codex config"),
        "comments must survive: {updated}"
    );
    assert!(
        updated.contains("[mcp_servers.sitecmd]"),
        "sitecmd table must be added: {updated}"
    );
    assert!(
        updated.contains("sitecmd-mcp.mjs") && updated.contains("/usr/bin/node"),
        "args must point at the persistent script via node: {updated}"
    );
    assert!(
        updated.contains("[mcp_servers.sitecmd.env]") && updated.contains("SITECMD_DB_PATH"),
        "the exact desktop database path must be persisted: {updated}"
    );
    assert!(codex_config_has_sitecmd(&updated));
}

#[test]
fn codex_unregister_removes_the_table() {
    let existing = upsert_codex_config("model = \"o4\"\n", &test_spec()).expect("seed config");
    let updated = remove_codex_config(&existing).expect("remove succeeds");

    assert!(!updated.contains("[mcp_servers.sitecmd]"), "{updated}");
    assert!(!codex_config_has_sitecmd(&updated));
    assert!(
        updated.contains("model = \"o4\""),
        "other content must survive unregister: {updated}"
    );
}

#[test]
fn codex_unregister_keeps_other_servers() {
    let existing = upsert_codex_config(
        "[mcp_servers.other]\ncommand = \"other-mcp\"\n",
        &test_spec(),
    )
    .expect("seed config");
    let updated = remove_codex_config(&existing).expect("remove succeeds");
    assert!(updated.contains("[mcp_servers.other]"), "{updated}");
    assert!(!codex_config_has_sitecmd(&updated));
}

#[test]
fn detection_reports_registered_from_config_contents() {
    // JSON shape (Cursor and Claude Code's ~/.claude.json).
    assert!(cursor_config_has_sitecmd(
        r#"{ "mcpServers": { "sitecmd": { "command": "npx" } } }"#
    ));
    assert!(!cursor_config_has_sitecmd(
        r#"{ "mcpServers": { "other": { "command": "other-mcp" } } }"#
    ));
    assert!(!cursor_config_has_sitecmd(""));
    assert!(!cursor_config_has_sitecmd("{}"));

    // TOML shape (Codex).
    assert!(codex_config_has_sitecmd(
        "[mcp_servers.sitecmd]\ncommand = \"npx\"\n"
    ));
    assert!(!codex_config_has_sitecmd(
        "[mcp_servers.other]\ncommand = \"other-mcp\"\n"
    ));
    assert!(!codex_config_has_sitecmd(""));
    assert!(!codex_config_has_sitecmd("model = \"o4\"\n"));
}

#[test]
fn json_registration_must_match_the_current_launch_spec() {
    let spec = test_spec();
    let current = upsert_cursor_config(r#"{"theme":"dark"}"#, &spec).expect("current config");
    assert!(cursor_config_matches_sitecmd_spec(&current, &spec));

    let mut stale: serde_json::Value = serde_json::from_str(&current).expect("json config");
    stale["mcpServers"]["sitecmd"]["command"] = serde_json::json!("/old/node");
    assert!(!cursor_config_matches_sitecmd_spec(
        &serde_json::to_string(&stale).expect("stale json"),
        &spec,
    ));

    let mut extra_env: serde_json::Value = serde_json::from_str(&current).expect("json config");
    extra_env["mcpServers"]["sitecmd"]["env"]["UNEXPECTED"] = serde_json::json!("value");
    assert!(!cursor_config_matches_sitecmd_spec(
        &serde_json::to_string(&extra_env).expect("extra env json"),
        &spec,
    ));

    let mut disabled: serde_json::Value = serde_json::from_str(&current).expect("json config");
    disabled["mcpServers"]["sitecmd"]["disabled"] = serde_json::json!(true);
    assert!(!cursor_config_matches_sitecmd_spec(
        &serde_json::to_string(&disabled).expect("disabled json"),
        &spec,
    ));

    let mut alternate_transport: serde_json::Value =
        serde_json::from_str(&current).expect("json config");
    alternate_transport["mcpServers"]["sitecmd"]["url"] =
        serde_json::json!("https://example.invalid/mcp");
    assert!(!cursor_config_matches_sitecmd_spec(
        &serde_json::to_string(&alternate_transport).expect("alternate transport json"),
        &spec,
    ));
}

#[test]
fn codex_registration_must_match_the_current_launch_spec() {
    let spec = test_spec();
    let current = upsert_codex_config("model = \"o4\"\n", &spec).expect("current config");
    assert!(codex_config_matches_sitecmd_spec(&current, &spec));

    let stale = current.replace("/usr/bin/node", "/old/node");
    assert!(!codex_config_matches_sitecmd_spec(&stale, &spec));

    let extra_env = current.replace(
        "SITECMD_DB_PATH =",
        "UNEXPECTED = \"value\"\nSITECMD_DB_PATH =",
    );
    assert!(!codex_config_matches_sitecmd_spec(&extra_env, &spec));

    let disabled = current.replace(
        "[mcp_servers.sitecmd.env]",
        "enabled = false\n\n[mcp_servers.sitecmd.env]",
    );
    assert!(!codex_config_matches_sitecmd_spec(&disabled, &spec));

    let alternate_transport = current.replace(
        "[mcp_servers.sitecmd.env]",
        "url = \"https://example.invalid/mcp\"\n\n[mcp_servers.sitecmd.env]",
    );
    assert!(!codex_config_matches_sitecmd_spec(
        &alternate_transport,
        &spec
    ));
}

#[test]
fn invalid_inputs_error_instead_of_panicking() {
    let spec = test_spec();

    let json_err = upsert_cursor_config("{ not json", &spec).expect_err("invalid JSON errors");
    assert!(json_err.contains("not valid JSON"), "{json_err}");
    assert!(remove_cursor_config("{ not json").is_err());
    assert!(upsert_cursor_config("[1, 2]", &spec).is_err());
    assert!(upsert_cursor_config(r#"{ "mcpServers": 7 }"#, &spec).is_err());

    let toml_err = upsert_codex_config("model = = nope", &spec).expect_err("invalid TOML errors");
    assert!(toml_err.contains("not valid TOML"), "{toml_err}");
    assert!(remove_codex_config("model = = nope").is_err());
    assert!(upsert_codex_config("mcp_servers = 3\n", &spec).is_err());
}

#[test]
fn codex_inline_table_supports_upsert_and_remove() {
    let existing = "mcp_servers = { other = { command = \"other-mcp\" } }\n";

    let updated = upsert_codex_config(existing, &test_spec()).expect("inline upsert");
    assert!(codex_config_has_sitecmd(&updated), "{updated}");
    assert!(
        updated.contains("other-mcp"),
        "other inline servers must survive upsert: {updated}"
    );

    let removed = remove_codex_config(&updated).expect("inline remove");
    assert!(
        !codex_config_has_sitecmd(&removed),
        "remove must work on inline tables: {removed}"
    );
    assert!(
        removed.contains("other-mcp"),
        "other inline servers must survive remove: {removed}"
    );
}

#[test]
fn rewrite_config_creates_parents_and_replaces_content_atomically() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested").join("deeper").join("config.json");

    rewrite_config(&path, |existing| {
        assert_eq!(existing, "", "missing file must read as empty");
        Ok("first\n".to_string())
    })
    .expect("first rewrite creates parent dirs");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        "first\n"
    );

    rewrite_config(&path, |existing| {
        assert_eq!(existing, "first\n");
        Ok("second\n".to_string())
    })
    .expect("second rewrite replaces content");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        "second\n"
    );

    let leftovers: Vec<String> = std::fs::read_dir(path.parent().expect("parent"))
        .expect("read dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("sitecmd-tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "no temp files may be left behind: {leftovers:?}"
    );
}

#[cfg(unix)]
#[test]
fn rewrite_config_follows_a_symlinked_target() {
    let dir = tempfile::tempdir().expect("tempdir");
    let real = dir.path().join("real-config.json");
    std::fs::write(&real, "{}\n").expect("seed real file");
    let link = dir.path().join("link-config.json");
    std::os::unix::fs::symlink(&real, &link).expect("create symlink");

    rewrite_config(&link, |_| Ok("updated\n".to_string())).expect("rewrite via symlink");

    assert!(
        link.symlink_metadata()
            .expect("link metadata")
            .file_type()
            .is_symlink(),
        "the symlink itself must survive the rewrite"
    );
    assert_eq!(
        std::fs::read_to_string(&real).expect("read real file"),
        "updated\n",
        "the real file behind the symlink must receive the content"
    );
}

#[test]
fn read_config_for_rewrite_distinguishes_missing_from_unreadable() {
    let dir = tempfile::tempdir().expect("tempdir");

    let missing = dir.path().join("missing.json");
    assert_eq!(
        read_config_for_rewrite(&missing).expect("missing file maps to empty"),
        ""
    );

    // A directory at the path is not NotFound, so it must surface an error
    // instead of being treated as an empty config we would then clobber.
    assert!(read_config_for_rewrite(dir.path()).is_err());
}

#[test]
fn unregister_via_config_is_a_noop_for_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("missing.json");

    unregister_via_config(&path, remove_cursor_config).expect("missing file is Ok no-op");
    assert!(!path.exists(), "no file may be created by unregister");
}

#[test]
fn agent_tool_as_str_matches_serde_tokens() {
    for tool in [AgentTool::ClaudeCode, AgentTool::Codex, AgentTool::Cursor] {
        assert_eq!(
            serde_json::to_string(&tool).expect("serialize tool"),
            format!("\"{}\"", tool.as_str()),
            "as_str must agree with the kebab-case serde token for {tool:?}"
        );
    }
}

// Worst-case kickoff content: a space, a quote, an ampersand, and a `#`
// from an issue id. Each must be percent-encoded so it cannot terminate or
// split the query parameter it rides in.
const HANDOFF_PROMPT: &str = r##"Fix "CSP & HSTS" for issue #12"##;
const ENCODED_PROMPT: &str = "Fix%20%22CSP%20%26%20HSTS%22%20for%20issue%20%2312";

#[test]
fn claude_handoff_link_encodes_prompt_and_folder() {
    assert_eq!(
        handoff_deep_link(
            AgentTool::ClaudeCode,
            HANDOFF_PROMPT,
            Some("/Users/dev/My Site"),
        ),
        format!("claude://code/new?q={ENCODED_PROMPT}&folder=%2FUsers%2Fdev%2FMy%20Site")
    );
}

#[test]
fn claude_handoff_link_omits_folder_when_path_is_missing_or_empty() {
    let expected = format!("claude://code/new?q={ENCODED_PROMPT}");
    assert_eq!(
        handoff_deep_link(AgentTool::ClaudeCode, HANDOFF_PROMPT, None),
        expected
    );
    assert_eq!(
        handoff_deep_link(AgentTool::ClaudeCode, HANDOFF_PROMPT, Some("")),
        expected,
        "an empty project path must not emit a dangling folder param"
    );
    assert_eq!(
        handoff_deep_link(AgentTool::ClaudeCode, HANDOFF_PROMPT, Some("   ")),
        expected,
        "a whitespace-only project path must not emit a folder param"
    );
}

#[test]
fn cursor_handoff_link_encodes_prompt_and_has_no_folder_param() {
    let expected = format!("cursor://anysphere.cursor-deeplink/prompt?text={ENCODED_PROMPT}");
    assert_eq!(
        handoff_deep_link(AgentTool::Cursor, HANDOFF_PROMPT, None),
        expected
    );
    assert_eq!(
        handoff_deep_link(
            AgentTool::Cursor,
            HANDOFF_PROMPT,
            Some("/Users/dev/My Site")
        ),
        expected,
        "Cursor's prompt deep link documents no workspace param; the path must be dropped"
    );
}

#[test]
fn codex_handoff_link_encodes_prompt_and_path() {
    assert_eq!(
        handoff_deep_link(AgentTool::Codex, HANDOFF_PROMPT, Some("/Users/dev/My Site")),
        format!("codex://threads/new?prompt={ENCODED_PROMPT}&path=%2FUsers%2Fdev%2FMy%20Site")
    );
    assert_eq!(
        handoff_deep_link(AgentTool::Codex, HANDOFF_PROMPT, None),
        format!("codex://threads/new?prompt={ENCODED_PROMPT}")
    );
}

#[test]
fn build_server_spec_uses_persistent_script_and_database_path() {
    let spec = build_server_spec(
        Path::new("/opt/homebrew/bin/node"),
        Path::new("/x/sitecmd-mcp/sitecmd-mcp.mjs"),
        Path::new("/x/sitecmd.db"),
    );
    assert_eq!(spec.command, "/opt/homebrew/bin/node");
    assert_eq!(
        spec.args,
        vec![
            "--disable-warning=ExperimentalWarning".to_string(),
            "/x/sitecmd-mcp/sitecmd-mcp.mjs".to_string(),
        ]
    );
    assert!(spec.args.last().unwrap().ends_with("sitecmd-mcp.mjs"));
    assert!(!spec.command.contains("npx"));
    assert!(spec
        .args
        .iter()
        .all(|a| !a.contains("npx") && !a.contains("sitecmd-mcp@")));
    assert_eq!(
        spec.env.get("SITECMD_DB_PATH").map(String::as_str),
        Some("/x/sitecmd.db")
    );
}

#[test]
fn mcp_node_version_floor_is_enforced() {
    assert!(!node_version_supported("v22.22.0"));
    assert!(node_version_supported("v22.22.1"));
    assert!(node_version_supported("22.22.2"));
    assert!(node_version_supported("v24.0.0"));
    assert!(!node_version_supported("v22.13.0"));
    assert!(!node_version_supported("not-a-version"));
}

#[test]
fn compatible_node_selection_skips_an_outdated_earlier_candidate() {
    let old = PathBuf::from("/usr/local/bin/node");
    let current = PathBuf::from("/home/tester/.nvm/versions/node/v24.1.0/bin/node");
    let selected = find_compatible_node([old.clone(), current.clone()], |candidate| {
        if candidate == old {
            Err("Node is too old".to_string())
        } else {
            Ok(())
        }
    })
    .expect("a later compatible Node must be selected");

    assert_eq!(selected, current);
}

// Finder-launched apps must find version-manager Node installs without shell shims.
#[cfg(not(windows))]
#[test]
fn fallback_dirs_cover_version_manager_toolchains() {
    let home = Path::new("/home/tester");
    let dirs = fallback_binary_dirs_for(Some(home));

    for expected in [
        // fnm default alias (XDG data dir and legacy ~/.fnm)
        "/home/tester/.local/share/fnm/aliases/default/bin",
        "/home/tester/.fnm/aliases/default/bin",
        "/home/tester/.volta/bin",
        "/home/tester/.asdf/shims",
        "/home/tester/.local/share/mise/shims",
        "/home/tester/.nvm/current/bin",
    ] {
        assert!(
            dirs.iter().any(|d| d == Path::new(expected)),
            "fallback dirs must include {expected}; got {dirs:?}"
        );
    }

    // The ephemeral fnm multishell path must never be scanned or persisted.
    assert!(
        !dirs
            .iter()
            .any(|d| d.to_string_lossy().contains("fnm_multishells")),
        "must not scan the per-shell fnm shim dir"
    );

    // Homebrew stays ahead of the per-user managers.
    assert_eq!(dirs.first(), Some(&PathBuf::from("/opt/homebrew/bin")));
}

// nvm's default alias is commonly symbolic (`lts/*`, `node`, or a major
// number), not the exact directory name. The fallback must still discover
// installed Node toolchains and prefer the newest one.
#[cfg(not(windows))]
#[test]
fn fallback_dirs_resolve_symbolic_nvm_defaults_through_installed_versions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let home = temp.path();
    let nvm = home.join(".nvm");
    std::fs::create_dir_all(nvm.join("alias")).expect("alias dir");
    std::fs::write(nvm.join("alias").join("default"), "lts/*\n").expect("default alias");

    for version in ["v22.14.0", "v24.1.0", "not-a-version"] {
        std::fs::create_dir_all(nvm.join("versions").join("node").join(version).join("bin"))
            .expect("installed node bin");
    }

    let dirs = fallback_binary_dirs_for(Some(home));
    let v24 = nvm.join("versions/node/v24.1.0/bin");
    let v22 = nvm.join("versions/node/v22.14.0/bin");
    let v24_index = dirs.iter().position(|dir| dir == &v24).expect("v24 dir");
    let v22_index = dirs.iter().position(|dir| dir == &v22).expect("v22 dir");

    assert!(v24_index < v22_index, "newest installed nvm Node must win");
    assert!(
        !dirs
            .iter()
            .any(|dir| dir.to_string_lossy().contains("not-a-version")),
        "non-version directories must not become executable search paths"
    );
}

// Without a resolvable home, detection still scans the system package
// managers rather than panicking.
#[cfg(not(windows))]
#[test]
fn fallback_dirs_without_home_keep_system_dirs() {
    let dirs = fallback_binary_dirs_for(None);
    assert_eq!(
        dirs,
        vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ]
    );
}
