//! Commands for detecting, registering, and launching supported MCP clients.
//! Registration only modifies each client config's `sitecmd` entry.

use crate::core::agent_tools::{self, AgentTool, AgentToolStatus, McpManualConfig};

use super::run_blocking;

#[tauri::command]
#[tracing::instrument(skip(app))]
pub async fn detect_agent_tools(app: tauri::AppHandle) -> Result<Vec<AgentToolStatus>, String> {
    run_blocking(move || agent_tools::detect_all(&app)).await
}

#[tracing::instrument(skip(app))]
pub async fn register_agent_tool(
    app: tauri::AppHandle,
    tool: AgentTool,
) -> Result<AgentToolStatus, String> {
    run_blocking(move || -> Result<AgentToolStatus, String> {
        agent_tools::register(&app, tool)?;
        let status = agent_tools::detect_one(&app, tool);
        if !status.healthy {
            return Err(status.repair_reason.clone().unwrap_or_else(|| {
                "The SiteCMD MCP connection did not pass its health check".to_string()
            }));
        }
        Ok(status)
    })
    .await?
}

#[tracing::instrument(skip(app))]
pub async fn unregister_agent_tool(
    app: tauri::AppHandle,
    tool: AgentTool,
) -> Result<AgentToolStatus, String> {
    run_blocking(move || -> Result<AgentToolStatus, String> {
        agent_tools::unregister(tool)?;
        Ok(agent_tools::detect_one(&app, tool))
    })
    .await?
}

/// Stage a kickoff prompt in the agent's visible app through an OS deep link.
/// This opens the composer but never executes the prompt or another command.
#[tracing::instrument(
    skip(_app, kickoff_prompt, project_path),
    fields(tool = tool.as_str(), kickoff_len = kickoff_prompt.len())
)]
pub async fn launch_agent_handoff(
    _app: tauri::AppHandle,
    tool: AgentTool,
    kickoff_prompt: String,
    project_path: Option<String>,
) -> Result<(), String> {
    let Some(url) = agent_tools::handoff_deep_link(tool, &kickoff_prompt, project_path.as_deref())
    else {
        return Err(format!(
            "{} has no prompt deep link. The kickoff prompt is on your clipboard; paste it into the agent.",
            tool.display_name()
        ));
    };
    run_blocking(move || open_deep_link(tool, &url)).await?
}

#[tauri::command]
#[tracing::instrument(skip(app))]
pub async fn get_agent_tool_manual_config(
    app: tauri::AppHandle,
    tool: AgentTool,
) -> Result<McpManualConfig, String> {
    run_blocking(move || agent_tools::manual_config(&app, tool)).await?
}

/// Open the handoff URL and surface missing protocol handlers to the caller.
fn open_deep_link(tool: AgentTool, url: &str) -> Result<(), String> {
    open::that(url).map_err(|e| {
        format!(
            "Could not open {} via its deep link - no app on this system \
             accepted its URL scheme, so it may not be installed ({})",
            tool.as_str(),
            e
        )
    })
}
