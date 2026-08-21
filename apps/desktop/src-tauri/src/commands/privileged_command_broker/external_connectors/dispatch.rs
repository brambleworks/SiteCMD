use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::db::Database;

#[path = "connected_notifications.rs"]
mod connected_notifications;
#[path = "connected_providers.rs"]
mod connected_providers;
#[path = "connected_reports.rs"]
mod connected_reports;
#[path = "connected_sites.rs"]
mod connected_sites;
#[path = "local_integrations.rs"]
mod local_integrations;
#[path = "local_runtime.rs"]
mod local_runtime;

pub(super) async fn dispatch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    command: String,
    args: Value,
) -> Result<Value, String> {
    let command_name = command.as_str();
    if local_integrations::COMMANDS.contains(&command_name) {
        return local_integrations::dispatch(app, db, command_name, args).await;
    }
    if local_runtime::COMMANDS.contains(&command_name) {
        return local_runtime::dispatch(app, db, command_name, args).await;
    }
    if connected_sites::COMMANDS.contains(&command_name) {
        return connected_sites::dispatch(app, db, command_name, args).await;
    }
    if connected_providers::COMMANDS.contains(&command_name) {
        return connected_providers::dispatch(app, db, command_name, args).await;
    }
    if connected_reports::COMMANDS.contains(&command_name) {
        return connected_reports::dispatch(app, db, command_name, args).await;
    }
    if connected_notifications::COMMANDS.contains(&command_name) {
        return connected_notifications::dispatch(app, db, command_name, args).await;
    }
    Err(format!("Unsupported {} command.", super::SCOPE_LABEL))
}

#[cfg(test)]
pub(super) fn routed_commands() -> Vec<&'static str> {
    [
        local_integrations::COMMANDS,
        local_runtime::COMMANDS,
        connected_sites::COMMANDS,
        connected_providers::COMMANDS,
        connected_reports::COMMANDS,
        connected_notifications::COMMANDS,
    ]
    .into_iter()
    .flatten()
    .copied()
    .collect()
}
