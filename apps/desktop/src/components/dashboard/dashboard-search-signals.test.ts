import { describe, expect, it } from "vitest";
import { deriveDashboardSearchSignals } from "./dashboard-search-signals";

describe("deriveDashboardSearchSignals", () => {
  it("uses real Search Console totals for dashboard SEO and search cards", () => {
    const signals = deriveDashboardSearchSignals({
      integrations: [
        {
          integrationType: "googlesearchconsole",
          error: null,
          data: {
            total_clicks: 123,
            total_impressions: 4567,
            average_ctr: 0.04,
            average_position: 8.4,
            top_queries: [],
            top_pages: [
              {
                page: "https://example.com/",
                clicks: 80,
                impressions: 3000,
                ctr: 0.03,
                position: 7,
              },
              {
                page: "https://example.com/pricing",
                clicks: 43,
                impressions: 1567,
                ctr: 0.03,
                position: 11,
              },
            ],
            daily: [],
            devices: [],
          },
        },
      ],
      searchRegression: { source: "gsc", deltaPct: -12.4 },
    });

    expect(signals.seoClicks).toMatchObject({
      clicks: 123,
      impressions: 4567,
      avgPosition: 8.4,
      deltaPct: -12,
    });
    expect(signals.searchIndex).toMatchObject({
      sourceLabel: "Search Console",
      visiblePageCount: 2,
      totalClicks: 123,
      totalImpressions: 4567,
    });
  });

  it("does not fabricate zero-click Search Console data from regression state alone", () => {
    const signals = deriveDashboardSearchSignals({
      integrations: [],
      searchRegression: { source: "gsc", deltaPct: -18 },
    });

    expect(signals.seoClicks).toBeNull();
    expect(signals.searchIndex).toBeNull();
  });

  it("uses Bing data for the Search & Index reference card when Search Console has no data", () => {
    const signals = deriveDashboardSearchSignals({
      integrations: [
        {
          integrationType: "bingwebmaster",
          error: null,
          data: {
            total_clicks: 34,
            total_impressions: 901,
            avg_position: 12,
            daily_stats: [],
            top_queries: [],
            top_pages: [
              { url: "https://example.com/", clicks: 34, impressions: 901, avg_position: 12 },
            ],
            crawl_errors: 0,
          },
        },
      ],
      searchRegression: null,
    });

    expect(signals.seoClicks).toMatchObject({
      clicks: 34,
      impressions: 901,
      avgPosition: 12,
    });
    expect(signals.searchIndex).toMatchObject({
      sourceLabel: "Bing",
      visiblePageCount: 1,
      totalClicks: 34,
      totalImpressions: 901,
    });
  });
});
