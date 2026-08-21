import { renderHook, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

const mockGetCurrentScore = vi.fn();

vi.mock("@/lib/current-score", () => ({
  loadCurrentScoreSnapshot: (projectId: unknown, envUrl: unknown) =>
    mockGetCurrentScore(projectId, envUrl),
}));

describe("useCurrentScore", () => {
  beforeEach(() => {
    mockGetCurrentScore.mockReset();
  });

  it("fetches current score on mount", async () => {
    mockGetCurrentScore.mockResolvedValue({
      overall: 87,
      perCategory: {},
      criticalCount: 0,
      highCount: 1,
      mediumCount: 2,
      lowCount: 0,
      computedAt: 123,
    });
    const { useCurrentScore } = await import("./useCurrentScore");
    const { result } = renderHook(() => useCurrentScore(1, "https://example.com"), {
      wrapper: withQueryClient(),
    });
    await waitFor(() => expect(result.current.score?.overall).toBe(87));
    expect(mockGetCurrentScore).toHaveBeenCalledWith(1, "https://example.com");
  });

  it("does not fetch when projectId is null", async () => {
    const { useCurrentScore } = await import("./useCurrentScore");
    renderHook(() => useCurrentScore(null, null), { wrapper: withQueryClient() });
    await new Promise((r) => setTimeout(r, 10));
    expect(mockGetCurrentScore).not.toHaveBeenCalled();
  });

  it("shares the cached score across page consumers", async () => {
    mockGetCurrentScore.mockResolvedValue({
      overall: 90,
      perCategory: {},
      criticalCount: 0,
      highCount: 0,
      mediumCount: 0,
      lowCount: 0,
      computedAt: 456,
    });
    const { useCurrentScore } = await import("./useCurrentScore");
    const client = createTestQueryClient();
    const wrapper = withQueryClient(client);
    const first = renderHook(() => useCurrentScore(1, "https://example.com"), { wrapper });
    await waitFor(() => expect(first.result.current.score?.overall).toBe(90));
    first.unmount();

    const second = renderHook(() => useCurrentScore(1, "https://example.com"), { wrapper });
    expect(second.result.current.score?.overall).toBe(90);
    expect(mockGetCurrentScore).toHaveBeenCalledTimes(1);
  });

  it("keeps refresh identity stable across re-renders", async () => {
    mockGetCurrentScore.mockResolvedValue({
      overall: 72,
      perCategory: {},
      criticalCount: 0,
      highCount: 0,
      mediumCount: 0,
      lowCount: 0,
      computedAt: 789,
    });
    const { useCurrentScore } = await import("./useCurrentScore");
    const { result, rerender } = renderHook(() => useCurrentScore(1, "https://example.com"), {
      wrapper: withQueryClient(),
    });
    await waitFor(() => expect(result.current.score?.overall).toBe(72));

    const firstRefresh = result.current.refresh;
    rerender();
    rerender();
    expect(result.current.refresh).toBe(firstRefresh);
  });
});
