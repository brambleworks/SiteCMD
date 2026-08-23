use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State, Window};

use super::super::scan::ScanControlState;
use super::{
    arg_string, issue_scoped_privileged_command_token, json_response, PrivilegedCommandRequest,
    PrivilegedCommandTokenRequest, PrivilegedCommandTokenState,
};
use crate::db::Database;

pub(super) const BROKER_COMMAND: &str = "run_project_execution_command";
pub(super) const SCOPE_LABEL: &str = "project execution";

pub const PROJECT_EXECUTION_COMMANDS: &[&str] = &["run_project_command"];

/// Scoped token issuer for project execution commands. Issues silently:
/// `run_project_command` confirms natively inside the handler (enforced by
/// command-security.json `nativeConfirmedCommands`).
#[tauri::command]
#[tracing::instrument(skip(app, window, token_state, request), fields(broker = "run_project_execution_command", command = %request.command))]
pub async fn issue_project_execution_command_token(
    app: AppHandle,
    window: Window,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandTokenRequest,
) -> Result<String, String> {
    issue_scoped_privileged_command_token(app, window, token_state, request, BROKER_COMMAND).await
}

/// Feature-scoped broker for explicitly confirmed project command execution.
#[tauri::command]
#[tracing::instrument(skip(app, db, _scan_control, token_state, request), fields(command = %request.command))]
pub async fn run_project_execution_command(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    _scan_control: State<'_, ScanControlState>,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandRequest,
) -> Result<Value, String> {
    super::BrokerScope::by_broker(BROKER_COMMAND)
        .expect("registered scope")
        .admit(&token_state, &request)?;
    dispatch(app, db, request.command, request.args).await
}

async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    command: String,
    args: Value,
) -> Result<Value, String> {
    use crate::commands as cmds;
    match command.as_str() {
        "run_project_command" => {
            let result = cmds::desktop::run_project_command(
                app,
                db,
                arg_string(&args, "projectPath", "project_path")?,
                arg_string(&args, "command", "command")?,
            )
            .await?;
            json_response(result)
        }
        // `BrokerScope::admit` already checked `command` against
        // `PROJECT_EXECUTION_COMMANDS`, so every string reaching here has a match arm.
        _ => unreachable!("admit validated {command} against the {SCOPE_LABEL} allowlist"),
    }
}
