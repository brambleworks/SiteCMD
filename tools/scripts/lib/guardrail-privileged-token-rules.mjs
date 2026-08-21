const FAILURE =
  "Privileged bridge requests must carry argument-bound tokens, per-family scoped issuers must stay mounted with in-handler native confirmations covering every destructive command, sensitive commands must use native user intent, and token issuers must reject non-main windows.";

// Scoped issuers rely on in-handler native confirmation; the shared sensitive
// issuer is reserved for commands without their own confirmation.
export function privilegedTokenIssuerFailures(read) {
  const bridge = read("apps/desktop/src/lib/privileged-command-bridge.ts");
  const invokeTests = read("apps/desktop/src/lib/tauri-invoke.test.ts");
  const desktopLib = read("apps/desktop/src-tauri/src/lib.rs");
  const permissions = read("apps/desktop/src-tauri/permissions/default.toml");
  const securityManifest = read("apps/desktop/src-tauri/permissions/command-security.json");
  const brokerFiles = [
    "mod.rs",
    "data_admin.rs",
    "external_connectors.rs",
    "filesystem_access.rs",
    "filesystem_export.rs",
    "project_execution.rs",
    "token_state.rs",
    "tests.rs",
  ];
  const broker = brokerFiles
    .map((file) => read(`apps/desktop/src-tauri/src/commands/privileged_command_broker/${file}`))
    .join("\n");
  const scopedIssuers = [
    "issue_data_admin_command_token",
    "issue_external_connector_command_token",
    "issue_filesystem_access_command_token",
    "issue_filesystem_export_command_token",
    "issue_project_execution_command_token",
  ];
  const scopedIssuerRoutes = [
    '["run_data_admin_command", "issue_data_admin_command_token"]',
    '["run_external_connector_command", "issue_external_connector_command_token"]',
    '["run_filesystem_access_command", "issue_filesystem_access_command_token"]',
    '["run_filesystem_export_command", "issue_filesystem_export_command_token"]',
    '["run_project_execution_command", "issue_project_execution_command_token"]',
  ];
  const valid =
    scopedIssuers.every(
      (issuer) =>
        broker.includes(`pub async fn ${issuer}`) &&
        desktopLib.includes(`commands::${issuer}`) &&
        permissions.includes(`allow-${issuer.replaceAll("_", "-")}`),
    ) &&
    scopedIssuerRoutes.every((route) => bridge.includes(route)) &&
    permissions.includes("allow-issue-sensitive-privileged-command-token") &&
    bridge.includes('"issue_sensitive_privileged_command_token"') &&
    // Destructive families confirm in their handlers, not during token issuance.
    !bridge.includes("NATIVE_INTENT_BROKERS") &&
    !broker.includes("SENSITIVE_TOKEN_BROKERS") &&
    bridge.includes("NATIVE_INTENT_CONNECTOR_COMMANDS") &&
    bridge.includes("NATIVE_INTENT_FILESYSTEM_COMMANDS") &&
    securityManifest.includes('"nativeIntentBrokerCommands"') &&
    // Keep this aligned with Rust SENSITIVE_FILESYSTEM_ACCESS_COMMANDS so
    // user-intent commands reach the correct token issuer.
    bridge.includes('"register_agent_tool"') &&
    bridge.includes('"unregister_agent_tool"') &&
    broker.includes("pub async fn issue_sensitive_privileged_command_token") &&
    desktopLib.includes("commands::issue_sensitive_privileged_command_token") &&
    !permissions.includes("allow-issue-privileged-command-token") &&
    !bridge.includes('"issue_privileged_command_token"') &&
    !broker.includes("pub async fn issue_privileged_command_token") &&
    bridge.includes("token: request.token") &&
    bridge.includes('typeof value.token !== "string"') &&
    broker.includes("fn consume(") &&
    [
      "data_admin.rs",
      "external_connectors.rs",
      "filesystem_access.rs",
      "filesystem_export.rs",
      "project_execution.rs",
    ].every((file) =>
      read(`apps/desktop/src-tauri/src/commands/privileged_command_broker/${file}`).includes(
        "token_state.consume(",
      ),
    ) &&
    broker.includes("ensure_main_token_issuer_window") &&
    broker.includes('window.label() == "main"') &&
    broker.includes("Privileged command tokens can only be issued from the main window") &&
    broker.includes("fn privileged_token_issue_requires_user_intent") &&
    broker.includes("SENSITIVE_CONNECTOR_COMMANDS.contains(&command)") &&
    broker.includes("SENSITIVE_FILESYSTEM_ACCESS_COMMANDS.contains(&command)") &&
    broker.includes("async fn confirm_sensitive_token_issue") &&
    broker.includes("super::confirm_sensitive_action") &&
    broker.includes("confirm_sensitive_token_issue(app, broker_command") &&
    broker.includes("sensitive_privileged_token_issuance_requires_user_intent") &&
    broker.includes("every_token_issue_prompted_command_has_purpose_written_copy") &&
    invokeTests.includes("ignore direct command events without a native-issued token") &&
    invokeTests.includes("pass native-issued tokens through to scoped brokers") &&
    invokeTests.includes(
      "issues destructive, export, and execution tokens through scoped issuers, never the sensitive one",
    );

  return valid ? [] : [FAILURE];
}
