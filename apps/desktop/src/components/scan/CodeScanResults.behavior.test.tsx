import React from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CodeIssue, CodeScanResult } from "@/lib/types";

const commandMocks = vi.hoisted(() => ({
  runScanExecution: vi.fn(),
}));

vi.mock("@/lib/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/commands")>();
  return { ...actual, runScanExecution: commandMocks.runScanExecution };
});

vi.mock("@/hooks/useScanPrefs", () => ({
  useScanPrefs: () => ({ prefs: { retentionLimit: 37 } }),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: vi.fn(() => Promise.resolve(null)) }));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock("@/components/ui/button", () => ({
  Button: ({
    children,
    type = "button",
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) =>
    React.createElement("button", { type, ...props }, children),
}));

vi.mock("@/components/ui/markdown", () => ({
  Markdown: ({ children }: { children: React.ReactNode }) =>
    React.createElement("div", null, children),
}));

vi.mock("@/components/ui/score-ring", () => ({
  ScoreRing: () => null,
}));

vi.mock("@/components/issues/IssueActionBar", () => ({
  IssueActionBar: ({
    extraActions,
    verifyAction,
  }: {
    extraActions?: React.ReactNode;
    verifyAction?: { label: string; onClick: () => void; verifying: boolean };
  }) =>
    React.createElement(
      "div",
      null,
      "IssueActionBar",
      verifyAction
        ? React.createElement(
            "button",
            {
              type: "button",
              disabled: verifyAction.verifying,
              onClick: verifyAction.onClick,
            },
            verifyAction.label,
          )
        : null,
      extraActions,
    ),
}));

vi.mock("@/components/issues/FixWithAgentAction", () => ({
  FixWithAgentAction: ({ onOpenIntegrations }: { onOpenIntegrations?: () => void }) =>
    onOpenIntegrations
      ? React.createElement(
          "button",
          { type: "button", onClick: onOpenIntegrations },
          "Mock agent integrations link",
        )
      : React.createElement("div", null, "Mock agent action without integrations link"),
}));

vi.mock("@/components/issues/IssueDossierPanel", async () => {
  const { buildDossierPanelMock } = await import("@/test-utils/dossier-panel-mock");
  return buildDossierPanelMock({ testId: "code-issue-dossier" });
});

vi.mock("@/components/issues/IssueScopeSummary", () => ({
  IssueScopeInline: ({ detail }: { detail?: string | null }) =>
    React.createElement("div", null, detail),
  IssueScopeSection: () => null,
}));

vi.mock("@/components/issues/IssueMemorySection", () => ({
  IssueMemoryRail: () => null,
}));

vi.mock("@/components/ui/AsyncFixGuideSteps", () => ({
  // Render guide selection as an assertion marker.
  AsyncFixGuideSteps: (props: { checkId: string; baselineOnly?: boolean }) =>
    React.createElement(
      "div",
      null,
      `fix-guide:${props.checkId}:${props.baselineOnly ? "baseline" : "resolved"}`,
    ),
}));

vi.mock("@/lib/clipboard", () => ({
  copyToClipboard: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/lib/desktop-actions", () => ({
  extractDesktopCommands: vi.fn(() => []),
  openPathInEditor: vi.fn(() => Promise.resolve()),
  revealPath: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/lib/issue-scope", () => ({
  getGuardrailIssueScope: vi.fn(() => ({ issueLabel: "Code issue" })),
}));

vi.mock("@/lib/pending-verification", () => ({
  buildPendingVerificationId: vi.fn(() => "pending-id"),
  queuePendingVerification: vi.fn(),
  resolvePendingVerification: vi.fn(),
}));

vi.mock("@/lib/project-summary-signals", () => ({
  primeLatestCodeScanSnapshot: vi.fn(),
}));

import {
  fixHandoffKey,
  resetFixHandoffStoreForTests,
  setFixHandoff,
} from "@/lib/fix-handoff-store";

import { CodeIssueDossier, CodeScanResults } from "./CodeScanResults";

function buildResult(overrides: Partial<CodeScanResult> = {}): CodeScanResult {
  const issues = overrides.issues ?? [];
  return {
    id: 91,
    projectId: 7,
    environmentUrl: "https://example.com",
    checkedAt: "2026-04-15T12:00:00Z",
    framework: "Next.js",
    overallScore: 74,
    durationMs: 1200,
    issueCount: issues.length,
    criticalCount: 0,
    highCount: issues.length,
    mediumCount: 0,
    lowCount: 0,
    domainSummaries: [],
    issues: issues as CodeIssue[],
    ...overrides,
  };
}

function buildIssue(index: number): CodeIssue {
  const label = String(index).padStart(3, "0");
  return {
    id: `code-${label}`,
    checkId: `code_scan.code-${label}`,
    category: "security",
    domain: "security",
    severity: "high",
    title: `Issue ${label}`,
    description: "Raw user input reaches a query.",
    relativePath: `src/db/query-${label}.ts`,
    absolutePath: `/tmp/project/src/db/query-${label}.ts`,
    line: 42,
    sourceExcerpt: "const sql = `select * from users where id = ${id}`;",
    evidence: null,
    whyNow: null,
    likelyFix: null,
    confidence: "high",
    verifyHint: "Confirm parameterization.",
  };
}

describe("CodeScanResults behavior", () => {
  beforeEach(() => {
    commandMocks.runScanExecution.mockReset();
    commandMocks.runScanExecution.mockResolvedValue({
      execution: { codeDetail: null },
      codeResult: buildResult({ id: 92, issues: [], issueCount: 0, highCount: 0 }),
    });
  });

  it("shows the real empty state when the latest Code Scan finds no issues", () => {
    render(
      <CodeScanResults
        result={buildResult({ issues: [], issueCount: 0, highCount: 0 })}
        projectPath="/tmp/project"
      />,
    );

    expect(screen.getByText("No code risks detected")).toBeInTheDocument();
  });

  it("renders the real issue row and opens the dossier when the user selects it", async () => {
    render(
      <CodeScanResults
        result={buildResult({
          issues: [
            {
              id: "code-1",
              checkId: "code_scan.code-1",
              category: "security",
              domain: "security",
              severity: "high",
              title: "Unsafe SQL string interpolation",
              description: "Raw user input reaches a query.",
              relativePath: "src/db/query.ts",
              absolutePath: "/tmp/project/src/db/query.ts",
              line: 42,
              sourceExcerpt: "const sql = `select * from users where id = ${id}`;",
              evidence: null,
              whyNow: null,
              likelyFix: null,
              confidence: "high",
              verifyHint: "Run the query path with a malicious id and confirm parameterization.",
            },
          ],
        })}
        projectPath="/tmp/project"
      />,
    );

    fireEvent.click(screen.getByText("Unsafe SQL string interpolation").closest("button")!);

    const dossier = await screen.findByTestId("code-issue-dossier");
    expect(dossier).toBeInTheDocument();
    expect(within(dossier).getByRole("tab", { name: "Description" })).toBeInTheDocument();
    expect(within(dossier).getByRole("tab", { name: "Locations (1)" })).toBeInTheDocument();
    const howToFixTab = within(dossier).getByRole("tab", { name: "How to fix" });
    expect(howToFixTab).toBeInTheDocument();
    fireEvent.click(howToFixTab);
    expect(within(dossier).getByText(/how to check it/i)).toBeInTheDocument();
    expect(within(dossier).queryByText(/last checked/i)).not.toBeInTheDocument();
  });

  it("shows complete remediation depth to every install", async () => {
    const issue = {
      ...buildIssue(1),
      title: "Unsafe SQL string interpolation",
      producerRuleId: "raw-sql-unsafe",
      sourceExcerpt: "const sql = `select * from users where id = ${id}`;",
      likelyFix: "Replace the interpolation with a parameterized query.",
      verifyHint: "Run the query path with a malicious id and confirm parameterization.",
    };

    render(
      <CodeScanResults result={buildResult({ issues: [issue] })} projectPath="/tmp/project" />,
    );

    expect(screen.getByText("Unsafe SQL string interpolation")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Unsafe SQL string interpolation").closest("button")!);

    const dossier = await screen.findByTestId("code-issue-dossier");
    expect(within(dossier).getByRole("tab", { name: "Locations (1)" })).toBeInTheDocument();

    fireEvent.click(within(dossier).getByRole("tab", { name: "How to fix" }));
    // The catalog-resolved guide is requested, never the baseline-only path,
    // and the per-finding verification hint renders alongside it.
    expect(within(dossier).getByText("fix-guide:raw-sql-unsafe:resolved")).toBeInTheDocument();
    expect(within(dossier).getByText(issue.verifyHint)).toBeInTheDocument();

    fireEvent.click(within(dossier).getByRole("tab", { name: "Evidence" }));
    expect(within(dossier).getByText(issue.sourceExcerpt)).toBeInTheDocument();
    expect(within(dossier).queryByText("CodeScanLockedCallout")).not.toBeInTheDocument();
  });

  it("applies the saved retention preference to Code verification", async () => {
    const issue = buildIssue(1);
    render(
      <CodeScanResults result={buildResult({ issues: [issue] })} projectPath="/tmp/project" />,
    );

    fireEvent.click(screen.getByText(issue.title).closest("button")!);
    fireEvent.click(await screen.findByRole("button", { name: "Verify" }));

    await waitFor(() => expect(commandMocks.runScanExecution).toHaveBeenCalledTimes(1));
    expect(commandMocks.runScanExecution).toHaveBeenCalledWith({
      request: expect.objectContaining({
        requestedMode: "code",
        retention: 37,
        trigger: "verification",
      }),
    });
  });

  it("shows the grouped source count in the Locations tab", () => {
    const primary = buildIssue(1);
    const related = {
      ...buildIssue(2),
      checkId: primary.checkId,
      title: primary.title,
    };

    render(
      <CodeIssueDossier
        issue={primary}
        groupedIssues={[primary, related]}
        projectId={7}
        scanUrl="https://example.com"
        projectPath="/tmp/project"
        onVerify={vi.fn()}
        verifying={false}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByRole("tab", { name: "Locations (2)" })).toBeInTheDocument();
  });

  it("shows the evidence limitation behind a code finding's confidence", async () => {
    render(
      <CodeScanResults
        result={buildResult({
          issues: [
            {
              ...buildIssue(1),
              confidence: "needs_review",
              confidenceReason:
                "Static matching does not prove unsanitized runtime flow or reachability.",
            },
          ],
        })}
        projectPath="/tmp/project"
      />,
    );

    fireEvent.click(screen.getByText("Issue 001").closest("button")!);

    const dossier = await screen.findByTestId("code-issue-dossier");
    expect(dossier).toHaveTextContent("Needs review");
    expect(within(dossier).getAllByText("Confidence", { exact: true })).toHaveLength(1);
    expect(dossier).not.toHaveTextContent("Why this confidence");
    expect(dossier).toHaveTextContent(
      "Static matching does not prove unsanitized runtime flow or reachability.",
    );
  });

  it("threads onNavigate into the dossier so the agent handoff can open Integrations", async () => {
    const onNavigate = vi.fn();
    render(
      <CodeScanResults
        result={buildResult({
          issues: [
            {
              id: "code-1",
              checkId: "code_scan.code-1",
              category: "security",
              domain: "security",
              severity: "high",
              title: "Unsafe SQL string interpolation",
              description: "Raw user input reaches a query.",
              relativePath: "src/db/query.ts",
              absolutePath: "/tmp/project/src/db/query.ts",
              line: 42,
              sourceExcerpt: "const sql = `select * from users where id = ${id}`;",
              evidence: null,
              whyNow: null,
              likelyFix: null,
              confidence: "high",
              verifyHint: "Confirm parameterization.",
            },
          ],
        })}
        projectPath="/tmp/project"
        onNavigate={onNavigate}
      />,
    );

    fireEvent.click(screen.getByText("Unsafe SQL string interpolation").closest("button")!);

    fireEvent.click(await screen.findByText("Mock agent integrations link"));
    expect(onNavigate).toHaveBeenCalledWith("integrations");
  });

  it("pages issue rows at 100 per page with group headers keeping full counts", async () => {
    const issues = Array.from({ length: 150 }, (_, index) => buildIssue(index + 1));
    render(<CodeScanResults result={buildResult({ issues })} projectPath="/tmp/project" />);

    expect(screen.getAllByText(/^Issue \d{3}$/)).toHaveLength(100);
    expect(screen.getByText("Issue 001")).toBeInTheDocument();
    expect(screen.queryByText("Issue 101")).not.toBeInTheDocument();
    expect(screen.getByText("Security - 150 Issues")).toBeInTheDocument();
    expect(screen.getByLabelText("Page 1")).toHaveAttribute("aria-current", "page");
    // The first page has nowhere to go back to, so the step is absent.
    expect(screen.queryByLabelText("Previous issues page")).not.toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Next issues page"));

    expect(screen.getAllByText(/^Issue \d{3}$/)).toHaveLength(50);
    expect(screen.getByText("Issue 101")).toBeInTheDocument();
    expect(screen.queryByText("Issue 001")).not.toBeInTheDocument();
    expect(screen.getByText("Security - 150 Issues")).toBeInTheDocument();
    expect(screen.getByLabelText("Page 2")).toHaveAttribute("aria-current", "page");
    expect(screen.queryByLabelText("Next issues page")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText("Issue 150").closest("button")!);
    expect(await screen.findByTestId("code-issue-dossier")).toBeInTheDocument();
  });

  it("does not render pagination controls at or below one page of issues", () => {
    const issues = Array.from({ length: 100 }, (_, index) => buildIssue(index + 1));
    render(<CodeScanResults result={buildResult({ issues })} projectPath="/tmp/project" />);

    expect(screen.getAllByText(/^Issue \d{3}$/)).toHaveLength(100);
    expect(screen.queryByLabelText("Next issues page")).not.toBeInTheDocument();
  });

  it("passes no integrations link to the agent handoff when onNavigate is absent", async () => {
    render(
      <CodeScanResults
        result={buildResult({
          issues: [
            {
              id: "code-1",
              checkId: "code_scan.code-1",
              category: "security",
              domain: "security",
              severity: "high",
              title: "Unsafe SQL string interpolation",
              description: "Raw user input reaches a query.",
              relativePath: "src/db/query.ts",
              absolutePath: "/tmp/project/src/db/query.ts",
              line: 42,
              sourceExcerpt: "const sql = `select * from users where id = ${id}`;",
              evidence: null,
              whyNow: null,
              likelyFix: null,
              confidence: "high",
              verifyHint: "Confirm parameterization.",
            },
          ],
        })}
        projectPath="/tmp/project"
      />,
    );

    fireEvent.click(screen.getByText("Unsafe SQL string interpolation").closest("button")!);

    expect(
      await screen.findByText("Mock agent action without integrations link"),
    ).toBeInTheDocument();
  });

  describe("when a refreshed result drops the open issue", () => {
    beforeEach(() => {
      resetFixHandoffStoreForTests();
    });

    it("closes the dossier when nothing is waiting on that issue", async () => {
      const issue = buildIssue(1);
      const { rerender } = render(
        <CodeScanResults result={buildResult({ issues: [issue] })} projectPath="/tmp/project" />,
      );
      fireEvent.click(screen.getByText(issue.title).closest("button")!);
      expect(await screen.findByTestId("code-issue-dossier")).toBeInTheDocument();

      rerender(
        <CodeScanResults result={buildResult({ id: 92, issues: [] })} projectPath="/tmp/project" />,
      );

      expect(screen.queryByTestId("code-issue-dossier")).not.toBeInTheDocument();
    });

    it("holds the dossier open while a fix handoff for it is still running", async () => {
      const issue = buildIssue(1);
      const { rerender } = render(
        <CodeScanResults result={buildResult({ issues: [issue] })} projectPath="/tmp/project" />,
      );
      fireEvent.click(screen.getByText(issue.title).closest("button")!);
      expect(await screen.findByTestId("code-issue-dossier")).toBeInTheDocument();

      setFixHandoff(fixHandoffKey(7, "https://example.com", issue.checkId), {
        mode: "handoff",
        tool: "claude-code",
        phase: "opened",
        attemptId: 2,
      });

      rerender(
        <CodeScanResults result={buildResult({ id: 92, issues: [] })} projectPath="/tmp/project" />,
      );

      expect(screen.getByTestId("code-issue-dossier")).toBeInTheDocument();
      expect(screen.getByText(issue.title)).toBeInTheDocument();
    });
  });
});
