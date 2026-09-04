import { invoke as rawInvoke } from "@tauri-apps/api/core";
import { emitTo, listen, once, type UnlistenFn } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { errorMessage } from "@/lib/error-message";

type PrivilegedBridgeScope =
  | "data-admin"
  | "external-connectors"
  | "filesystem-access"
  | "filesystem-export"
  | "project-execution";

type PrivilegedBrokerCommand =
  | "run_data_admin_command"
  | "run_external_connector_command"
  | "run_filesystem_access_command"
  | "run_filesystem_export_command"
  | "run_project_execution_command";

type PrivilegedTokenIssuerCommand =
  | "issue_data_admin_command_token"
  | "issue_external_connector_command_token"
  | "issue_filesystem_access_command_token"
  | "issue_filesystem_export_command_token"
  | "issue_project_execution_command_token"
  | "issue_sensitive_privileged_command_token";

interface PrivilegedBridgeRequest {
  id: string;
  scope: PrivilegedBridgeScope;
  command: string;
  args: Record<string, unknown>;
  token: string;
  nativeResponseEvent?: string;
}

interface PrivilegedBridgeResponse {
  ok: boolean;
  value?: unknown;
  error?: string;
}

interface PrivilegedBridgePing {
  id: string;
  // Bridge listeners receive broadcasts and must filter by scope.
  target: PrivilegedBridgeScope;
}

interface PrivilegedBridgePong {
  scope: PrivilegedBridgeScope;
}

interface PrivilegedCommandTokenRequest {
  command: string;
  args: Record<string, unknown>;
  broker_command?: PrivilegedBrokerCommand;
}

const PRIVILEGED_BRIDGE_QUERY_KEY = "sitecmd_privileged_bridge";
const PRIVILEGED_BRIDGE_EVENT = "sitecmd://privileged-command";
const PRIVILEGED_BRIDGE_PING_EVENT = "sitecmd://privileged-ping";
const PRIVILEGED_BRIDGE_READY_EVENT = "sitecmd://privileged-ready";
const PRIVILEGED_BRIDGE_DEFAULT_TIMEOUT_MS = 15_000;
const PRIVILEGED_BRIDGE_STARTUP_TIMEOUT_MS = 3_000;
const PRIVILEGED_BRIDGE_PING_TIMEOUT_MS = 350;

// Native confirmation commands need human-scale headroom. A test keeps this
// list aligned with Rust's dialog registry.
const HUMAN_CONFIRMATION_TIMEOUT_MS = 3 * 60_000;

const PRIVILEGED_BRIDGE_COMMAND_TIMEOUTS_MS: Record<string, number> = {
  activate_license: HUMAN_CONFIRMATION_TIMEOUT_MS,
  clear_scan_history: HUMAN_CONFIRMATION_TIMEOUT_MS,
  deactivate_license: HUMAN_CONFIRMATION_TIMEOUT_MS,
  delete_environment: HUMAN_CONFIRMATION_TIMEOUT_MS,
  delete_event: HUMAN_CONFIRMATION_TIMEOUT_MS,
  delete_integration: HUMAN_CONFIRMATION_TIMEOUT_MS,
  delete_project: HUMAN_CONFIRMATION_TIMEOUT_MS,
  delete_report_history: HUMAN_CONFIRMATION_TIMEOUT_MS,
  delete_scan: HUMAN_CONFIRMATION_TIMEOUT_MS,
  delete_site_scans: HUMAN_CONFIRMATION_TIMEOUT_MS,
  delete_webhook_config: HUMAN_CONFIRMATION_TIMEOUT_MS,
  export_database: HUMAN_CONFIRMATION_TIMEOUT_MS,
  import_database: HUMAN_CONFIRMATION_TIMEOUT_MS,
  run_project_command: HUMAN_CONFIRMATION_TIMEOUT_MS,
  write_export_bytes: HUMAN_CONFIRMATION_TIMEOUT_MS,
  write_export_file: HUMAN_CONFIRMATION_TIMEOUT_MS,
  complete_github_oauth: 3 * 60_000,
  complete_google_oauth: 3 * 60_000,
  run_scan_execution: 10 * 60_000,
  run_code_scan_audit: 10 * 60_000,
  detect_project_urls: 60_000,
  inspect_desktop_watch_files: 60_000,
  resolve_project_files: 60_000,
  resolve_fix_locations_for_check: 60_000,
  // Allow provider HTTP deadlines plus bridge overhead.
  fetch_integration_data: 45_000,
  fetch_analytics: 45_000,
  fetch_github_data: 45_000,
  // Covers two validation passes, database work, keychain prompts, and HTTP.
  validate_license: 13 * 60_000,
  // Outlast the native 60-second PageSpeed deadline.
  get_pagespeed_report: 90_000,
  // Update downloads stream without a native deadline.
  download_and_install_app_update: 30 * 60_000,
  sync_connected_site: 3 * 60_000,
  sync_connected_scan_scope: 3 * 60_000,
  // Covers the serialized database, keychain, label, and issue-creation chain.
  create_issue_link: 14 * 60_000,
  // Includes the license read before the device-code request.
  connect_github: 90_000,
  // Includes the project lookup and five bounded git processes.
  get_git_status: 2 * 60_000,
  // Dependency registry lookups run in bounded waves across ecosystems.
  detect_updates: 90_000,
};

const NATIVE_RESPONSE_COMMANDS = new Set([
  "complete_github_oauth",
  "complete_google_oauth",
  "detect_updates",
  "run_scan_execution",
]);

export function usesNativeResponseEvent(command: string): boolean {
  return NATIVE_RESPONSE_COMMANDS.has(command);
}

export function resolveCommandTimeoutMs(command: string): number {
  return PRIVILEGED_BRIDGE_COMMAND_TIMEOUTS_MS[command] ?? PRIVILEGED_BRIDGE_DEFAULT_TIMEOUT_MS;
}

function createPrivilegedCommandTimeoutError(
  scope: PrivilegedBridgeScope,
  command: string,
  timeoutMs: number,
) {
  return Object.assign(
    new Error(
      "That action took too long to finish. Click the same button again; if it keeps happening, restart SiteCMD.",
    ),
    {
      command,
      scope,
      timeoutMs,
    },
  );
}

const PRIVILEGED_WINDOW_BROKER_COMMANDS: ReadonlyMap<
  PrivilegedBrokerCommand,
  PrivilegedBridgeScope
> = new Map([
  ["run_data_admin_command", "data-admin"],
  ["run_external_connector_command", "external-connectors"],
  ["run_filesystem_access_command", "filesystem-access"],
  ["run_filesystem_export_command", "filesystem-export"],
  ["run_project_execution_command", "project-execution"],
]);

// Commands with handler-owned confirmations use scoped issuers to avoid double prompts.
const PRIVILEGED_TOKEN_COMMAND_BY_BROKER: ReadonlyMap<
  PrivilegedBrokerCommand,
  PrivilegedTokenIssuerCommand
> = new Map([
  ["run_data_admin_command", "issue_data_admin_command_token"],
  ["run_external_connector_command", "issue_external_connector_command_token"],
  ["run_filesystem_access_command", "issue_filesystem_access_command_token"],
  ["run_filesystem_export_command", "issue_filesystem_export_command_token"],
  ["run_project_execution_command", "issue_project_execution_command_token"],
]);

// Keep caller-chosen credential and data destinations aligned with Rust's
// SENSITIVE_CONNECTOR_COMMANDS.
export const NATIVE_INTENT_CONNECTOR_COMMANDS: ReadonlySet<string> = new Set([
  "create_connected_alert_webhook",
  "create_connected_destination",
  "delete_connected_alert_webhook",
  "delete_connected_destination",
  "disconnect_connected_site",
  "erase_connected_site",
  "export_connected_connection",
  "import_connected_connection",
  "resend_connected_destination_verification",
  "revoke_connected_provider_connection",
  "revoke_connected_report",
  "revoke_connected_site_credential",
  "save_integration",
  "save_webhook_config",
  "sync_connected_site",
  "test_connected_alert_webhook",
  "test_webhook",
  "unlink_connected_site",
]);

// Keep aligned with Rust's SENSITIVE_FILESYSTEM_ACCESS_COMMANDS.
export const NATIVE_INTENT_FILESYSTEM_COMMANDS: ReadonlySet<string> = new Set([
  "update_project_path",
  "open_path_in_editor",
  "reveal_path",
  "register_agent_tool",
  "unregister_agent_tool",
  // launch_agent_handoff stays off this list on purpose: it only opens the
  // agent's app with a prompt staged in its composer.
]);

const BROKER_COMMAND_BY_SCOPE: Record<PrivilegedBridgeScope, PrivilegedBrokerCommand> = {
  "data-admin": "run_data_admin_command",
  "external-connectors": "run_external_connector_command",
  "filesystem-access": "run_filesystem_access_command",
  "filesystem-export": "run_filesystem_export_command",
  "project-execution": "run_project_execution_command",
};

const bridgeStartupPromises = new Map<PrivilegedBridgeScope, Promise<void>>();
let requestSeq = 0;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isPrivilegedBridgeScope(value: unknown): value is PrivilegedBridgeScope {
  return (
    value === "data-admin" ||
    value === "external-connectors" ||
    value === "filesystem-access" ||
    value === "filesystem-export" ||
    value === "project-execution"
  );
}

function privilegedCommandEventName(scope: PrivilegedBridgeScope) {
  return `${PRIVILEGED_BRIDGE_EVENT}/${scope}`;
}

function responseEventName(requestId: string) {
  return `sitecmd://privileged-command-response/${requestId}`;
}

function pongEventName(requestId: string) {
  return `sitecmd://privileged-pong/${requestId}`;
}

function readBridgeScopeFromLocation(): PrivilegedBridgeScope | null {
  if (typeof window === "undefined") return null;
  const scope = new URLSearchParams(window.location.search).get(PRIVILEGED_BRIDGE_QUERY_KEY);
  return isPrivilegedBridgeScope(scope) ? scope : null;
}

function parseBridgeRequest(value: unknown): PrivilegedBridgeRequest | null {
  if (!isRecord(value)) return null;
  if (
    typeof value.id !== "string" ||
    !isPrivilegedBridgeScope(value.scope) ||
    typeof value.command !== "string" ||
    typeof value.token !== "string"
  ) {
    return null;
  }
  return {
    id: value.id,
    scope: value.scope,
    command: value.command,
    args: isRecord(value.args) ? value.args : {},
    token: value.token,
    nativeResponseEvent:
      typeof value.nativeResponseEvent === "string" ? value.nativeResponseEvent : undefined,
  };
}

async function issuePrivilegedCommandToken(
  brokerCommand: PrivilegedBrokerCommand,
  command: string,
  args: Record<string, unknown>,
): Promise<string> {
  const requiresNativeIntent =
    (brokerCommand === "run_external_connector_command" &&
      NATIVE_INTENT_CONNECTOR_COMMANDS.has(command)) ||
    (brokerCommand === "run_filesystem_access_command" &&
      NATIVE_INTENT_FILESYSTEM_COMMANDS.has(command));
  const issuerCommand = requiresNativeIntent
    ? "issue_sensitive_privileged_command_token"
    : PRIVILEGED_TOKEN_COMMAND_BY_BROKER.get(brokerCommand);
  if (!issuerCommand) {
    throw new Error(`Unsupported privileged token issuer: ${brokerCommand}`);
  }

  const token = await rawInvoke<unknown>(issuerCommand, {
    request: {
      command,
      args,
      ...(requiresNativeIntent ? { broker_command: brokerCommand } : {}),
    } satisfies PrivilegedCommandTokenRequest,
  });
  if (typeof token !== "string" || token.length === 0) {
    throw new Error("Privileged command token response was invalid.");
  }
  return token;
}

async function ensurePrivilegedBridge(scope: PrivilegedBridgeScope): Promise<void> {
  const existingStartup = bridgeStartupPromises.get(scope);
  if (existingStartup) return existingStartup;

  const startup = (async () => {
    const existing = await WebviewWindow.getByLabel(scope);
    if (!existing) throw new Error(`Privileged ${scope} bridge window is not available.`);
    await waitForPrivilegedBridge(scope);
  })();

  bridgeStartupPromises.set(scope, startup);
  try {
    await startup;
  } catch (error) {
    bridgeStartupPromises.delete(scope);
    throw error;
  }
}

async function waitForPrivilegedBridge(scope: PrivilegedBridgeScope): Promise<void> {
  const deadline = Date.now() + PRIVILEGED_BRIDGE_STARTUP_TIMEOUT_MS;
  let lastError: unknown = null;

  while (Date.now() < deadline) {
    try {
      await pingPrivilegedBridge(scope);
      return;
    } catch (error) {
      lastError = error;
      await sleep(100);
    }
  }

  throw new Error(
    `Privileged ${scope} bridge window did not become ready: ${errorMessage(lastError)}`,
  );
}

async function pingPrivilegedBridge(scope: PrivilegedBridgeScope): Promise<void> {
  requestSeq += 1;
  const id = `${Date.now()}-startup-${requestSeq}`;
  const event = pongEventName(id);

  let timer: number | null = null;
  let unlisten: UnlistenFn | null = null;
  let settled = false;
  const cleanup = () => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
    unlisten?.();
    unlisten = null;
  };

  const response = new Promise<void>((resolve, reject) => {
    timer = window.setTimeout(() => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(new Error(`Timed out waiting for ${scope} privileged bridge readiness.`));
    }, PRIVILEGED_BRIDGE_PING_TIMEOUT_MS);

    void once<PrivilegedBridgePong>(event, (eventPayload) => {
      if (settled) return;
      settled = true;
      cleanup();
      if (eventPayload.payload?.scope === scope) {
        resolve();
      } else {
        reject(new Error(`Unexpected privileged bridge readiness response for ${scope}.`));
      }
    }).then((unsubscribe) => {
      if (settled) {
        unsubscribe();
        return;
      }
      unlisten = unsubscribe;
    }, reject);
  });

  try {
    await emitTo(scope, PRIVILEGED_BRIDGE_PING_EVENT, {
      id,
      target: scope,
    } satisfies PrivilegedBridgePing);
  } catch (error) {
    settled = true;
    cleanup();
    throw error;
  }

  return response;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

export function isPrivilegedBridgeWindow(): boolean {
  return readBridgeScopeFromLocation() != null;
}

export function shouldUsePrivilegedWindowBroker(
  command: string,
): command is PrivilegedBrokerCommand {
  return PRIVILEGED_WINDOW_BROKER_COMMANDS.has(command as PrivilegedBrokerCommand);
}

export function __resetPrivilegedBridgeForTests(): void {
  bridgeStartupPromises.clear();
  requestSeq = 0;
}

// Rust tests pin this marker because it controls the one-time token retry.
export const PRIVILEGED_TOKEN_EXPIRED_MARKER = "Privileged command token is invalid or expired.";

export function isPrivilegedTokenExpiredError(error: unknown): boolean {
  if (!error) return false;
  const message = error instanceof Error ? error.message : String(error);
  return message.includes(PRIVILEGED_TOKEN_EXPIRED_MARKER);
}

/** Distinguish a client deadline from a reported native failure. */
export function isPrivilegedCommandTimeoutError(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  const stamped = error as Error & { command?: unknown; scope?: unknown; timeoutMs?: unknown };
  return (
    typeof stamped.timeoutMs === "number" &&
    typeof stamped.command === "string" &&
    typeof stamped.scope === "string"
  );
}

/** A native outcome that arrived after its bridge deadline. */
export interface LatePrivilegedResolution {
  command: string;
  ok: boolean;
  /** Native result for a successful late outcome. */
  value?: unknown;
  /** Native error for a failed late outcome. */
  error?: string;
}

const lateResolutionHandlers = new Set<(late: LatePrivilegedResolution) => void>();

// Only the newest invocation may publish a late result for a command.
const latestInvocationSeq = new Map<string, number>();

/** Subscribe to native outcomes that arrive after their bridge deadline. */
export function onLatePrivilegedResolution(
  handler: (late: LatePrivilegedResolution) => void,
): () => void {
  lateResolutionHandlers.add(handler);
  return () => {
    lateResolutionHandlers.delete(handler);
  };
}

function deliverLateResolution(late: LatePrivilegedResolution): void {
  for (const handler of [...lateResolutionHandlers]) handler(late);
}

export async function invokeThroughPrivilegedBridge<T>(
  brokerCommand: PrivilegedBrokerCommand,
  command: string,
  args: Record<string, unknown> | undefined,
): Promise<T> {
  // Reissue once when a short-lived token expires before dispatch.
  try {
    return await invokeThroughPrivilegedBridgeOnce<T>(brokerCommand, command, args);
  } catch (error) {
    if (!isPrivilegedTokenExpiredError(error)) throw error;
    return await invokeThroughPrivilegedBridgeOnce<T>(brokerCommand, command, args);
  }
}

async function invokeThroughPrivilegedBridgeOnce<T>(
  brokerCommand: PrivilegedBrokerCommand,
  command: string,
  args: Record<string, unknown> | undefined,
): Promise<T> {
  const scope = PRIVILEGED_WINDOW_BROKER_COMMANDS.get(brokerCommand);
  if (!scope) throw new Error(`Unsupported privileged bridge command: ${brokerCommand}`);

  const commandArgs = args ?? {};
  await ensurePrivilegedBridge(scope);
  const token = await issuePrivilegedCommandToken(brokerCommand, command, commandArgs);

  requestSeq += 1;
  const invocationSeq = requestSeq;
  latestInvocationSeq.set(command, invocationSeq);
  const id = `${Date.now()}-${requestSeq}`;
  const responseEvent = responseEventName(id);
  const nativeResponseEvent = usesNativeResponseEvent(command) ? responseEvent : undefined;

  let unlisten: UnlistenFn | null = null;
  let timer: number | null = null;
  let settled = false;
  const cleanup = () => {
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
    unlisten?.();
    unlisten = null;
  };

  const responseTimeoutMs = resolveCommandTimeoutMs(command);
  let resolveResponse!: (value: T) => void;
  let rejectResponse!: (reason: unknown) => void;
  const response = new Promise<T>((resolve, reject) => {
    resolveResponse = resolve;
    rejectResponse = reject;
  });

  // Install the listener before dispatch so fast native replies cannot be lost.
  let timedOut = false;
  try {
    unlisten = await once<PrivilegedBridgeResponse>(responseEvent, (event) => {
      if (settled) {
        if (timedOut) {
          cleanup();
          // A newer invocation supersedes this stale outcome.
          if (latestInvocationSeq.get(command) !== invocationSeq) return;
          const payload = event.payload;
          deliverLateResolution({
            command,
            ok: payload?.ok === true,
            value: payload?.ok ? payload.value : undefined,
            error: payload?.ok ? undefined : (payload?.error ?? "Privileged command failed."),
          });
        }
        return;
      }
      settled = true;
      cleanup();
      const payload = event.payload;
      if (payload?.ok) {
        resolveResponse(payload.value as T);
      } else {
        rejectResponse(new Error(payload?.error ?? "Privileged command failed."));
      }
    });
  } catch (error) {
    settled = true;
    cleanup();
    throw error;
  }

  timer = window.setTimeout(() => {
    if (settled) return;
    settled = true;
    timedOut = true;
    // Keep the one-shot listener alive for a late native outcome.
    timer = null;
    rejectResponse(createPrivilegedCommandTimeoutError(scope, command, responseTimeoutMs));
  }, responseTimeoutMs);

  try {
    await emitTo(scope, privilegedCommandEventName(scope), {
      id,
      scope,
      command,
      args: commandArgs,
      token,
      nativeResponseEvent,
    } satisfies PrivilegedBridgeRequest);
  } catch (error) {
    settled = true;
    cleanup();
    throw error;
  }

  return response;
}

export async function installPrivilegedCommandBridge(): Promise<void> {
  const scope = readBridgeScopeFromLocation();
  if (!scope) return;

  const currentWindow = WebviewWindow.getCurrent();
  if (currentWindow.label !== scope) {
    throw new Error(`Privileged bridge loaded in unexpected window: ${currentWindow.label}`);
  }

  const brokerCommand = BROKER_COMMAND_BY_SCOPE[scope];
  await listen<PrivilegedBridgePing>(PRIVILEGED_BRIDGE_PING_EVENT, async (event) => {
    const payload = event.payload;
    if (!isRecord(payload) || typeof payload.id !== "string") return;
    // Bridges share a single event name; only respond if this ping was for us.
    if (payload.target !== scope) return;
    await emitTo("main", pongEventName(payload.id), { scope } satisfies PrivilegedBridgePong);
  });

  await listen<PrivilegedBridgeRequest>(privilegedCommandEventName(scope), async (event) => {
    const request = parseBridgeRequest(event.payload);
    if (!request) return;
    if (request.scope !== scope) return;

    try {
      const value = await rawInvoke(brokerCommand, {
        request: {
          command: request.command,
          args: request.args,
          token: request.token,
          responseEvent: request.nativeResponseEvent,
        },
      });
      if (request.nativeResponseEvent) {
        return;
      }
      await emitTo("main", responseEventName(request.id), {
        ok: true,
        value,
      } satisfies PrivilegedBridgeResponse);
    } catch (error) {
      await emitTo("main", responseEventName(request.id), {
        ok: false,
        error: errorMessage(error) || "Unknown privileged command error",
      } satisfies PrivilegedBridgeResponse);
    }
  });

  await emitTo("main", PRIVILEGED_BRIDGE_READY_EVENT, { scope });
}
