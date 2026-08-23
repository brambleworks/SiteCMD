use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State, Window};

use super::super::scan::ScanControlState;
use super::{
    arg_from_value, arg_i64, arg_optional_bool, arg_optional_string, arg_optional_u32,
    arg_optional_usize, arg_string, emit_privileged_command_response,
    issue_scoped_privileged_command_token, json_response, PrivilegedCommandRequest,
    PrivilegedCommandTokenRequest, PrivilegedCommandTokenState,
};
use crate::db::Database;

pub(super) const BROKER_COMMAND: &str = "run_filesystem_access_command";
pub(super) const SCOPE_LABEL: &str = "filesystem access";

pub const FILESYSTEM_ACCESS_COMMANDS: &[&str] = &[
    "detect_project_urls",
    "update_project_path",
    "get_git_status",
    "get_commits_since",
    "get_db_path",
    "run_scan_execution",
    "run_code_scan_audit",
    "get_log_path",
    "read_recent_logs",
    "inspect_desktop_watch_files",
    "resolve_project_files",
    "open_path_in_editor",
    "reveal_path",
    "resolve_fix_locations_for_check",
    "register_agent_tool",
    "unregister_agent_tool",
    "launch_agent_handoff",
];

#[tauri::command]
#[tracing::instrument(skip(app, window, token_state, request), fields(broker = "run_filesystem_access_command", command = %request.command))]
pub async fn issue_filesystem_access_command_token(
    app: AppHandle,
    window: Window,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandTokenRequest,
) -> Result<String, String> {
    issue_scoped_privileged_command_token(app, window, token_state, request, BROKER_COMMAND).await
}

/// Feature-scoped broker for local filesystem and registered project access.
#[tauri::command]
pub async fn run_filesystem_access_command(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    scan_control: State<'_, ScanControlState>,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandRequest,
) -> Result<Value, String> {
    super::BrokerScope::by_broker(BROKER_COMMAND)
        .expect("registered scope")
        .admit(&token_state, &request)?;
    let PrivilegedCommandRequest {
        command,
        args,
        response_event,
        ..
    } = request;
    let outcome = dispatch(app.clone(), db, scan_control, command, args).await;
    emit_privileged_command_response(&app, response_event.as_deref(), &outcome);
    outcome
}

async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    scan_control: State<'_, ScanControlState>,
    command: String,
    args: Value,
) -> Result<Value, String> {
    use crate::commands as cmds;
    match command.as_str() {
        "detect_project_urls" => {
            let result =
                cmds::project::detect_project_urls(arg_string(&args, "path", "path")?).await?;
            json_response(result)
        }
        "update_project_path" => {
            cmds::project::update_project_path(
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "path", "path")?,
            )
            .await?;
            json_response(())
        }
        "get_git_status" => {
            let result = cmds::project_git::get_git_status(
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_optional_u32(&args, "limit", "limit")?,
            )
            .await?;
            json_response(result)
        }
        "get_commits_since" => {
            let result = cmds::project_git::get_commits_since(
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "since", "since")?,
            )
            .await?;
            json_response(result)
        }
        "get_db_path" => {
            let result = cmds::data::get_db_path(db).await?;
            json_response(result)
        }
        "run_scan_execution" => {
            let request = arg_from_value::<cmds::scan::execution::RunScanExecutionRequest>(
                &args, "request", "request",
            )?;
            let result =
                cmds::scan::execution::run_scan_execution(app, db, scan_control, request).await?;
            json_response(result)
        }
        "run_code_scan_audit" => {
            let result = cmds::code_scan::run_code_scan_audit(
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_optional_string(&args, "projectPath", "project_path")?,
                arg_optional_bool(&args, "inspectLocalDatabases", "inspect_local_databases")?,
            )
            .await?;
            json_response(result)
        }
        "get_log_path" => {
            let result = cmds::data::get_log_path(app).await?;
            json_response(result)
        }
        "read_recent_logs" => {
            let result =
                cmds::data::read_recent_logs(app, arg_optional_usize(&args, "lines", "lines")?)
                    .await?;
            json_response(result)
        }
        "inspect_desktop_watch_files" => {
            let result = cmds::desktop::inspect_desktop_watch_files(
                db,
                arg_from_value::<Vec<cmds::desktop::DesktopWatchRequest>>(
                    &args, "requests", "requests",
                )?,
            )
            .await?;
            json_response(result)
        }
        "resolve_project_files" => {
            let result = cmds::desktop::resolve_project_files(
                db,
                arg_string(&args, "projectPath", "project_path")?,
                arg_from_value::<Vec<String>>(&args, "relativePaths", "relative_paths")?,
            )
            .await?;
            json_response(result)
        }
        "open_path_in_editor" => {
            cmds::desktop::open_path_in_editor(db, arg_string(&args, "path", "path")?).await?;
            json_response(())
        }
        "reveal_path" => {
            cmds::desktop::reveal_path(db, arg_string(&args, "path", "path")?).await?;
            json_response(())
        }
        "resolve_fix_locations_for_check" => {
            let result = cmds::correlation::resolve_fix_locations_for_check(
                db,
                arg_string(&args, "checkId", "check_id")?,
                arg_i64(&args, "projectId", "project_id")?,
            )
            .await?;
            json_response(result)
        }
        "register_agent_tool" => {
            let result = cmds::agent_tools::register_agent_tool(
                app,
                arg_from_value::<crate::core::agent_tools::AgentTool>(&args, "tool", "tool")?,
            )
            .await?;
            json_response(result)
        }
        "unregister_agent_tool" => {
            let result = cmds::agent_tools::unregister_agent_tool(
                app,
                arg_from_value::<crate::core::agent_tools::AgentTool>(&args, "tool", "tool")?,
            )
            .await?;
            json_response(result)
        }
        "launch_agent_handoff" => {
            cmds::launch_agent_handoff(
                app,
                arg_from_value::<crate::core::agent_tools::AgentTool>(&args, "tool", "tool")?,
                arg_string(&args, "kickoffPrompt", "kickoff_prompt")?,
                arg_optional_string(&args, "projectPath", "project_path")?,
            )
            .await?;
            json_response(())
        }
        // `BrokerScope::admit` already checked `command` against
        // `FILESYSTEM_ACCESS_COMMANDS`, so every string reaching here has a match arm.
        _ => unreachable!("admit validated {command} against the {SCOPE_LABEL} allowlist"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::privileged_command_broker::PrivilegedCommandTokenState;
    use serde_json::json;

    #[test]
    fn missing_token_is_rejected_before_any_work() {
        let tokens = PrivilegedCommandTokenState::default();
        let result = tokens.consume(None, BROKER_COMMAND, "run_scan_execution", &json!({}));
        let error = result.expect_err("missing token must be rejected");
        assert!(
            error.contains("Missing privileged command token"),
            "unexpected error message: {error}"
        );
    }

    #[test]
    fn stale_token_is_rejected_before_any_work() {
        let tokens = PrivilegedCommandTokenState::default();
        let result = tokens.consume(
            Some("0000000000000000000000000000000000000000000000000000000000000000"),
            BROKER_COMMAND,
            "run_scan_execution",
            &json!({}),
        );
        let error = result.expect_err("stale token must be rejected");
        assert!(
            error.contains("invalid or expired"),
            "unexpected error message: {error}"
        );
    }

    #[test]
    fn known_commands_are_routed_via_the_public_allowlist() {
        for command in [
            "detect_project_urls",
            "update_project_path",
            "get_git_status",
            "get_commits_since",
            "get_db_path",
            "run_scan_execution",
            "run_code_scan_audit",
            "get_log_path",
            "read_recent_logs",
            "inspect_desktop_watch_files",
            "resolve_project_files",
            "open_path_in_editor",
            "reveal_path",
            "resolve_fix_locations_for_check",
            "register_agent_tool",
            "unregister_agent_tool",
            "launch_agent_handoff",
        ] {
            assert!(
                FILESYSTEM_ACCESS_COMMANDS.contains(&command),
                "{command} must be present in FILESYSTEM_ACCESS_COMMANDS",
            );
        }
        assert_eq!(
            FILESYSTEM_ACCESS_COMMANDS.len(),
            17,
            "FILESYSTEM_ACCESS_COMMANDS must cover exactly the dispatcher's known commands",
        );
    }

    #[test]
    fn broker_command_constant_matches_token_issue_name() {
        assert_eq!(BROKER_COMMAND, "run_filesystem_access_command");
    }
}
