// Tauri IPC stub that validates fixture commands, reports unstubbed calls, and
// routes named events before the application loads.
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Page } from "@playwright/test";
import type { LicenseInfo } from "../../src/generated/ipc-bindings";

export type InvokeResponses = Record<string, unknown>;

/** Keep an invoke pending until the test resolves it explicitly. */
export const DEFERRED = { __e2eDeferred: true } as const;

interface StubWindowApi {
  emit(event: string, payload: unknown): number;
  listenerCount(event: string): number;
  resolveDeferred(cmd: string, value: unknown): number;
}

const here = dirname(fileURLToPath(import.meta.url));

let appCommandsCache: Set<string> | null = null;
let brokerCommandsCache: Map<string, string> | null = null;

// Parse the registered command surface from build.rs.
function appCommands(): Set<string> {
  if (!appCommandsCache) {
    const buildRs = readFileSync(resolve(here, "../../src-tauri/build.rs"), "utf8");
    const match = buildRs.match(/APP_COMMANDS\s*:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/);
    if (!match) throw new Error("APP_COMMANDS not found in src-tauri/build.rs");
    appCommandsCache = new Set(Array.from(match[1].matchAll(/"([^"]+)"/g), (m) => m[1]));
  }
  return appCommandsCache;
}

// Map logical commands to the privileged broker carriers registered with Tauri.
function brokerCommands(): Map<string, string> {
  if (!brokerCommandsCache) {
    const source = readFileSync(resolve(here, "../../src/lib/tauri-invoke.ts"), "utf8");
    const block = source.match(/PRIVILEGED_BROKER_COMMANDS[\s\S]*?new Map\(\[([\s\S]*?)\]\)/);
    if (!block) throw new Error("PRIVILEGED_BROKER_COMMANDS not found in lib/tauri-invoke.ts");
    brokerCommandsCache = new Map(
      Array.from(block[1].matchAll(/\[\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\]/g), (m) => [m[1], m[2]]),
    );
  }
  return brokerCommandsCache;
}

const FREE_LICENSE: LicenseInfo = {
  tier: "free",
  status: "free",
  planName: "Free",
  billingInterval: null,
  isActive: false,
  expiresAt: null,
  checkoutUrls: {
    core: "",
    pro: "",
    coreMonthly: "",
    coreAnnual: "",
    proMonthly: "",
    proAnnual: "",
  },
  customerPortalUrl: "",
  validationWarning: "none",
};

/** Default responses for the no-project boot path. */
const DEFAULT_INVOKE_RESPONSES: InvokeResponses = {
  get_projects: [],
  get_license_status: FREE_LICENSE,
  get_telemetry_consent: {
    usageAnalytics: false,
    crashReports: false,
    consentVersion: 1,
    updatedAt: null,
  },
  get_alerts: [],
  count_unread_alerts: 0,
  update_tray_summary: null,
  log_frontend: null,
};

export async function installTauriStub(page: Page, overrides: InvokeResponses = {}): Promise<void> {
  const responses = { ...DEFAULT_INVOKE_RESPONSES, ...overrides };
  const broker = brokerCommands();
  // Brokered logical commands are valid even though only their carriers are registered.
  const known = new Set([...appCommands(), ...broker.keys()]);
  const dead = Object.keys(responses).filter((cmd) => !known.has(cmd));
  if (dead.length > 0) {
    throw new Error(
      `tauri-stub fixtures name commands missing from build.rs APP_COMMANDS ` +
        `(dead or renamed): ${dead.join(", ")}`,
    );
  }

  await page.addInitScript((canned: InvokeResponses) => {
    // Prevent the telemetry prompt from covering the tested surface.
    try {
      window.localStorage.setItem(
        "sitecmd_telemetry_consent_v1",
        JSON.stringify({
          usageAnalytics: false,
          crashReports: false,
          promptStatus: "saved",
          subjectId: null,
          deleteSecret: null,
          consentVersion: 1,
          updatedAt: null,
        }),
      );
    } catch {
      // localStorage unavailable: the prompt may appear, tests will say so.
    }

    let nextCallbackId = 1;
    const callbacks = new Map<number, (...args: unknown[]) => void>();
    const listenersByEvent = new Map<string, Set<number>>();
    const deferredResolvers = new Map<string, Array<(value: unknown) => void>>();

    // Shared by the test API and privileged-bridge emulation.
    const deliverEvent = (event: string, payload: unknown): number => {
      const ids = listenersByEvent.get(event);
      if (!ids) return 0;
      for (const id of ids) callbacks.get(id)?.({ event, id, payload });
      return ids.size;
    };

    const stub: StubWindowApi = {
      emit: deliverEvent,
      listenerCount(event) {
        return listenersByEvent.get(event)?.size ?? 0;
      },
      resolveDeferred(cmd, value) {
        const waiting = deferredResolvers.get(cmd) ?? [];
        deferredResolvers.delete(cmd);
        for (const resolveInvoke of waiting) resolveInvoke(value);
        return waiting.length;
      },
    };
    Object.defineProperty(window, "__E2E_TAURI_STUB__", { configurable: true, value: stub });

    const isDeferredMarker = (value: unknown): boolean =>
      typeof value === "object" &&
      value !== null &&
      (value as { __e2eDeferred?: boolean }).__e2eDeferred === true;

    // Emulate the event handshake normally handled by privileged bridge windows.
    const BRIDGE_WINDOW_LABELS = [
      "main",
      "data-admin",
      "external-connectors",
      "filesystem-access",
      "filesystem-export",
      "project-execution",
    ];
    const PING_EVENT = "sitecmd://privileged-ping";
    const COMMAND_EVENT_PREFIX = "sitecmd://privileged-command/";
    const pongEvent = (id: string) => `sitecmd://privileged-pong/${id}`;
    const responseEvent = (id: string) => `sitecmd://privileged-command-response/${id}`;
    const isTokenIssuer = (cmd: string) => /^issue_[a-z_]+_token$/.test(cmd);

    // DEFERRED lets a spec control when a brokered command resolves.
    const answerBrokeredCommand = (id: string, command: string) => {
      const respond = (value: unknown) => deliverEvent(responseEvent(id), { ok: true, value });
      // Background broker calls without fixtures resolve silently.
      if (!(command in canned)) {
        respond(null);
        return;
      }
      const value = canned[command];
      if (isDeferredMarker(value)) {
        if (!deferredResolvers.has(command)) deferredResolvers.set(command, []);
        deferredResolvers.get(command)?.push(respond);
        return;
      }
      respond(value);
    };

    // A frontend emit that targets a bridge window. Returns true when it was
    // a handshake event this stub answered.
    const handleBridgeEmit = (event: string, payload: unknown): boolean => {
      const body = (payload ?? {}) as { id?: unknown; command?: unknown; target?: unknown };
      const id = typeof body.id === "string" ? body.id : null;
      if (!id) return false;
      if (event === PING_EVENT) {
        deliverEvent(pongEvent(id), { scope: body.target });
        return true;
      }
      if (event.startsWith(COMMAND_EVENT_PREFIX) && typeof body.command === "string") {
        answerBrokeredCommand(id, body.command);
        return true;
      }
      return false;
    };

    // Direct broker invokes match the bridge's quiet timeout behavior.
    const SILENT_NULL = new Set([
      "run_data_admin_command",
      "run_external_connector_command",
      "run_filesystem_access_command",
      "run_filesystem_export_command",
      "run_project_execution_command",
    ]);

    // Match the Tauri 2 API shape used by @tauri-apps/api internals.
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {
        transformCallback: (cb?: (...args: unknown[]) => void) => {
          const id = nextCallbackId++;
          if (cb) callbacks.set(id, cb);
          return id;
        },
        invoke: (cmd: string, args?: Record<string, unknown>) => {
          if (cmd === "plugin:event|listen") {
            const event = String(args?.event);
            const handler = Number(args?.handler);
            if (!listenersByEvent.has(event)) listenersByEvent.set(event, new Set());
            listenersByEvent.get(event)?.add(handler);
            // listen() resolves to the eventId later passed to unlisten.
            return Promise.resolve(handler);
          }
          if (cmd === "plugin:event|unlisten") {
            const event = String(args?.event);
            const eventId = Number(args?.eventId);
            listenersByEvent.get(event)?.delete(eventId);
            callbacks.delete(eventId);
            return Promise.resolve(null);
          }
          // WebviewWindow.getByLabel() enumerates windows; the bridge
          // startup fails fast unless its scope window is present.
          if (cmd === "plugin:window|get_all_windows") {
            return Promise.resolve(BRIDGE_WINDOW_LABELS);
          }
          // A frontend emit to a bridge window: answer the handshake in-page.
          if (cmd === "plugin:event|emit" || cmd === "plugin:event|emit_to") {
            handleBridgeEmit(String(args?.event), args?.payload);
            return Promise.resolve(null);
          }
          if (cmd.startsWith("plugin:")) return Promise.resolve(null);
          // Every elevated call mints a token first; the carrier windows
          // above never run in-browser, so any issuer returns a stub token.
          if (isTokenIssuer(cmd)) return Promise.resolve("e2e-privileged-token");
          if (cmd in canned) {
            const value = canned[cmd];
            if (isDeferredMarker(value)) {
              return new Promise((resolveInvoke) => {
                if (!deferredResolvers.has(cmd)) deferredResolvers.set(cmd, []);
                deferredResolvers.get(cmd)?.push(resolveInvoke);
              });
            }
            return Promise.resolve(value);
          }
          if (SILENT_NULL.has(cmd)) return Promise.resolve(null);
          console.error(`[tauri-stub] unstubbed invoke: ${cmd}`);
          return Promise.resolve(null);
        },
        unregisterListener: (_event: string, id: number) => {
          callbacks.delete(id);
          return Promise.resolve();
        },
        metadata: {
          currentWindow: { label: "main" },
          currentWebview: { label: "main" },
        },
      },
    });

    // Event plugin stores its registry on a separate global in Tauri 2.
    Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
      configurable: true,
      value: {
        unregisterListener: () => {},
      },
    });
  }, responses);
}

type StubWindow = Window & { __E2E_TAURI_STUB__?: StubWindowApi };

/** Push a Tauri event into every app listener registered for it. */
export async function emitTauriEvent(page: Page, event: string, payload: unknown): Promise<void> {
  const delivered = await page.evaluate(
    ([eventName, eventPayload]) => {
      const stub = (window as StubWindow).__E2E_TAURI_STUB__;
      if (!stub) throw new Error("tauri stub is not installed on this page");
      return stub.emit(eventName as string, eventPayload);
    },
    [event, payload] as const,
  );
  if (delivered === 0) {
    throw new Error(`emitTauriEvent("${event}"): the app has no listener registered`);
  }
}

/** Wait for event registration so an emit cannot race listener setup. */
export async function waitForTauriListener(page: Page, event: string): Promise<void> {
  await page.waitForFunction((eventName) => {
    const stub = (window as StubWindow).__E2E_TAURI_STUB__;
    return (stub?.listenerCount(eventName) ?? 0) > 0;
  }, event);
}

/** Complete a DEFERRED command; throws if nothing is awaiting it. */
export async function resolveDeferredInvoke(
  page: Page,
  cmd: string,
  value: unknown,
): Promise<void> {
  const resolved = await page.evaluate(
    ([command, commandValue]) => {
      const stub = (window as StubWindow).__E2E_TAURI_STUB__;
      if (!stub) throw new Error("tauri stub is not installed on this page");
      return stub.resolveDeferred(command as string, commandValue);
    },
    [cmd, value] as const,
  );
  if (resolved === 0) {
    throw new Error(`resolveDeferredInvoke("${cmd}"): no pending invoke to resolve`);
  }
}
