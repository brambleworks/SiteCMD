use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, State, Window};

mod data_admin;
mod external_connectors;
mod filesystem_access;
mod filesystem_export;
mod project_execution;
mod token_state;

#[cfg(test)]
mod tests;

// Glob re-exports expose Tauri's generated command helpers to
// `tauri::generate_handler!` through the broker module.
pub use data_admin::*;
pub use external_connectors::*;
pub use filesystem_access::*;
pub use filesystem_export::*;
pub use project_execution::*;
pub use token_state::TokenStore;

/// Compatibility alias for existing Tauri state imports.
pub type PrivilegedCommandTokenState = TokenStore;

#[derive(Debug, Deserialize)]
pub struct PrivilegedCommandRequest {
    pub(super) command: String,
    #[serde(default)]
    pub(super) args: Value,
    pub(super) token: Option<String>,
    #[serde(default, alias = "responseEvent")]
    pub(super) response_event: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PrivilegedCommandTokenRequest {
    pub(super) command: String,
    #[serde(default)]
    pub(super) args: Value,
    #[serde(default)]
    pub(super) broker_command: Option<String>,
}

// Families whose handlers already confirm issue tokens silently to avoid a duplicate prompt.

/// Connector commands requiring native confirmation before token issuance.
/// Keep synchronized with `NATIVE_INTENT_CONNECTOR_COMMANDS`.
pub(super) const SENSITIVE_CONNECTOR_COMMANDS: &[&str] = &[
    "save_integration",
    "save_webhook_config",
    "test_webhook",
    "sync_connected_site",
    "import_connected_connection",
    "export_connected_connection",
    "unlink_connected_site",
    "disconnect_connected_site",
    "erase_connected_site",
    "create_connected_alert_webhook",
    "test_connected_alert_webhook",
    "delete_connected_alert_webhook",
    "create_connected_destination",
    "resend_connected_destination_verification",
    "delete_connected_destination",
    "revoke_connected_site_credential",
    "revoke_connected_provider_connection",
    "revoke_connected_report",
];

pub(super) const SENSITIVE_FILESYSTEM_ACCESS_COMMANDS: &[&str] = &[
    // Repointing a project decides which folder every non-sensitive read
    // (run_code_scan_audit, resolve_project_files, get_git_status,
    // read_recent_logs) exposes, so it needs the same native intent as
    // opening a path. See the broker threat model's scope table.
    "update_project_path",
    "open_path_in_editor",
    "reveal_path",
    "register_agent_tool",
    "unregister_agent_tool",
    // `launch_agent_handoff` is deliberately absent: it opens the agent's own
    // app through a deep link with a prompt staged in its composer, and the
    // agent never runs that prompt on its own, so a system dialog would only
    // stand between the person and the button they just pressed.
];

/// One row per broker: its command name, human label, command allowlist, and
/// the subset of that allowlist needing native user-intent confirmation
/// before a token is issued. `admit` is the seam every `run_*` entry calls;
/// `tauri::test::mock_app()` yields an `App<MockRuntime>` while the `run_*`
/// entry points are typed on the Wry `AppHandle`, so this seam is what unit
/// tests can exercise instead of the entry points themselves.
pub(crate) struct BrokerScope {
    pub(crate) broker_command: &'static str,
    pub(crate) label: &'static str,
    pub(crate) allowlist: &'static [&'static str],
    /// Commands that need a native user-intent confirmation before a token is issued.
    pub(crate) sensitive: &'static [&'static str],
}

pub(crate) const SCOPES: &[BrokerScope] = &[
    BrokerScope {
        broker_command: "run_data_admin_command",
        label: "data administration",
        allowlist: DATA_ADMIN_COMMANDS,
        sensitive: &[],
    },
    BrokerScope {
        broker_command: "run_external_connector_command",
        label: "external connector",
        allowlist: EXTERNAL_CONNECTOR_COMMANDS,
        sensitive: SENSITIVE_CONNECTOR_COMMANDS,
    },
    BrokerScope {
        broker_command: "run_filesystem_access_command",
        label: "filesystem access",
        allowlist: FILESYSTEM_ACCESS_COMMANDS,
        sensitive: SENSITIVE_FILESYSTEM_ACCESS_COMMANDS,
    },
    BrokerScope {
        broker_command: "run_filesystem_export_command",
        label: "filesystem export",
        allowlist: FILESYSTEM_EXPORT_COMMANDS,
        sensitive: &[],
    },
    BrokerScope {
        broker_command: "run_project_execution_command",
        label: "project execution",
        allowlist: PROJECT_EXECUTION_COMMANDS,
        sensitive: &[],
    },
];

impl BrokerScope {
    pub(crate) fn by_broker(broker_command: &str) -> Option<&'static BrokerScope> {
        SCOPES
            .iter()
            .find(|scope| scope.broker_command == broker_command)
    }

    /// The admission every `run_*` entry performs before dispatch: allowlist,
    /// then single-use token bound to (broker, command, args).
    pub(crate) fn admit(
        &self,
        tokens: &TokenStore,
        request: &PrivilegedCommandRequest,
    ) -> Result<(), String> {
        if !self.allowlist.contains(&request.command.as_str()) {
            return Err(format!("Unsupported {} command.", self.label));
        }
        tokens.consume(
            request.token.as_deref(),
            self.broker_command,
            &request.command,
            &request.args,
        )
    }
}

pub(super) fn broker_allowed_commands(broker_command: &str) -> Option<&'static [&'static str]> {
    BrokerScope::by_broker(broker_command).map(|scope| scope.allowlist)
}

pub(super) fn privileged_token_issue_requires_user_intent(
    broker_command: &str,
    command: &str,
) -> bool {
    BrokerScope::by_broker(broker_command).is_some_and(|scope| scope.sensitive.contains(&command))
}

pub(super) fn privileged_token_issue_scope_label(broker_command: &str) -> &'static str {
    BrokerScope::by_broker(broker_command)
        .map(|scope| scope.label)
        .unwrap_or("privileged")
}

pub(super) async fn confirm_sensitive_token_issue(
    app: AppHandle,
    broker_command: &str,
    command: &str,
    args: &Value,
) -> Result<(), String> {
    if !privileged_token_issue_requires_user_intent(broker_command, command) {
        return Ok(());
    }

    let message = match privileged_action_sentence(command, args) {
        Some(sentence) => format!(
            "{sentence}\n\nSiteCMD confirms this kind of action in a system dialog so nothing inside the app can trigger it without you. Approve only if you just requested it."
        ),
        None => {
            // Fallback for a command added to the sensitive lists without a
            // purpose-written sentence; the broker tests fail when that
            // happens, so users should never see this wording.
            let scope_label = privileged_token_issue_scope_label(broker_command);
            let argument_summary = privileged_action_argument_summary(args);
            format!(
                "SiteCMD is preparing a protected {scope_label} action.\n\nAction: {command}{argument_summary}\n\nApprove this only if you just requested it."
            )
        }
    };
    super::confirm_sensitive_action(app, "Allow Protected Action", message, "Allow")
        .await
        .map_err(String::from)
}

/// Build token-bound confirmation copy for every sensitive broker command.
pub(super) fn privileged_action_sentence(command: &str, args: &Value) -> Option<String> {
    let sentence = match command {
        "create_issue_link" => {
            let provider = sanitized_arg(args, "provider", "provider")
                .map(|value| match value.as_str() {
                    "github" => "GitHub".to_string(),
                    "jira" => "Jira".to_string(),
                    _ => value,
                })
                .unwrap_or_else(|| "an external issue tracker".to_string());
            match sanitized_arg(args, "checkId", "check_id") {
                Some(check_id) => {
                    format!("Send finding {check_id} and its fix details to {provider}?")
                }
                None => format!("Send this finding and its fix details to {provider}?"),
            }
        }
        "update_project_path" => match sanitized_arg(args, "path", "path") {
            Some(path) if !path.trim().is_empty() => format!(
                "Change this project's linked folder to {path}? SiteCMD will read code, logs, and git history from it."
            ),
            _ => "Unlink this project's folder so SiteCMD stops reading code from it?".to_string(),
        },
        "save_integration" => integration_save_sentence(args),
        "save_webhook_config" => match sanitized_arg(args, "url", "url") {
            Some(url) => format!("Save webhook settings that send scan results to {url}?"),
            None => "Save webhook settings that send scan results to an external URL?".to_string(),
        },
        "test_webhook" => {
            "Send a test webhook with sample scan data to your saved webhook URL?".to_string()
        }
        "sync_connected_site" => {
            "Send this project's current scan payload to the SiteCMD connected service?".to_string()
        }
        "import_connected_connection" => {
            "Import this encrypted site connection and store its installation token?".to_string()
        }
        "export_connected_connection" => {
            "Reveal an encrypted copy of this site's connection for transfer?".to_string()
        }
        "unlink_connected_site" => {
            "Remove this site's connection and fingerprint key from this desktop?".to_string()
        }
        "disconnect_connected_site" => {
            "Stop connected monitoring for this site and revoke its active delivery credentials?"
                .to_string()
        }
        "erase_connected_site" => {
            "Permanently erase all connected-service data for this site and unlink this desktop? This cannot be undone."
                .to_string()
        }
        "create_connected_alert_webhook" => match sanitized_arg(args, "url", "url") {
            Some(url) => format!("Send this site's alert notifications to {url}?"),
            None => "Send this site's alert notifications to an external webhook URL?".to_string(),
        },
        "test_connected_alert_webhook" => {
            "Send a signed test delivery to this site's saved alert webhook endpoint?".to_string()
        }
        "delete_connected_alert_webhook" => {
            "Delete this site's alert webhook endpoint and revoke its signing secret?".to_string()
        }
        "create_connected_destination" => match sanitized_arg(args, "address", "address") {
            Some(address) => format!("Email {address} to ask whether it wants SiteCMD alerts?"),
            None => "Email an address to ask whether it wants SiteCMD alerts?".to_string(),
        },
        "resend_connected_destination_verification" => {
            "Send another confirmation email to this alert address?".to_string()
        }
        "delete_connected_destination" => {
            "Remove this alert address from the connected account?".to_string()
        }
        "revoke_connected_site_credential" => {
            "Revoke this site's selected credential immediately?".to_string()
        }
        "revoke_connected_provider_connection" => {
            "Revoke this connected provider account and stop using its access?".to_string()
        }
        "revoke_connected_report" => {
            "Revoke this shareable report link so it stops opening immediately?".to_string()
        }
        "open_path_in_editor" => match sanitized_arg(args, "path", "path") {
            Some(path) => format!("Open {path} in your code editor?"),
            None => "Open a project file in your code editor?".to_string(),
        },
        "reveal_path" => match sanitized_arg(args, "path", "path") {
            Some(path) => format!("Show {path} in your file manager?"),
            None => "Show a project file in your file manager?".to_string(),
        },
        "register_agent_tool" => match sanitized_arg(args, "tool", "tool") {
            Some(tool) => format!(
                "Let SiteCMD launch {} to work on fixes?",
                crate::core::agent_tools::agent_tool_display_name(&tool)
            ),
            None => "Let SiteCMD launch a coding agent tool to work on fixes?".to_string(),
        },
        "unregister_agent_tool" => match sanitized_arg(args, "tool", "tool") {
            Some(tool) => format!(
                "Stop SiteCMD from launching {} for fixes?",
                crate::core::agent_tools::agent_tool_display_name(&tool)
            ),
            None => "Remove a coding agent tool from SiteCMD?".to_string(),
        },
        _ => return None,
    };
    Some(sentence)
}

/// Render one argument as dialog-safe text: control characters stripped,
/// length capped, `None` when absent or non-scalar.
fn sanitized_arg(args: &Value, camel_key: &str, snake_key: &str) -> Option<String> {
    let value = args.get(camel_key).or_else(|| args.get(snake_key))?;
    sanitized_value(value)
}

fn sanitized_nested_arg(
    args: &Value,
    parent_key: &str,
    camel_key: &str,
    snake_key: &str,
) -> Option<String> {
    let parent = args.get(parent_key)?.as_object()?;
    let value = parent.get(camel_key).or_else(|| parent.get(snake_key))?;
    sanitized_value(value)
}

fn sanitized_value(value: &Value) -> Option<String> {
    let raw = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => return None,
    };
    let safe = raw
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(240)
        .collect::<String>();
    let trimmed = safe.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn integration_save_sentence(args: &Value) -> String {
    let integration_type =
        sanitized_nested_arg(args, "config", "integrationType", "integration_type");
    let display_name = integration_type
        .as_deref()
        .and_then(|value| value.parse::<crate::integrations::IntegrationType>().ok())
        .map(|value| value.display_name().to_string())
        .unwrap_or_else(|| "integration".to_string());
    let destination = sanitized_nested_arg(args, "config", "siteId", "site_id").or_else(|| {
        let config = args.get("config")?.as_object()?;
        let extra = config.get("extra")?.as_object()?;
        sanitized_value(extra.get("instance_url")?)
    });

    match destination {
        Some(destination) => {
            format!("Save {display_name} credentials and settings for {destination}?")
        }
        None => format!("Save {display_name} credentials and connection settings?"),
    }
}

pub(super) fn privileged_action_argument_summary(args: &Value) -> String {
    const SAFE_ARGUMENTS: &[(&str, &str, &str)] = &[
        ("URL", "url", "url"),
        ("Path", "path", "path"),
        ("Project folder", "projectPath", "project_path"),
        ("Source", "srcPath", "src_path"),
        ("Destination", "destPath", "dest_path"),
        ("Command", "command", "command"),
        ("Agent tool", "tool", "tool"),
        ("Project ID", "projectId", "project_id"),
        ("Environment ID", "environmentId", "environment_id"),
        ("Scan ID", "scanId", "scan_id"),
        ("Site ID", "siteId", "site_id"),
        ("Event ID", "eventId", "event_id"),
        ("Record ID", "id", "id"),
    ];

    let mut details = Vec::new();
    for (label, camel_key, snake_key) in SAFE_ARGUMENTS {
        let Some(value) = args.get(camel_key).or_else(|| args.get(snake_key)) else {
            continue;
        };
        let raw = match value {
            Value::String(value) => value.clone(),
            Value::Number(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            _ => continue,
        };
        let safe = raw
            .chars()
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .take(240)
            .collect::<String>();
        if !safe.trim().is_empty() {
            details.push(format!("{label}: {safe}"));
        }
    }

    if details.is_empty() {
        String::new()
    } else {
        format!("\n\nDetails:\n{}", details.join("\n"))
    }
}

pub(super) async fn issue_scoped_privileged_command_token(
    _app: AppHandle,
    window: Window,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandTokenRequest,
    broker_command: &'static str,
) -> Result<String, String> {
    ensure_main_token_issuer_window(&window)?;
    if privileged_token_issue_requires_user_intent(broker_command, &request.command) {
        return Err("This privileged command requires a native user-intent token.".to_string());
    }
    token_state.issue(broker_command, &request.command, &request.args)
}

pub(super) fn emit_privileged_command_response(
    app: &AppHandle,
    response_event: Option<&str>,
    outcome: &Result<Value, String>,
) {
    let Some(event) = response_event else {
        return;
    };
    if !event.starts_with("sitecmd://privileged-command-response/") {
        tracing::warn!(
            response_event = %event,
            "Ignoring invalid privileged command native response event"
        );
        return;
    }

    let payload = match outcome {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    super::emit_event(app, event, payload);
}

pub(super) fn ensure_main_token_issuer_window(window: &Window) -> Result<(), String> {
    if window.label() == "main" {
        return Ok(());
    }

    Err("Privileged command tokens can only be issued from the main window.".to_string())
}

#[tauri::command]
#[tracing::instrument(skip(app, window, token_state, request), fields(command = %request.command))]
pub async fn issue_sensitive_privileged_command_token(
    app: AppHandle,
    window: Window,
    token_state: State<'_, PrivilegedCommandTokenState>,
    request: PrivilegedCommandTokenRequest,
) -> Result<String, String> {
    ensure_main_token_issuer_window(&window)?;
    let broker_command = request
        .broker_command
        .as_deref()
        .ok_or_else(|| "Missing sensitive privileged command broker.".to_string())?;
    if !privileged_token_issue_requires_user_intent(broker_command, &request.command) {
        return Err("This command does not require a sensitive token issuer.".to_string());
    }
    confirm_sensitive_token_issue(app, broker_command, &request.command, &request.args).await?;
    token_state.issue(broker_command, &request.command, &request.args)
}

// Argument helpers shared across every broker submodule.

pub(super) fn arg_value<'a>(
    args: &'a Value,
    camel_key: &str,
    snake_key: &str,
) -> Result<&'a Value, String> {
    args.get(camel_key)
        .or_else(|| args.get(snake_key))
        .ok_or_else(|| format!("Missing required argument: {}", camel_key))
}

pub(super) fn arg_i64(args: &Value, camel_key: &str, snake_key: &str) -> Result<i64, String> {
    arg_value(args, camel_key, snake_key)?
        .as_i64()
        .ok_or_else(|| format!("Argument {} must be a number.", camel_key))
}

pub(super) fn arg_string(args: &Value, camel_key: &str, snake_key: &str) -> Result<String, String> {
    arg_value(args, camel_key, snake_key)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("Argument {} must be a string.", camel_key))
}

pub(super) fn arg_bytes(args: &Value, camel_key: &str, snake_key: &str) -> Result<Vec<u8>, String> {
    serde_json::from_value(arg_value(args, camel_key, snake_key)?.clone())
        .map_err(|_| format!("Argument {} must be a byte array.", camel_key))
}

pub(super) fn arg_bool(args: &Value, camel_key: &str, snake_key: &str) -> Result<bool, String> {
    arg_value(args, camel_key, snake_key)?
        .as_bool()
        .ok_or_else(|| format!("Argument {} must be a boolean.", camel_key))
}

pub(super) fn arg_optional_bool(
    args: &Value,
    camel_key: &str,
    snake_key: &str,
) -> Result<Option<bool>, String> {
    match args.get(camel_key).or_else(|| args.get(snake_key)) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("Argument {} must be a boolean.", camel_key)),
    }
}

pub(super) fn arg_optional_i64(
    args: &Value,
    camel_key: &str,
    snake_key: &str,
) -> Result<Option<i64>, String> {
    match args.get(camel_key).or_else(|| args.get(snake_key)) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_i64()
            .map(Some)
            .ok_or_else(|| format!("Argument {} must be a number.", camel_key)),
    }
}

pub(super) fn arg_optional_string(
    args: &Value,
    camel_key: &str,
    snake_key: &str,
) -> Result<Option<String>, String> {
    match args.get(camel_key).or_else(|| args.get(snake_key)) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_string()))
            .ok_or_else(|| format!("Argument {} must be a string.", camel_key)),
    }
}

pub(super) fn arg_optional_u32(
    args: &Value,
    camel_key: &str,
    snake_key: &str,
) -> Result<Option<u32>, String> {
    match args.get(camel_key).or_else(|| args.get(snake_key)) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| format!("Argument {} must be a number.", camel_key)),
    }
}

pub(super) fn arg_optional_usize(
    args: &Value,
    camel_key: &str,
    snake_key: &str,
) -> Result<Option<usize>, String> {
    match args.get(camel_key).or_else(|| args.get(snake_key)) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| format!("Argument {} must be a number.", camel_key)),
    }
}

pub(super) fn arg_from_value<T: DeserializeOwned>(
    args: &Value,
    camel_key: &str,
    snake_key: &str,
) -> Result<T, String> {
    serde_json::from_value(arg_value(args, camel_key, snake_key)?.clone())
        .map_err(|_| format!("Argument {} has an invalid shape.", camel_key))
}

pub(super) fn json_response<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("Could not serialize response: {error}"))
}
