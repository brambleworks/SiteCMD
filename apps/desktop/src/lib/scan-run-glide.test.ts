import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ScanProgressEvent } from "@/hooks/useScan";
import {
  beginScanRun,
  publishScanProgress,
  readScanRunPercent,
  resetScanProgress,
} from "@/lib/scan-progress-store";
import { useScanRunPercent } from "@/components/scan/useScanRunPercent";
import { useScanRunWholePercent } from "@/components/scan/useScanRunWholePercent";
import { stepToward } from "./scan-run-glide";

function ev(check_id: string, status: ScanProgressEvent["status"]): ScanProgressEvent {
  return {
    check_id,
    category: "config",
    status,
    results_count: 0,
    checks_done: 0,
    checks_total: 0,
  };
}

function setVisibility(state: DocumentVisibilityState) {
  Object.defineProperty(document, "visibilityState", { configurable: true, get: () => state });
  document.dispatchEvent(new Event("visibilitychange"));
}

describe("stepToward", () => {
  it("glides toward a distant target at no more than four points per tick", () => {
    expect(stepToward(0, 80)).toBe(4);
    expect(stepToward(80, 0)).toBe(76);
  });

  it("eases in as the gap closes instead of holding one speed", () => {
    const wide = stepToward(0, 10) - 0;
    const narrow = stepToward(8, 10) - 8;
    expect(wide).toBeGreaterThan(narrow);
    expect(narrow).toBeGreaterThan(0);
  });

  it("finishes a glide instead of crawling asymptotically", () => {
    let current = 0;
    let ticks = 0;
    while (current !== 30 && ticks < 200) {
      current = stepToward(current, 30);
      ticks += 1;
    }
    expect(current).toBe(30);
    expect(ticks).toBeLessThan(60);
  });

  it("holds still on a target it has already reached", () => {
    expect(stepToward(42.1, 42)).toBe(42);
    expect(stepToward(42, 42)).toBe(42);
  });
});

describe("scan run glide clock", () => {
  afterEach(() => {
    Reflect.deleteProperty(document, "visibilityState");
    vi.restoreAllMocks();
    vi.useRealTimers();
    resetScanProgress();
  });

  it("runs one clock for every subscriber and stops it when the last one leaves", () => {
    vi.useFakeTimers();
    const setInterval = vi.spyOn(window, "setInterval");
    const clearInterval = vi.spyOn(window, "clearInterval");
    beginScanRun({ web: "health", code: false });

    const ring = renderHook(() => useScanRunPercent());
    const bar = renderHook(() => useScanRunPercent());
    const footer = renderHook(() => useScanRunWholePercent());
    expect(setInterval).toHaveBeenCalledTimes(1);

    ring.unmount();
    bar.unmount();
    expect(clearInterval).not.toHaveBeenCalled();
    footer.unmount();
    expect(clearInterval).toHaveBeenCalledTimes(1);
  });

  it("wakes no subscriber while the ring holds still", () => {
    vi.useFakeTimers();
    beginScanRun({ web: "health", code: true });
    // The web step is done and holds at 100 until the first code event.
    publishScanProgress(ev("browser-analysis", "complete"));
    let renders = 0;
    const { result } = renderHook(() => {
      renders += 1;
      return useScanRunPercent();
    });
    expect(result.current).toBe(100);
    const settled = renders;

    act(() => {
      vi.advanceTimersByTime(2_000);
    });
    expect(renders).toBe(settled);
  });

  it("re-renders the whole-number hook only when the number changes", () => {
    vi.useFakeTimers();
    beginScanRun({ web: "health", code: false });
    publishScanProgress(ev("fetch", "running"));
    let fractional = 0;
    let whole = 0;
    renderHook(() => {
      fractional += 1;
      return useScanRunPercent();
    });
    renderHook(() => {
      whole += 1;
      return useScanRunWholePercent();
    });

    // One tick per act: React folds every notification inside one act into
    // a single render, so the count is only meaningful per tick.
    for (let tick = 0; tick < 40; tick += 1) {
      act(() => {
        vi.advanceTimersByTime(50);
      });
    }
    expect(fractional).toBeGreaterThan(30);
    expect(whole).toBeLessThan(fractional / 3);
  });

  it("stops the clock while the window is hidden and snaps to the model on return", () => {
    vi.useFakeTimers();
    beginScanRun({ web: "health", code: false });
    publishScanProgress(ev("fetch", "running"));
    let renders = 0;
    const { result } = renderHook(() => {
      renders += 1;
      return useScanRunPercent();
    });

    act(() => {
      setVisibility("hidden");
    });
    const hiddenAt = renders;
    act(() => {
      vi.advanceTimersByTime(5_000);
    });
    expect(renders).toBe(hiddenAt);

    act(() => {
      setVisibility("visible");
    });
    expect(result.current).toBeGreaterThan(5);
    expect(result.current).toBeCloseTo(readScanRunPercent(), 5);
  });
});
