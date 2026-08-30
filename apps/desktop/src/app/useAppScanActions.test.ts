import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AppShellHooks } from "@/app/AppProviders";
import type { EnvironmentRecord, ProjectRecord } from "@/hooks/useProject";
import type { ScanPreferences } from "@/hooks/useScanPrefs";

import { useAppScanActions } from "./useAppScanActions";

const activeEnv: EnvironmentRecord = {
  id: 1,
  url: "https://example.com",
  label: "Production",
  environment: "production",
  source: null,
  lastScannedAt: null,
  latestScore: null,
};

const activeProject: ProjectRecord = {
  id: 7,
  name: "Example",
  path: "/tmp/example",
  framework: "astro",
  createdAt: "2026-05-06T00:00:00Z",
  environments: [activeEnv],
};

const prefs: ScanPreferences = {
  timeout: 30,
  retentionLimit: 50,
  categories: {
    security: true,
    performance: true,
    seo: true,
    accessibility: true,
    compliance: true,
    config: true,
  },
};

function renderScanActions(
  scanHookOverrides: Record<string, unknown>,
  options: { projectFolder?: string | null; activeEnv?: EnvironmentRecord | null } = {},
) {
  const scanHook = {
    state: "idle",
    currentScanType: null,
    currentExecutionMode: null,
    result: null,
    codeResult: null,
    codeResultFromBackground: false,
    multiResult: null,
    executionIncompleteDetail: null,
    error: null,
    progress: null,
    multiProgress: null,
    cancelScan: vi.fn(),
    reset: vi.fn(),
    scan: vi.fn().mockResolvedValue({ ok: true }),
    scanCode: vi.fn().mockResolvedValue({ ok: true }),
    scanExecution: vi.fn().mockResolvedValue({
      ok: true,
      result: {
        execution: { id: 1, status: "complete" },
        reused: false,
        webResult: null,
        multiResult: null,
        codeResult: null,
      },
    }),
    ...scanHookOverrides,
  };

  const toast = { error: vi.fn() };
  const result = renderHook(() =>
    useAppScanActions({
      activeEnv: options.activeEnv !== undefined ? options.activeEnv : activeEnv,
      activeProject,
      enabledCategories: ["security", "performance", "seo"],
      prefs,
      projectFolder: options.projectFolder !== undefined ? options.projectFolder : "/tmp/example",
      scanHook: scanHook as AppShellHooks["scanHook"],
      toast,
    }),
  );

  return { ...result, scanHook, toast };
}

describe("useAppScanActions", () => {
  it("leaves partial Full execution reporting to the completion lifecycle", async () => {
    const { result, scanHook, toast } = renderScanActions({
      scanExecution: vi.fn().mockResolvedValue({
        ok: true,
        result: {
          execution: {
            id: 2,
            status: "partial",
            webStatus: "failed",
            webDetail: "Network error: Failed to fetch",
            codeStatus: "complete",
          },
          reused: false,
          webResult: null,
          multiResult: null,
          codeResult: { id: 9 },
        },
      }),
    });

    await act(async () => {
      await result.current.handleScan({
        urls: ["https://example.com"],
        axeEnabled: false,
        scanType: "full",
      });
    });

    expect(scanHook.scanExecution).toHaveBeenCalledTimes(1);
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("does not duplicate incomplete Web coverage notifications", async () => {
    const { result, toast } = renderScanActions({
      scanExecution: vi.fn().mockResolvedValue({
        ok: true,
        result: {
          execution: {
            id: 3,
            status: "partial",
            webStatus: "complete",
            webDetail: "Browser analysis failed: unavailable",
            codeStatus: "complete",
          },
          reused: false,
          webResult: { overallScore: 80 },
          multiResult: null,
          codeResult: { id: 10 },
        },
      }),
    });

    await act(async () => {
      await result.current.handleScan({
        urls: ["https://example.com"],
        axeEnabled: true,
        scanType: "full",
      });
    });

    expect(toast.error).not.toHaveBeenCalled();
  });

  it("runs nothing at all when the daily scan limit blocks the run", async () => {
    const { result, scanHook, toast } = renderScanActions({
      scanExecution: vi.fn().mockResolvedValue({
        ok: false,
        error: "Daily scan limit reached (3/3). Upgrade to Plus for unlimited scans.",
      }),
    });

    await act(async () => {
      await result.current.handleScan({
        urls: ["https://example.com"],
        axeEnabled: false,
        scanType: "full",
      });
    });

    expect(scanHook.scanExecution).toHaveBeenCalledTimes(1);
    // Nothing ran after it, so the shell's own error effect reports it once.
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("stops without double-reporting when the failing half is the last one", async () => {
    const { result, scanHook, toast } = renderScanActions(
      {
        scanExecution: vi
          .fn()
          .mockResolvedValue({ ok: false, error: "Network error: Failed to fetch" }),
      },
      { projectFolder: null },
    );

    await act(async () => {
      await result.current.handleScan({
        urls: ["https://example.com"],
        axeEnabled: false,
        scanType: "full",
      });
    });

    expect(scanHook.scanExecution).toHaveBeenCalledTimes(1);
    // Nothing followed this half, so the shell's own error effect reports it.
    // Toasting here too would show the user the same failure twice.
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("runs a code-only project's full scan with no web step", async () => {
    const { result, scanHook } = renderScanActions({}, { activeEnv: null });

    await act(async () => {
      await result.current.handleScan({ urls: [], axeEnabled: false, scanType: "full" });
    });

    expect(scanHook.scanExecution).toHaveBeenCalledWith(
      expect.objectContaining({
        requestedMode: "full",
        urls: [],
        projectId: 7,
        projectPath: "/tmp/example",
      }),
    );
  });

  it("runs code scan after the web step succeeds in a full scan", async () => {
    const { result, scanHook } = renderScanActions({});

    await act(async () => {
      await result.current.handleScan({
        urls: ["https://example.com"],
        axeEnabled: false,
        scanType: "full",
      });
    });

    expect(scanHook.scanExecution).toHaveBeenCalledWith(
      expect.objectContaining({
        requestedMode: "full",
        urls: ["https://example.com"],
        projectId: 7,
        projectPath: "/tmp/example",
      }),
    );
    expect(result.current.scanRunStep).toEqual({
      mode: "full",
      stepIndex: 1,
      stepCount: 2,
      label: "Web Scan",
    });
  });

  it("scopes a multi-page scan to the active environment regardless of page order", async () => {
    const { result, scanHook } = renderScanActions({});

    await act(async () => {
      await result.current.handleScan({
        urls: ["https://example.com/about", "https://example.com/contact"],
        axeEnabled: false,
        scanType: "web",
      });
    });

    expect(scanHook.scanExecution).toHaveBeenCalledWith(
      expect.objectContaining({
        requestedMode: "web",
        urls: ["https://example.com/about", "https://example.com/contact"],
        environmentUrl: "https://example.com",
      }),
    );
  });

  it("refuses to start a second scan while one is running, and says why", async () => {
    const { result, scanHook, toast } = renderScanActions({ state: "scanning" });

    await act(async () => {
      await result.current.handleScan({
        urls: ["https://example.com"],
        axeEnabled: false,
        scanType: "web",
      });
    });

    expect(scanHook.scanExecution).not.toHaveBeenCalled();
    expect(toast.error).toHaveBeenCalledWith(
      "A scan is already running",
      expect.stringContaining("cancel it"),
    );
  });

  it("refuses a shortcut scan while one is running, because it bypasses handleScan", async () => {
    // `handleShortcutScan` calls `scan` directly, so guarding only handleScan
    // would have left the keyboard route wide open.
    const { result, scanHook, toast } = renderScanActions({ state: "scanning" });

    await act(async () => {
      await result.current.handleShortcutScan("https://example.com");
    });

    expect(scanHook.scan).not.toHaveBeenCalled();
    expect(toast.error).toHaveBeenCalledWith(
      "A scan is already running",
      expect.stringContaining("cancel it"),
    );
  });

  it("applies the saved retention preference to shortcut scans", async () => {
    const { result, scanHook } = renderScanActions({});

    await act(async () => {
      await result.current.handleShortcutScan("https://example.com");
    });

    expect(scanHook.scan).toHaveBeenCalledWith(
      "https://example.com",
      expect.objectContaining({ retention: prefs.retentionLimit }),
    );
  });

  it("still starts a scan when the previous one has finished", async () => {
    const { result, scanHook, toast } = renderScanActions({ state: "complete" });

    await act(async () => {
      await result.current.handleScan({
        urls: ["https://example.com"],
        axeEnabled: false,
        scanType: "web",
      });
    });

    expect(scanHook.scanExecution).toHaveBeenCalledTimes(1);
    expect(toast.error).not.toHaveBeenCalled();
  });
});
