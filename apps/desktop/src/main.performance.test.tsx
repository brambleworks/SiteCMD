import React from "react";
import { cleanup, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./App", () => ({
  default: () => React.createElement("div", null, "SiteCMD app ready"),
}));

vi.mock("./hooks/useScanPrefs", () => ({
  ScanPrefsProvider: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
}));

vi.mock("./hooks/useTheme", () => ({
  ThemeProvider: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
}));

vi.mock("./hooks/useToast", () => ({
  ToastProvider: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
}));

const { installGlobalErrorHandlersMock, loggerInfoMock, recordErrorReportMock } = vi.hoisted(
  () => ({
    installGlobalErrorHandlersMock: vi.fn(),
    loggerInfoMock: vi.fn(),
    recordErrorReportMock: vi.fn(),
  }),
);

vi.mock("./lib/logger", () => ({
  installGlobalErrorHandlers: () => installGlobalErrorHandlersMock(),
  logger: {
    info: (...args: unknown[]) => loggerInfoMock(...args),
    error: vi.fn(),
  },
}));

vi.mock("./lib/observability", () => ({
  recordErrorReport: (...args: unknown[]) => recordErrorReportMock(...args),
}));

import {
  PERFORMANCE_BUDGETS,
  clearPerformanceSnapshot,
  readPerformanceSnapshot,
} from "@/lib/performance-metrics";

function average(values: number[]) {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function latestColdStartDuration() {
  return (
    readPerformanceSnapshot().find((metric) => metric.key === "app.cold_start_ms")
      ?.latestDurationMs ?? null
  );
}

describe("main renderer cold-start baseline", () => {
  beforeEach(() => {
    cleanup();
    clearPerformanceSnapshot();
    window.localStorage.clear();
    document.body.innerHTML = '<div id="root"></div>';
    installGlobalErrorHandlersMock.mockReset();
    loggerInfoMock.mockReset();
    recordErrorReportMock.mockReset();
  });

  afterEach(() => {
    cleanup();
    clearPerformanceSnapshot();
    document.body.innerHTML = "";
  });

  it("captures a repeatable renderer cold-start baseline", async () => {
    const samples: number[] = [];

    for (let iteration = 0; iteration < 5; iteration += 1) {
      cleanup();
      clearPerformanceSnapshot();
      window.localStorage.clear();
      document.body.innerHTML = '<div id="root"></div>';
      vi.resetModules();

      await import("./main");

      await screen.findByText("SiteCMD app ready");
      await waitFor(() => {
        expect(document.documentElement.getAttribute("data-sitecmd-startup")).toBe("mounted");
        expect(latestColdStartDuration()).not.toBeNull();
      });

      samples.push(latestColdStartDuration() ?? 0);
    }

    const averageMs = Math.round(average(samples));
    console.info(`[perf-baseline] cold_start_ms avg=${averageMs} samples=${samples.join(",")}`);
    expect(averageMs).toBeLessThanOrEqual(PERFORMANCE_BUDGETS["app.cold_start_ms"].budgetMs);
  });
});
