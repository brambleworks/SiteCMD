import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useBaselineScanQueue } from "./useBaselineScanQueue";

interface HookProps {
  activeProjectId: number | null;
  canScan: boolean;
  runBaselineScan: () => void;
}

function renderQueue(initial: Partial<HookProps> = {}) {
  const runBaselineScan = vi.fn();
  const utils = renderHook((props: HookProps) => useBaselineScanQueue(props), {
    initialProps: {
      activeProjectId: null,
      canScan: false,
      runBaselineScan,
      ...initial,
    },
  });
  return { ...utils, runBaselineScan };
}

describe("useBaselineScanQueue", () => {
  it("does not scan while the created project is not yet the active one", () => {
    const { result, runBaselineScan } = renderQueue({ activeProjectId: 3, canScan: true });

    act(() => {
      result.current.queueBaselineScan(7);
    });

    expect(runBaselineScan).not.toHaveBeenCalled();
  });

  it("runs the scan once the queued project becomes active with an environment", () => {
    const { result, rerender, runBaselineScan } = renderQueue({
      activeProjectId: null,
      canScan: false,
    });

    act(() => {
      result.current.queueBaselineScan(7);
    });
    expect(runBaselineScan).not.toHaveBeenCalled();

    rerender({ activeProjectId: 7, canScan: true, runBaselineScan });

    expect(runBaselineScan).toHaveBeenCalledTimes(1);
  });

  it("fires exactly once, not again on later renders", () => {
    const { result, rerender, runBaselineScan } = renderQueue();

    act(() => {
      result.current.queueBaselineScan(7);
    });
    rerender({ activeProjectId: 7, canScan: true, runBaselineScan });
    rerender({ activeProjectId: 7, canScan: true, runBaselineScan });
    rerender({ activeProjectId: 8, canScan: true, runBaselineScan });
    rerender({ activeProjectId: 7, canScan: true, runBaselineScan });

    expect(runBaselineScan).toHaveBeenCalledTimes(1);
  });

  it("waits until the project has something to scan", () => {
    const { result, rerender, runBaselineScan } = renderQueue();

    act(() => {
      result.current.queueBaselineScan(7);
    });
    rerender({ activeProjectId: 7, canScan: false, runBaselineScan });
    expect(runBaselineScan).not.toHaveBeenCalled();

    rerender({ activeProjectId: 7, canScan: true, runBaselineScan });
    expect(runBaselineScan).toHaveBeenCalledTimes(1);
  });

  it("runs the baseline scan for a code-only project that will never have an environment", () => {
    const { result, rerender, runBaselineScan } = renderQueue();

    act(() => {
      result.current.queueBaselineScan(7);
    });
    rerender({ activeProjectId: 7, canScan: true, runBaselineScan });

    expect(runBaselineScan).toHaveBeenCalledTimes(1);
  });
});
