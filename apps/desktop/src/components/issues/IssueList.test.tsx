import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { IssueList } from "./IssueList";
import type { CheckResult, CodeIssue } from "@/lib/types";
import { buildProjectIssueSummary } from "@/lib/project-issue-summary";
import { rankUnified } from "@/lib/issue-ranking";
import { getIssuesSourceFocus, getIssuesWebCategoryFocus } from "@/lib/app-targets";

// Stub the Tauri clipboard used by copyToClipboard.
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: vi.fn().mockResolvedValue(undefined),
}));

// Stub fix-guide loading, which is outside this test's scope.
vi.mock("@/lib/async-fix-guides", () => ({
  loadWebFixGuide: vi.fn().mockResolvedValue(null),
  loadCodeFixGuide: vi.fn().mockResolvedValue(null),
  loadWebBaseline: vi.fn().mockResolvedValue(null),
  loadCodeBaseline: vi.fn().mockResolvedValue(null),
}));

const webIssue: CheckResult = {
  checkId: "missing-hsts",
  category: "security",
  title: "Missing HSTS header",
  description: "Strict-Transport-Security header is not set.",
  status: "fail",
  severity: "high",
  fixPrompt: null,
  manualFix: null,
  rawData: null,
  confidence: "high",
};

const codeIssue: CodeIssue = {
  id: "code-1",
  checkId: "code_scan.code-1",
  category: "security",
  domain: "security",
  severity: "medium",
  title: "Exposed .env file",
  description: ".env file is checked into version control.",
  relativePath: ".env",
  absolutePath: "/tmp/project/.env",
  line: null,
  sourceExcerpt: null,
  evidence: null,
  whyNow: null,
  likelyFix: null,
  confidence: "high",
  verifyHint: null,
};

const databaseCodeIssue: CodeIssue = {
  ...codeIssue,
  id: "code-db-1",
  checkId: "code_scan.code-db-1",
  category: "data",
  domain: "database",
  title: "List endpoint returns unbounded query results",
  relativePath: "db/queries.ts",
  absolutePath: "/tmp/project/db/queries.ts",
  line: 18,
};

function buildIssueSummary(overrides?: { webIssues?: CheckResult[]; codeIssues?: CodeIssue[] }) {
  const webIssues = overrides?.webIssues ?? [];
  const codeIssues = overrides?.codeIssues ?? [];
  return buildProjectIssueSummary({
    webIssues,
    codeIssues,
  });
}

// Tests rank fixtures before passing them to IssueList.
function rankIssues(webIssues: CheckResult[], codeIssues: CodeIssue[]) {
  return rankUnified(webIssues, codeIssues, [], {});
}

function getIssueRowTitles(container: HTMLElement): string[] {
  return Array.from(container.querySelectorAll('button[data-dossier-switch="true"]'))
    .map((button) => button.querySelector(".issue-row-title")?.textContent?.trim() ?? "")
    .filter(Boolean);
}

describe("IssueList", () => {
  it("renders rows exclusively from the rankedIssues prop (no internal re-ranking)", () => {
    render(
      <IssueList
        rankedIssues={[]}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ codeIssues: [codeIssue] })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.queryByText("Exposed .env file")).not.toBeInTheDocument();
    expect(screen.getByText("No web or code issues open")).toBeInTheDocument();
  });

  it("renders web and code issues", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [codeIssue])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: [webIssue], codeIssues: [codeIssue] })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.getByText("Missing HSTS header")).toBeInTheDocument();
    expect(screen.getByText("Exposed .env file")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: /issue source/i })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: /issue subcategory/i })).toBeInTheDocument();
  });

  it("keeps confidence details out of compact issue rows", () => {
    const confirmedIssue: CheckResult = {
      ...webIssue,
      checkId: "confirmed-structure",
      title: "Confirmed structure",
      confidence: "confirmed",
    };
    const reviewIssue: CodeIssue = {
      ...codeIssue,
      id: "code-review",
      checkId: "code_scan.code-review",
      title: "Heuristic review",
      confidence: "needs_review",
    };

    render(
      <IssueList
        rankedIssues={rankIssues([webIssue, confirmedIssue], [reviewIssue])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({
          webIssues: [webIssue, confirmedIssue],
          codeIssues: [reviewIssue],
        })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.queryByText("Confirmed")).not.toBeInTheDocument();
    expect(screen.queryByText("High confidence")).not.toBeInTheDocument();
    expect(screen.queryByText("Needs review")).not.toBeInTheDocument();
  });

  it("renders issue rows without severity left-border accents", () => {
    const { container } = render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: [webIssue] })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    const row = container.querySelector('button[data-dossier-switch="true"]');
    expect(row?.className).not.toMatch(/severity-border|source-border|\bborder-l/);
  });

  it("sorts visible issues by severity before lower-priority tie-breakers", () => {
    const criticalWebIssue: CheckResult = {
      ...webIssue,
      checkId: "security-csp",
      title: "Critical CSP gap",
      severity: "critical",
    };
    const lowWebIssue: CheckResult = {
      ...webIssue,
      checkId: "polish-contrast",
      category: "polish",
      title: "Low polish fix",
      severity: "low",
    };
    const highCodeIssue: CodeIssue = {
      ...codeIssue,
      id: "code-high",
      checkId: "code_scan.code-high",
      title: "High-risk API issue",
      severity: "high",
      relativePath: "src/api.ts",
      absolutePath: "/tmp/project/src/api.ts",
    };
    const mediumCodeIssue: CodeIssue = {
      ...codeIssue,
      id: "code-medium",
      checkId: "code_scan.code-medium",
      title: "Medium database issue",
      severity: "medium",
      relativePath: "src/db.ts",
      absolutePath: "/tmp/project/src/db.ts",
    };

    const { container } = render(
      <IssueList
        rankedIssues={rankIssues([lowWebIssue, criticalWebIssue], [mediumCodeIssue, highCodeIssue])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({
          webIssues: [lowWebIssue, criticalWebIssue],
          codeIssues: [mediumCodeIssue, highCodeIssue],
        })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(getIssueRowTitles(container)).toEqual([
      "Critical CSP gap",
      "High-risk API issue",
      "Medium database issue",
      "Low polish fix",
    ]);
  });

  it("restores severity ordering after using the severity filter", () => {
    const criticalWebIssue: CheckResult = {
      ...webIssue,
      checkId: "security-csp",
      title: "Critical CSP gap",
      severity: "critical",
    };
    const highWebIssue: CheckResult = {
      ...webIssue,
      checkId: "security-hsts",
      title: "High HSTS gap",
      severity: "high",
    };
    const mediumCodeIssue: CodeIssue = {
      ...codeIssue,
      id: "code-medium",
      checkId: "code_scan.code-medium",
      title: "Medium database issue",
      severity: "medium",
      relativePath: "src/db.ts",
      absolutePath: "/tmp/project/src/db.ts",
    };

    const { container } = render(
      <IssueList
        rankedIssues={rankIssues([highWebIssue, criticalWebIssue], [mediumCodeIssue])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({
          webIssues: [highWebIssue, criticalWebIssue],
          codeIssues: [mediumCodeIssue],
        })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByRole("combobox", { name: /issue severity/i }), {
      target: { value: "critical" },
    });
    expect(getIssueRowTitles(container)).toEqual(["Critical CSP gap"]);

    fireEvent.change(screen.getByRole("combobox", { name: /issue severity/i }), {
      target: { value: "all" },
    });
    expect(getIssueRowTitles(container)).toEqual([
      "Critical CSP gap",
      "High HSTS gap",
      "Medium database issue",
    ]);
  });

  it("calls onSelect when an issue row is clicked", () => {
    const onSelect = vi.fn();

    render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: [webIssue] })}
        selectedId={null}
        onSelect={onSelect}
      />,
    );

    fireEvent.click(screen.getByText("Missing HSTS header"));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "web", id: "web:missing-hsts" }),
    );
  });

  it("does not show issue score-gain points in rows", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: [webIssue] })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.queryByText(/\+\d+ pts/i)).not.toBeInTheDocument();
  });

  it("applies a deep-linked web category filter", () => {
    const seoIssue: CheckResult = {
      ...webIssue,
      checkId: "seo-canonical",
      category: "seo",
      title: "Missing canonical tag",
    };

    render(
      <IssueList
        rankedIssues={rankIssues([webIssue, seoIssue], [codeIssue])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({
          webIssues: [webIssue, seoIssue],
          codeIssues: [codeIssue],
        })}
        selectedId={null}
        focus={getIssuesWebCategoryFocus("security")}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.getByText("Missing HSTS header")).toBeInTheDocument();
    expect(screen.queryByText("Missing canonical tag")).not.toBeInTheDocument();
    expect(screen.queryByText("Exposed .env file")).not.toBeInTheDocument();
    expect(
      (screen.getByRole("combobox", { name: /issue source/i }) as HTMLSelectElement).value,
    ).toBe("web");
    expect(
      (screen.getByRole("combobox", { name: /issue subcategory/i }) as HTMLSelectElement).value,
    ).toBe("web:security");
  });

  it("applies a deep-linked issue source filter", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [codeIssue])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({
          webIssues: [webIssue],
          codeIssues: [codeIssue],
        })}
        selectedId={null}
        focus={getIssuesSourceFocus("code")}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.queryByText("Missing HSTS header")).not.toBeInTheDocument();
    expect(screen.getByText("Exposed .env file")).toBeInTheDocument();
    expect(
      (screen.getByRole("combobox", { name: /issue source/i }) as HTMLSelectElement).value,
    ).toBe("code");
  });

  it("shows code-domain dropdown filters and narrows the queue to the selected domain", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [codeIssue, databaseCodeIssue])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({
          webIssues: [webIssue],
          codeIssues: [codeIssue, databaseCodeIssue],
        })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByRole("combobox", { name: /issue source/i }), {
      target: { value: "code" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: /issue subcategory/i }), {
      target: { value: "code:database" },
    });

    expect(screen.getByText("List endpoint returns unbounded query results")).toBeInTheDocument();
    expect(screen.queryByText("Exposed .env file")).not.toBeInTheDocument();
    expect(screen.queryByText("Missing HSTS header")).not.toBeInTheDocument();
  });

  it("groups duplicate code findings into one actionable row", () => {
    const groupedCodeIssues: CodeIssue[] = [
      {
        ...codeIssue,
        id: "primary-rate-limit",
        title: "Public-facing route has no clear rate limiting",
        category: "security",
        severity: "high",
        relativePath: "app/api/foo/route.ts",
        line: 12,
      },
      {
        ...codeIssue,
        id: "duplicate-rate-limit",
        title: "Public-facing route has no clear rate limiting",
        category: "security",
        severity: "high",
        relativePath: "app/api/bar/route.ts",
        line: 44,
      },
    ];

    render(
      <IssueList
        rankedIssues={rankIssues([], groupedCodeIssues)}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ codeIssues: groupedCodeIssues })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.getByRole("combobox", { name: /issue source/i })).toBeInTheDocument();
    expect(screen.getAllByText("Public-facing route has no clear rate limiting")).toHaveLength(1);
    // Category stays next to severity; location counts belong in the dossier.
    expect(screen.queryByText(/2 locations/i)).not.toBeInTheDocument();
    expect(screen.getByText(/- Security/i)).toBeInTheDocument();
  });

  it("highlights the selected row", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [codeIssue])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: [webIssue], codeIssues: [codeIssue] })}
        selectedId="web:missing-hsts"
        onSelect={vi.fn()}
      />,
    );

    const selectedRow = screen.getByText("Missing HSTS header").closest("button");
    const unselectedRow = screen.getByText("Exposed .env file").closest("button");

    expect(selectedRow?.className).toContain("issue-row--selected");
    expect(unselectedRow?.className).not.toContain("issue-row--selected");
  });

  it("shows the empty state when there are no issues", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([], [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary()}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.getByText("No web or code issues open")).toBeInTheDocument();
  });

  it("paginates unusually large issue sets instead of mounting every row", () => {
    const issues: CheckResult[] = Array.from({ length: 110 }, (_, i) => ({
      ...webIssue,
      checkId: `check-${i}`,
      title: `Large issue ${String(i).padStart(3, "0")}`,
    }));

    render(
      <IssueList
        rankedIssues={rankIssues(issues, [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: issues })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.getByText("Large issue 000")).toBeInTheDocument();
    expect(screen.queryByText("Large issue 109")).not.toBeInTheDocument();
    expect(screen.getByText("1/2")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next issues page" }));

    expect(screen.queryByText("Large issue 000")).not.toBeInTheDocument();
    expect(screen.getByText("Large issue 109")).toBeInTheDocument();
    expect(screen.getByText("2/2")).toBeInTheDocument();
  });

  it("does not show a quick wins control in the toolbar", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: [webIssue] })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.queryByText(/quick wins/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/load quick wins/i)).not.toBeInTheDocument();
  });

  it("shows the batch prompt button whenever visible issues exist", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: [webIssue] })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.getByText(/batch prompt/i)).toBeInTheDocument();
  });

  it("does not show active projection rows in work-item status views", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([], [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary()}
        selectedId={null}
        onSelect={vi.fn()}
        statusFilter="blocked"
        onStatusChange={vi.fn()}
      />,
    );

    expect(screen.getByRole("combobox", { name: /issue status/i })).toHaveValue("blocked");
    expect(screen.queryByText(/^Issues$/)).not.toBeInTheDocument();
    expect(screen.queryByText(/All clear/i)).not.toBeInTheDocument();
  });

  it("clears the selected dossier when the selected row is not in the list", async () => {
    const onClearSelection = vi.fn();

    render(
      <IssueList
        rankedIssues={rankIssues([], [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: [] })}
        selectedId="web:missing-hsts"
        onSelect={vi.fn()}
        onClearSelection={onClearSelection}
        url="https://example.com"
      />,
    );

    await waitFor(() => expect(onClearSelection).toHaveBeenCalledOnce());
  });

  it("does not offer alerts in the primary source dropdown", () => {
    render(
      <IssueList
        rankedIssues={rankIssues([webIssue], [])}
        issueLinks={[]}
        issueSummary={buildIssueSummary({ webIssues: [webIssue] })}
        selectedId={null}
        onSelect={vi.fn()}
      />,
    );

    expect(screen.queryByRole("option", { name: /alerts/i })).not.toBeInTheDocument();
  });
});
