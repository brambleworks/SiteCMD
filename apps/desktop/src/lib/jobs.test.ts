import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import {
  addJob,
  clearJobsByType,
  completeJob,
  failJob,
  recordCompletedJob,
  removeJob,
  removeRunningJob,
  updateJob,
  useJobs,
  useJobsCenter,
  useRunningJobsCount,
} from "./jobs";

function resetJobs() {
  // Clear every type so tests start from a clean slate
  clearJobsByType("scan");
  clearJobsByType("probes");
  clearJobsByType("sync");
}

describe("jobs store", () => {
  let nowSpy: ReturnType<typeof vi.spyOn>;
  let currentTime = 1_000_000;

  beforeEach(() => {
    resetJobs();
    currentTime = 1_000_000;
    nowSpy = vi.spyOn(Date, "now").mockImplementation(() => currentTime);
  });

  afterEach(() => {
    nowSpy.mockRestore();
    resetJobs();
  });

  it("starts empty", () => {
    const { result } = renderHook(() => useJobs());
    expect(result.current).toEqual([]);
  });

  it("addJob appends a running job", () => {
    const { result } = renderHook(() => useJobs());
    act(() => {
      addJob({ id: "a", type: "scan", label: "Scan foo" });
    });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].id).toBe("a");
    expect(result.current[0].status).toBe("running");
    expect(result.current[0].startedAt).toBe(1_000_000);
  });

  it("addJob with an existing id replaces rather than duplicates", () => {
    const { result } = renderHook(() => useJobs());
    act(() => {
      addJob({ id: "a", type: "scan", label: "First" });
    });
    act(() => {
      addJob({ id: "a", type: "scan", label: "Second" });
    });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].label).toBe("Second");
  });

  it("addJob skips the publish when the running job payload is unchanged", () => {
    const { result } = renderHook(() => useJobs());
    act(() => {
      addJob({ id: "scan", type: "scan", label: "Web scan", progress: 10, detail: "x" });
    });
    const firstRef = result.current;
    expect(firstRef).toHaveLength(1);

    act(() => {
      addJob({ id: "scan", type: "scan", label: "Web scan", progress: 10, detail: "x" });
    });
    expect(result.current).toBe(firstRef);

    // An equal-by-value but distinct target object must also count as unchanged.
    act(() => {
      addJob({
        id: "scan",
        type: "scan",
        label: "Web scan",
        progress: 10,
        detail: "x",
        target: { restoreScan: true, projectId: 1, url: "https://example.com" },
      });
    });
    const withTarget = result.current;
    expect(withTarget).not.toBe(firstRef);
    act(() => {
      addJob({
        id: "scan",
        type: "scan",
        label: "Web scan",
        progress: 10,
        detail: "x",
        target: { restoreScan: true, projectId: 1, url: "https://example.com" },
      });
    });
    expect(result.current).toBe(withTarget);

    // A changed field publishes: new reference, updated value.
    act(() => {
      addJob({ id: "scan", type: "scan", label: "Web scan", progress: 20, detail: "x" });
    });
    expect(result.current).not.toBe(withTarget);
    expect(result.current[0].progress).toBe(20);
  });

  it("re-adding preserves the original startedAt", () => {
    const { result } = renderHook(() => useJobs());
    act(() => {
      addJob({ id: "a", type: "scan", label: "First" });
    });
    currentTime += 5_000;
    act(() => {
      addJob({ id: "a", type: "scan", label: "Second" });
    });
    expect(result.current[0].startedAt).toBe(1_000_000);
  });

  it("updateJob patches running job fields", () => {
    const { result } = renderHook(() => useJobs());
    act(() => {
      addJob({ id: "a", type: "scan", label: "Scan" });
    });
    act(() => {
      updateJob("a", { progress: 42, detail: "halfway" });
    });
    expect(result.current[0].progress).toBe(42);
    expect(result.current[0].detail).toBe("halfway");
  });

  it("updateJob on an unknown id is a no-op", () => {
    const { result } = renderHook(() => useJobs());
    act(() => {
      addJob({ id: "a", type: "scan", label: "Scan" });
    });
    act(() => {
      updateJob("ghost", { progress: 99 });
    });
    expect(result.current[0].progress).toBeUndefined();
  });

  it("removeJob deletes from both running and recent", () => {
    const { result } = renderHook(() => useJobsCenter());
    act(() => {
      addJob({ id: "a", type: "scan", label: "Scan" });
    });
    act(() => {
      completeJob("a");
    });
    expect(result.current.recent).toHaveLength(1);
    act(() => {
      removeJob("a");
    });
    expect(result.current.running).toHaveLength(0);
    expect(result.current.recent).toHaveLength(0);
  });

  it("removeRunningJob only touches running", () => {
    const { result } = renderHook(() => useJobsCenter());
    act(() => {
      addJob({ id: "a", type: "scan", label: "Scan" });
    });
    act(() => {
      removeRunningJob("a");
    });
    expect(result.current.running).toHaveLength(0);
    expect(result.current.recent).toHaveLength(0);
  });

  it("completeJob moves the job from running to recent and marks success", () => {
    const { result } = renderHook(() => useJobsCenter());
    act(() => {
      addJob({ id: "a", type: "scan", label: "Scan" });
    });
    currentTime += 3_000;
    act(() => {
      completeJob("a", { detail: "all good" });
    });
    expect(result.current.running).toHaveLength(0);
    expect(result.current.recent).toHaveLength(1);
    expect(result.current.recent[0].status).toBe("success");
    expect(result.current.recent[0].progress).toBe(100);
    expect(result.current.recent[0].endedAt).toBe(1_003_000);
    expect(result.current.recent[0].detail).toBe("all good");
  });

  it("completeJob preserves the original target when no new target is supplied", () => {
    const { result } = renderHook(() => useJobsCenter());
    act(() => {
      addJob({
        id: "a",
        type: "scan",
        label: "Scan",
        target: {
          page: "issues",
          projectId: 1,
          url: "https://example.com",
          scanId: 12,
          scanKind: "site",
        },
      });
    });
    act(() => {
      completeJob("a", { detail: "done" });
    });
    expect(result.current.recent[0].target).toEqual({
      page: "issues",
      projectId: 1,
      url: "https://example.com",
      scanId: 12,
      scanKind: "site",
    });
  });

  it("completeJob is a no-op when the job is not running", () => {
    const { result } = renderHook(() => useJobsCenter());
    act(() => {
      completeJob("never-started");
    });
    expect(result.current.running).toHaveLength(0);
    expect(result.current.recent).toHaveLength(0);
  });

  it("failJob moves the job to recent with status=error", () => {
    const { result } = renderHook(() => useJobsCenter());
    act(() => {
      addJob({ id: "a", type: "scan", label: "Scan" });
    });
    act(() => {
      failJob("a", { detail: "timeout" });
    });
    expect(result.current.running).toHaveLength(0);
    expect(result.current.recent[0].status).toBe("error");
    expect(result.current.recent[0].detail).toBe("timeout");
  });

  it("recordCompletedJob inserts directly into recent without a running phase", () => {
    const { result } = renderHook(() => useJobsCenter());
    act(() => {
      recordCompletedJob({ id: "a", type: "sync", label: "Sync" });
    });
    expect(result.current.running).toHaveLength(0);
    expect(result.current.recent).toHaveLength(1);
    expect(result.current.recent[0].status).toBe("success");
    expect(result.current.recent[0].progress).toBe(100);
  });

  it("clearJobsByType removes running and recent jobs of that type only", () => {
    const { result } = renderHook(() => useJobsCenter());
    act(() => {
      addJob({ id: "scan-1", type: "scan", label: "Scan" });
      addJob({ id: "probe-1", type: "probes", label: "Probe" });
    });
    act(() => {
      completeJob("probe-1");
    });
    act(() => {
      clearJobsByType("scan");
    });
    expect(result.current.running.map((j) => j.id)).toEqual([]);
    expect(result.current.recent.map((j) => j.id)).toEqual(["probe-1"]);
  });

  it("recent list is capped at 6 entries", () => {
    const { result } = renderHook(() => useJobsCenter());
    for (let i = 0; i < 8; i++) {
      const id = `job-${i}`;
      act(() => {
        recordCompletedJob({ id, type: "sync", label: `Sync ${i}` });
      });
    }
    expect(result.current.recent.length).toBeLessThanOrEqual(6);
  });

  it("useRunningJobsCount ignores progress-only publishes while useJobs sees them", () => {
    let countRenders = 0;
    let listRenders = 0;
    const count = renderHook(() => {
      countRenders += 1;
      return useRunningJobsCount();
    });
    const list = renderHook(() => {
      listRenders += 1;
      return useJobs();
    });

    act(() => {
      addJob({ id: "scan", type: "scan", label: "Web scan", progress: 1, detail: "1 of 124" });
    });
    expect(count.result.current).toBe(1);
    const countRendersAfterStart = countRenders;
    const listRendersAfterStart = listRenders;

    for (let tick = 2; tick <= 11; tick++) {
      act(() => {
        addJob({
          id: "scan",
          type: "scan",
          label: "Web scan",
          progress: tick,
          detail: `${tick} of 124`,
        });
      });
    }

    expect(list.result.current[0].detail).toBe("11 of 124");
    expect(listRenders).toBeGreaterThan(listRendersAfterStart);
    expect(countRenders).toBe(countRendersAfterStart);
    expect(count.result.current).toBe(1);

    // A lifecycle change (job finishes) still reaches the count subscriber.
    act(() => {
      completeJob("scan");
    });
    expect(count.result.current).toBe(0);
    expect(countRenders).toBeGreaterThan(countRendersAfterStart);
  });

  it("recent entries older than the TTL get pruned on the next mutation", () => {
    const { result } = renderHook(() => useJobsCenter());
    act(() => {
      recordCompletedJob({ id: "old", type: "sync", label: "Old" });
    });
    // Jump 20 minutes into the future (TTL is 15m)
    currentTime += 20 * 60 * 1000;
    act(() => {
      recordCompletedJob({ id: "fresh", type: "sync", label: "Fresh" });
    });
    expect(result.current.recent.map((j) => j.id)).toEqual(["fresh"]);
  });
});
