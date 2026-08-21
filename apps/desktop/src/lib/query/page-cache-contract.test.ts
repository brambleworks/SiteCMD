import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const SRC = join(dirname(fileURLToPath(import.meta.url)), "../..");

const PAGE_CACHE_CONTRACTS = [
  [
    "project shell",
    "hooks/useProject.tsx",
    ["queryKeys.projects.list", "useQuery<ProjectRecord[]>"],
  ],
  ["dashboard", "components/dashboard/Dashboard.tsx", ["useDashboardData", "useCurrentScore"]],
  [
    "dashboard activity",
    "components/dashboard/useDashboardRecentEvents.ts",
    ["queryKeys.events.dashboardRecent", "queryKeys.events.dashboardUpdates"],
  ],
  ["analytics", "components/dashboard/AnalyticsPage.tsx", ["useAnalyticsQuery"]],
  [
    "issues",
    "pages/IssuesPage.tsx",
    ["useIssuesPageSnapshot", "useIssueStatusResources", "useCurrentScore"],
  ],
  [
    "issues by page",
    "components/issues/ByPageList.tsx",
    ["queryKeys.issuePages.forEnv", "useQuery<PageSummary[]>"],
  ],
  [
    "issue memory",
    "components/issues/IssueMemorySection.tsx",
    ["queryKeys.issueMemory.forCheck", "queryKeys.projects.list"],
  ],
  [
    "issue tracker actions",
    "components/issues/SendToTrackerAction.tsx",
    ["queryKeys.issueLinks.forCheck", "useIntegrationsQuery"],
  ],
  ["deploys", "components/dashboard/DeploysPage.tsx", ["useDeploysPageData"]],
  [
    "updates",
    "components/dashboard/UpdatesPage.tsx",
    ["queryKeys.updates.report", "useUpdatesHistory"],
  ],
  [
    "activity",
    "components/events/EventsPage.tsx",
    ["useEvents", "queryKeys.projectSummary.signals"],
  ],
  [
    "search",
    "components/dashboard/SearchConsolePage.tsx",
    ["useAnalyticsQuery", "useSearchScanQuery"],
  ],
  ["integrations", "components/integrations/IntegrationsPage.tsx", ["useIntegrationsQuery"]],
  ["sites", "components/sites/SitesOverview.tsx", ["queryKeys.sites.overview"]],
  ["alerts", "pages/AlertsPage.tsx", ["useAlerts", "useIntegrationsQuery"]],
  ["reports", "components/reports/ReportsPage.tsx", ["useReportSnapshot", "useReportsHistory"]],
] as const;

const SETTINGS_CACHE_CONTRACTS = [
  ["scan schedule", "components/scan/ScanScheduleCard.tsx", "queryKeys.settings.scanSchedule"],
  ["sitemap", "hooks/useSitemap.ts", "queryKeys.settings.sitemapSite"],
  ["webhooks", "components/settings/WebhooksSection.tsx", "queryKeys.settings.webhooks"],
  [
    "integration setup",
    "components/settings/useInlineIntegrationSetupState.ts",
    "useIntegrationsQuery",
  ],
  ["PageSpeed key", "components/settings/PageSpeedKeyCard.tsx", "queryKeys.settings.pagespeedKey"],
  ["agent tools", "components/settings/AgentToolCards.tsx", "queryKeys.settings.agentTools"],
  [
    "database info",
    "components/settings/SettingsDataSection.tsx",
    "queryKeys.settings.databaseInfo",
  ],
  ["autostart", "components/settings/SettingsGeneralSection.tsx", "queryKeys.settings.autostart"],
  ["app version", "components/settings/UpdatesSettingsCard.tsx", "queryKeys.settings.appVersion"],
] as const;

const PAGE_SKELETON_CONTRACTS = [
  ["lazy routes", "app/ShellHeader.tsx", "PageSkeleton"],
  ["startup shell", "app/StartupShell.tsx", "PageSkeleton"],
  ["dashboard", "components/dashboard/DashboardEmptyState.tsx", "LoadingRegion"],
  ["analytics", "components/dashboard/AnalyticsPageLoadingState.tsx", "PageSkeleton"],
  ["issues", "components/issues/IssuePanelSkeleton.tsx", "LoadingRegion"],
  ["deploys", "components/dashboard/DeploysPageSections.tsx", "LoadingRegion"],
  ["updates", "components/dashboard/UpdatesOverviewSections.tsx", "LoadingRegion"],
  ["activity", "components/events/EventsPage.tsx", "LoadingRegion"],
  ["search", "components/dashboard/SearchConsoleLoadingState.tsx", "LoadingRegion"],
  ["integrations", "components/integrations/IntegrationsPage.tsx", "LoadingRegion"],
  ["sites", "components/sites/SitesOverview.tsx", "LoadingRegion"],
  ["alerts", "components/alerts/AlertList.tsx", "LoadingRegion"],
  ["reports", "components/reports/ReportsPageSections.tsx", "LoadingRegion"],
] as const;

describe("routed page cache contracts", () => {
  it.each(PAGE_CACHE_CONTRACTS)(
    "keeps %s data behind its shared cache owner",
    (label, file, owners) => {
      const source = readFileSync(join(SRC, file), "utf8");
      for (const owner of owners) {
        expect(source, `${label} lost cache owner ${owner}`).toContain(owner);
      }
    },
  );

  it.each(SETTINGS_CACHE_CONTRACTS)(
    "keeps the %s settings read behind its shared cache owner",
    (label, file, owner) => {
      const source = readFileSync(join(SRC, file), "utf8");
      expect(source, `${label} lost cache owner ${owner}`).toContain(owner);
    },
  );

  it.each(PAGE_SKELETON_CONTRACTS)(
    "keeps the %s initial load behind a page-shaped skeleton boundary",
    (label, file, boundary) => {
      const source = readFileSync(join(SRC, file), "utf8");
      expect(source, `${label} lost skeleton boundary ${boundary}`).toContain(boundary);
    },
  );

  it("keeps loading out of the generic empty/error state component", () => {
    const source = readFileSync(join(SRC, "components/ui/surface-state.tsx"), "utf8");
    expect(source).not.toContain('"loading"');
  });
});
