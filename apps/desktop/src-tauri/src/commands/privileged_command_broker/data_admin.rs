use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State, Window};

use super::super::scan::ScanControlState;
use super::{
    arg_i64, arg_string, issue_scoped_privileged_command_token, json_response,
    PrivilegedCommandRequest, PrivilegedCommandTokenRequest, PrivilegedCommandTokenState,
};
use crate::db::Database;

pub(super) const BROKER_COMMAND: &str = "run_data_admin_command";
pub(super) const SCOPE_LABEL: &str = "data administration";

pub const DATA_ADMIN_COMMANDS: &[&str] = &[
    "delete_project",
    "delete_environment",
    "import_database",
    "clear_scan_history",
    "delete_scan",
    "delete_site_scans",
    "delete_event",
    "delete_report_history",
    "delete_webhook_config",
    "deactivate_license",
];

/// Issue a data-administration token without duplicating each handler's confirmation.
#[tauri::command]
#[tracing::instrument(skip(app, window, token_state, request), fields(broker = "run_data_admin_command", command = %request.command))]
pub async fn issue_data_admin_command_token(
    app: AppHandle,
    window: Window,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandTokenRequest,
) -> Result<String, String> {
    issue_scoped_privileged_command_token(app, window, token_state, request, BROKER_COMMAND).await
}

/// Feature-scoped broker for destructive SiteCMD data administration commands.
#[tauri::command]
#[tracing::instrument(skip(app, db, _scan_control, token_state, request), fields(command = %request.command))]
pub async fn run_data_admin_command(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    _scan_control: State<'_, ScanControlState>,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandRequest,
) -> Result<Value, String> {
    let command = request.command;
    if !DATA_ADMIN_COMMANDS.contains(&command.as_str()) {
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
        "delete_project" => {
            cmds::project::delete_project(app, db, arg_i64(&args, "projectId", "project_id")?)
                .await?;
            json_response(())
        }
        "delete_environment" => {
            cmds::project::delete_environment(
                app,
                db,
                arg_i64(&args, "environmentId", "environment_id")?,
            )
            .await?;
            json_response(())
        }
        "import_database" => {
            let result =
                cmds::data::import_database(app, db, arg_string(&args, "srcPath", "src_path")?)
                    .await?;
            json_response(result)
        }
        "clear_scan_history" => {
            let result = cmds::data::clear_scan_history(app, db).await?;
            json_response(result)
        }
        "delete_scan" => {
            cmds::data::delete_scan(app, db, arg_i64(&args, "scanId", "scan_id")?).await?;
            json_response(())
        }
        "delete_site_scans" => {
            let result =
                cmds::data::delete_site_scans(app, db, arg_i64(&args, "siteId", "site_id")?)
                    .await?;
            json_response(result)
        }
        "delete_event" => {
            cmds::events::delete_event(app, db, arg_i64(&args, "eventId", "event_id")?).await?;
            json_response(())
        }
        "delete_report_history" => {
            cmds::reports::delete_report_history(app, db, arg_i64(&args, "id", "id")?).await?;
            json_response(())
        }
        "delete_webhook_config" => {
            cmds::webhooks::delete_webhook_config(app, db, arg_i64(&args, "id", "id")?).await?;
            json_response(())
        }
        "deactivate_license" => {
            crate::licensing::commands::deactivate_license(app, db).await?;
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
        let result = tokens.consume(None, BROKER_COMMAND, "delete_project", &json!({}));
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
            "delete_project",
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
        assert_eq!(unsupported, "Unsupported data administration command.");
    }

    #[test]
    fn known_commands_are_routed_via_the_public_allowlist() {
        for command in [
            "delete_project",
            "delete_environment",
            "import_database",
            "clear_scan_history",
            "delete_scan",
            "delete_site_scans",
            "delete_event",
            "delete_report_history",
            "delete_webhook_config",
            "deactivate_license",
        ] {
            assert!(
                DATA_ADMIN_COMMANDS.contains(&command),
                "{command} must be present in DATA_ADMIN_COMMANDS",
            );
        }
        assert_eq!(
            DATA_ADMIN_COMMANDS.len(),
            10,
            "DATA_ADMIN_COMMANDS must cover exactly the dispatcher's known commands",
        );
    }

    #[test]
    fn broker_command_constant_matches_token_issue_name() {
        assert_eq!(BROKER_COMMAND, "run_data_admin_command");
    }
}
