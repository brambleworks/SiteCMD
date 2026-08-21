use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::commands::privileged_command_broker::{arg_i64, arg_string, json_response};
use crate::db::Database;

pub(super) const COMMANDS: &[&str] = &[
    "sync_connected_site",
    "sync_connected_scan_scope",
    "import_connected_connection",
    "export_connected_connection",
    "activate_connected_service",
    "disconnect_connected_site",
    "erase_connected_site",
    "create_connected_site",
    "fetch_connected_site_state",
    "list_connected_site_credentials",
    "mint_connected_webhook_secret",
    "rotate_connected_site_credential",
    "revoke_connected_site_credential",
    "reconnect_connected_site",
    "verify_connected_site",
    "mint_connected_ci_token",
    "unlink_connected_site",
];

pub(super) async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    command: &str,
    args: Value,
) -> Result<Value, String> {
    use crate::commands as cmds;
    match command {
        "sync_connected_site" => {
            let result = cmds::sync_connected_site(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "sync_connected_scan_scope" => {
            let result =
                cmds::sync_connected_scan_scope(app, db, arg_i64(&args, "siteId", "site_id")?)
                    .await?;
            json_response(result)
        }
        "import_connected_connection" => {
            let result = cmds::import_connected_connection(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "encryptedExport", "encrypted_export")?,
                arg_string(&args, "passphrase", "passphrase")?,
                arg_string(&args, "installationToken", "installation_token")?,
            )
            .await?;
            json_response(result)
        }
        "export_connected_connection" => {
            let result = cmds::export_connected_connection(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "passphrase", "passphrase")?,
            )
            .await?;
            json_response(result)
        }
        "activate_connected_service" => {
            let result = cmds::activate_connected_service(app, db).await?;
            json_response(result)
        }
        "disconnect_connected_site" => {
            cmds::disconnect_connected_site(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(())
        }
        "erase_connected_site" => {
            let result = cmds::erase_connected_site(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "create_connected_site" => {
            let result = cmds::create_connected_site(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "url", "url")?,
                arg_string(&args, "installationToken", "installation_token")?,
            )
            .await?;
            json_response(result)
        }
        "fetch_connected_site_state" => {
            let result = cmds::fetch_connected_site_state(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "list_connected_site_credentials" => {
            let result = cmds::list_connected_site_credentials(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "mint_connected_webhook_secret" => {
            let result = cmds::mint_connected_webhook_secret(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "rotate_connected_site_credential" => {
            let result = cmds::rotate_connected_site_credential(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "tokenId", "token_id")?,
            )
            .await?;
            json_response(result)
        }
        "revoke_connected_site_credential" => {
            cmds::revoke_connected_site_credential(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "tokenId", "token_id")?,
            )
            .await?;
            json_response(())
        }
        "reconnect_connected_site" => {
            let result = cmds::reconnect_connected_site(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "verify_connected_site" => {
            let result = cmds::verify_connected_site(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "method", "method")?,
            )
            .await?;
            json_response(result)
        }
        "mint_connected_ci_token" => {
            let result = cmds::mint_connected_ci_token(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "repository", "repository")?,
                arg_string(&args, "workflowRef", "workflow_ref")?,
                arg_string(&args, "gitRef", "git_ref")?,
            )
            .await?;
            json_response(result)
        }
        "unlink_connected_site" => {
            cmds::unlink_connected_site(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(())
        }
        _ => Err(format!(
            "Unsupported {} command.",
            super::super::SCOPE_LABEL
        )),
    }
}
