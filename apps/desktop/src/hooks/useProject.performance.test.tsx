import { type ReactNode } from "react";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

import {
  PERFORMANCE_BUDGETS,
  clearPerformanceSnapshot,
  readPerformanceSnapshot,
} from "@/lib/performance-metrics";
import { ProjectProvider, useProject, type ProjectRecord } from "./useProject";
import { resetActiveSelectionForTest } from "@/lib/active-selection-store";
import { createTestQueryClient } from "@/test-utils/query-client";

let queryClient: QueryClient;

function buildProject(id: number): ProjectRecord {
  return {
    id,
    name: `Project ${id}`,
    path: `/tmp/project-${id}`,
    framework: "astro",
    createdAt: `2026-04-14T12:${String(id).padStart(2, "0")}:00Z`,
    environments: [
      {
        id: id * 10 + 1,
        url: `https://project-${id}.example.com`,
        label: `Project ${id}`,
        environment: "production",
        source: "manual",
        lastScannedAt: null,
        latestScore: 82,
      },
      {
        id: id * 10 + 2,
        url: `https://staging.project-${id}.example.com`,
        label: `Project ${id} Staging`,
        environment: "staging",
        source: "manual",
        lastScannedAt: null,
        latestScore: 78,
      },
    ],
  };
}

function wrapper({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      <ProjectProvider>{children}</ProjectProvider>
    </QueryClientProvider>
  );
}

function average(values: number[]) {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function metricLatestDuration() {
  return (
    readPerformanceSnapshot().find((metric) => metric.key === "app.first_project_load_ms")
      ?.latestDurationMs ?? null
  );
}

describe("useProject performance baseline", () => {
  beforeEach(() => {
    queryClient = createTestQueryClient();
    cleanup();
    clearPerformanceSnapshot();
    window.localStorage.clear();
    invokeMock.mockReset();
    resetActiveSelectionForTest();
  });

  afterEach(() => {
    cleanup();
    clearPerformanceSnapshot();
  });

  it("captures a repeatable first-project-load baseline", async () => {
    const projects = Array.from({ length: 24 }, (_, index) => buildProject(index + 1));
    const samples: number[] = [];

    for (let iteration = 0; iteration < 5; iteration += 1) {
      cleanup();
      queryClient = createTestQueryClient();
      clearPerformanceSnapshot();
      window.localStorage.clear();
      invokeMock.mockReset();
      invokeMock.mockImplementation(async (command: string) => {
        if (command === "get_projects") return projects;
        return null;
      });

      const { result, unmount } = renderHook(() => useProject(), { wrapper });

      await waitFor(() => {
        expect(result.current.activeProject?.id).toBe(1);
      });

      await waitFor(() => {
        expect(metricLatestDuration()).not.toBeNull();
      });

      samples.push(metricLatestDuration() ?? 0);
      unmount();
    }

    const averageMs = Math.round(average(samples));
    console.info(
      `[perf-baseline] first_project_load_ms avg=${averageMs} samples=${samples.join(",")}`,
    );
    expect(averageMs).toBeLessThanOrEqual(
      PERFORMANCE_BUDGETS["app.first_project_load_ms"].budgetMs,
    );
  });
});
