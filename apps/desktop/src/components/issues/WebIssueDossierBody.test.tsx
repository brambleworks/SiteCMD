import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  CheckResult,
  FixLocation,
  IssueGroup,
  RecentEventRef,
  TransitiveCause,
  Enrichment,
  Evidence,
  CrossEnvSignal,
  CrossProjectPattern,
} from "@/lib/types";

vi.mock("@/lib/async-fix-guides", () => ({
  loadWebFixGuide: vi.fn().mockResolvedValue(null),
  loadCodeFixGuide: vi.fn().mockResolvedValue(null),
  loadWebBaseline: vi.fn().mockResolvedValue({
    effort: "quick",
    effortMinutes: 5,
    steps: ["Set the header at the layer that owns this response."],
  }),
  loadCodeBaseline: vi.fn().mockResolvedValue(null),
}));

import { WebIssueRichSections } from "./WebIssueDossierBody";

const NOW = Date.UTC(2026, 4, 19, 12, 0, 0);

const baseIssue: CheckResult = {
  checkId: "test.missing-csp",
  category: "security",
  title: "Content-Security-Policy header is missing",
  description: "The response did not include a Content-Security-Policy header.",
  status: "fail",
  severity: "high",
  fixPrompt: null,
  manualFix: null,
  rawData: { header_name: "content-security-policy", current_value: "missing" },
  confidence: "high",
  whyItMatters:
    "Without a CSP header, a single injected script can exfiltrate session cookies from any page on the site.",
};

const baseGroup: IssueGroup = {
  checkId: "test.missing-csp",
  category: "security",
  severity: "high",
  title: "CSP missing",
  description: "desc",
  instances: [
    {
      id: 1,
      source: "web_scan",
      signalId: "https://example.com/",
      producerCheckId: "test.missing-csp",
      url: "https://example.com/",
      pageUrl: "https://example.com/",
      severity: "high",
      title: "CSP missing",
      description: "desc",
      detailJson: null,
      firstSeenAt: NOW - 86_400_000,
      lastSeenAt: NOW,
      confidence: null,
      domain: null,
      relativePath: null,
      line: null,
    },
  ],
  sources: ["web_scan"],
  status: "new",
  snoozeUntil: null,
  blockReason: null,
  impactScore: 0,
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

const makeFile = (path: string, label: string, reason: string): FixLocation => ({
  label,
  reason,
  relativePath: path,
  absolutePath: `/abs/${path}`,
});

const fiveFiles: FixLocation[] = [
  makeFile("src/middleware/security.ts", "Middleware", "Sets response headers"),
  makeFile("src/layout/Head.tsx", "Layout head", "Per-page meta wrapper"),
  makeFile("src/server/middleware.ts", "Server middleware", "Express config"),
  makeFile("vercel.json", "Vercel config", "Edge headers"),
  makeFile("public/_headers", "Cloudflare headers", "Static headers"),
];

const noopProps = {
  fixText: "",
  locationCount: 1,
  projectId: 1,
  projectPath: "/abs" as string | null,
  verifying: false,
  onOpenEditor: async () => {},
  onVerifyFor: async () => {},
  onOpenFile: () => {},
  onRevealFile: () => {},
};

describe("WebIssueRichSections", () => {
  it("renders every correlated file without capping the list", () => {
    render(
      <WebIssueRichSections
        {...noopProps}
        issue={baseIssue}
        group={baseGroup}
        groupedOccurrenceLabels={["/"]}
        pageUrl="https://example.com/"
        primaryCorrelatedFile={fiveFiles[0]}
        correlatedFiles={fiveFiles}
      />,
    );

    // Files live in the Locations tab; select it before asserting.
    fireEvent.click(screen.getByRole("tab", { name: "Locations (1)" }));
    for (const file of fiveFiles) {
      expect(screen.getByText(file.relativePath)).toBeInTheDocument();
    }
  });

  it("shows the section tabs in order with the location count", () => {
    render(
      <WebIssueRichSections
        {...noopProps}
        issue={baseIssue}
        group={baseGroup}
        groupedOccurrenceLabels={["/"]}
        pageUrl="https://example.com/"
        primaryCorrelatedFile={null}
        correlatedFiles={fiveFiles}
      />,
    );

    const tabLabels = screen.getAllByRole("tab").map((tab) => tab.textContent);
    expect(tabLabels).toEqual(["Description", "Locations (1)", "How to fix", "Evidence"]);

    expect(screen.queryByText("Why it matters")).toBeNull();
  });

  it("lists every affected page from the issue group", () => {
    const multiPageGroup: IssueGroup = {
      ...baseGroup,
      instances: [
        { ...baseGroup.instances[0], id: 1, url: "https://example.com/", signalId: "a" },
        { ...baseGroup.instances[0], id: 2, url: "https://example.com/pricing", signalId: "b" },
        { ...baseGroup.instances[0], id: 3, url: "https://example.com/blog/launch", signalId: "c" },
      ],
    };

    render(
      <WebIssueRichSections
        {...noopProps}
        issue={baseIssue}
        group={multiPageGroup}
        groupedOccurrenceLabels={[]}
        locationCount={3}
        pageUrl="https://example.com/"
        primaryCorrelatedFile={null}
        correlatedFiles={[]}
      />,
    );

    // Pages live in the counted Locations tab; select it before asserting.
    fireEvent.click(screen.getByRole("tab", { name: "Locations (3)" }));
    expect(screen.getByText("/pricing")).toBeInTheDocument();
    expect(screen.getByText("/blog/launch")).toBeInTheDocument();
  });

  it("shows cross-source evidence inside the polished dossier tabs", () => {
    const crossSourceGroup: IssueGroup = {
      ...baseGroup,
      sources: ["web_scan", "code_scan"],
      instances: [
        baseGroup.instances[0],
        {
          ...baseGroup.instances[0],
          id: 2,
          source: "code_scan",
          signalId: "code_scan:missing-csp:src/server.ts:14",
          producerCheckId: "missing-csp",
          url: null,
          pageUrl: null,
          relativePath: "src/server.ts",
          line: 14,
        },
      ],
    };

    render(
      <WebIssueRichSections
        {...noopProps}
        issue={baseIssue}
        group={crossSourceGroup}
        groupedOccurrenceLabels={["/"]}
        pageUrl="https://example.com/"
        primaryCorrelatedFile={null}
        correlatedFiles={[]}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: "Evidence" }));
    expect(screen.getByText("Web Scan")).toBeInTheDocument();
    expect(screen.getByText("Code Scan")).toBeInTheDocument();
    expect(screen.getByText(/src\/server\.ts:14/)).toBeInTheDocument();
  });
});

const v3RecentEvent: RecentEventRef = {
  eventId: 1,
  eventType: "deploy",
  occurredAtMs: NOW - 60 * 60 * 1000,
  title: "deploy v2.1.0",
  correlationConfidence: "high",
};

const v3TransitiveCause: TransitiveCause = {
  checkId: "upstream.check",
  path: ["upstream.check", "test.missing-csp"],
  confidence: "medium",
  depth: 1,
};

const v3Enrichment: Enrichment = {
  kind: "fieldLcp",
  p75_ms: 3200,
  url: "https://example.com/",
  source: "gsc",
};

const v3Evidence: Evidence = {
  kind: "scan_signal",
  timestamp: new Date(NOW - 2 * 60 * 60 * 1000).toISOString(),
  source: "web_scan",
  detail: "header absent on all sampled responses",
};

const v3CrossEnv: CrossEnvSignal = {
  stagingObservedAt: new Date(NOW - 3 * 24 * 60 * 60 * 1000).toISOString(),
  daysBeforeProd: 3,
};

const v3CrossProject: CrossProjectPattern = {
  projectCount: 2,
  lastSeenAt: new Date(NOW - 5 * 24 * 60 * 60 * 1000).toISOString(),
};

function mkFullyEnrichedIssue(): IssueGroup {
  return {
    ...baseGroup,
    recentEvents: [v3RecentEvent],
    transitiveCauses: [v3TransitiveCause],
    enrichments: [v3Enrichment],
    correlationEvidence: [v3Evidence],
    crossEnvSignal: v3CrossEnv,
    crossProjectPattern: v3CrossProject,
    observationCount: 4,
    anomalyScore: 6.2,
    affectedPages: ["https://example.com/", "https://example.com/about"],
  };
}

function mkMinimalIssue(): IssueGroup {
  return { ...baseGroup };
}

describe("WebIssueRichSections - v3 integration", () => {
  it("renders dossier-relevant v3 sections without recent-event cards", () => {
    const enrichedGroup = mkFullyEnrichedIssue();
    render(
      <WebIssueRichSections
        {...noopProps}
        issue={baseIssue}
        group={enrichedGroup}
        groupedOccurrenceLabels={["/"]}
        pageUrl="https://example.com/"
        primaryCorrelatedFile={null}
        correlatedFiles={[]}
      />,
    );

    // Scan/deploy history belongs on History, not above the dossier tabs.
    expect(screen.queryByText("deploy")).not.toBeInTheDocument();

    expect(screen.getByText(/root-cause chain/i)).toBeInTheDocument();

    // Enrichment section - "Real-user LCP p75: 3.2s on https://example.com/"
    expect(screen.getByText(/real-user lcp/i)).toBeInTheDocument();

    expect(screen.getByText(/seen on a non-production environment/i)).toBeInTheDocument();

    expect(screen.getByText(/you have hit this in 2 other projects/i)).toBeInTheDocument();

    expect(screen.getByText(/resolved this pattern/i)).toBeInTheDocument();

    // Correlation enrichment must not expose a what-if action.
    expect(
      screen.queryByRole("button", { name: /what else does fixing this resolve/i }),
    ).not.toBeInTheDocument();
  });

  it("does not render v3 sections when fields are absent", () => {
    const minimalGroup = mkMinimalIssue();
    render(
      <WebIssueRichSections
        {...noopProps}
        issue={baseIssue}
        group={minimalGroup}
        groupedOccurrenceLabels={["/"]}
        pageUrl="https://example.com/"
        primaryCorrelatedFile={null}
        correlatedFiles={[]}
      />,
    );

    expect(screen.queryByText(/root-cause chain/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/real-user lcp/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/seen on a non-production/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/resolved this pattern/i)).not.toBeInTheDocument();

    expect(screen.getByText(/location/i)).toBeInTheDocument();
    expect(screen.getByText(/how to fix/i)).toBeInTheDocument();
  });

  it("renders without group prop and still shows core sections", () => {
    render(
      <WebIssueRichSections
        {...noopProps}
        issue={baseIssue}
        group={undefined}
        groupedOccurrenceLabels={["/"]}
        pageUrl="https://example.com/"
        primaryCorrelatedFile={null}
        correlatedFiles={[]}
      />,
    );

    expect(screen.getByText(/location/i)).toBeInTheDocument();
    expect(screen.getByText(/how to fix/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /what else/i })).not.toBeInTheDocument();
  });
});

describe("WebIssueRichSections - complete dossier for every install", () => {
  function renderSections(issue: CheckResult = baseIssue) {
    return render(
      <WebIssueRichSections
        {...noopProps}
        fixText="Add the header in middleware."
        issue={issue}
        group={baseGroup}
        groupedOccurrenceLabels={["/"]}
        pageUrl="https://example.com/"
        primaryCorrelatedFile={null}
        correlatedFiles={[]}
      />,
    );
  }

  it("renders the full fix guidance and evidence with nothing to sell", async () => {
    renderSections();

    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      "Description",
      "Locations (1)",
      "How to fix",
      "Evidence",
    ]);

    fireEvent.click(screen.getByRole("tab", { name: "How to fix" }));
    expect(await screen.findByText(/add the header in middleware/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /get full guide/i })).toBeNull();
    // Generic verification filler must not replace issue-specific guidance.
    expect(screen.queryByText(/verify after the first focused change/i)).toBeNull();

    fireEvent.click(screen.getByRole("tab", { name: "Evidence" }));
    expect(screen.getByText(/supporting evidence captured/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /see evidence/i })).toBeNull();
  });

  it("drops the Evidence tab when no proof content was captured", () => {
    renderSections({ ...baseIssue, rawData: null });

    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      "Description",
      "Locations (1)",
      "How to fix",
    ]);
    expect(screen.queryByText(/supporting evidence captured/i)).toBeNull();
  });
});
