use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::commands::privileged_command_broker::{
    arg_bool, arg_from_value, arg_i64, arg_string, json_response,
};
use crate::db::Database;

pub(super) const COMMANDS: &[&str] = &[
    "create_connected_report",
    "list_connected_reports",
    "revoke_connected_report",
    "list_connected_alerts",
    "create_connected_alert_webhook",
    "list_connected_alert_webhooks",
    "test_connected_alert_webhook",
    "rotate_connected_alert_webhook",
    "delete_connected_alert_webhook",
];

pub(super) async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    command: &str,
    args: Value,
) -> Result<Value, String> {
    use crate::commands as cmds;
    match command {
        "create_connected_report" => {
            let result = cmds::create_connected_report(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_bool(&args, "includeRoutes", "include_routes")?,
                arg_from_value::<u32>(&args, "ttlDays", "ttl_days")?,
            )
            .await?;
            json_response(result)
        }
        "list_connected_reports" => {
            let result = cmds::list_connected_reports(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "revoke_connected_report" => {
            let result = cmds::revoke_connected_report(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "reportId", "report_id")?,
            )
            .await?;
            json_response(result)
        }
        "list_connected_alerts" => {
            let result = cmds::list_connected_alerts(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "create_connected_alert_webhook" => {
            let result = cmds::create_connected_alert_webhook(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "url", "url")?,
            )
            .await?;
            json_response(result)
        }
        "list_connected_alert_webhooks" => {
            let result = cmds::list_connected_alert_webhooks(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "test_connected_alert_webhook" => {
            let result = cmds::test_connected_alert_webhook(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "webhookId", "webhook_id")?,
            )
            .await?;
            json_response(result)
        }
        "rotate_connected_alert_webhook" => {
            let result = cmds::rotate_connected_alert_webhook(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "webhookId", "webhook_id")?,
            )
            .await?;
            json_response(result)
        }
        "delete_connected_alert_webhook" => {
            cmds::delete_connected_alert_webhook(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_string(&args, "webhookId", "webhook_id")?,
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
