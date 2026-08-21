import { describe, expect, it, vi, beforeEach } from "vitest";

const { rawInvokeMock } = vi.hoisted(() => ({
  rawInvokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: (...args: unknown[]) => rawInvokeMock(...args) }));

import { getResolvedIssues } from "./resolved-issues";

describe("getResolvedIssues", () => {
  beforeEach(() => {
    rawInvokeMock.mockReset();
  });

  it("invokes get_resolved_issues with url and limit", async () => {
    rawInvokeMock.mockResolvedValue([]);
    await getResolvedIssues(7, "https://example.com", 20);
    expect(rawInvokeMock).toHaveBeenCalledWith("get_resolved_issues", {
      projectId: 7,
      url: "https://example.com",
      limit: 20,
    });
  });

  it("returns parsed resolved issues array", async () => {
    const sample = [
      {
        checkId: "security.csp",
        title: "Missing CSP",
        category: "security",
        severity: "high",
        resolvedScanId: 2,
        resolvedAt: "2026-04-19T00:00:00Z",
        firstSeenScanId: 1,
        firstSeenAt: "2026-04-18T00:00:00Z",
        durationHours: 24.0,
        recurrenceCount: 1,
      },
    ];
    rawInvokeMock.mockResolvedValue(sample);
    const result = await getResolvedIssues(7, "https://example.com", 20);
    expect(result).toEqual(sample);
  });
});
