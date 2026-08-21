import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { IssueGroup } from "@/lib/types";
import { IssuesByPagePanel } from "./IssuesByPagePanel";

function makeGroup(overrides: Partial<IssueGroup>): IssueGroup {
  return {
    checkId: "performance.lcp",
    category: "performance",
    severity: "high",
    title: "Largest Contentful Paint is slow",
    description: "The main page content takes too long to appear for visitors.",
    instances: [],
    sources: ["web_scan"],
    status: "new",
    snoozeUntil: null,
    blockReason: null,
    impactScore: 12,
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
    observationCount: 1,
    anomalyScore: null,
    ...overrides,
  };
}

describe("IssuesByPagePanel", () => {
  it("renders a structured issue list and opens the normal dossier", () => {
    const onSelectIssue = vi.fn();
    const pageGroups = [
      makeGroup({
        checkId: "seo.meta_description",
        category: "seo",
        severity: "low",
        title: "Meta description needs attention",
      }),
      makeGroup({}),
      makeGroup({
        checkId: "security.headers",
        category: "security",
        severity: "critical",
        title: "Blocked finding",
        status: "blocked",
      }),
    ];

    render(
      <IssuesByPagePanel
        projectId={1}
        url="https://example.com"
        selectedPageUrl="https://example.com/pricing"
        pageGroups={pageGroups}
        pageGroupsLoading={false}
        pageGroupsError={null}
        onRetryPageGroups={vi.fn()}
        onSelectPage={vi.fn()}
        onSelectIssue={onSelectIssue}
      />,
    );

    expect(screen.getByText("/pricing")).toBeInTheDocument();
    expect(screen.getByText("example.com")).toBeInTheDocument();
    expect(screen.getByText("2 open issues")).toBeInTheDocument();
    expect(screen.getByText("Findings on this page")).toBeInTheDocument();
    expect(screen.queryByText("Blocked finding")).not.toBeInTheDocument();
    expect(screen.queryByText("web_scan")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Open Largest Contentful Paint is slow" }));
    expect(onSelectIssue).toHaveBeenCalledWith("performance.lcp");
  });

  it("gives an empty page drill-down a clear way back", () => {
    const onSelectPage = vi.fn();
    render(
      <IssuesByPagePanel
        projectId={1}
        url="https://example.com"
        selectedPageUrl="https://example.com/pricing"
        pageGroups={[]}
        pageGroupsLoading={false}
        pageGroupsError={null}
        onRetryPageGroups={vi.fn()}
        onSelectPage={onSelectPage}
        onSelectIssue={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Back to pages" }));
    expect(onSelectPage).toHaveBeenCalledWith(null);
  });
});
