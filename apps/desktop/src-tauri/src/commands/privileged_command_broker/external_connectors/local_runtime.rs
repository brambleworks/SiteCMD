use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::commands::privileged_command_broker::{
    arg_bool, arg_i64, arg_optional_bool, arg_optional_string, arg_string, json_response,
};
use crate::db::Database;

pub(super) const COMMANDS: &[&str] = &[
    "detect_updates",
    "get_pagespeed_report",
    "set_pagespeed_api_key",
    "pagespeed_api_key_is_set",
    "check_app_update",
    "download_and_install_app_update",
    "save_webhook_config",
    "test_webhook",
    "activate_license",
    "validate_license",
    "open_external_url",
];

pub(super) async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    command: &str,
    args: Value,
) -> Result<Value, String> {
    use crate::commands as cmds;
    match command {
        "detect_updates" => {
            let result = cmds::updates::detect_updates(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_optional_string(&args, "projectPath", "project_path")?,
            )
            .await?;
            json_response(result)
        }
        "get_pagespeed_report" => {
            let result = cmds::scan::get_pagespeed_report(
                app,
                arg_string(&args, "url", "url")?,
                arg_string(&args, "strategy", "strategy")?,
            )
            .await?;
            json_response(result)
        }
        "set_pagespeed_api_key" => {
            cmds::scan::set_pagespeed_api_key(app, arg_string(&args, "key", "key")?).await?;
            json_response(())
        }
        "pagespeed_api_key_is_set" => {
            let result = cmds::scan::pagespeed_api_key_is_set(app).await?;
            json_response(result)
        }
        "check_app_update" => {
            let result = cmds::updates::check_app_update(app).await?;
            json_response(result)
        }
        "download_and_install_app_update" => {
            let result = cmds::updates::download_and_install_app_update(app).await?;
            json_response(result)
        }
        "save_webhook_config" => {
            let result = cmds::webhooks::save_webhook_config(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "url", "url")?,
                arg_string(&args, "events", "events")?,
                arg_optional_string(&args, "secret", "secret")?,
                arg_bool(&args, "enabled", "enabled")?,
            )
            .await?;
            json_response(result)
        }
        "test_webhook" => {
            let result = cmds::webhooks::test_webhook(app, db, arg_i64(&args, "id", "id")?).await?;
            json_response(result)
        }
        "activate_license" => {
            let result = crate::licensing::commands::activate_license(
                app,
                arg_string(&args, "key", "key")?,
                db,
            )
            .await?;
            json_response(result)
        }
        "validate_license" => {
            let result = crate::licensing::commands::validate_license(
                app,
                db,
                arg_optional_bool(&args, "force", "force")?,
            )
            .await?;
            json_response(result)
        }
        "open_external_url" => {
            cmds::desktop::open_external_url(app, arg_string(&args, "url", "url")?).await?;
            json_response(())
        }
        _ => Err(format!(
            "Unsupported {} command.",
            super::super::SCOPE_LABEL
        )),
    }
}
