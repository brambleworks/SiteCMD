import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { IssueGroup } from "@/lib/types";
import { withQueryClient } from "@/test-utils/query-client";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@/lib/tauri-invoke", () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));

const getResolvedIssuesMock = vi.hoisted(() => vi.fn());
vi.mock("@/lib/resolved-issues", () => ({
  getResolvedIssues: (...a: unknown[]) => getResolvedIssuesMock(...a),
}));

import { useIssueStatusResources } from "./useIssueStatusResources";

function makeGroup(overrides: Partial<IssueGroup>): IssueGroup {
  return {
    checkId: "security.csp",
    category: "security",
    severity: "high",
    title: "CSP missing",
    description: "No CSP header.",
    instances: [],
    sources: ["web_scan"],
    status: "blocked",
    snoozeUntil: null,
    blockReason: "intended",
    impactScore: 1,
    likelyCauses: [],
    suggestedIntegrations: [],
    fixLocations: [],
    transitiveCauses: [],
    downstreamEffects: [],
    recentEvents: [],
    enrichments: [],
    correlationEvidence: [],
    affectedPages: [],
    crossEnvSignal: null,
    crossProjectPattern: null,
    displayConfidence: null,
    observationCount: 0,
    anomalyScore: null,
    ...overrides,
  };
}

const PARAMS = { projectId: 7, normalizedUrl: "https://example.com" };

describe("useIssueStatusResources", () => {
  beforeEach(() => {
    invokeMock.mockReset().mockResolvedValue([]);
    getResolvedIssuesMock.mockReset().mockResolvedValue([]);
  });

  it("sources the blocked tab from get_work_items (project_issue_states)", async () => {
    invokeMock.mockResolvedValue([
      makeGroup({ checkId: "security.csp", status: "blocked" }),
      makeGroup({ checkId: "seo.title", status: "new", title: "Title" }),
      makeGroup({
        checkId: "code_scan.typo",
        status: "ignored",
        title: "Typosquat",
        sources: ["code_scan"],
      }),
    ]);

    const { result } = renderHook(() => useIssueStatusResources(PARAMS), {
      wrapper: withQueryClient(),
    });
    act(() => result.current.setStatusFilter("blocked"));

    await waitFor(() => expect(result.current.pausedWorkItems.length).toBe(1));
    expect(invokeMock).toHaveBeenCalledWith("get_work_items", {
      projectId: 7,
      envUrl: "https://example.com",
    });
    expect(result.current.pausedWorkItems[0]).toMatchObject({
      stableKey: "security.csp",
      status: "blocked",
      kind: "web",
      title: "CSP missing",
    });
  });

  it("reports a paused-state read failure instead of an empty blocked tab", async () => {
    invokeMock.mockRejectedValue(new Error("database unavailable"));
    const { result } = renderHook(() => useIssueStatusResources(PARAMS), {
      wrapper: withQueryClient(),
    });

    act(() => result.current.setStatusFilter("blocked"));

    await waitFor(() => expect(result.current.resourceError).not.toBeNull());
    expect(result.current.pausedWorkItems).toEqual([]);
  });

  it("reports a resolved-history read failure instead of an empty history", async () => {
    getResolvedIssuesMock.mockRejectedValue(new Error("database unavailable"));
    const { result } = renderHook(() => useIssueStatusResources(PARAMS), {
      wrapper: withQueryClient(),
    });

    act(() => result.current.setStatusFilter("resolved"));

    await waitFor(() => expect(result.current.resourceError).not.toBeNull());
    expect(result.current.resolvedList).toEqual([]);
  });

  it("fetches resolved history with the same url its cache key is built from", async () => {
    const { result } = renderHook(
      () => useIssueStatusResources({ projectId: 7, normalizedUrl: "https://example.com" }),
      { wrapper: withQueryClient() },
    );

    act(() => result.current.setStatusFilter("resolved"));

    await waitFor(() => expect(getResolvedIssuesMock).toHaveBeenCalled());
    expect(getResolvedIssuesMock).toHaveBeenCalledWith(7, "https://example.com", 100);
  });
});
