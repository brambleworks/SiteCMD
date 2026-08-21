import { beforeEach, describe, expect, it, vi } from "vitest";

const { eventMock, rawInvokeMock, webviewMock } = vi.hoisted(() => {
  const eventHandlers = new Map<string, (event: { payload: unknown }) => void>();
  return {
    eventMock: {
      emitTo: vi.fn(async (target: string, event: string, payload?: unknown) => {
        if (event === "sitecmd://privileged-ping") {
          const requestId = (payload as { id?: string } | undefined)?.id;
          if (requestId) {
            eventHandlers.get(`sitecmd://privileged-pong/${requestId}`)?.({
              payload: { scope: target },
            });
          }
        }
        if (event.startsWith("sitecmd://privileged-command/")) {
          const requestId = (payload as { id?: string } | undefined)?.id;
          if (requestId) {
            eventHandlers.get(`sitecmd://privileged-command-response/${requestId}`)?.({
              payload: { ok: true, value: null },
            });
          }
        }
        void target;
      }),
      handlerCount: () => eventHandlers.size,
      clearHandlers: () => eventHandlers.clear(),
      __emitForTests: (event: string, payload: unknown) => {
        eventHandlers.get(event)?.({ payload });
      },
      listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
        eventHandlers.set(event, handler);
        return () => {
          eventHandlers.delete(event);
        };
      }),
      once: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
        eventHandlers.set(event, handler);
        if (event === "sitecmd://privileged-ready") {
          handler({ payload: { scope: "data-admin" } });
        }
        return () => {
          eventHandlers.delete(event);
        };
      }),
    },
    rawInvokeMock: vi.fn(),
    webviewMock: {
      currentLabel: "main",
      getByLabel: vi.fn(async (label: string) => ({ label })),
    },
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => rawInvokeMock(...(args as [string, Record<string, unknown>?])),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emitTo: (...args: unknown[]) => eventMock.emitTo(...(args as [string, string, unknown?])),
  listen: (...args: unknown[]) =>
    eventMock.listen(...(args as [string, (event: { payload: unknown }) => void])),
  once: (...args: unknown[]) =>
    eventMock.once(...(args as [string, (event: { payload: unknown }) => void])),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: class {
    label: string;

    constructor(label: string, options: unknown) {
      this.label = label;
      void options;
    }

    static getByLabel(label: string) {
      return webviewMock.getByLabel(label);
    }

    static getCurrent() {
      return { label: webviewMock.currentLabel };
    }

    async once(event: string, handler: (event: { payload: unknown }) => void) {
      if (event === "tauri://created") {
        handler({ payload: null });
      }
      return () => {};
    }
  },
}));

import {
  invoke,
  resetTauriInvokeTestState,
  setTauriInvokeGuardsForTests,
  setTauriPrivilegedBrokerForTests,
} from "./tauri-invoke";
import {
  installPrivilegedCommandBridge,
  resolveCommandTimeoutMs,
  __resetPrivilegedBridgeForTests,
} from "./privileged-command-bridge";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    void rej;
  });
  return { promise, resolve };
}

function installDefaultEmitToMock() {
  eventMock.emitTo.mockImplementation(async (target: string, event: string, payload?: unknown) => {
    if (event === "sitecmd://privileged-ping") {
      const requestId = (payload as { id?: string } | undefined)?.id;
      if (requestId) {
        eventMock.__emitForTests(`sitecmd://privileged-pong/${requestId}`, {
          scope: target,
        });
      }
    }
    if (event.startsWith("sitecmd://privileged-command/")) {
      const requestId = (payload as { id?: string } | undefined)?.id;
      if (requestId) {
        eventMock.__emitForTests(`sitecmd://privileged-command-response/${requestId}`, {
          ok: true,
          value: null,
        });
      }
    }
  });
}

describe("tauri invoke", () => {
  beforeEach(() => {
    rawInvokeMock.mockReset();
    rawInvokeMock.mockImplementation(async (command: string) =>
      command.startsWith("issue_") && command.endsWith("_command_token") ? "native-token" : null,
    );
    installDefaultEmitToMock();
    eventMock.emitTo.mockClear();
    eventMock.listen.mockClear();
    eventMock.once.mockClear();
    eventMock.clearHandlers();
    webviewMock.getByLabel.mockClear();
    webviewMock.currentLabel = "main";
    webviewMock.getByLabel.mockImplementation(async (label: string) => ({ label }));
    window.history.pushState({}, "", "/");
    vi.useRealTimers();
    resetTauriInvokeTestState();
    __resetPrivilegedBridgeForTests();
  });

  it("does not retry non-read commands", async () => {
    rawInvokeMock.mockResolvedValueOnce("ok");

    await expect(invoke("ignore_issue", { projectId: 1 })).resolves.toBe("ok");
    expect(rawInvokeMock).toHaveBeenCalledTimes(1);
  });

  it("routes data administration commands through a privileged bridge window", async () => {
    setTauriPrivilegedBrokerForTests(true);

    await expect(invoke("delete_project", { projectId: 7 })).resolves.toBeNull();

    // Data-admin tokens issue silently through the scoped issuer: the
    // native confirmation lives inside the Rust command handler.
    expect(rawInvokeMock).toHaveBeenCalledWith("issue_data_admin_command_token", {
      request: {
        command: "delete_project",
        args: { projectId: 7 },
      },
    });
    expect(webviewMock.getByLabel).toHaveBeenCalledWith("data-admin");
    expect(eventMock.emitTo).toHaveBeenCalledWith(
      "data-admin",
      "sitecmd://privileged-command/data-admin",
      expect.objectContaining({
        scope: "data-admin",
        command: "delete_project",
        args: { projectId: 7 },
        token: "native-token",
      }),
    );
  });

  it("routes sensitive connector commands through the native-intent token issuer", async () => {
    setTauriPrivilegedBrokerForTests(true);

    await expect(
      invoke("save_webhook_config", {
        projectId: 7,
        url: "https://example.com/hook",
        events: "scan_completed",
        enabled: true,
      }),
    ).resolves.toBeNull();

    expect(rawInvokeMock).toHaveBeenCalledWith("issue_sensitive_privileged_command_token", {
      request: {
        command: "save_webhook_config",
        args: {
          projectId: 7,
          url: "https://example.com/hook",
          events: "scan_completed",
          enabled: true,
        },
        broker_command: "run_external_connector_command",
      },
    });
    expect(webviewMock.getByLabel).toHaveBeenCalledWith("external-connectors");
    expect(eventMock.emitTo).toHaveBeenCalledWith(
      "external-connectors",
      "sitecmd://privileged-command/external-connectors",
      expect.objectContaining({
        scope: "external-connectors",
        command: "save_webhook_config",
      }),
    );
  });

  it("routes OAuth connect through the routine scoped issuer without a native prompt", async () => {
    setTauriPrivilegedBrokerForTests(true);

    await expect(invoke("connect_google", { projectId: 7 })).resolves.toBeNull();

    // OAuth is gated by the browser consent screen, so it uses the routine
    // scoped issuer (no native confirmation), not the sensitive issuer.
    expect(rawInvokeMock).toHaveBeenCalledWith("issue_external_connector_command_token", {
      request: {
        command: "connect_google",
        args: { projectId: 7 },
      },
    });
    expect(rawInvokeMock).not.toHaveBeenCalledWith(
      "issue_sensitive_privileged_command_token",
      expect.anything(),
    );
  });

  it.each(["get_site_baseline", "decide_site_baseline"])(
    "routes %s through the external connector broker",
    async (command) => {
      setTauriPrivilegedBrokerForTests(true);
      const args = {
        siteId: 11,
        projectId: 7,
        environmentScopeKey: "production:https://example.com",
      };

      await expect(invoke(command, args)).resolves.toBeNull();

      expect(rawInvokeMock).toHaveBeenCalledWith("issue_external_connector_command_token", {
        request: { command, args },
      });
      expect(webviewMock.getByLabel).toHaveBeenCalledWith("external-connectors");
      expect(eventMock.emitTo).toHaveBeenCalledWith(
        "external-connectors",
        "sitecmd://privileged-command/external-connectors",
        expect.objectContaining({
          scope: "external-connectors",
          command,
          args,
          token: "native-token",
        }),
      );
    },
  );

  it("uses user-facing copy when privileged bridge commands time out", async () => {
    vi.useFakeTimers();
    setTauriPrivilegedBrokerForTests(true);
    eventMock.emitTo.mockImplementation(
      async (target: string, event: string, payload?: unknown) => {
        if (event === "sitecmd://privileged-ping") {
          const requestId = (payload as { id?: string } | undefined)?.id;
          if (requestId) {
            eventMock.__emitForTests(`sitecmd://privileged-pong/${requestId}`, {
              scope: target,
            });
          }
        }
      },
    );

    const command = "check_app_update";
    const request = expect(invoke(command, {})).rejects.toThrow(
      "That action took too long to finish. Click the same button again; if it keeps happening, restart SiteCMD.",
    );
    await vi.advanceTimersByTimeAsync(resolveCommandTimeoutMs(command));
    await request;
  });

  it("keeps waiting for browser-based OAuth completion past the default bridge timeout", async () => {
    vi.useFakeTimers();
    setTauriPrivilegedBrokerForTests(true);
    eventMock.emitTo.mockImplementation(
      async (target: string, event: string, payload?: unknown) => {
        if (event === "sitecmd://privileged-ping") {
          const requestId = (payload as { id?: string } | undefined)?.id;
          if (requestId) {
            eventMock.__emitForTests(`sitecmd://privileged-pong/${requestId}`, {
              scope: target,
            });
          }
          return;
        }
        if (event === "sitecmd://privileged-command/external-connectors") {
          const bridgePayload = payload as
            { id?: string; nativeResponseEvent?: string } | undefined;
          expect(bridgePayload?.nativeResponseEvent).toMatch(
            /^sitecmd:\/\/privileged-command-response\//,
          );
          window.setTimeout(() => {
            eventMock.__emitForTests(bridgePayload!.nativeResponseEvent!, {
              ok: true,
              value: { gsc_sites: [], ga4_properties: [] },
            });
          }, 16_000);
        }
      },
    );

    const request = invoke("complete_google_oauth", { projectId: 7, flowId: "google-flow" });
    await vi.advanceTimersByTimeAsync(16_000);

    await expect(request).resolves.toEqual({ gsc_sites: [], ga4_properties: [] });
  });

  it("routes filesystem access commands through a privileged bridge window", async () => {
    setTauriPrivilegedBrokerForTests(true);

    await expect(invoke("run_scan_execution", { request: { projectId: 7 } })).resolves.toBeNull();

    expect(rawInvokeMock).toHaveBeenCalledWith("issue_filesystem_access_command_token", {
      request: {
        command: "run_scan_execution",
        args: { request: { projectId: 7 } },
      },
    });
    expect(webviewMock.getByLabel).toHaveBeenCalledWith("filesystem-access");
    expect(eventMock.emitTo).toHaveBeenCalledWith(
      "filesystem-access",
      "sitecmd://privileged-command/filesystem-access",
      expect.objectContaining({
        scope: "filesystem-access",
        command: "run_scan_execution",
        args: { request: { projectId: 7 } },
      }),
    );
  });

  it("lets canonical scan execution resolve from the native broker response event", async () => {
    setTauriPrivilegedBrokerForTests(true);
    eventMock.emitTo.mockImplementation(
      async (target: string, event: string, payload?: unknown) => {
        if (event === "sitecmd://privileged-ping") {
          const requestId = (payload as { id?: string } | undefined)?.id;
          if (requestId) {
            eventMock.__emitForTests(`sitecmd://privileged-pong/${requestId}`, {
              scope: target,
            });
          }
          return;
        }
        if (event === "sitecmd://privileged-command/filesystem-access") {
          const nativeResponseEvent = (payload as { nativeResponseEvent?: string } | undefined)
            ?.nativeResponseEvent;
          expect(nativeResponseEvent).toMatch(/^sitecmd:\/\/privileged-command-response\//);
          eventMock.__emitForTests(nativeResponseEvent!, {
            ok: true,
            value: { id: 99, issueCount: 0 },
          });
        }
      },
    );

    await expect(invoke("run_scan_execution", { request: { projectId: 7 } })).resolves.toEqual({
      id: 99,
      issueCount: 0,
    });

    expect(eventMock.emitTo).toHaveBeenCalledWith(
      "filesystem-access",
      "sitecmd://privileged-command/filesystem-access",
      expect.objectContaining({
        command: "run_scan_execution",
        nativeResponseEvent: expect.stringMatching(/^sitecmd:\/\/privileged-command-response\//),
      }),
    );
  });

  it("routes project command execution through its privileged bridge window", async () => {
    setTauriPrivilegedBrokerForTests(true);
    await expect(invoke("run_project_command", { command: "pnpm install" })).resolves.toBeNull();

    expect(rawInvokeMock).toHaveBeenCalledWith("issue_project_execution_command_token", {
      request: {
        command: "run_project_command",
        args: { command: "pnpm install" },
      },
    });
    expect(webviewMock.getByLabel).toHaveBeenCalledWith("project-execution");
    expect(eventMock.emitTo).toHaveBeenCalledWith(
      "project-execution",
      "sitecmd://privileged-command/project-execution",
      expect.objectContaining({
        scope: "project-execution",
        command: "run_project_command",
        args: { command: "pnpm install" },
      }),
    );
  });

  it("routes filesystem export commands through their scoped token issuer", async () => {
    setTauriPrivilegedBrokerForTests(true);

    await expect(invoke("write_export_file", { path: "/tmp/report.md" })).resolves.toBeNull();

    expect(rawInvokeMock).toHaveBeenCalledWith("issue_filesystem_export_command_token", {
      request: {
        command: "write_export_file",
        args: { path: "/tmp/report.md" },
      },
    });
    expect(webviewMock.getByLabel).toHaveBeenCalledWith("filesystem-export");
    expect(eventMock.emitTo).toHaveBeenCalledWith(
      "filesystem-export",
      "sitecmd://privileged-command/filesystem-export",
      expect.objectContaining({
        scope: "filesystem-export",
        command: "write_export_file",
        args: { path: "/tmp/report.md" },
      }),
    );
  });

  it("issues destructive, export, and execution tokens through scoped issuers, never the sensitive one", async () => {
    setTauriPrivilegedBrokerForTests(true);

    await expect(invoke("delete_project", { projectId: 7 })).resolves.toBeNull();
    await expect(invoke("write_export_file", { path: "/tmp/report.md" })).resolves.toBeNull();
    await expect(invoke("run_project_command", { command: "pnpm install" })).resolves.toBeNull();

    expect(rawInvokeMock).toHaveBeenCalledWith("issue_data_admin_command_token", expect.anything());
    expect(rawInvokeMock).toHaveBeenCalledWith(
      "issue_filesystem_export_command_token",
      expect.anything(),
    );
    expect(rawInvokeMock).toHaveBeenCalledWith(
      "issue_project_execution_command_token",
      expect.anything(),
    );
    expect(rawInvokeMock).not.toHaveBeenCalledWith(
      "issue_sensitive_privileged_command_token",
      expect.anything(),
    );
  });

  it("pings privileged bridge windows before sending command requests", async () => {
    setTauriPrivilegedBrokerForTests(true);

    await expect(invoke("delete_project", { projectId: 7 })).resolves.toBeNull();

    const emittedEvents = eventMock.emitTo.mock.calls.map((call) => call[1]);
    expect(emittedEvents).toEqual([
      "sitecmd://privileged-ping",
      "sitecmd://privileged-command/data-admin",
    ]);
  });

  it("bridge windows ignore direct command events without a native-issued token", async () => {
    webviewMock.currentLabel = "data-admin";
    window.history.pushState({}, "", "/?sitecmd_privileged_bridge=data-admin");

    await installPrivilegedCommandBridge();
    eventMock.__emitForTests("sitecmd://privileged-command/data-admin", {
      id: "direct-event",
      scope: "data-admin",
      command: "delete_project",
      args: { projectId: 7 },
    });

    expect(rawInvokeMock).not.toHaveBeenCalledWith("run_data_admin_command", expect.anything());
  });

  it("bridge windows pass native-issued tokens through to scoped brokers", async () => {
    webviewMock.currentLabel = "data-admin";
    window.history.pushState({}, "", "/?sitecmd_privileged_bridge=data-admin");

    await installPrivilegedCommandBridge();
    eventMock.__emitForTests("sitecmd://privileged-command/data-admin", {
      id: "native-token-event",
      scope: "data-admin",
      command: "delete_project",
      args: { projectId: 7 },
      token: "native-token",
    });

    expect(rawInvokeMock).toHaveBeenCalledWith("run_data_admin_command", {
      request: {
        command: "delete_project",
        args: { projectId: 7 },
        token: "native-token",
      },
    });
  });

  it("bridge windows pass native response events through to scoped brokers", async () => {
    webviewMock.currentLabel = "filesystem-access";
    window.history.pushState({}, "", "/?sitecmd_privileged_bridge=filesystem-access");

    await installPrivilegedCommandBridge();
    eventMock.__emitForTests("sitecmd://privileged-command/filesystem-access", {
      id: "native-response-event",
      scope: "filesystem-access",
      command: "run_scan_execution",
      args: { request: { projectId: 7 } },
      token: "native-token",
      nativeResponseEvent: "sitecmd://privileged-command-response/native-response-event",
    });

    expect(rawInvokeMock).toHaveBeenCalledWith("run_filesystem_access_command", {
      request: {
        command: "run_scan_execution",
        args: { request: { projectId: 7 } },
        token: "native-token",
        responseEvent: "sitecmd://privileged-command-response/native-response-event",
      },
    });
  });

  it("bridge windows ignore privileged command events for a different bridge scope", async () => {
    webviewMock.currentLabel = "filesystem-export";
    window.history.pushState({}, "", "/?sitecmd_privileged_bridge=filesystem-export");

    await installPrivilegedCommandBridge();
    eventMock.__emitForTests("sitecmd://privileged-command/filesystem-export", {
      id: "wrong-scope-event",
      scope: "external-connectors",
      command: "detect_updates",
      args: { projectId: 7 },
      token: "native-token",
    });

    expect(rawInvokeMock).not.toHaveBeenCalledWith(
      "run_filesystem_export_command",
      expect.anything(),
    );
  });

  it("cleans up privileged command listeners when command dispatch fails", async () => {
    setTauriPrivilegedBrokerForTests(true);
    eventMock.emitTo.mockImplementationOnce(
      async (target: string, event: string, payload?: unknown) => {
        if (event === "sitecmd://privileged-ping") {
          const requestId = (payload as { id?: string } | undefined)?.id;
          if (requestId) {
            eventMock.__emitForTests(`sitecmd://privileged-pong/${requestId}`, {
              scope: target,
            });
          }
        }
      },
    );
    eventMock.emitTo.mockRejectedValueOnce(new Error("bridge send failed"));

    await expect(invoke("delete_project", { projectId: 7 })).rejects.toThrow("bridge send failed");
    await Promise.resolve();

    expect(eventMock.handlerCount()).toBe(0);
  });

  it("retries read commands after a transient bridge failure", async () => {
    setTauriInvokeGuardsForTests(true);
    rawInvokeMock.mockRejectedValueOnce(new TypeError("Load failed")).mockResolvedValueOnce("ok");

    await expect(invoke("get_dashboard_snapshot", { projectId: 1 })).resolves.toBe("ok");
    expect(rawInvokeMock).toHaveBeenCalledTimes(2);
  });

  it("does not retry a stalled read command while it remains in flight", async () => {
    setTauriInvokeGuardsForTests(true);
    const pending = deferred<string>();
    rawInvokeMock.mockImplementationOnce(() => pending.promise);

    const resultPromise = invoke("get_dashboard_snapshot", { projectId: 1 });

    await Promise.resolve();
    expect(rawInvokeMock).toHaveBeenCalledTimes(1);

    pending.resolve("ok");

    await expect(resultPromise).resolves.toBe("ok");
    expect(rawInvokeMock).toHaveBeenCalledTimes(1);
  });

  it("coalesces identical read commands while one is already in flight", async () => {
    setTauriInvokeGuardsForTests(true);
    const pending = deferred<string>();
    rawInvokeMock.mockImplementation(() => pending.promise);

    const first = invoke("get_dashboard_snapshot", { projectId: 1, url: "https://test" });
    const second = invoke("get_dashboard_snapshot", { projectId: 1, url: "https://test" });

    expect(rawInvokeMock).toHaveBeenCalledTimes(1);

    pending.resolve("ok");

    await expect(first).resolves.toBe("ok");
    await expect(second).resolves.toBe("ok");
    expect(rawInvokeMock).toHaveBeenCalledTimes(1);
  });

  it("limits distinct read command concurrency without forcing an app-wide waterfall", async () => {
    setTauriInvokeGuardsForTests(true);
    const first = deferred<string>();
    const second = deferred<string>();
    const third = deferred<string>();
    const fourth = deferred<string>();
    rawInvokeMock
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise)
      .mockImplementationOnce(() => third.promise)
      .mockImplementationOnce(() => fourth.promise)
      .mockResolvedValueOnce("fifth");

    const firstCall = invoke("get_dashboard_snapshot", { projectId: 1 });
    const secondCall = invoke("get_work_items", { projectId: 1 });
    const thirdCall = invoke("get_integrations", { projectId: 1 });
    const fourthCall = invoke("get_current_score", { projectId: 1 });
    const fifthCall = invoke("get_alerts", { projectId: 1 });

    expect(rawInvokeMock).toHaveBeenCalledTimes(4);

    first.resolve("first");
    await expect(firstCall).resolves.toBe("first");

    await Promise.resolve();
    expect(rawInvokeMock).toHaveBeenCalledTimes(5);

    second.resolve("second");
    third.resolve("third");
    fourth.resolve("fourth");
    await expect(secondCall).resolves.toBe("second");
    await expect(thirdCall).resolves.toBe("third");
    await expect(fourthCall).resolves.toBe("fourth");
    await expect(fifthCall).resolves.toBe("fifth");
  });
});
