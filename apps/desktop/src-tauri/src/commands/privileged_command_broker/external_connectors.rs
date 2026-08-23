use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State, Window};

use super::super::scan::ScanControlState;
use super::{
    emit_privileged_command_response, issue_scoped_privileged_command_token,
    PrivilegedCommandRequest, PrivilegedCommandTokenRequest, PrivilegedCommandTokenState,
};
use crate::db::Database;

mod dispatch;

pub(super) const BROKER_COMMAND: &str = "run_external_connector_command";
pub(super) const SCOPE_LABEL: &str = "external connector";

pub const EXTERNAL_CONNECTOR_COMMANDS: &[&str] = &[
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
    "sync_connected_site",
    "sync_connected_scan_scope",
    "import_connected_connection",
    "export_connected_connection",
    "unlink_connected_site",
    "disconnect_connected_site",
    "erase_connected_site",
    "activate_connected_service",
    "create_connected_site",
    "verify_connected_site",
    "mint_connected_ci_token",
    "fetch_connected_site_state",
    "list_connected_site_credentials",
    "mint_connected_webhook_secret",
    "rotate_connected_site_credential",
    "revoke_connected_site_credential",
    "reconnect_connected_site",
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
    "create_connected_report",
    "list_connected_reports",
    "revoke_connected_report",
    "list_connected_alerts",
    "create_connected_alert_webhook",
    "list_connected_alert_webhooks",
    "test_connected_alert_webhook",
    "rotate_connected_alert_webhook",
    "delete_connected_alert_webhook",
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

#[tauri::command]
#[tracing::instrument(skip(app, window, token_state, request), fields(broker = "run_external_connector_command", command = %request.command))]
pub async fn issue_external_connector_command_token(
    app: AppHandle,
    window: Window,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandTokenRequest,
) -> Result<String, String> {
    issue_scoped_privileged_command_token(app, window, token_state, request, BROKER_COMMAND).await
}

/// Feature-scoped broker for connector, OAuth, update, webhook, and license commands.
#[tauri::command]
#[tracing::instrument(skip(app, db, _scan_control, token_state, request), fields(command = %request.command))]
pub async fn run_external_connector_command(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    _scan_control: State<'_, ScanControlState>,
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
    let outcome = dispatch::dispatch(app.clone(), db, command, args).await;
    emit_privileged_command_response(&app, response_event.as_deref(), &outcome);
    outcome
}

#[cfg(test)]
#[path = "external_connectors_tests.rs"]
mod tests;
