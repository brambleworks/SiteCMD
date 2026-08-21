import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }));

import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import { useRelatedCodeIssues } from "./useRelatedCodeIssues";
import type { CodeIssue, CodeScanReport, FixLocation, Severity } from "@/lib/types";

function codeIssue(relativePath: string, severity: Severity): CodeIssue {
  return {
    id: `${relativePath}:${severity}`,
    checkId: "code.example",
    category: "security",
    domain: "security",
    severity,
    title: `Issue ${relativePath}`,
    description: "desc",
    relativePath,
    absolutePath: `/abs/${relativePath}`,
    line: null,
    sourceExcerpt: null,
    evidence: null,
    whyNow: null,
    likelyFix: null,
    confidence: "high",
    verifyHint: null,
  };
}

function report(issues: CodeIssue[]): CodeScanReport {
  return {
    checkedAt: "2026-05-19T12:00:00Z",
    framework: null,
    issueCount: issues.length,
    criticalCount: 0,
    highCount: 0,
    mediumCount: 0,
    lowCount: 0,
    issues,
  };
}

function fixLocation(relativePath: string): FixLocation {
  return {
    label: relativePath,
    reason: "match",
    relativePath,
    absolutePath: `/abs/${relativePath}`,
  };
}

describe("useRelatedCodeIssues", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("keeps only issues on the correlated files, ordered by severity, capped at four", async () => {
    invokeMock.mockResolvedValue(
      report([
        codeIssue("src/a.ts", "low"),
        codeIssue("src/a.ts", "critical"),
        codeIssue("src/a.ts", "high"),
        codeIssue("src/a.ts", "medium"),
        codeIssue("src/a.ts", "high"),
        // Off the correlated files - must never surface.
        codeIssue("src/elsewhere.ts", "critical"),
      ]),
    );

    const { result } = renderHook(
      () =>
        useRelatedCodeIssues({
          correlatedFiles: [fixLocation("src/a.ts")],
          projectId: 1,
          projectPath: "/tmp/app",
        }),
      { wrapper: withQueryClient(createTestQueryClient()) },
    );

    await waitFor(() => expect(result.current).toHaveLength(4));
    expect(result.current.map((issue) => issue.severity)).toEqual([
      "critical",
      "high",
      "high",
      "medium",
    ]);
    expect(result.current.every((issue) => issue.relativePath === "src/a.ts")).toBe(true);
  });

  it("runs the audit once when two dossiers share the same project path", async () => {
    invokeMock.mockResolvedValue(report([codeIssue("src/a.ts", "high")]));
    const wrapper = withQueryClient(createTestQueryClient());

    const { result } = renderHook(
      () => ({
        first: useRelatedCodeIssues({
          correlatedFiles: [fixLocation("src/a.ts")],
          projectId: 1,
          projectPath: "/tmp/app",
        }),
        second: useRelatedCodeIssues({
          correlatedFiles: [fixLocation("src/a.ts")],
          projectId: 1,
          projectPath: "/tmp/app",
        }),
      }),
      { wrapper },
    );

    await waitFor(() => expect(result.current.first).toHaveLength(1));
    expect(result.current.second).toHaveLength(1);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("run_code_scan_audit", {
      projectId: 1,
      projectPath: "/tmp/app",
      inspectLocalDatabases: false,
    });
  });

  it("never runs the audit when there are no correlated files", () => {
    invokeMock.mockResolvedValue(report([]));

    const { result } = renderHook(
      () => useRelatedCodeIssues({ correlatedFiles: [], projectId: 1, projectPath: "/tmp/app" }),
      { wrapper: withQueryClient(createTestQueryClient()) },
    );

    expect(result.current).toHaveLength(0);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("stays empty when the audit fails rather than surfacing a partial view", async () => {
    invokeMock.mockRejectedValue(new Error("audit blew up"));

    const { result } = renderHook(
      () =>
        useRelatedCodeIssues({
          correlatedFiles: [fixLocation("src/a.ts")],
          projectId: 1,
          projectPath: "/tmp/app",
        }),
      { wrapper: withQueryClient(createTestQueryClient()) },
    );

    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    expect(result.current).toHaveLength(0);
  });
});
