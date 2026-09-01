import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UnifiedFixIssue } from "@/lib/issue-ranking";
import type { CheckResult, CodeIssue, IssueGroup } from "@/lib/types";

const { verifyIssueMock, toastError, toastSuccess, toastWarning } = vi.hoisted(() => ({
  verifyIssueMock: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
  toastWarning: vi.fn(),
}));

vi.mock("@/lib/issues", () => ({ verifyIssue: verifyIssueMock }));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({ error: toastError, success: toastSuccess, warning: toastWarning }),
}));
vi.mock("@/components/scan/CodeScanResults", () => ({
  CodeIssueDossier: ({
    group,
    onVerify,
    verifying,
  }: {
    group?: IssueGroup;
    onVerify: () => void;
    verifying: boolean;
  }) => (
    <>
      <span>{group ? "Polished code dossier" : "Ungrouped code dossier"}</span>
      <button type="button" onClick={onVerify}>
        {verifying ? "Verifying code" : "Verify code"}
      </button>
    </>
  ),
}));
vi.mock("@/components/dashboard/DashboardComponents", () => ({
  WebIssueDossier: ({ group }: { group?: IssueGroup }) => (
    <span>{group?.sources.join(" + ") ?? "Ungrouped web dossier"}</span>
  ),
}));
vi.mock("@/components/issues/AlertDetail", () => ({ AlertDetail: () => null }));

import { IssueDossier } from "./IssueDossier";

const CODE_ISSUE: CodeIssue = {
  id: "n-plus-one-query:src/data.ts:10",
  checkId: "code_scan.n-plus-one-query",
  category: "database",
  domain: "database",
  severity: "high",
  title: "Query inside a loop",
  description: "A query runs once per item.",
  relativePath: "src/data.ts",
  absolutePath: "/tmp/project/src/data.ts",
  line: 10,
  sourceExcerpt: null,
  evidence: null,
  whyNow: null,
  likelyFix: null,
  confidence: "high",
  verifyHint: null,
};

const CODE_GROUP: IssueGroup = {
  checkId: CODE_ISSUE.checkId,
  category: "code_quality",
  severity: "high",
  title: CODE_ISSUE.title,
  description: CODE_ISSUE.description,
  instances: [
    {
      id: 1,
      source: "code_scan",
      signalId: CODE_ISSUE.id,
      producerCheckId: "n-plus-one-query",
      url: null,
      pageUrl: null,
      severity: "high",
      title: CODE_ISSUE.title,
      description: CODE_ISSUE.description,
      detailJson: null,
      firstSeenAt: 1,
      lastSeenAt: 1,
      confidence: "high",
      domain: "database",
      relativePath: CODE_ISSUE.relativePath,
      line: CODE_ISSUE.line,
    },
  ],
  sources: ["code_scan"],
  status: "new",
  snoozeUntil: null,
  blockReason: null,
  impactScore: 6,
  likelyCauses: [],
  suggestedIntegrations: [],
  fixLocations: [],
  transitiveCauses: [],
  downstreamEffects: [],
  recentEvents: [],
  enrichments: [],
  correlationEvidence: [],
  affectedPages: [],
  crossEnvSignal: null,
  crossProjectPattern: null,
  displayConfidence: null,
  observationCount: 0,
  anomalyScore: null,
};

const SELECTED_CODE_ISSUE: UnifiedFixIssue = {
  kind: "code",
  id: "code-group:database:high:query-inside-a-loop",
  issue: CODE_ISSUE,
  groupedIssues: [CODE_ISSUE],
  occurrenceCount: 1,
  occurrenceLabels: ["src/data.ts:10"],
  impact: 6,
  categoryLabel: "Database",
  effort: null,
  effortMinutes: null,
  group: CODE_GROUP,
};

const WEB_ISSUE: CheckResult = {
  checkId: "security.csp",
  category: "security",
  severity: "high",
  status: "fail",
  title: "Content Security Policy is missing",
  description: "The site and source configuration both omit a CSP.",
  fixPrompt: null,
  manualFix: null,
  rawData: null,
  confidence: "high",
};

const CROSS_SOURCE_GROUP: IssueGroup = {
  ...CODE_GROUP,
  checkId: WEB_ISSUE.checkId,
  category: "security",
  title: WEB_ISSUE.title,
  description: WEB_ISSUE.description,
  sources: ["web_scan", "code_scan"],
  instances: [
    {
      ...CODE_GROUP.instances[0],
      id: 2,
      source: "web_scan",
      signalId: "https://example.com/",
      producerCheckId: WEB_ISSUE.checkId,
      url: "https://example.com/",
      pageUrl: "https://example.com/",
      title: WEB_ISSUE.title,
      description: WEB_ISSUE.description,
      domain: null,
      relativePath: null,
      line: null,
    },
    {
      ...CODE_GROUP.instances[0],
      id: 3,
      producerCheckId: "missing-csp-config",
      title: WEB_ISSUE.title,
      description: WEB_ISSUE.description,
      relativePath: "src/server.ts",
      line: 14,
    },
  ],
};

const SELECTED_WEB_ISSUE: UnifiedFixIssue = {
  kind: "web",
  id: `web-group:${WEB_ISSUE.checkId}`,
  issue: WEB_ISSUE,
  groupedIssues: [WEB_ISSUE],
  occurrenceCount: 2,
  occurrenceLabels: ["/", "src/server.ts:14"],
  impact: 6,
  categoryLabel: "Security",
  effort: null,
  effortMinutes: null,
  group: CROSS_SOURCE_GROUP,
};

function renderCodeDossier(onClose = vi.fn()) {
  render(
    <IssueDossier
      selected={SELECTED_CODE_ISSUE}
      projectId={7}
      url="https://example.com/"
      projectPath="/tmp/project"
      onClose={onClose}
    />,
  );
  return onClose;
}

describe("IssueDossier code verification", () => {
  beforeEach(() => {
    verifyIssueMock.mockReset();
    toastError.mockReset();
    toastSuccess.mockReset();
    toastWarning.mockReset();
  });

  it("routes Verify through the source-aware issue verifier", async () => {
    verifyIssueMock.mockResolvedValue({ status: "still_present", sources: ["code_scan"] });
    renderCodeDossier();

    fireEvent.click(await screen.findByRole("button", { name: "Verify code" }));

    await waitFor(() =>
      expect(verifyIssueMock).toHaveBeenCalledWith(7, "https://example.com", CODE_ISSUE.checkId),
    );
    expect(toastWarning).toHaveBeenCalledWith(
      "Still present",
      expect.stringMatching(/fresh Code Scan/i),
    );
  });

  it("routes canonical code groups through the polished code dossier", async () => {
    renderCodeDossier();

    expect(await screen.findByText("Polished code dossier")).toBeInTheDocument();
  });

  it("closes the dossier after a fresh scan verifies the issue", async () => {
    verifyIssueMock.mockResolvedValue({ status: "verified", sources: ["code_scan"] });
    const onClose = renderCodeDossier();

    fireEvent.click(await screen.findByRole("button", { name: "Verify code" }));

    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect(toastSuccess).toHaveBeenCalledWith(
      "Verified",
      expect.stringMatching(/fresh Code Scan/i),
    );
  });
});

describe("IssueDossier source layouts", () => {
  it("keeps cross-source evidence inside the polished web dossier", async () => {
    render(
      <IssueDossier
        selected={SELECTED_WEB_ISSUE}
        projectId={7}
        url="https://example.com/"
        projectPath="/tmp/project"
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByText("web_scan + code_scan")).toBeInTheDocument();
  });
});
