import { beforeEach, describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import {
  clearUpdatesBadgeForProject,
  setEnabledIntegrations,
  setUpdatesBadge,
  useNavBadges,
  useNavIntegrations,
} from "./nav-badges";

describe("nav-badges store", () => {
  beforeEach(() => {
    setUpdatesBadge(null);
  });

  it("starts with no updates badge", () => {
    const { result } = renderHook(() => useNavBadges(1));
    expect(result.current.updates).toBeNull();
  });

  it("setUpdatesBadge publishes the new badge to subscribers", () => {
    const { result } = renderHook(() => useNavBadges(1));
    act(() => {
      setUpdatesBadge({ projectId: 1, total: 5, critical: 2 });
    });
    expect(result.current.updates).toEqual({ projectId: 1, total: 5, critical: 2 });
  });

  it("is a no-op when called with identical values (prevents extra re-renders)", () => {
    const { result } = renderHook(() => useNavBadges(1));
    act(() => {
      setUpdatesBadge({ projectId: 1, total: 5, critical: 2 });
    });
    const first = result.current;
    act(() => {
      setUpdatesBadge({ projectId: 1, total: 5, critical: 2 });
    });
    // Snapshot identity is stable when nothing changed
    expect(result.current).toBe(first);
  });

  it("setUpdatesBadge(null) clears the badge", () => {
    const { result } = renderHook(() => useNavBadges(1));
    act(() => {
      setUpdatesBadge({ projectId: 1, total: 5, critical: 2 });
    });
    act(() => {
      setUpdatesBadge(null);
    });
    expect(result.current.updates).toBeNull();
  });

  it("clearUpdatesBadgeForProject only clears the matching project", () => {
    const { result } = renderHook(() => useNavBadges(7));
    act(() => {
      setUpdatesBadge({ projectId: 7, total: 3, critical: 0 });
    });
    act(() => {
      clearUpdatesBadgeForProject(99); // different project - no-op
    });
    expect(result.current.updates?.projectId).toBe(7);

    act(() => {
      clearUpdatesBadgeForProject(7);
    });
    expect(result.current.updates).toBeNull();
  });

  it("retains last-known badges for other projects", () => {
    const { result: alpha } = renderHook(() => useNavBadges(1));
    const { result: beta } = renderHook(() => useNavBadges(2));

    act(() => {
      setUpdatesBadge({ projectId: 1, total: 5, critical: 2 });
      setUpdatesBadge({ projectId: 2, total: 1, critical: 0 });
    });

    expect(alpha.current.updates).toEqual({ projectId: 1, total: 5, critical: 2 });
    expect(beta.current.updates).toEqual({ projectId: 2, total: 1, critical: 0 });
  });
});

describe("nav-badges store: enabled integrations", () => {
  // Distinct project ids per test keep the module-level store from bleeding
  // across cases (there is no global clear for the integration slice).
  it("starts with an empty integration set", () => {
    const { result } = renderHook(() => useNavIntegrations(30));
    expect(result.current.size).toBe(0);
  });

  it("publishes connected integrations to subscribers", () => {
    const { result } = renderHook(() => useNavIntegrations(31));
    act(() => {
      setEnabledIntegrations(31, ["plausible", "github"]);
    });
    expect([...result.current].sort()).toEqual(["github", "plausible"]);
  });

  it("keeps integration sets isolated per project", () => {
    const { result: alpha } = renderHook(() => useNavIntegrations(32));
    const { result: beta } = renderHook(() => useNavIntegrations(33));
    act(() => {
      setEnabledIntegrations(32, ["plausible"]);
    });
    expect(alpha.current.has("plausible")).toBe(true);
    expect(beta.current.size).toBe(0);
  });

  it("is a no-op when membership is unchanged regardless of order", () => {
    const { result } = renderHook(() => useNavIntegrations(34));
    act(() => {
      setEnabledIntegrations(34, ["plausible", "github"]);
    });
    const first = result.current;
    act(() => {
      // Same members, reordered: must not republish, so the derived set is stable.
      setEnabledIntegrations(34, ["github", "plausible"]);
    });
    expect(result.current).toBe(first);
  });

  it("republishes when membership actually changes", () => {
    const { result } = renderHook(() => useNavIntegrations(35));
    act(() => {
      setEnabledIntegrations(35, ["plausible"]);
    });
    act(() => {
      setEnabledIntegrations(35, ["plausible", "github"]);
    });
    expect(result.current.has("github")).toBe(true);
  });
});
