use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::{Manager, Runtime};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
#[cfg(not(target_os = "macos"))]
use tauri_plugin_notification::NotificationExt;
use ts_rs::TS;

#[cfg(target_os = "macos")]
use mac_notification_sys::{
    MainButton, Notification as MacNotification, NotificationResponse as MacNotificationResponse,
};

use crate::TrayState;

use super::desktop_project_commands::{
    parse_project_command, resolve_registered_project_target, run_project_command_process,
    validate_project_command_policy, DesktopCommandResult,
};
pub(crate) use super::desktop_watch::inspect_watch_files;
use super::desktop_watch::resolve_existing_path;
#[cfg(test)]
use super::desktop_watch::{matches_watch_pattern, resolve_existing_watch_paths};
#[cfg(target_os = "macos")]
use super::emit_event;
use super::{confirm_sensitive_action, sanitize_error, SensitiveActionError, SensitiveActionTone};

const MAX_EXTERNAL_BROWSER_URL_CHARS: usize = 2_048;

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DesktopWatchRequest {
    pub project_id: i64,
    pub project_path: String,
    pub primary_url: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DesktopWatchSignal {
    pub project_id: i64,
    pub url: Option<String>,
    pub kind: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub modified_ms: u64,
    pub page: String,
    pub focus: Option<String>,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ResolvedProjectPath {
    pub relative_path: String,
    pub absolute_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DesktopNotificationTarget {
    pub page: String,
    pub project_id: Option<i64>,
    pub url: Option<String>,
    pub scan_id: Option<i64>,
    pub session_id: Option<i64>,
    pub scan_kind: Option<String>,
    pub focus: Option<String>,
    pub item_id: Option<String>,
    pub prompt_id: Option<String>,
    pub lane: Option<String>,
    pub reason: Option<String>,
    pub file_path: Option<String>,
    #[serde(default)]
    pub restore_scan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DesktopNotificationAction {
    pub id: String,
    pub label: String,
    pub target: Option<DesktopNotificationTarget>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ActionableDesktopNotificationRequest {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub click_target: Option<DesktopNotificationTarget>,
    #[serde(default)]
    pub actions: Vec<DesktopNotificationAction>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopNotificationActionEvent {
    pub source_id: Option<String>,
    pub action_id: String,
    pub target: Option<DesktopNotificationTarget>,
    pub file_path: Option<String>,
}

fn format_summary(
    attention_count: u32,
    pending_count: u32,
    prompt_count: u32,
    running_count: u32,
) -> String {
    let mut parts = Vec::new();
    if attention_count > 0 {
        parts.push(format!("{} need attention", attention_count));
    }
    if pending_count > 0 {
        parts.push(format!("{} pending verify", pending_count));
    }
    if prompt_count > 0 {
        parts.push(format!("{} suggestions", prompt_count));
    }
    if running_count > 0 {
        parts.push(format!("{} running", running_count));
    }
    if parts.is_empty() {
        "All caught up".to_string()
    } else {
        parts.join(" • ")
    }
}

#[cfg(target_os = "macos")]
fn focus_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(target_os = "macos")]
fn build_macos_notification_response(
    app: AppHandle,
    request: ActionableDesktopNotificationRequest,
) {
    let app_identifier = app.config().identifier.clone();
    std::thread::spawn(move || {
        let _ = mac_notification_sys::set_application(&app_identifier);

        let has_interaction = request.click_target.is_some() || !request.actions.is_empty();

        let mut notification = MacNotification::new();
        notification.title(&request.title).message(&request.body);

        if has_interaction {
            notification.wait_for_click(true);
        } else {
            notification.asynchronous(true);
        }

        let action_labels = request
            .actions
            .iter()
            .map(|action| action.label.clone())
            .collect::<Vec<_>>();
        let action_label_refs = action_labels.iter().map(String::as_str).collect::<Vec<_>>();

        match action_label_refs.as_slice() {
            [single] => {
                notification.main_button(MainButton::SingleAction(single));
                notification.close_button("Dismiss");
            }
            [] => {}
            many => {
                notification.main_button(MainButton::DropdownActions("Actions", many));
                notification.close_button("Dismiss");
            }
        }

        let response = match notification.send() {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(
                    "Failed to deliver actionable desktop notification: {}",
                    error
                );
                return;
            }
        };

        let payload = match response {
            MacNotificationResponse::Click => {
                request
                    .click_target
                    .clone()
                    .map(|target| DesktopNotificationActionEvent {
                        source_id: request.id.clone(),
                        action_id: "notification-click".to_string(),
                        target: Some(target),
                        file_path: None,
                    })
            }
            MacNotificationResponse::ActionButton(label) => request
                .actions
                .iter()
                .find(|action| action.label == label)
                .map(|action| DesktopNotificationActionEvent {
                    source_id: request.id.clone(),
                    action_id: action.id.clone(),
                    target: action.target.clone(),
                    file_path: action.file_path.clone(),
                }),
            MacNotificationResponse::CloseButton(_) => None,
            MacNotificationResponse::Reply(_) => None,
            MacNotificationResponse::None => None,
        };

        if let Some(payload) = payload {
            focus_main_window(&app);
            emit_event(&app, "desktop-notification-action", payload);
        }
    });
}

#[tracing::instrument(skip(db, requests))]
pub async fn inspect_desktop_watch_files(
    db: tauri::State<'_, std::sync::Arc<crate::db::Database>>,
    requests: Vec<DesktopWatchRequest>,
) -> Result<Vec<DesktopWatchSignal>, String> {
    let db = (*db).clone();
    super::run_blocking(move || -> Result<Vec<DesktopWatchSignal>, String> {
        let projects = db.get_projects().map_err(super::sanitize_error)?;
        let requests = requests
            .into_iter()
            .filter(|request| {
                resolve_registered_project_target(&projects, &request.project_path)
                    .map(|path| path.is_dir())
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        Ok(inspect_watch_files(&requests))
    })
    .await?
}

#[tracing::instrument(skip(db, project_path, relative_paths), fields(relative_path_count = relative_paths.len()))]
pub async fn resolve_project_files(
    db: tauri::State<'_, std::sync::Arc<crate::db::Database>>,
    project_path: String,
    relative_paths: Vec<String>,
) -> Result<Vec<ResolvedProjectPath>, String> {
    let db = (*db).clone();
    super::run_blocking(move || -> Result<Vec<ResolvedProjectPath>, String> {
        let projects = db.get_projects().map_err(super::sanitize_error)?;
        let project_root = resolve_registered_project_target(&projects, &project_path)?;
        if !project_root.is_dir() {
            return Ok(Vec::new());
        }

        let mut resolved = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for relative_path in relative_paths {
            let normalized = relative_path.trim().replace('\\', "/");
            if normalized.is_empty() || !seen.insert(normalized.clone()) {
                continue;
            }

            let Some(existing_path) = resolve_existing_path(&project_root, &normalized) else {
                continue;
            };
            let absolute_path = existing_path
                .canonicalize()
                .unwrap_or(existing_path)
                .to_string_lossy()
                .to_string();

            resolved.push(ResolvedProjectPath {
                relative_path: normalized,
                absolute_path,
            });
        }

        Ok(resolved)
    })
    .await?
}

#[tauri::command]
#[tracing::instrument(
    skip(tray_state),
    fields(attention_count, pending_count, prompt_count, running_count)
)]
pub async fn update_tray_summary(
    tray_state: tauri::State<'_, TrayState>,
    attention_count: u32,
    pending_count: u32,
    prompt_count: u32,
    running_count: u32,
) -> Result<(), String> {
    let summary = format_summary(attention_count, pending_count, prompt_count, running_count);
    let tooltip = format!("SiteCMD - {}", summary);

    {
        let mut stored_tooltip = tray_state
            .summary_tooltip
            .write()
            .map_err(|_| "Failed to update tray summary".to_string())?;
        *stored_tooltip = tooltip.clone();
    }

    let _ = tray_state.summary_item.set_text(summary);
    if !tray_state
        .is_scanning
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        let _ = tray_state.tray_icon.set_tooltip(Some(&tooltip));
    }
    Ok(())
}

/// Confirms activation from the externally triggerable `sitecmd://activate` link.
/// The frontend owns the call because startup and running-app links converge there.
#[tauri::command]
#[tracing::instrument(skip(app))]
/// Returns `Ok(false)` for a decline and `Err` when the dialog could not ask.
pub async fn confirm_link_license_activation(app: AppHandle) -> Result<bool, String> {
    match confirm_sensitive_action(
        app,
        "Activate this license?",
        SensitiveActionTone::Warning,
        "A link asked SiteCMD to activate a license key on this computer.\n\nOnly continue if you just purchased SiteCMD or asked for this activation yourself."
            .to_string(),
        "Activate License",
    )
    .await
    {
        Ok(()) => Ok(true),
        Err(SensitiveActionError::Declined) => Ok(false),
        Err(error @ SensitiveActionError::Failed(_)) => Err(String::from(error)),
    }
}

#[tauri::command]
#[tracing::instrument(skip(app, request))]
pub async fn send_actionable_desktop_notification(
    app: AppHandle,
    request: ActionableDesktopNotificationRequest,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        build_macos_notification_response(app, request);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut builder = app.notification().builder();
        builder = builder.title(request.title).body(request.body);
        builder.show().map_err(sanitize_error)?;
        Ok(())
    }
}

async fn confirm_project_command(
    app: AppHandle,
    working_dir: &Path,
    command: &str,
) -> Result<(), String> {
    let message = format!(
        "Run this command in the linked project folder?\n\nFolder:\n{}\n\nCommand:\n{}\n\nBuild tools and package managers can execute code from this project or its dependencies. Continue only if you trust the project and recognize this command.",
        working_dir.display(),
        command
    );
    let approved = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .message(message)
            .title("Run suggested command?")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Run Command".to_string(),
                "Cancel".to_string(),
            ))
            .blocking_show()
    })
    .await
    .map_err(sanitize_error)?;

    if approved {
        Ok(())
    } else {
        Err("Project command cancelled".to_string())
    }
}

#[tracing::instrument(skip(app, db, project_path, command), fields(command_len = command.len()))]
pub async fn run_project_command(
    app: AppHandle,
    db: tauri::State<'_, std::sync::Arc<crate::db::Database>>,
    project_path: String,
    command: String,
) -> Result<DesktopCommandResult, String> {
    let (executable, args) = parse_project_command(&command)?;
    let working_dir = {
        let db = (*db).clone();
        super::run_blocking(move || -> Result<std::path::PathBuf, String> {
            let projects = db.get_projects().map_err(super::sanitize_error)?;
            resolve_registered_project_target(&projects, &project_path)
        })
        .await??
    };
    if !working_dir.is_dir() {
        return Err("Project folder is missing or unavailable".into());
    }
    validate_project_command_policy(&executable, &args)?;
    confirm_project_command(app, &working_dir, &command).await?;

    run_project_command_process(executable, args, &working_dir).await
}

#[tracing::instrument(skip(db, path), fields(path_len = path.len()))]
pub async fn open_path_in_editor(
    db: tauri::State<'_, std::sync::Arc<crate::db::Database>>,
    path: String,
) -> Result<(), String> {
    let db = (*db).clone();
    super::run_blocking(move || -> Result<(), String> {
        let projects = db.get_projects().map_err(super::sanitize_error)?;
        let target = resolve_registered_project_target(&projects, &path)?;

        #[cfg(target_os = "macos")]
        {
            for app_name in ["Cursor", "Visual Studio Code", "Zed", "Windsurf", "Xcode"] {
                if Command::new("open")
                    .arg("-a")
                    .arg(app_name)
                    .arg(&target)
                    .status()
                    .map(|status| status.success())
                    .unwrap_or(false)
                {
                    return Ok(());
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            for editor in ["cursor", "code", "windsurf", "zed"] {
                if Command::new(editor)
                    .arg(&target)
                    .spawn()
                    .map(|_| true)
                    .unwrap_or(false)
                {
                    return Ok(());
                }
            }
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            for editor in ["cursor", "code", "windsurf", "zed"] {
                if Command::new(editor)
                    .arg(&target)
                    .spawn()
                    .map(|_| true)
                    .unwrap_or(false)
                {
                    return Ok(());
                }
            }
        }

        open::that_detached(&target).map_err(sanitize_error)
    })
    .await?
}

#[tracing::instrument(skip(db, path), fields(path_len = path.len()))]
pub async fn reveal_path(
    db: tauri::State<'_, std::sync::Arc<crate::db::Database>>,
    path: String,
) -> Result<(), String> {
    let db = (*db).clone();
    super::run_blocking(move || -> Result<(), String> {
        let projects = db.get_projects().map_err(super::sanitize_error)?;
        let target = resolve_registered_project_target(&projects, &path)?;

        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg("-R")
                .arg(&target)
                .status()
                .map_err(sanitize_error)?;
            Ok(())
        }

        #[cfg(target_os = "windows")]
        {
            Command::new("explorer")
                .arg("/select,")
                .arg(&target)
                .status()
                .map_err(sanitize_error)?;
            Ok(())
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let folder = target.parent().unwrap_or(&target);
            open::that_detached(folder).map_err(sanitize_error)?;
            Ok(())
        }
    })
    .await?
}

pub(super) fn validate_external_browser_url(value: &str) -> Result<url::Url, String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_EXTERNAL_BROWSER_URL_CHARS
        || value.chars().any(char::is_control)
    {
        return Err("External URL is empty, too long, or contains control characters.".to_string());
    }

    let parsed = url::Url::parse(value).map_err(|_| "External URL is invalid.".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only HTTP and HTTPS links can be opened.".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("External URLs cannot contain credentials.".to_string());
    }
    if parsed.host_str().is_none() {
        return Err("External URL must include a host.".to_string());
    }
    Ok(parsed)
}

#[tracing::instrument(skip(url), fields(url_len = url.len()))]
pub fn open_external_url(url: String) -> Result<(), String> {
    let parsed = validate_external_browser_url(&url)?;
    let audit_detail = serde_json::json!({ "host": parsed.host_str() });

    match open::that(parsed.as_str()).map_err(sanitize_error) {
        Ok(()) => {
            crate::audit_log::record("external.open", audit_detail, "ok");
            Ok(())
        }
        Err(error) => {
            crate::audit_log::record("external.open", audit_detail, "fail");
            Err(error)
        }
    }
}

#[cfg(test)]
#[path = "desktop_tests.rs"]
mod tests;
