use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::commands::privileged_command_broker::{arg_i64, arg_string, json_response};
use crate::db::Database;

pub(super) const COMMANDS: &[&str] = &[
    "create_connected_provider_connection",
    "list_connected_provider_connections",
    "list_connected_provider_projects",
    "revoke_connected_provider_connection",
    "verify_connected_site_provider",
    "rotate_connected_fingerprint_key",
    "abort_connected_key_rotation",
    "request_account_recovery",
    "get_account_recovery",
    "acknowledge_account_recovery",
    "cancel_account_recovery",
];

pub(super) async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    command: &str,
    args: Value,
) -> Result<Value, String> {
    use crate::commands as cmds;
    match command {
        "create_connected_provider_connection" => {
            let result = cmds::create_connected_provider_connection(
                app,
                arg_string(&args, "provider", "provider")?,
            )
            .await?;
            json_response(result)
        }
        "list_connected_provider_connections" => {
            let result = cmds::list_connected_provider_connections(app).await?;
            json_response(result)
        }
        "list_connected_provider_projects" => {
            let result = cmds::list_connected_provider_projects(
                app,
                arg_string(&args, "connectionId", "connection_id")?,
            )
            .await?;
            json_response(result)
        }
        "revoke_connected_provider_connection" => {
            cmds::revoke_connected_provider_connection(
                app,
                arg_string(&args, "connectionId", "connection_id")?,
            )
            .await?;
            json_response(())
        }
        "verify_connected_site_provider" => {
            let result = cmds::verify_connected_site_provider(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "connectionId", "connection_id")?,
                arg_string(&args, "externalProjectId", "external_project_id")?,
            )
            .await?;
            json_response(result)
        }
        "rotate_connected_fingerprint_key" => {
            let result = cmds::rotate_connected_fingerprint_key(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "abort_connected_key_rotation" => {
            cmds::abort_connected_key_rotation(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(())
        }
        "request_account_recovery" => {
            let result = cmds::request_account_recovery(app).await?;
            json_response(result)
        }
        "get_account_recovery" => {
            let result = cmds::get_account_recovery(app).await?;
            json_response(result)
        }
        "acknowledge_account_recovery" => {
            let result = cmds::acknowledge_account_recovery(app).await?;
            json_response(result)
        }
        "cancel_account_recovery" => {
            cmds::cancel_account_recovery(app).await?;
            json_response(())
        }
        _ => Err(format!(
            "Unsupported {} command.",
            super::super::SCOPE_LABEL
        )),
    }
}
