import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { withQueryClient } from "@/test-utils/query-client";

const getPageIssues = vi.fn();
vi.mock("@/lib/issues", () => ({
  getPageIssues: (...args: unknown[]) => getPageIssues(...args),
}));

import { useIssuesPageGroups } from "./useIssuesPageGroups";

describe("useIssuesPageGroups", () => {
  beforeEach(() => getPageIssues.mockReset());

  it("stays empty and does not fetch when no page is selected", () => {
    const { result } = renderHook(
      () => useIssuesPageGroups({ projectId: 1, selectedPageUrl: null, url: "https://a.com" }),
      { wrapper: withQueryClient() },
    );
    expect(result.current.pageGroups).toEqual([]);
    expect(getPageIssues).not.toHaveBeenCalled();
  });

  it("loads the selected page's issue groups", async () => {
    const groups = [{ id: "g1" }];
    getPageIssues.mockResolvedValue(groups);
    const { result } = renderHook(
      () =>
        useIssuesPageGroups({
          projectId: 7,
          selectedPageUrl: "https://a.com/p",
          url: "https://a.com",
        }),
      { wrapper: withQueryClient() },
    );
    await waitFor(() => expect(result.current.pageGroups).toEqual(groups));
    expect(getPageIssues).toHaveBeenCalledWith(7, expect.any(String), "https://a.com/p");
  });

  it("reports a later fetch failure instead of presenting an empty page", async () => {
    getPageIssues.mockResolvedValueOnce([{ id: "g1" }]);
    const { result, rerender } = renderHook((props) => useIssuesPageGroups(props), {
      initialProps: {
        projectId: 1,
        selectedPageUrl: "https://a.com/p1" as string | null,
        url: "https://a.com",
      },
      wrapper: withQueryClient(),
    });
    await waitFor(() => expect(result.current.pageGroups).toEqual([{ id: "g1" }]));

    getPageIssues.mockRejectedValueOnce(new Error("boom"));
    rerender({ projectId: 1, selectedPageUrl: "https://a.com/p2", url: "https://a.com" });
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.pageGroups).toEqual([]);
  });

  it("ignores a stale in-flight result after the selected page changes", async () => {
    let resolveFirst: (value: unknown) => void = () => {};
    getPageIssues.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
    );
    const secondGroups = [{ id: "second" }];
    getPageIssues.mockImplementationOnce(() => Promise.resolve(secondGroups));

    const { result, rerender } = renderHook((props) => useIssuesPageGroups(props), {
      initialProps: {
        projectId: 1,
        selectedPageUrl: "https://a.com/first" as string | null,
        url: "https://a.com",
      },
      wrapper: withQueryClient(),
    });

    // Switch pages before the first fetch resolves.
    rerender({
      projectId: 1,
      selectedPageUrl: "https://a.com/second",
      url: "https://a.com",
    });
    await waitFor(() => expect(result.current.pageGroups).toEqual(secondGroups));

    // Discard the stale first result.
    resolveFirst([{ id: "first" }]);
    await Promise.resolve();
    await Promise.resolve();
    expect(result.current.pageGroups).toEqual(secondGroups);
  });
});
