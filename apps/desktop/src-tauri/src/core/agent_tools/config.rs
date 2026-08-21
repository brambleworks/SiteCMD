//! Pure config editors for Cursor, Claude Code, and Codex MCP registrations.

use super::McpServerSpec;

/// Set `mcpServers.sitecmd` in a Cursor-style JSON config, preserving every
/// other key. Empty input is treated as a fresh config. Claude Code uses the
/// same shape for status checks, while registration still goes through its CLI.
pub fn upsert_cursor_config(existing: &str, spec: &McpServerSpec) -> Result<String, String> {
    let mut root = parse_json_config(existing)?;
    let object = root.as_object_mut().ok_or_else(|| {
        "the existing config file is not valid JSON: top level must be an object".to_string()
    })?;
    let servers = object
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    let servers = servers.as_object_mut().ok_or_else(|| {
        "the existing config file is not valid JSON: mcpServers must be an object".to_string()
    })?;
    servers.insert(
        "sitecmd".to_string(),
        serde_json::json!({
            "command": spec.command.as_str(),
            "args": spec.args,
            "env": spec.env,
        }),
    );
    render_json_config(&root)
}

/// Remove only `mcpServers.sitecmd`; every other key is left in place.
pub fn remove_cursor_config(existing: &str) -> Result<String, String> {
    let mut root = parse_json_config(existing)?;
    if let Some(servers) = root
        .as_object_mut()
        .and_then(|object| object.get_mut("mcpServers"))
        .and_then(serde_json::Value::as_object_mut)
    {
        servers.remove("sitecmd");
    }
    render_json_config(&root)
}

pub fn cursor_config_has_sitecmd(existing: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(existing)
        .map(|root| root.pointer("/mcpServers/sitecmd").is_some())
        .unwrap_or(false)
}

/// Match every launch-affecting field against the spec SiteCMD writes today.
pub fn cursor_config_matches_sitecmd_spec(existing: &str, spec: &McpServerSpec) -> bool {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(existing) else {
        return false;
    };
    let Some(server) = root
        .pointer("/mcpServers/sitecmd")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    let command_matches =
        server.get("command").and_then(serde_json::Value::as_str) == Some(spec.command.as_str());
    let args_match = server
        .get("args")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|args| {
            args.len() == spec.args.len()
                && args.iter().zip(&spec.args).all(|(actual, expected)| {
                    actual.as_str().is_some_and(|actual| actual == expected)
                })
        });
    let env_matches = server
        .get("env")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|env| {
            env.len() == spec.env.len()
                && spec.env.iter().all(|(key, expected)| {
                    env.get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|actual| actual == expected)
                })
        });
    let enabled_matches = match server.get("enabled") {
        None => true,
        Some(value) => value.as_bool() == Some(true),
    };
    let disabled_matches = match server.get("disabled") {
        None => true,
        Some(value) => value.as_bool() == Some(false),
    };
    let transport_matches = !server.contains_key("url")
        && !server.contains_key("serverUrl")
        && server
            .get("type")
            .is_none_or(|value| value.as_str() == Some("stdio"));
    command_matches
        && args_match
        && env_matches
        && enabled_matches
        && disabled_matches
        && transport_matches
}

fn parse_json_config(existing: &str) -> Result<serde_json::Value, String> {
    let trimmed = existing.trim();
    if trimmed.is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(trimmed)
        .map_err(|e| format!("the existing config file is not valid JSON: {e}"))
}

fn render_json_config(root: &serde_json::Value) -> Result<String, String> {
    serde_json::to_string_pretty(root)
        .map(|rendered| rendered + "\n")
        .map_err(|e| format!("could not serialize the updated config: {e}"))
}

/// Set `[mcp_servers.sitecmd]` in Codex config while preserving other content.
pub fn upsert_codex_config(existing: &str, spec: &McpServerSpec) -> Result<String, String> {
    let mut doc = parse_toml_config(existing)?;
    let mut server = toml_edit::Table::new();
    server["command"] = toml_edit::value(spec.command.as_str());
    server["args"] = toml_edit::value(
        spec.args
            .iter()
            .map(String::as_str)
            .collect::<toml_edit::Array>(),
    );
    let mut env = toml_edit::Table::new();
    for (key, value) in &spec.env {
        env[key] = toml_edit::value(value.as_str());
    }
    server["env"] = toml_edit::Item::Table(env);

    if doc.get("mcp_servers").is_none() {
        let mut parent = toml_edit::Table::new();
        parent.set_implicit(true);
        doc.insert("mcp_servers", toml_edit::Item::Table(parent));
    }
    let parent = doc["mcp_servers"].as_table_like_mut().ok_or_else(|| {
        "the existing config file already uses mcp_servers as something other than a table"
            .to_string()
    })?;
    parent.insert("sitecmd", toml_edit::Item::Table(server));
    Ok(doc.to_string())
}

/// Remove only `[mcp_servers.sitecmd]`; other keys and comments survive.
pub fn remove_codex_config(existing: &str) -> Result<String, String> {
    let mut doc = parse_toml_config(existing)?;
    if let Some(parent) = doc
        .get_mut("mcp_servers")
        .and_then(toml_edit::Item::as_table_like_mut)
    {
        parent.remove("sitecmd");
    }
    if let Some(parent) = doc
        .get_mut("mcp_servers")
        .and_then(toml_edit::Item::as_table_mut)
    {
        if parent.is_empty() {
            parent.set_implicit(true);
        }
    }
    Ok(doc.to_string())
}

pub fn codex_config_has_sitecmd(existing: &str) -> bool {
    existing
        .parse::<toml_edit::DocumentMut>()
        .map(|doc| {
            doc.get("mcp_servers")
                .and_then(toml_edit::Item::as_table_like)
                .is_some_and(|table| table.get("sitecmd").is_some())
        })
        .unwrap_or(false)
}

pub fn codex_config_matches_sitecmd_spec(existing: &str, spec: &McpServerSpec) -> bool {
    let Ok(doc) = existing.parse::<toml_edit::DocumentMut>() else {
        return false;
    };
    let Some(server) = doc
        .get("mcp_servers")
        .and_then(toml_edit::Item::as_table_like)
        .and_then(|table| table.get("sitecmd"))
        .and_then(toml_edit::Item::as_table_like)
    else {
        return false;
    };
    let command_matches =
        server.get("command").and_then(toml_edit::Item::as_str) == Some(spec.command.as_str());
    let args_match = server
        .get("args")
        .and_then(toml_edit::Item::as_array)
        .is_some_and(|args| {
            args.len() == spec.args.len()
                && args.iter().zip(&spec.args).all(|(actual, expected)| {
                    actual.as_str().is_some_and(|actual| actual == expected)
                })
        });
    let env_matches = server
        .get("env")
        .and_then(toml_edit::Item::as_table_like)
        .is_some_and(|env| {
            env.len() == spec.env.len()
                && spec.env.iter().all(|(key, expected)| {
                    env.get(key)
                        .and_then(toml_edit::Item::as_str)
                        .is_some_and(|actual| actual == expected)
                })
        });
    let enabled_matches = match server.get("enabled") {
        None => true,
        Some(value) => value.as_bool() == Some(true),
    };
    let disabled_matches = match server.get("disabled") {
        None => true,
        Some(value) => value.as_bool() == Some(false),
    };
    let transport_matches = !server.contains_key("url")
        && !server.contains_key("server_url")
        && server
            .get("type")
            .is_none_or(|value| value.as_str() == Some("stdio"));
    command_matches
        && args_match
        && env_matches
        && enabled_matches
        && disabled_matches
        && transport_matches
}

fn parse_toml_config(existing: &str) -> Result<toml_edit::DocumentMut, String> {
    existing
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("the existing config file is not valid TOML: {e}"))
}
