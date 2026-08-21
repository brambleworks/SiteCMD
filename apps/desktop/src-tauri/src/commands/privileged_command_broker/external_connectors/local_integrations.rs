use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::commands::privileged_command_broker::{
    arg_from_value, arg_i64, arg_optional_string, arg_string, json_response,
};
use crate::db::Database;

pub(super) const COMMANDS: &[&str] = &[
    "create_issue_link",
    "save_integration",
    "delete_integration",
    "fetch_integration_data",
    "fetch_analytics",
    "fetch_github_data",
    "github_latest_release",
    "connect_github",
    "complete_github_oauth",
    "save_github_integration",
    "invalidate_analytics_cache",
    "connect_google",
    "complete_google_oauth",
    "save_google_integration",
];

pub(super) async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    command: &str,
    args: Value,
) -> Result<Value, String> {
    use crate::commands as cmds;
    match command {
        "create_issue_link" => {
            let result = cmds::create_issue_link(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "checkId", "check_id")?,
                arg_i64(&args, "scanId", "scan_id")?,
                arg_string(&args, "provider", "provider")?,
                arg_from_value::<u32>(&args, "estimatedImpact", "estimated_impact")?,
            )
            .await?;
            json_response(result)
        }
        "save_integration" => {
            cmds::integrations::save_integration(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_from_value::<crate::integrations::IntegrationConfig>(
                    &args, "config", "config",
                )?,
            )
            .await?;
            json_response(())
        }
        "delete_integration" => {
            cmds::integrations::delete_integration(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "integrationType", "integration_type")?,
            )
            .await?;
            json_response(())
        }
        "fetch_integration_data" => {
            let result = cmds::integrations::fetch_integration_data(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "integrationType", "integration_type")?,
                arg_optional_string(&args, "urlFilter", "url_filter")?,
            )
            .await?;
            json_response(result)
        }
        "fetch_analytics" => {
            let result = cmds::integrations::fetch_analytics(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "period", "period")?,
                arg_optional_string(&args, "siteUrl", "site_url")?,
            )
            .await?;
            json_response(result)
        }
        "fetch_github_data" => {
            let result = cmds::integrations::fetch_github_data(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
            )
            .await?;
            json_response(result)
        }
        "github_latest_release" => {
            let result = cmds::integrations::github_latest_release(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
            )
            .await?;
            json_response(result)
        }
        "connect_github" => {
            let result =
                cmds::oauth::connect_github(arg_i64(&args, "projectId", "project_id")?).await?;
            json_response(result)
        }
        "complete_github_oauth" => {
            let result = cmds::oauth::complete_github_oauth(
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "flowId", "flow_id")?,
            )
            .await?;
            json_response(result)
        }
        "save_github_integration" => {
            let result = cmds::oauth::save_github_integration(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "flowId", "flow_id")?,
                arg_string(&args, "repo", "repo")?,
            )
            .await?;
            json_response(result)
        }
        "invalidate_analytics_cache" => {
            cmds::integrations::invalidate_analytics_cache(
                db,
                arg_i64(&args, "projectId", "project_id")?,
            )
            .await?;
            json_response(())
        }
        "connect_google" => {
            let result =
                cmds::oauth::connect_google(arg_i64(&args, "projectId", "project_id")?).await?;
            json_response(result)
        }
        "complete_google_oauth" => {
            let result = cmds::oauth::complete_google_oauth(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "flowId", "flow_id")?,
            )
            .await?;
            json_response(result)
        }
        "save_google_integration" => {
            let result = cmds::oauth::save_google_integration(
                app,
                db,
                arg_i64(&args, "projectId", "project_id")?,
                arg_string(&args, "flowId", "flow_id")?,
                arg_string(&args, "integrationType", "integration_type")?,
                arg_string(&args, "siteId", "site_id")?,
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
