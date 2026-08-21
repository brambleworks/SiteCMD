import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@/lib/tauri-invoke", () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }));

import {
  blockIssue,
  getIssueState,
  ignoreIssue,
  reopenIssue,
  snoozeIssue,
  countActionableCheckResults,
  countPassingCheckResults,
  filterActionableCheckResults,
  formatCheckStatus,
  isActionableCheckStatus,
  isPassingCheckStatus,
  summarizeActionableCheckSeverities,
  summarizeIssueSeverities,
} from "./issues";

describe("issue status helpers", () => {
  it("treats fail and warn as actionable web check statuses", () => {
    expect(isActionableCheckStatus("fail")).toBe(true);
    expect(isActionableCheckStatus("warn")).toBe(true);
    expect(isActionableCheckStatus("pass")).toBe(false);
    expect(isActionableCheckStatus("skipped")).toBe(false);
  });

  it("keeps passing checks separate from actionable checks", () => {
    expect(isPassingCheckStatus("pass")).toBe(true);
    expect(isPassingCheckStatus("warn")).toBe(false);
  });

  it("formats web check statuses from one shared label helper", () => {
    expect(formatCheckStatus("pass")).toBe("Pass");
    expect(formatCheckStatus("fail")).toBe("Fail");
    expect(formatCheckStatus("warn")).toBe("Warn");
    expect(formatCheckStatus("skipped")).toBe("Skipped");
    expect(formatCheckStatus("custom")).toBe("custom");
  });

  it("filters and counts actionable checks consistently", () => {
    const issues = [
      { status: "fail", id: "a" },
      { status: "warn", id: "b" },
      { status: "pass", id: "c" },
      { status: "skipped", id: "d" },
    ];

    expect(filterActionableCheckResults(issues).map((issue) => issue.id)).toEqual(["a", "b"]);
    expect(countActionableCheckResults(issues)).toBe(2);
    expect(countPassingCheckResults(issues)).toBe(1);
  });

  it("summarizes severity counts from one shared helper", () => {
    expect(
      summarizeIssueSeverities([
        { severity: "critical" },
        { severity: "high" },
        { severity: "high" },
        { severity: "medium" },
        { severity: "low" },
        { severity: "unknown" },
        { severity: null },
      ]),
    ).toEqual({ critical: 1, high: 2, medium: 1, low: 1 });
  });

  it("summarizes actionable web check severities without counting passing checks", () => {
    expect(
      summarizeActionableCheckSeverities([
        { status: "fail", severity: "critical" },
        { status: "warn", severity: "high" },
        { status: "pass", severity: "critical" },
        { status: "skipped", severity: "medium" },
      ]),
    ).toEqual({ critical: 1, high: 1, medium: 0, low: 0 });
  });
});

describe("issue lifecycle wrappers", () => {
  beforeEach(() => invokeMock.mockReset());

  it("invokes the issue_states commands with check_id + env_url", async () => {
    invokeMock.mockResolvedValue(undefined);

    await blockIssue(7, "https://example.com", "code_scan.supply_chain_typosquat", "intended");
    expect(invokeMock).toHaveBeenCalledWith("block_issue", {
      projectId: 7,
      envUrl: "https://example.com",
      checkId: "code_scan.supply_chain_typosquat",
      reason: "intended",
    });

    await ignoreIssue(7, "https://example.com", "security.csp");
    expect(invokeMock).toHaveBeenCalledWith("ignore_issue", {
      projectId: 7,
      envUrl: "https://example.com",
      checkId: "security.csp",
    });

    await reopenIssue(7, "https://example.com", "security.csp");
    expect(invokeMock).toHaveBeenCalledWith("reopen_issue", {
      projectId: 7,
      envUrl: "https://example.com",
      checkId: "security.csp",
    });

    await snoozeIssue(7, "https://example.com", "security.csp", 123);
    expect(invokeMock).toHaveBeenCalledWith("snooze_issue", {
      projectId: 7,
      envUrl: "https://example.com",
      checkId: "security.csp",
      snoozeUntil: 123,
    });
  });

  it("getIssueState returns the status tuple or null", async () => {
    invokeMock.mockResolvedValueOnce(["blocked", null, "intended", null]);
    expect(await getIssueState(7, "https://example.com", "security.csp")).toEqual([
      "blocked",
      null,
      "intended",
      null,
    ]);

    // A verified row names who verified it; the badge must never read a user's
    // claim as a scan's proof.
    invokeMock.mockResolvedValueOnce(["verified", null, null, "user_claim"]);
    expect(await getIssueState(7, "https://example.com", "security.hsts")).toEqual([
      "verified",
      null,
      null,
      "user_claim",
    ]);

    invokeMock.mockResolvedValueOnce(null);
    expect(await getIssueState(7, "https://example.com", "seo.title")).toBeNull();
  });
});
