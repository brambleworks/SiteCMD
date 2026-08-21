import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { readOnboardingSetupSteps } from "@/lib/onboarding-setup";
import { useAppProjectCreation } from "./useAppProjectCreation";

const project = {
  id: 7,
  name: "Example Site",
  path: "/Users/dev/example",
  framework: "Astro",
  createdAt: "2026-05-05T12:00:00Z",
  environments: [
    {
      id: 11,
      url: "https://example.com/",
      label: "Production",
      environment: "production",
      source: null,
      lastScannedAt: null,
      latestScore: null,
    },
  ],
};

describe("useAppProjectCreation", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("selects the created project and queues its baseline scan", async () => {
    const queueBaselineScan = vi.fn();
    const selectProject = vi.fn();

    const refreshProjects = vi.fn(async () => ({ projects: [project], newProject: project }));

    const { result } = renderHook(() =>
      useAppProjectCreation({
        refreshProjects,
        queueBaselineScan,
        selectProject,
      }),
    );

    act(() => {
      result.current.openAddProject();
    });
    expect(result.current.showAddProject).toBe(true);

    await act(async () => {
      await result.current.handleProjectCreated(project.id);
    });

    expect(result.current.showAddProject).toBe(false);
    expect(selectProject).toHaveBeenCalledWith(project);
    // Queued, not run directly: the scan must wait for the selection flush so
    // it sees the fresh environment URL and linked folder (code scan included).
    expect(queueBaselineScan).toHaveBeenCalledTimes(1);
    expect(queueBaselineScan).toHaveBeenCalledWith(project.id);
    expect(refreshProjects).toHaveBeenCalledTimes(1);
    // The pending baseline-review step is what gates the first-run walkthrough.
    expect(readOnboardingSetupSteps(project.id)).toEqual(["baseline-review"]);
  });

  it("does not queue a scan or write onboarding state when no projects come back", async () => {
    const queueBaselineScan = vi.fn();

    const refreshProjects = vi.fn(async () => ({ projects: [], newProject: null }));

    const { result } = renderHook(() =>
      useAppProjectCreation({
        refreshProjects,
        queueBaselineScan,
        selectProject: vi.fn(),
      }),
    );

    await act(async () => {
      await result.current.handleProjectCreated(1);
    });

    expect(queueBaselineScan).not.toHaveBeenCalled();
    expect(readOnboardingSetupSteps(1)).toEqual([]);
  });
});
