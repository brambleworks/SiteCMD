use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State, Window};

use super::super::scan::ScanControlState;
use super::{
    arg_bytes, arg_string, issue_scoped_privileged_command_token, json_response,
    PrivilegedCommandRequest, PrivilegedCommandTokenRequest, PrivilegedCommandTokenState,
};
use crate::db::Database;

pub(super) const BROKER_COMMAND: &str = "run_filesystem_export_command";
pub(super) const SCOPE_LABEL: &str = "filesystem export";

pub const FILESYSTEM_EXPORT_COMMANDS: &[&str] =
    &["write_export_file", "write_export_bytes", "export_database"];

/// Scoped token issuer for filesystem export commands. Issues silently:
/// every command this broker can dispatch confirms natively inside the
/// handler (enforced by command-security.json `nativeConfirmedCommands`).
#[tauri::command]
#[tracing::instrument(skip(app, window, token_state, request), fields(broker = "run_filesystem_export_command", command = %request.command))]
pub async fn issue_filesystem_export_command_token(
    app: AppHandle,
    window: Window,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandTokenRequest,
) -> Result<String, String> {
    issue_scoped_privileged_command_token(app, window, token_state, request, BROKER_COMMAND).await
}

/// Feature-scoped broker for confirmed filesystem export writes.
#[tauri::command]
#[tracing::instrument(skip(app, db, _scan_control, token_state, request), fields(command = %request.command))]
pub async fn run_filesystem_export_command(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    _scan_control: State<'_, ScanControlState>,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandRequest,
) -> Result<Value, String> {
    let command = request.command;
    if !FILESYSTEM_EXPORT_COMMANDS.contains(&command.as_str()) {
        return Err(format!("Unsupported {SCOPE_LABEL} command."));
    }
    token_state.consume(
        request.token.as_deref(),
        BROKER_COMMAND,
        &command,
        &request.args,
    )?;
    dispatch(app, db, command, request.args).await
}

async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    command: String,
    args: Value,
) -> Result<Value, String> {
    use crate::commands as cmds;
    match command.as_str() {
        "export_database" => {
            let result =
                cmds::data::export_database(app, db, arg_string(&args, "destPath", "dest_path")?)
                    .await?;
            json_response(result)
        }
        "write_export_file" => {
            cmds::data::write_export_file(
                app,
                arg_string(&args, "path", "path")?,
                arg_string(&args, "content", "content")?,
            )
            .await?;
            json_response(())
        }
        "write_export_bytes" => {
            cmds::data::write_export_bytes(
                app,
                arg_string(&args, "path", "path")?,
                arg_bytes(&args, "bytes", "bytes")?,
            )
            .await?;
            json_response(())
        }
        _ => Err(format!("Unsupported {SCOPE_LABEL} command.")),
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
        let result = tokens.consume(None, BROKER_COMMAND, "write_export_file", &json!({}));
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
            "write_export_file",
            &json!({}),
        );
        let error = result.expect_err("stale token must be rejected");
        assert!(
            error.contains("invalid or expired"),
            "unexpected error message: {error}"
        );
    }

    #[test]
    fn unknown_command_returns_scope_labelled_error() {
        let unsupported = format!("Unsupported {SCOPE_LABEL} command.");
        assert_eq!(unsupported, "Unsupported filesystem export command.");
    }

    #[test]
    fn known_commands_are_routed_via_the_public_allowlist() {
        for command in ["export_database", "write_export_file", "write_export_bytes"] {
            assert!(
                FILESYSTEM_EXPORT_COMMANDS.contains(&command),
                "{command} must be present in FILESYSTEM_EXPORT_COMMANDS",
            );
        }
        assert_eq!(
            FILESYSTEM_EXPORT_COMMANDS.len(),
            3,
            "FILESYSTEM_EXPORT_COMMANDS must cover exactly the dispatcher's known commands",
        );
    }

    #[test]
    fn broker_command_constant_matches_token_issue_name() {
        assert_eq!(BROKER_COMMAND, "run_filesystem_export_command");
    }
}
