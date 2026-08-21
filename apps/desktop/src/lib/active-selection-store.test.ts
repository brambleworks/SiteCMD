import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getActiveSelection,
  resetActiveSelectionForTest,
  setActiveSelection,
  subscribeActiveSelection,
} from "./active-selection-store";
import { readStoredProjectSelection } from "./project-selection-state";

describe("active-selection-store", () => {
  beforeEach(() => {
    window.localStorage.clear();
    resetActiveSelectionForTest();
  });

  it("starts empty", () => {
    expect(getActiveSelection()).toEqual({ projectId: null, envUrl: null });
  });

  it("commits a selection, normalizes the url, persists it, and reports the change", () => {
    const changed = setActiveSelection(7, "https://example.com/");
    expect(changed).toBe(true);
    // Trailing slash normalized away by normalizeHttpTargetUrl.
    expect(getActiveSelection()).toEqual({ projectId: 7, envUrl: "https://example.com" });
    expect(readStoredProjectSelection()).toEqual({ projectId: 7, envUrl: "https://example.com" });
  });

  it("no-ops on an unchanged key and preserves the snapshot reference", () => {
    setActiveSelection(7, "https://example.com");
    const before = getActiveSelection();
    // Same key after normalization (the trailing slash collapses).
    const changed = setActiveSelection(7, "https://example.com/");
    expect(changed).toBe(false);
    expect(getActiveSelection()).toBe(before);
  });

  it("mints a new snapshot object only on a real change", () => {
    setActiveSelection(7, "https://example.com");
    const first = getActiveSelection();
    setActiveSelection(7, "https://other.example.com");
    const second = getActiveSelection();
    expect(second).not.toBe(first);
    expect(second.envUrl).toBe("https://other.example.com");
  });

  it("notifies subscribers on change and not on a no-op, and stops after unsubscribe", () => {
    const listener = vi.fn();
    const unsubscribe = subscribeActiveSelection(listener);

    setActiveSelection(1, "https://a.example.com");
    expect(listener).toHaveBeenCalledTimes(1);

    setActiveSelection(1, "https://a.example.com");
    expect(listener).toHaveBeenCalledTimes(1);

    setActiveSelection(2, "https://b.example.com");
    expect(listener).toHaveBeenCalledTimes(2);

    unsubscribe();
    setActiveSelection(3, "https://c.example.com");
    expect(listener).toHaveBeenCalledTimes(2);
  });

  it("clears the persisted key when the selection is cleared", () => {
    setActiveSelection(9, "https://example.com");
    expect(readStoredProjectSelection()).not.toBeNull();

    const changed = setActiveSelection(null, null);
    expect(changed).toBe(true);
    expect(getActiveSelection()).toEqual({ projectId: null, envUrl: null });
    expect(readStoredProjectSelection()).toBeNull();
  });
});
