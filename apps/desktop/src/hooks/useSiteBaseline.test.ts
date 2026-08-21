import { renderHook, waitFor } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { withQueryClient } from "@/test-utils/query-client";
import type { SiteBaselineField } from "@/generated/ipc-bindings";

const mockGetSiteId = vi.fn();
const mockGetBaseline = vi.fn();
const mockDecide = vi.fn();

vi.mock("@/lib/commands", () => ({
  getOrCreateSiteId: (args: unknown) => mockGetSiteId(args),
  getSiteBaseline: (args: unknown) => mockGetBaseline(args),
  decideSiteBaseline: (args: unknown) => mockDecide(args),
}));

const changedField: SiteBaselineField = {
  field: "security_headers",
  label: "Security headers",
  status: "changed",
  origin: "Recorded from the first scan that saw it",
  recordedAt: 1,
  goodLines: ["x-frame-options: DENY"],
  changedLines: ["x-frame-options: SAMEORIGIN"],
  changeDigest: "abc123",
  canDismiss: true,
  changeFirstSeenAt: 2,
};

describe("useSiteBaseline", () => {
  beforeEach(() => {
    mockGetSiteId.mockReset().mockResolvedValue(7);
    mockGetBaseline.mockReset().mockResolvedValue({ revision: 4, fields: [changedField] });
    mockDecide.mockReset().mockResolvedValue({
      applied: true,
      refusal: "",
      message: "",
      revision: 5,
    });
  });

  it("reads nothing until a site url exists", async () => {
    const { useSiteBaseline } = await import("./useSiteBaseline");
    renderHook(() => useSiteBaseline(null), { wrapper: withQueryClient() });

    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(mockGetSiteId).not.toHaveBeenCalled();
    expect(mockGetBaseline).not.toHaveBeenCalled();
  });

  it("sends the revision and digest the person was looking at", async () => {
    const { useSiteBaseline } = await import("./useSiteBaseline");
    const { result } = renderHook(() => useSiteBaseline("https://example.com"), {
      wrapper: withQueryClient(),
    });
    await waitFor(() => expect(result.current.baseline?.revision).toBe(4));

    await act(async () => {
      result.current.decide(changedField, true);
    });

    await waitFor(() =>
      expect(mockDecide).toHaveBeenCalledWith({
        siteId: 7,
        field: "security_headers",
        basedOnRevision: 4,
        expectedDigest: "abc123",
        accept: true,
        projectId: undefined,
        environmentScopeKey: "https://example.com",
      }),
    );
  });

  it("does not reuse a baseline across project environments that share a local site id", async () => {
    const { useSiteBaseline } = await import("./useSiteBaseline");
    const { rerender } = renderHook(
      ({ siteUrl, projectId }: { siteUrl: string; projectId: number }) =>
        useSiteBaseline(siteUrl, projectId),
      {
        initialProps: { siteUrl: "https://staging.example.com", projectId: 10 },
        wrapper: withQueryClient(),
      },
    );
    await waitFor(() =>
      expect(mockGetBaseline).toHaveBeenCalledWith({
        environmentScopeKey: "https://staging.example.com",
        projectId: 10,
        siteId: 7,
      }),
    );

    rerender({ siteUrl: "https://example.com", projectId: 11 });

    await waitFor(() =>
      expect(mockGetBaseline).toHaveBeenCalledWith({
        environmentScopeKey: "https://example.com",
        projectId: 11,
        siteId: 7,
      }),
    );
    expect(mockGetBaseline).toHaveBeenCalledTimes(2);
  });

  it("surfaces a refusal instead of pretending the decision landed", async () => {
    mockDecide.mockResolvedValue({
      applied: false,
      refusal: "stale_revision",
      message: "The site changed again while this was open.",
      revision: 6,
    });
    const { useSiteBaseline } = await import("./useSiteBaseline");
    const { result } = renderHook(() => useSiteBaseline("https://example.com"), {
      wrapper: withQueryClient(),
    });
    await waitFor(() => expect(result.current.baseline?.revision).toBe(4));

    await act(async () => {
      result.current.decide(changedField, true);
    });

    await waitFor(() =>
      expect(result.current.refusal).toBe("The site changed again while this was open."),
    );
  });

  it("dismissing takes the same guard as accepting", async () => {
    const { useSiteBaseline } = await import("./useSiteBaseline");
    const { result } = renderHook(() => useSiteBaseline("https://example.com"), {
      wrapper: withQueryClient(),
    });
    await waitFor(() => expect(result.current.baseline?.revision).toBe(4));

    await act(async () => {
      result.current.decide(changedField, false);
    });

    await waitFor(() =>
      expect(mockDecide).toHaveBeenCalledWith(
        expect.objectContaining({ accept: false, basedOnRevision: 4, expectedDigest: "abc123" }),
      ),
    );
  });
});
