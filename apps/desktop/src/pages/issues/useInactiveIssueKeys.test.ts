import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import { queryKeys } from "@/lib/query/query-keys";
import { fetchInactiveKeys, useInactiveIssueKeys } from "./useInactiveIssueKeys";

describe("fetchInactiveKeys", () => {
  afterEach(() => {
    invokeMock.mockReset();
  });

  it("coalesces concurrent fetches for the same project + URL into one IPC call", async () => {
    invokeMock.mockResolvedValue([]);
    const client = createTestQueryClient();
    const [a, b] = await Promise.all([
      fetchInactiveKeys(client, 1, "https://example.com"),
      fetchInactiveKeys(client, 1, "https://example.com"),
    ]);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(a).toEqual(b);
  });

  it("fetches fresh on every call so post-scan data is never stale", async () => {
    invokeMock.mockResolvedValue([]);
    const client = createTestQueryClient();
    await fetchInactiveKeys(client, 1, "https://example.com");
    await fetchInactiveKeys(client, 1, "https://example.com");
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });
});

describe("useInactiveIssueKeys", () => {
  afterEach(() => {
    invokeMock.mockReset();
  });

  it("keeps the same Set identity when a refresh returns the same members", async () => {
    invokeMock.mockResolvedValue([
      { checkId: "security.csp", status: "blocked" },
      { checkId: "seo.title", status: "ignored" },
      { checkId: "perf.lcp", status: "new" },
    ]);
    const client = createTestQueryClient();
    const { result } = renderHook(() => useInactiveIssueKeys(1, "https://identity.example"), {
      wrapper: withQueryClient(client),
    });

    await waitFor(() => expect(result.current.keys.size).toBe(2));
    const firstKeys = result.current.keys;
    expect([...firstKeys].sort()).toEqual(["security.csp", "seo.title"]);

    await act(async () => {
      await client.invalidateQueries({ queryKey: queryKeys.workItems.all });
    });
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));

    expect(result.current.keys).toBe(firstKeys);
  });

  it("swaps to a new Set when a refresh changes the membership", async () => {
    invokeMock.mockResolvedValueOnce([{ checkId: "security.csp", status: "blocked" }]);
    const client = createTestQueryClient();
    const { result } = renderHook(() => useInactiveIssueKeys(1, "https://membership.example"), {
      wrapper: withQueryClient(client),
    });

    await waitFor(() => expect(result.current.keys.size).toBe(1));
    const firstKeys = result.current.keys;

    invokeMock.mockResolvedValueOnce([
      { checkId: "security.csp", status: "blocked" },
      { checkId: "seo.title", status: "verified" },
    ]);
    await act(async () => {
      await client.invalidateQueries({ queryKey: queryKeys.workItems.all });
    });

    await waitFor(() => expect(result.current.keys.size).toBe(2));
    expect(result.current.keys).not.toBe(firstKeys);
    expect([...result.current.keys].sort()).toEqual(["security.csp", "seo.title"]);
  });

  it("surfaces an initial lifecycle read failure instead of reporting an empty set", async () => {
    invokeMock.mockRejectedValue(new Error("database unavailable"));
    const client = createTestQueryClient();
    const { result } = renderHook(() => useInactiveIssueKeys(1, "https://failure.example"), {
      wrapper: withQueryClient(client),
    });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(result.current.keys.size).toBe(0);
  });
});
