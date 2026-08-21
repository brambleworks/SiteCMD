import { invoke as rawInvoke } from "@tauri-apps/api/core";
import {
  invokeThroughPrivilegedBridge,
  shouldUsePrivilegedWindowBroker,
} from "@/lib/privileged-command-bridge";
import { trackDiagnosticEvent } from "@/lib/telemetry";
import { errorMessage } from "@/lib/error-message";

const RETRIABLE_READ_MAX_ATTEMPTS = 2;
const RETRIABLE_READ_RETRY_DELAY_MS = 150;
const MAX_CONCURRENT_READ_INVOCATIONS = 4;
const inFlightReadInvokes = new Map<string, Promise<unknown>>();
const queuedReadInvocations: Array<() => void> = [];
let activeReadInvocations = 0;
const TAURI_INVOKE_TEST_GUARD_FLAG = "__SITECMD_ENABLE_INVOKE_GUARDS_IN_TESTS__";
const TAURI_PRIVILEGED_BROKER_TEST_FLAG = "__SITECMD_ENABLE_PRIVILEGED_BROKER_IN_TESTS__";
const PRIVILEGED_BROKER_COMMANDS: ReadonlyMap<string, string> = new Map([
  ["activate_connected_service", "run_external_connector_command"],
  ["activate_license", "run_external_connector_command"],
  ["check_app_update", "run_external_connector_command"],
  ["clear_scan_history", "run_data_admin_command"],
  ["complete_github_oauth", "run_external_connector_command"],
  ["complete_google_oauth", "run_external_connector_command"],
  ["connect_github", "run_external_connector_command"],
  ["connect_google", "run_external_connector_command"],
  ["create_connected_alert_webhook", "run_external_connector_command"],
  ["create_connected_destination", "run_external_connector_command"],
  ["create_connected_provider_connection", "run_external_connector_command"],
  ["create_connected_report", "run_external_connector_command"],
  ["create_connected_site", "run_external_connector_command"],
  ["create_issue_link", "run_external_connector_command"],
  ["deactivate_license", "run_data_admin_command"],
  ["decide_site_baseline", "run_external_connector_command"],
  ["delete_connected_alert_webhook", "run_external_connector_command"],
  ["delete_connected_destination", "run_external_connector_command"],
  ["delete_environment", "run_data_admin_command"],
  ["delete_event", "run_data_admin_command"],
  ["delete_integration", "run_external_connector_command"],
  ["delete_project", "run_data_admin_command"],
  ["delete_report_history", "run_data_admin_command"],
  ["delete_scan", "run_data_admin_command"],
  ["delete_site_scans", "run_data_admin_command"],
  ["delete_webhook_config", "run_data_admin_command"],
  ["detect_project_urls", "run_filesystem_access_command"],
  ["detect_updates", "run_external_connector_command"],
  ["disconnect_connected_site", "run_external_connector_command"],
  ["download_and_install_app_update", "run_external_connector_command"],
  ["erase_connected_site", "run_external_connector_command"],
  ["export_connected_connection", "run_external_connector_command"],
  ["export_database", "run_filesystem_export_command"],
  ["fetch_analytics", "run_external_connector_command"],
  ["fetch_connected_site_state", "run_external_connector_command"],
  ["fetch_github_data", "run_external_connector_command"],
  ["fetch_integration_data", "run_external_connector_command"],
  ["get_commits_since", "run_filesystem_access_command"],
  ["get_connected_notification_settings", "run_external_connector_command"],
  ["get_site_baseline", "run_external_connector_command"],
  ["get_db_path", "run_filesystem_access_command"],
  ["get_git_status", "run_filesystem_access_command"],
  ["get_log_path", "run_filesystem_access_command"],
  ["get_pagespeed_report", "run_external_connector_command"],
  ["github_latest_release", "run_external_connector_command"],
  ["import_connected_connection", "run_external_connector_command"],
  ["import_database", "run_data_admin_command"],
  ["inspect_desktop_watch_files", "run_filesystem_access_command"],
  ["invalidate_analytics_cache", "run_external_connector_command"],
  ["launch_agent_handoff", "run_filesystem_access_command"],
  ["list_connected_alert_webhooks", "run_external_connector_command"],
  ["list_connected_alerts", "run_external_connector_command"],
  ["list_connected_destinations", "run_external_connector_command"],
  ["list_connected_reports", "run_external_connector_command"],
  ["list_connected_provider_connections", "run_external_connector_command"],
  ["list_connected_provider_projects", "run_external_connector_command"],
  ["list_connected_site_credentials", "run_external_connector_command"],
  ["mint_connected_ci_token", "run_external_connector_command"],
  ["mint_connected_webhook_secret", "run_external_connector_command"],
  ["reconnect_connected_site", "run_external_connector_command"],
  ["revoke_connected_site_credential", "run_external_connector_command"],
  ["rotate_connected_site_credential", "run_external_connector_command"],
  ["open_path_in_editor", "run_filesystem_access_command"],
  ["open_external_url", "run_external_connector_command"],
  ["pagespeed_api_key_is_set", "run_external_connector_command"],
  ["put_connected_notification_settings", "run_external_connector_command"],
  ["read_recent_logs", "run_filesystem_access_command"],
  ["register_agent_tool", "run_filesystem_access_command"],
  ["resend_connected_destination_verification", "run_external_connector_command"],
  ["resolve_fix_locations_for_check", "run_filesystem_access_command"],
  ["resolve_project_files", "run_filesystem_access_command"],
  ["reveal_path", "run_filesystem_access_command"],
  ["revoke_connected_provider_connection", "run_external_connector_command"],
  ["rotate_connected_fingerprint_key", "run_external_connector_command"],
  ["abort_connected_key_rotation", "run_external_connector_command"],
  ["request_account_recovery", "run_external_connector_command"],
  ["get_account_recovery", "run_external_connector_command"],
  ["acknowledge_account_recovery", "run_external_connector_command"],
  ["cancel_account_recovery", "run_external_connector_command"],
  ["revoke_connected_report", "run_external_connector_command"],
  ["rotate_connected_alert_webhook", "run_external_connector_command"],
  ["run_scan_execution", "run_filesystem_access_command"],
  ["run_code_scan_audit", "run_filesystem_access_command"],
  ["run_project_command", "run_project_execution_command"],
  ["save_github_integration", "run_external_connector_command"],
  ["save_google_integration", "run_external_connector_command"],
  ["save_integration", "run_external_connector_command"],
  ["save_webhook_config", "run_external_connector_command"],
  ["set_pagespeed_api_key", "run_external_connector_command"],
  ["sync_connected_site", "run_external_connector_command"],
  ["sync_connected_scan_scope", "run_external_connector_command"],
  ["test_connected_alert_webhook", "run_external_connector_command"],
  ["test_webhook", "run_external_connector_command"],
  ["unlink_connected_site", "run_external_connector_command"],
  ["unregister_agent_tool", "run_filesystem_access_command"],
  ["update_connected_destination_policy", "run_external_connector_command"],
  ["update_project_path", "run_filesystem_access_command"],
  ["validate_license", "run_external_connector_command"],
  ["verify_connected_site", "run_external_connector_command"],
  ["verify_connected_site_provider", "run_external_connector_command"],
  ["write_export_bytes", "run_filesystem_export_command"],
  ["write_export_file", "run_filesystem_export_command"],
] as const);

function sleep(ms: number) {
  return new Promise((resolve) => {
    globalThis.setTimeout(resolve, ms);
  });
}

function callRawInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
  options?: Parameters<typeof rawInvoke>[2],
) {
  if (typeof options === "undefined") {
    return rawInvoke<T>(cmd, args);
  }
  return rawInvoke<T>(cmd, args, options);
}

function isRetriableReadCommand(cmd: string) {
  return (
    cmd.startsWith("get_") ||
    cmd.startsWith("count_") ||
    cmd.startsWith("list_") ||
    cmd.startsWith("load_") ||
    cmd.startsWith("peek_") ||
    cmd.startsWith("inspect_")
  );
}

function isRetriableInvokeError(error: unknown) {
  const message = errorMessage(error);
  return (
    message.includes("Load failed") ||
    message.includes("postMessage") ||
    message.includes("ipc") ||
    message.includes("Failed to fetch")
  );
}

function buildReadInvokeKey(
  cmd: string,
  args?: Record<string, unknown>,
  options?: Parameters<typeof rawInvoke>[2],
) {
  return JSON.stringify([cmd, args ?? null, options ?? null]);
}

function shouldGuardReadInvocations() {
  const testFlags = globalThis as typeof globalThis & Record<string, unknown>;
  if (import.meta.env.MODE !== "test") {
    return true;
  }
  return testFlags[TAURI_INVOKE_TEST_GUARD_FLAG] === true;
}

function privilegedBrokerCommandFor(cmd: string): string | null {
  if (import.meta.env.MODE === "test") {
    const testFlags = globalThis as typeof globalThis & Record<string, unknown>;
    if (testFlags[TAURI_PRIVILEGED_BROKER_TEST_FLAG] !== true) return null;
  }
  return PRIVILEGED_BROKER_COMMANDS.get(cmd) ?? null;
}

function resetInvokeGuards() {
  inFlightReadInvokes.clear();
  queuedReadInvocations.length = 0;
  activeReadInvocations = 0;
}

export function resetTauriInvokeTestState() {
  const testFlags = globalThis as typeof globalThis & Record<string, unknown>;
  if (import.meta.env.MODE !== "test") {
    return;
  }
  resetInvokeGuards();
  delete testFlags[TAURI_INVOKE_TEST_GUARD_FLAG];
  delete testFlags[TAURI_PRIVILEGED_BROKER_TEST_FLAG];
}

export function setTauriInvokeGuardsForTests(enabled: boolean) {
  const testFlags = globalThis as typeof globalThis & Record<string, unknown>;
  if (import.meta.env.MODE !== "test") {
    return;
  }
  resetInvokeGuards();
  testFlags[TAURI_INVOKE_TEST_GUARD_FLAG] = enabled;
}

export function setTauriPrivilegedBrokerForTests(enabled: boolean) {
  const testFlags = globalThis as typeof globalThis & Record<string, unknown>;
  if (import.meta.env.MODE !== "test") {
    return;
  }
  testFlags[TAURI_PRIVILEGED_BROKER_TEST_FLAG] = enabled;
}

function pumpReadInvokeQueue() {
  while (
    activeReadInvocations < MAX_CONCURRENT_READ_INVOCATIONS &&
    queuedReadInvocations.length > 0
  ) {
    const next = queuedReadInvocations.shift();
    if (!next) return;
    activeReadInvocations += 1;
    next();
  }
}

function scheduleReadInvoke<T>(task: () => Promise<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    queuedReadInvocations.push(() => {
      task()
        .then(resolve, reject)
        .finally(() => {
          activeReadInvocations = Math.max(0, activeReadInvocations - 1);
          pumpReadInvokeQueue();
        });
    });
    pumpReadInvokeQueue();
  });
}

export async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
  options?: Parameters<typeof rawInvoke>[2],
): Promise<T> {
  try {
    const brokerCommand = privilegedBrokerCommandFor(cmd);
    if (brokerCommand) {
      if (shouldUsePrivilegedWindowBroker(brokerCommand)) {
        return await invokeThroughPrivilegedBridge<T>(brokerCommand, cmd, args);
      }

      return await callRawInvoke<T>(
        brokerCommand,
        { request: { command: cmd, args: args ?? {} } },
        options,
      );
    }

    if (!isRetriableReadCommand(cmd) || !shouldGuardReadInvocations()) {
      return await callRawInvoke<T>(cmd, args, options);
    }

    const requestKey = buildReadInvokeKey(cmd, args, options);
    const existing = inFlightReadInvokes.get(requestKey);
    if (existing) {
      return (await existing) as T;
    }

    const request = scheduleReadInvoke(async () => {
      let lastError: unknown = null;

      for (let attempt = 1; attempt <= RETRIABLE_READ_MAX_ATTEMPTS; attempt += 1) {
        try {
          return await callRawInvoke<T>(cmd, args, options);
        } catch (error) {
          lastError = error;
          if (!isRetriableInvokeError(error) || attempt >= RETRIABLE_READ_MAX_ATTEMPTS) {
            throw error;
          }
          await sleep(RETRIABLE_READ_RETRY_DELAY_MS * attempt);
        }
      }

      throw lastError ?? new Error(`Tauri invoke failed for "${cmd}"`);
    });

    inFlightReadInvokes.set(requestKey, request);
    void request.finally(() => {
      if (inFlightReadInvokes.get(requestKey) === request) {
        inFlightReadInvokes.delete(requestKey);
      }
    });

    return await request;
  } catch (error) {
    trackDiagnosticEvent("tauri_command_failed", error, {
      command: cmd,
      brokered: Boolean(privilegedBrokerCommandFor(cmd)),
    });
    throw error;
  }
}
