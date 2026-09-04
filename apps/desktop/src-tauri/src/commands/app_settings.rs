//! Renderer access to the fixed application settings store.

use serde_json::Value;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use super::{CommandError, CommandResult};

pub(crate) const APP_SETTINGS_FILE: &str = "settings.json";

async fn with_app_settings<R, T, F>(
    app: AppHandle<R>,
    operation: &'static str,
    access: F,
) -> CommandResult<T>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(&tauri_plugin_store::Store<R>) -> T + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        tracing::trace!(operation, "Accessing application settings");
        let store = app
            .store(APP_SETTINGS_FILE)
            .map_err(|error| CommandError::new(error.to_string()))?;
        Ok(access(&store))
    })
    .await
    .map_err(|error| CommandError::new(format!("Application settings task failed: {error}")))?
}

#[tauri::command]
pub async fn get_app_setting<R: Runtime>(
    app: AppHandle<R>,
    key: String,
) -> Result<Option<Value>, CommandError> {
    with_app_settings(app, "get_app_setting", move |store| store.get(key)).await
}

#[tauri::command]
pub async fn set_app_setting<R: Runtime>(
    app: AppHandle<R>,
    key: String,
    value: Value,
) -> Result<(), CommandError> {
    with_app_settings(app, "set_app_setting", move |store| {
        store.set(key, value);
    })
    .await
}

#[cfg(test)]
#[path = "app_settings_tests.rs"]
mod tests;
