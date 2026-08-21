import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useAppTargetNavigation } from "./useAppTargetNavigation";
import type { AppTarget } from "@/lib/app-targets";

type Params = Parameters<typeof useAppTargetNavigation>[0];

function makeParams(overrides: Partial<Params> = {}): Params {
  return {
    activeEnv: { url: "https://example.com" },
    activeProject: null,
    projects: [],
    projectsLoading: false,
    selectProject: vi.fn(),
    selectEnv: vi.fn(),
    dispatch: vi.fn(),
    openScanConfig: vi.fn(),
    updateScanBackgrounded: vi.fn(),
    ...overrides,
  } as unknown as Params;
}

function target(t: Record<string, unknown>): AppTarget {
  return t as unknown as AppTarget;
}

describe("useAppTargetNavigation routing glue", () => {
  it("dispatches a generic page navigation when already in context", () => {
    const dispatch = vi.fn();
    const { result } = renderHook(() =>
      useAppTargetNavigation(makeParams({ dispatch, activeEnv: null })),
    );

    result.current.openAppTarget(target({ page: "reports" }));

    expect(dispatch).toHaveBeenCalledWith({ type: "NAVIGATE_GENERIC", page: "reports" });
  });

  it("routes a notification target to the correct project and environment", () => {
    const selectProject = vi.fn();
    const selectEnv = vi.fn();
    const projectA = { id: 1, environments: [{ url: "https://a.com" }] };
    const projectB = { id: 2, environments: [{ url: "https://b.com" }] };

    const { result } = renderHook(() =>
      useAppTargetNavigation(
        makeParams({
          projects: [projectA, projectB] as unknown as Params["projects"],
          activeProject: projectA as unknown as Params["activeProject"],
          activeEnv: { url: "https://a.com" } as unknown as Params["activeEnv"],
          selectProject,
          selectEnv,
        }),
      ),
    );

    result.current.openAppTarget(target({ page: "issues", url: "https://b.com" }));

    expect(selectProject).toHaveBeenCalledWith(projectB);
    expect(selectEnv).toHaveBeenCalledWith(projectB.environments[0]);
  });

  it("defers a URL-scoped target while projects load, then routes it once ready", async () => {
    const dispatch = vi.fn();
    const projectA = { id: 1, environments: [{ url: "https://a.com" }] };

    const { result, rerender } = renderHook((p: Params) => useAppTargetNavigation(p), {
      initialProps: makeParams({
        projects: [],
        projectsLoading: true,
        activeProject: null,
        activeEnv: null,
        dispatch,
      }),
    });

    // While projects are still loading, a URL-scoped target must be parked, not
    // acted on (we don't yet know which project/env it belongs to).
    result.current.openAppTarget(target({ page: "issues", url: "https://a.com", scanId: 7 }));
    expect(dispatch).not.toHaveBeenCalled();

    // Projects finish loading and the matching project/env is now active: the
    // parked target is processed by the pending-target effect.
    rerender(
      makeParams({
        projects: [projectA] as unknown as Params["projects"],
        projectsLoading: false,
        activeProject: projectA as unknown as Params["activeProject"],
        activeEnv: { url: "https://a.com" } as unknown as Params["activeEnv"],
        dispatch,
      }),
    );

    await waitFor(() =>
      expect(dispatch).toHaveBeenCalledWith({
        type: "NAVIGATE_ISSUES",
        target: { focus: null, itemId: null },
      }),
    );
  });

  it("opens an overview project by selecting it and landing on the dashboard", () => {
    const selectProject = vi.fn();
    const dispatch = vi.fn();
    const projectA = { id: 1, environments: [{ url: "https://a.com" }] };

    const { result } = renderHook(() =>
      useAppTargetNavigation(
        makeParams({
          projects: [projectA] as unknown as Params["projects"],
          selectProject,
          dispatch,
        }),
      ),
    );

    result.current.openOverviewProject(1);

    expect(selectProject).toHaveBeenCalledWith(projectA);
    expect(dispatch).toHaveBeenCalledWith({ type: "NAVIGATE_GENERIC", page: "dashboard" });
  });
  it("carries a connected alert deep link to the timeline as a target", () => {
    const dispatch = vi.fn();
    const { result } = renderHook(() =>
      useAppTargetNavigation(makeParams({ dispatch, activeEnv: null })),
    );

    result.current.openAppTarget(target({ page: "alerts", itemId: "alr_0123456789ab" }));

    expect(dispatch).toHaveBeenCalledWith({
      type: "OPEN_ALERTS",
      target: { alertId: "alr_0123456789ab", reason: null },
    });
  });

  it("carries the not-found reason to the timeline instead of a bare page change", () => {
    const dispatch = vi.fn();
    const { result } = renderHook(() =>
      useAppTargetNavigation(makeParams({ dispatch, activeEnv: null })),
    );

    result.current.openAppTarget(target({ page: "alerts", reason: "connected-alert-unavailable" }));

    expect(dispatch).toHaveBeenCalledWith({
      type: "OPEN_ALERTS",
      target: { alertId: null, reason: "connected-alert-unavailable" },
    });
  });

  it("opens the settings tab a target names in focus", () => {
    const dispatch = vi.fn();
    const { result } = renderHook(() =>
      useAppTargetNavigation(makeParams({ dispatch, activeEnv: null })),
    );

    result.current.openAppTarget(target({ page: "settings", focus: "connected" }));

    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_SETTINGS", tab: "connected" });
  });
});
