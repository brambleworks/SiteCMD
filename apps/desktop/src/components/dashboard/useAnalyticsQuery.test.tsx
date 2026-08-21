import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }));
// The snapshot cache mirrors to the Tauri Store; stub it so the persistence tier
// is pure localStorage under test.
vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import { __resetAnalyticsSnapshotCacheForTests } from "@/lib/analytics-snapshot-cache";
import { useAnalyticsQuery } from "./useAnalyticsQuery";

const SNAPSHOT_KEY = "7:30d:https://example.com:traffic";

function fetchCount() {
  return invokeMock.mock.calls.filter(([name]) => name === "fetch_analytics").length;
}

function useTrafficQuery() {
  return useAnalyticsQuery({
    projectId: 7,
    period: "30d",
    siteUrl: "https://example.com",
    snapshotKey: SNAPSHOT_KEY,
  });
}

describe("useAnalyticsQuery", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    window.localStorage.clear();
    __resetAnalyticsSnapshotCacheForTests();
  });

  it("dedupes two observers on the same key to a single fetch", async () => {
    invokeMock.mockResolvedValue({ plausible: { aggregate: { visitors: 10 } } });
    const wrapper = withQueryClient(createTestQueryClient());

    const { result } = renderHook(() => ({ a: useTrafficQuery(), b: useTrafficQuery() }), {
      wrapper,
    });

    await waitFor(() => expect(result.current.a.data).not.toBeNull());
    expect(result.current.b.data).toEqual(result.current.a.data);
    expect(fetchCount()).toBe(1);
  });

  it("persists on success so a fresh client hydrates without re-fetching", async () => {
    invokeMock.mockResolvedValue({ plausible: { aggregate: { visitors: 10 } } });

    const first = renderHook(() => useTrafficQuery(), {
      wrapper: withQueryClient(createTestQueryClient()),
    });
    await waitFor(() => expect(first.result.current.data).not.toBeNull());
    first.unmount();
    invokeMock.mockClear();

    // A fresh client (a reload) starts with an empty in-memory cache; the
    // still-fresh snapshot seeds it instantly, so no second fetch is issued.
    const reloaded = renderHook(() => useTrafficQuery(), {
      wrapper: withQueryClient(createTestQueryClient()),
    });

    expect(reloaded.result.current.data).toEqual({ plausible: { aggregate: { visitors: 10 } } });
    expect(fetchCount()).toBe(0);
  });

  it("busts the backend cache and refetches on refresh", async () => {
    invokeMock.mockImplementation((name: string) => {
      if (name === "invalidate_analytics_cache") return Promise.resolve(null);
      return Promise.resolve({ plausible: { aggregate: { visitors: 10 } } });
    });
    const { result } = renderHook(() => useTrafficQuery(), {
      wrapper: withQueryClient(createTestQueryClient()),
    });
    await waitFor(() => expect(result.current.data).not.toBeNull());
    expect(fetchCount()).toBe(1);

    await act(async () => {
      await result.current.refresh();
    });

    expect(invokeMock).toHaveBeenCalledWith("invalidate_analytics_cache", { projectId: 7 });
    await waitFor(() => expect(fetchCount()).toBe(2));
  });

  it("resolves a no-integrations error to the empty state, not a failure", async () => {
    invokeMock.mockRejectedValue(
      "No analytics integrations configured. Connect an analytics service on the Settings page.",
    );
    const { result } = renderHook(() => useTrafficQuery(), {
      wrapper: withQueryClient(createTestQueryClient()),
    });

    await waitFor(() => expect(result.current.data).toEqual({}));
    expect(result.current.isError).toBe(false);
  });
});
