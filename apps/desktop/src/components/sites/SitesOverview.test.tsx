import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { TodayProjectWorkSummary } from "@/lib/project-summary-signals";

const { getAllProjectsWorkSummaryMock } = vi.hoisted(() => ({
  getAllProjectsWorkSummaryMock: vi.fn(),
}));

vi.mock("@/lib/project-summary-signals", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/project-summary-signals")>();
  return {
    ...actual,
    getAllProjectsWorkSummary: (...args: unknown[]) => getAllProjectsWorkSummaryMock(...args),
  };
});

vi.mock("@/lib/open-url", () => ({
  openUrl: vi.fn(),
}));

import { SitesOverview } from "./SitesOverview";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";
import { queryKeys } from "@/lib/query/query-keys";

function renderSites(ui: React.ReactElement, queryClient = createTestQueryClient()) {
  return render(ui, { wrapper: withQueryClient(queryClient) });
}

function buildProjectSummary(
  overrides: Partial<TodayProjectWorkSummary> &
    Pick<TodayProjectWorkSummary, "id" | "name" | "primaryUrl">,
): TodayProjectWorkSummary {
  return {
    id: overrides.id,
    name: overrides.name,
    framework: overrides.framework ?? "nextjs",
    primaryUrl: overrides.primaryUrl,
    latestScore: overrides.latestScore ?? 82,
    siteScore: overrides.siteScore ?? 82,
    siteIssueCount: overrides.siteIssueCount ?? 0,
    siteCriticalCount: overrides.siteCriticalCount ?? 0,
    siteHighCount: overrides.siteHighCount ?? 0,
    lastScannedAt: overrides.lastScannedAt ?? "2026-04-15T12:00:00Z",
    issuesCritical: overrides.issuesCritical ?? 0,
    issuesHigh: overrides.issuesHigh ?? 0,
    environmentCount: overrides.environmentCount ?? 1,
    projectPath: overrides.projectPath ?? `/tmp/${overrides.name.toLowerCase()}`,
    primarySecurityIssueId: overrides.primarySecurityIssueId ?? null,
    primarySecurityFocus: overrides.primarySecurityFocus ?? null,
    enabledIntegrations: overrides.enabledIntegrations ?? [],
    securityUpdateCount: overrides.securityUpdateCount ?? 0,
    pendingUpdateCount: overrides.pendingUpdateCount ?? 0,
    searchRegression: overrides.searchRegression ?? null,
    integrationFailureCount: overrides.integrationFailureCount ?? 0,
    staleIntegrationCount: overrides.staleIntegrationCount ?? 0,
    guardrailCriticalCount: overrides.guardrailCriticalCount ?? 0,
    guardrailHighCount: overrides.guardrailHighCount ?? 0,
    topGuardrailIssue: overrides.topGuardrailIssue ?? null,
    topGuardrailDomain: overrides.topGuardrailDomain ?? null,
    topGuardrailDomainCount: overrides.topGuardrailDomainCount ?? 0,
    guardrailsCheckedAt: overrides.guardrailsCheckedAt ?? null,
    codeScanCheckedAt: overrides.codeScanCheckedAt ?? "2026-04-15T12:00:00Z",
    workSummary: overrides.workSummary ?? {
      unresolvedCount: 0,
      newCount: 0,
      workingCount: 0,
      regressedCount: 0,
      ignoredCount: 0,
      blockedCount: 0,
      launchBlockerCount: 0,
      maintenanceCount: 0,
      primaryAction: null,
      regressedAction: null,
      workingAction: null,
      blockedAction: null,
      ignoredAction: null,
      launchBlockerAction: null,
      weeklySummary: null,
    },
  };
}

describe("SitesOverview", () => {
  beforeEach(() => {
    getAllProjectsWorkSummaryMock.mockReset();
  });

  it("shows the current overview loading shell while project summaries are still in flight", () => {
    getAllProjectsWorkSummaryMock.mockReturnValue(new Promise(() => {}));

    renderSites(<SitesOverview onSelectProject={vi.fn()} onAddProject={vi.fn()} />);

    expect(screen.getByLabelText("Overview loading state")).toBeInTheDocument();
  });

  it("renders the real overview stats and lets the user open a site dashboard", async () => {
    const onSelectProject = vi.fn();

    getAllProjectsWorkSummaryMock.mockResolvedValue([
      buildProjectSummary({
        id: 1,
        name: "Alpha",
        primaryUrl: "https://alpha.test",
        latestScore: 91,
        siteScore: 47,
        siteIssueCount: 3,
        siteCriticalCount: 1,
        issuesCritical: 1,
        guardrailCriticalCount: 1,
        workSummary: {
          unresolvedCount: 3,
          newCount: 1,
          workingCount: 1,
          regressedCount: 0,
          ignoredCount: 0,
          blockedCount: 1,
          launchBlockerCount: 1,
          maintenanceCount: 0,
          primaryAction: null,
          regressedAction: null,
          workingAction: null,
          blockedAction: null,
          ignoredAction: null,
          launchBlockerAction: null,
          weeklySummary: null,
        },
      }),
      buildProjectSummary({
        id: 2,
        name: "Beta",
        primaryUrl: "https://beta.test",
        latestScore: 73,
        siteScore: 73,
        siteIssueCount: 1,
        siteHighCount: 1,
        workSummary: {
          unresolvedCount: 1,
          newCount: 0,
          workingCount: 1,
          regressedCount: 0,
          ignoredCount: 0,
          blockedCount: 0,
          launchBlockerCount: 0,
          maintenanceCount: 0,
          primaryAction: null,
          regressedAction: null,
          workingAction: null,
          blockedAction: null,
          ignoredAction: null,
          launchBlockerAction: null,
          weeklySummary: null,
        },
      }),
    ]);

    renderSites(
      <SitesOverview
        currentProjectId={1}
        onSelectProject={onSelectProject}
        onAddProject={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /alpha/i })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /beta/i })).toBeInTheDocument();
    });

    expect(screen.getByText("Total Sites")).toBeInTheDocument();
    expect(screen.getByText("Active Issues")).toBeInTheDocument();
    expect(screen.getByText("4")).toBeInTheDocument();
    expect(screen.getByText("Avg. SiteCMD Score")).toBeInTheDocument();
    expect(screen.getByText("60")).toBeInTheDocument();
    expect(screen.getByText("47")).toBeInTheDocument();
    expect(screen.getByText("Scanned Sites")).toBeInTheDocument();
    expect(screen.getByText(/live SiteCMD health score/i)).toBeInTheDocument();
    expect(screen.getByText(/average above covers the 2 scanned sites/i)).toBeInTheDocument();
    expect(screen.getByText(/left out rather than counted as zero/i)).toBeInTheDocument();
    expect(screen.getByText("alpha.test")).toBeInTheDocument();
    expect(screen.getByText("beta.test")).toBeInTheDocument();
    expect(screen.getByText("current")).toHaveClass("overview-current-label");
    expect(screen.queryByText("nextjs")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /alpha/i })).not.toHaveClass("ring-1");

    fireEvent.click(screen.getByRole("button", { name: /beta/i }));
    expect(onSelectProject).toHaveBeenCalledWith(2);
  });

  it("keeps cached site rows visible during a background refresh", async () => {
    const queryClient = createTestQueryClient();
    getAllProjectsWorkSummaryMock.mockResolvedValueOnce([
      buildProjectSummary({
        id: 1,
        name: "Cached Site",
        primaryUrl: "https://cached.test",
      }),
    ]);

    renderSites(<SitesOverview onSelectProject={vi.fn()} onAddProject={vi.fn()} />, queryClient);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /cached site/i })).toBeInTheDocument();
    });

    getAllProjectsWorkSummaryMock.mockReturnValueOnce(new Promise(() => {}));
    act(() => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.sites.all });
    });

    await waitFor(() => {
      expect(getAllProjectsWorkSummaryMock).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByRole("button", { name: /cached site/i })).toBeInTheDocument();
    expect(screen.queryByLabelText("Overview loading state")).not.toBeInTheDocument();
  });

  it("shows a truthful retry state when the project summary load fails", async () => {
    const onAddProject = vi.fn();

    getAllProjectsWorkSummaryMock
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce([
        buildProjectSummary({
          id: 7,
          name: "Recovered",
          primaryUrl: "https://recovered.test",
        }),
      ]);

    renderSites(<SitesOverview onSelectProject={vi.fn()} onAddProject={onAddProject} />);

    await waitFor(() => {
      expect(screen.getByText("Sites could not load")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /recovered/i })).toBeInTheDocument();
    });
  });

  it("treats an empty project list as a real empty state, not a fake load success", async () => {
    const onAddProject = vi.fn();
    getAllProjectsWorkSummaryMock.mockResolvedValue([]);

    renderSites(<SitesOverview onSelectProject={vi.fn()} onAddProject={onAddProject} />);

    await waitFor(() => {
      expect(screen.getByText("No projects yet")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Add your first site" }));
    expect(onAddProject).toHaveBeenCalled();
  });
});

describe("SitesOverview open access", () => {
  beforeEach(() => {
    getAllProjectsWorkSummaryMock.mockReset();
    getAllProjectsWorkSummaryMock.mockResolvedValue([]);
  });

  it("renders the overview for every install with no Professional upsell", async () => {
    renderSites(<SitesOverview onSelectProject={vi.fn()} onAddProject={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("No projects yet")).toBeInTheDocument();
    });
    expect(screen.queryByText("See every site on one screen")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /See Professional/i })).not.toBeInTheDocument();
  });
});
