import type { BingSearchData, SearchConsoleData } from "@/lib/analytics-types";
import type { SearchRegressionSignal } from "@/lib/project-summary-signals";
import type { IntegrationData } from "./dashboard-data-state";

interface DashboardSeoClicksData {
  clicks: number;
  impressions: number;
  avgPosition: number | null;
  deltaPct: number | null;
}

interface DashboardSearchIndexData {
  sourceLabel: string;
  visiblePageCount: number | null;
  totalClicks: number | null;
  totalImpressions: number | null;
}

export interface DashboardSearchSignals {
  seoClicks: DashboardSeoClicksData | null;
  searchIndex: DashboardSearchIndexData | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function finiteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function arrayValue(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function readSearchConsoleData(value: unknown): SearchConsoleData | null {
  if (!isRecord(value)) return null;
  const totalClicks = finiteNumber(value.total_clicks);
  const totalImpressions = finiteNumber(value.total_impressions);
  if (totalClicks === null || totalImpressions === null) return null;
  return value as unknown as SearchConsoleData;
}

function readBingData(value: unknown): BingSearchData | null {
  if (!isRecord(value)) return null;
  const totalClicks = finiteNumber(value.total_clicks);
  const totalImpressions = finiteNumber(value.total_impressions);
  if (totalClicks === null || totalImpressions === null) return null;
  return value as unknown as BingSearchData;
}

function readHealthyIntegration(integrations: IntegrationData[], integrationType: string) {
  return integrations.find(
    (integration) => integration.integrationType === integrationType && !integration.error,
  );
}

function roundedSearchDelta(searchRegression: SearchRegressionSignal | null) {
  return searchRegression ? Math.round(searchRegression.deltaPct) : null;
}

export function deriveDashboardSearchSignals({
  integrations,
  searchRegression,
}: {
  integrations: IntegrationData[];
  searchRegression: SearchRegressionSignal | null;
}): DashboardSearchSignals {
  const gscData = readSearchConsoleData(
    readHealthyIntegration(integrations, "googlesearchconsole")?.data,
  );
  const bingData = readBingData(readHealthyIntegration(integrations, "bingwebmaster")?.data);

  if (gscData) {
    return {
      seoClicks: {
        clicks: gscData.total_clicks,
        impressions: gscData.total_impressions,
        avgPosition: finiteNumber(gscData.average_position),
        deltaPct: roundedSearchDelta(searchRegression),
      },
      searchIndex: {
        sourceLabel: "Search Console",
        visiblePageCount: arrayValue(gscData.top_pages).length,
        totalClicks: gscData.total_clicks,
        totalImpressions: gscData.total_impressions,
      },
    };
  }

  if (bingData) {
    return {
      seoClicks: {
        clicks: bingData.total_clicks,
        impressions: bingData.total_impressions,
        avgPosition: finiteNumber(bingData.avg_position),
        deltaPct: roundedSearchDelta(searchRegression),
      },
      searchIndex: {
        sourceLabel: "Bing",
        visiblePageCount: arrayValue(bingData.top_pages).length,
        totalClicks: bingData.total_clicks,
        totalImpressions: bingData.total_impressions,
      },
    };
  }

  return {
    seoClicks: null,
    searchIndex: null,
  };
}
