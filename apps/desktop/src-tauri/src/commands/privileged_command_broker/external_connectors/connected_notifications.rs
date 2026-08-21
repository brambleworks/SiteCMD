use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::commands::privileged_command_broker::{
    arg_bool, arg_i64, arg_optional_i64, arg_optional_string, arg_string, json_response,
};
use crate::db::Database;

pub(super) const COMMANDS: &[&str] = &[
    "create_connected_destination",
    "list_connected_destinations",
    "update_connected_destination_policy",
    "resend_connected_destination_verification",
    "delete_connected_destination",
    "get_connected_notification_settings",
    "put_connected_notification_settings",
    "get_site_baseline",
    "decide_site_baseline",
];

pub(super) async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    command: &str,
    args: Value,
) -> Result<Value, String> {
    use crate::commands as cmds;
    match command {
        "create_connected_destination" => {
            let result =
                cmds::create_connected_destination(app, arg_string(&args, "address", "address")?)
                    .await?;
            json_response(result)
        }
        "list_connected_destinations" => {
            let result = cmds::list_connected_destinations(app).await?;
            json_response(result)
        }
        "update_connected_destination_policy" => {
            let result = cmds::update_connected_destination_policy(
                app,
                arg_string(&args, "destinationId", "destination_id")?,
                arg_i64(&args, "revision", "revision")?,
                arg_bool(&args, "immediateDisabled", "immediate_disabled")?,
                arg_bool(&args, "digestDisabled", "digest_disabled")?,
            )
            .await?;
            json_response(result)
        }
        "resend_connected_destination_verification" => {
            let result = cmds::resend_connected_destination_verification(
                app,
                arg_string(&args, "destinationId", "destination_id")?,
            )
            .await?;
            json_response(result)
        }
        "delete_connected_destination" => {
            let result = cmds::delete_connected_destination(
                app,
                arg_string(&args, "destinationId", "destination_id")?,
            )
            .await?;
            json_response(result)
        }
        "get_connected_notification_settings" => {
            let result = cmds::get_connected_notification_settings(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "put_connected_notification_settings" => {
            let result = cmds::put_connected_notification_settings(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "environmentScopeKey", "environment_scope_key")?,
                arg_i64(&args, "revision", "revision")?,
                arg_optional_string(&args, "destinationId", "destination_id")?,
                arg_bool(&args, "mute", "mute")?,
                arg_bool(&args, "allQuietHeartbeat", "all_quiet_heartbeat")?,
                arg_optional_string(&args, "severityFloor", "severity_floor")?,
                arg_string(&args, "digestCadence", "digest_cadence")?,
                arg_string(&args, "contentMode", "content_mode")?,
            )
            .await?;
            json_response(result)
        }
        "get_site_baseline" => {
            let result = cmds::get_site_baseline(
                app,
                db,
                arg_i64(&args, "siteId", "site_id")?,
                arg_optional_i64(&args, "projectId", "project_id")?,
                arg_optional_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        "decide_site_baseline" => {
            let result = cmds::decide_site_baseline(
                app,
                db,
                arg_i64(&args, "siteId", "site_id")?,
                arg_string(&args, "field", "field")?,
                arg_i64(&args, "basedOnRevision", "based_on_revision")?,
                arg_string(&args, "expectedDigest", "expected_digest")?,
                arg_bool(&args, "accept", "accept")?,
                arg_optional_i64(&args, "projectId", "project_id")?,
                arg_optional_string(&args, "environmentScopeKey", "environment_scope_key")?,
            )
            .await?;
            json_response(result)
        }
        _ => Err(format!(
            "Unsupported {} command.",
            super::super::SCOPE_LABEL
        )),
    }
}
