import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, hasFeatureMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  hasFeatureMock: vi.fn(() => true),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@/app/ShellHeader", () => ({
  HeaderActions: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
}));

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    hasFeature: hasFeatureMock,
  }),
}));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock("@/components/settings/InlineIntegrationSetup", () => ({
  InlineIntegrationSetup: ({
    serviceTypes = [],
    allowReconnect = [],
  }: {
    serviceTypes?: string[];
    allowReconnect?: string[];
  }) =>
    React.createElement(
      "div",
      {
        "data-testid": "inline-integration-setup",
        "data-service-types": serviceTypes.join(","),
        "data-allow-reconnect": allowReconnect.join(","),
      },
      "InlineIntegrationSetup",
    ),
}));

import { AnalyticsPage } from "./AnalyticsPage";
import { __resetAnalyticsSnapshotCacheForTests } from "@/lib/analytics-snapshot-cache";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

function renderAnalyticsPage(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient(createTestQueryClient()) });
}

function plausibleData() {
  return {
    period: "30d",
    aggregate: {
      visitors: 42892,
      pageviews: 184200,
      bounce_rate: 24.8,
      visit_duration: 252,
    },
    points: [
      { date: "2026-04-10", visitors: 1200, pageviews: 4000, bounce_rate: 22, visit_duration: 240 },
      { date: "2026-04-11", visitors: 1500, pageviews: 4600, bounce_rate: 25, visit_duration: 250 },
      { date: "2026-04-12", visitors: 1800, pageviews: 5200, bounce_rate: 24, visit_duration: 255 },
    ],
    top_pages: [
      { page: "/", visitors: 3200 },
      { page: "/pricing", visitors: 2100 },
    ],
    top_sources: [
      { source: "Google", visitors: 2800 },
      { source: "Direct / None", visitors: 1700 },
    ],
    countries: [{ country: "US", visitors: 2400 }],
    devices: [{ device: "Desktop", visitors: 2600 }],
    browsers: [{ browser: "Chrome", visitors: 2200 }],
  };
}

describe("AnalyticsPage behavior", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    hasFeatureMock.mockReset();
    hasFeatureMock.mockReturnValue(true);
    window.localStorage.clear();
    __resetAnalyticsSnapshotCacheForTests();
  });

  it("shows the real empty state when no analytics providers are connected", async () => {
    invokeMock.mockResolvedValue({});

    renderAnalyticsPage(
      <AnalyticsPage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("No analytics data yet")).toBeInTheDocument();
    });

    expect(screen.getByText("InlineIntegrationSetup")).toBeInTheDocument();
  });

  it("treats a backend no-integrations response as the empty analytics state", async () => {
    invokeMock.mockRejectedValue(
      "No analytics integrations configured. Connect an analytics service on the Settings page.",
    );

    renderAnalyticsPage(
      <AnalyticsPage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("No analytics data yet")).toBeInTheDocument();
    });

    expect(screen.queryByText("Analytics could not load")).not.toBeInTheDocument();
    expect(screen.getByText("InlineIntegrationSetup")).toBeInTheDocument();
  });

  it("offers reconnect (not a blocking surface) for a connected provider returning no data", async () => {
    invokeMock.mockResolvedValue({
      plausible_error: "Plausible API returned 404 Not Found",
    });

    renderAnalyticsPage(
      <AnalyticsPage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("No analytics data yet")).toBeInTheDocument();
    });

    // Two states only: no full-page "connected but not returning data" takeover.
    expect(
      screen.queryByText("Plausible is connected but not returning data"),
    ).not.toBeInTheDocument();

    // The broken provider is offered for reconnect, not hidden.
    const setup = screen.getByTestId("inline-integration-setup");
    expect(setup.getAttribute("data-allow-reconnect")?.split(",")).toContain("plausible");
  });

  it("shows a page-shaped loading skeleton while analytics is still loading", () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "fetch_analytics") {
        return new Promise(() => {});
      }
      return Promise.resolve(null);
    });

    renderAnalyticsPage(
      <AnalyticsPage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    expect(screen.getByLabelText("Analytics loading state")).toBeInTheDocument();
  });

  it("retries a failed analytics load instead of leaving the page stranded", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "fetch_analytics") {
        const fetchCalls = invokeMock.mock.calls.filter(
          ([name]) => name === "fetch_analytics",
        ).length;
        return fetchCalls === 1
          ? Promise.reject(new Error("provider timeout"))
          : Promise.resolve({});
      }
      if (command === "invalidate_analytics_cache") {
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });

    renderAnalyticsPage(
      <AnalyticsPage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Analytics could not load")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() => {
      expect(screen.getByText("No analytics data yet")).toBeInTheDocument();
    });
  });

  it("renders the real traffic surface and opens the sources modal", async () => {
    const onNavigate = vi.fn();

    invokeMock.mockResolvedValue({
      plausible: plausibleData(),
      search_console: {
        total_clicks: 900,
        total_impressions: 18000,
        average_ctr: 5,
        average_position: 11.2,
        top_queries: [],
        top_pages: [],
        daily: [],
        devices: [],
      },
    });

    renderAnalyticsPage(
      <AnalyticsPage projectId={7} url="https://example.com" onNavigate={onNavigate} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Recent Traffic")).toBeInTheDocument();
    });

    expect(screen.getByRole("button", { name: "30 Days" })).toBeInTheDocument();
    expect(screen.getByText(/^Updated/)).toBeInTheDocument();
    expect(screen.queryByText("Last updated")).not.toBeInTheDocument();

    expect(
      screen.queryByText("Traffic, uptime, and CDN health in one place"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("4.5K")).toBeInTheDocument();
    expect(screen.getByText("30 Days total")).toBeInTheDocument();
    expect(screen.getByText("Top Pages")).toBeInTheDocument();
    expect(screen.getByText("Traffic Sources")).toBeInTheDocument();

    // Sources live behind a button that opens a modal, not an always-on card.
    expect(screen.queryByRole("button", { name: /Plausible: Added/i })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Sources" }));

    expect(screen.getByRole("button", { name: /Plausible: Added/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /GA4: Not added/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Cloudflare: Not added/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /UptimeRobot: Not added/i })).toBeInTheDocument();
    // Search providers must remain absent from Traffic.
    expect(screen.queryByRole("button", { name: /Search Console/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Bing/i })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /GA4: Not added/i }));
    expect(onNavigate).toHaveBeenCalledWith("integrations:googleanalytics");
  });
});
