import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useVisibilityRefresh } from "./useVisibilityRefresh";

function setVisibility(state: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", {
    configurable: true,
    get: () => state,
  });
  document.dispatchEvent(new Event("visibilitychange"));
}

describe("useVisibilityRefresh", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    setVisibility("visible");
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("fires onRefresh when the document becomes visible after being hidden long enough", () => {
    const onRefresh = vi.fn();
    renderHook(() => useVisibilityRefresh({ staleAfterMs: 5 * 60 * 1000, onRefresh }));

    setVisibility("hidden");
    vi.advanceTimersByTime(6 * 60 * 1000);
    setVisibility("visible");

    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("does not fire onRefresh when the hidden interval is shorter than the threshold", () => {
    const onRefresh = vi.fn();
    renderHook(() => useVisibilityRefresh({ staleAfterMs: 5 * 60 * 1000, onRefresh }));

    setVisibility("hidden");
    vi.advanceTimersByTime(60 * 1000);
    setVisibility("visible");

    expect(onRefresh).not.toHaveBeenCalled();
  });

  it("does not fire onRefresh on the initial visible state with no prior hidden transition", () => {
    const onRefresh = vi.fn();
    renderHook(() => useVisibilityRefresh({ staleAfterMs: 5 * 60 * 1000, onRefresh }));

    expect(onRefresh).not.toHaveBeenCalled();
  });

  it("treats a mount-while-hidden state as the start of the hidden window", () => {
    setVisibility("hidden");
    const onRefresh = vi.fn();
    renderHook(() => useVisibilityRefresh({ staleAfterMs: 5 * 60 * 1000, onRefresh }));

    vi.advanceTimersByTime(6 * 60 * 1000);
    setVisibility("visible");

    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("does nothing when disabled", () => {
    const onRefresh = vi.fn();
    renderHook(() =>
      useVisibilityRefresh({ staleAfterMs: 5 * 60 * 1000, onRefresh, enabled: false }),
    );

    setVisibility("hidden");
    vi.advanceTimersByTime(10 * 60 * 1000);
    setVisibility("visible");

    expect(onRefresh).not.toHaveBeenCalled();
  });

  it("calls the latest onRefresh callback even after re-renders", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = renderHook(
      ({ onRefresh }) => useVisibilityRefresh({ staleAfterMs: 5 * 60 * 1000, onRefresh }),
      { initialProps: { onRefresh: first } },
    );

    rerender({ onRefresh: second });

    setVisibility("hidden");
    vi.advanceTimersByTime(6 * 60 * 1000);
    setVisibility("visible");

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledTimes(1);
  });
});
