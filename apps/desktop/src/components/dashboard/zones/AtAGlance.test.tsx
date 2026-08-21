import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AtAGlance } from "./AtAGlance";

const noop = vi.fn();

const cleanBreakdown = {
  overall: 87,
  base: 100,
  deductions: [],
  hasDeductions: false,
  exploitableCapped: false,
  floorApplied: false,
  capNote: null,
  floorNote: null,
};

const baseProps = {
  siteScore: {
    value: 87,
    delta: -2,
    issueCount: 12,
    criticalCount: 2,
    scanAgeLabel: "14m",
    breakdown: cleanBreakdown,
  },
  lastChecked: {
    label: "14m ago",
    kind: "web" as const,
    sub: "Web 14m ago · Code 2h ago",
    stale: false,
  },
  uptime: { ratio: 99.97, avgResponseMs: 312, outageCount: 0 },
  visitors: { visitors: 42100, pageviews: 138000, bouncePct: 24, deltaPct: 12 },
  seoClicks: { clicks: 2400, impressions: 89000, avgPosition: 14.2, deltaPct: -18 },
  onOpenIssues: noop,
  onOpenUptime: noop,
  onOpenAnalytics: noop,
  onOpenSearchConsole: noop,
  onOpenIntegrations: noop,
};

describe("AtAGlance", () => {
  it("renders the four tile labels and no separate Last Checked tile", () => {
    render(<AtAGlance {...baseProps} />);
    expect(screen.getByText("SiteCMD Score")).toBeInTheDocument();
    expect(screen.getByText("Uptime 30d")).toBeInTheDocument();
    expect(screen.getByText("Visitors 30d")).toBeInTheDocument();
    expect(screen.getByText("SEO clicks 28d")).toBeInTheDocument();
    expect(screen.queryByText("Last Checked")).not.toBeInTheDocument();
  });

  it("routes tile clicks to correct handlers", () => {
    const handlers = {
      onOpenIssues: vi.fn(),
      onOpenUptime: vi.fn(),
      onOpenAnalytics: vi.fn(),
      onOpenSearchConsole: vi.fn(),
      onOpenIntegrations: vi.fn(),
    };
    render(<AtAGlance {...baseProps} {...handlers} />);

    fireEvent.click(screen.getByText("SiteCMD Score").closest("button")!);
    expect(handlers.onOpenIssues).toHaveBeenCalled();

    fireEvent.click(screen.getByText("Visitors 30d").closest("button")!);
    expect(handlers.onOpenAnalytics).toHaveBeenCalled();

    fireEvent.click(screen.getByText("SEO clicks 28d").closest("button")!);
    expect(handlers.onOpenSearchConsole).toHaveBeenCalled();
  });

  it("renders the SiteCMD score tile as a compact number without percent symbols", () => {
    render(<AtAGlance {...baseProps} />);

    const siteScoreTile = screen.getByText("SiteCMD Score").closest("button")!;

    expect(siteScoreTile).toHaveTextContent("87");
    expect(siteScoreTile).toHaveTextContent("Updated 14m ago");
    expect(siteScoreTile).not.toHaveTextContent("issues");
    expect(siteScoreTile).not.toHaveTextContent("%");
    expect(siteScoreTile).not.toHaveTextContent("/100");
  });

  // Deduction details belong to the Issues page, not the score tile.
  it("keeps the per-tier deduction math off the score tile", () => {
    render(
      <AtAGlance
        {...baseProps}
        siteScore={{
          ...baseProps.siteScore,
          breakdown: {
            ...cleanBreakdown,
            deductions: [
              { tier: "high", label: "High", points: 5 },
              { tier: "medium", label: "Medium", points: 21 },
              { tier: "low", label: "Low", points: 6 },
            ],
            hasDeductions: true,
          },
        }}
      />,
    );
    const siteScoreTile = screen.getByText("SiteCMD Score").closest("button")!;
    expect(siteScoreTile).not.toHaveTextContent("-5 High");
    expect(siteScoreTile).not.toHaveTextContent("-21 Medium");
    expect(siteScoreTile).not.toHaveTextContent("-6 Low");
    // The number and its freshness are all the tile owes the reader.
    expect(siteScoreTile).toHaveTextContent("87");
    expect(siteScoreTile).toHaveTextContent("Updated 14m ago");
  });

  it("still shows the capped note when the score is capped (D7)", () => {
    render(
      <AtAGlance
        {...baseProps}
        siteScore={{
          ...baseProps.siteScore,
          breakdown: {
            ...cleanBreakdown,
            deductions: [{ tier: "critical", label: "Critical", points: 70 }],
            hasDeductions: true,
            exploitableCapped: true,
            capNote: "Score capped: a confirmed-exploitable critical issue was found.",
          },
        }}
      />,
    );
    const siteScoreTile = screen.getByText("SiteCMD Score").closest("button")!;
    expect(siteScoreTile).toHaveTextContent("Score capped");
  });

  it("shows empty-state CTA when siteScore is null", () => {
    render(<AtAGlance {...baseProps} siteScore={null} />);
    expect(screen.getByText(/Run Scan/i)).toBeInTheDocument();
  });

  it("shows empty-state CTA when uptime is null", () => {
    render(<AtAGlance {...baseProps} uptime={null} />);
    expect(screen.getByText("Connect")).toBeInTheDocument();
  });

  it("shows a loading message on the uptime tile while configured uptime data is loading", () => {
    render(<AtAGlance {...baseProps} uptime={null} uptimeConfigured uptimeLoading />);
    expect(screen.getByText("Loading uptime...")).toBeInTheDocument();
    expect(screen.queryByText("Connect")).not.toBeInTheDocument();
  });

  it("routes configured uptime without loaded data to the uptime view instead of Integrations", () => {
    const onOpenUptime = vi.fn();
    const onOpenIntegrations = vi.fn();
    render(
      <AtAGlance
        {...baseProps}
        uptime={null}
        uptimeConfigured
        onOpenUptime={onOpenUptime}
        onOpenIntegrations={onOpenIntegrations}
      />,
    );

    const uptimeTile = screen.getByText("Uptime 30d").closest("button")!;
    expect(uptimeTile).toHaveTextContent("View Uptime");

    fireEvent.click(uptimeTile);
    expect(onOpenUptime).toHaveBeenCalled();
    expect(onOpenIntegrations).not.toHaveBeenCalled();
  });

  it("shows empty-state CTA when visitors is null", () => {
    render(<AtAGlance {...baseProps} visitors={null} />);
    expect(screen.getByText("Connect")).toBeInTheDocument();
  });

  it("routes integration-backed connect states to Integrations", () => {
    const onOpenIntegrations = vi.fn();
    render(
      <AtAGlance
        {...baseProps}
        uptime={null}
        visitors={null}
        analyticsConfigured={false}
        seoClicks={null}
        onOpenIntegrations={onOpenIntegrations}
      />,
    );

    const connectButtons = screen.getAllByRole("button", { name: /connect/i });
    fireEvent.click(connectButtons[0]!);
    expect(onOpenIntegrations).toHaveBeenCalled();
  });

  it("routes configured search without loaded totals to Search instead of Integrations", () => {
    const onOpenSearchConsole = vi.fn();
    const onOpenIntegrations = vi.fn();

    render(
      <AtAGlance
        {...baseProps}
        seoClicks={null}
        searchConfigured
        onOpenSearchConsole={onOpenSearchConsole}
        onOpenIntegrations={onOpenIntegrations}
      />,
    );

    const seoTile = screen.getByText("SEO clicks 28d").closest("button")!;
    expect(seoTile).toHaveTextContent("View Search");

    fireEvent.click(seoTile);
    expect(onOpenSearchConsole).toHaveBeenCalled();
    expect(onOpenIntegrations).not.toHaveBeenCalled();
  });

  it("shows stale last-checked timing in the score tile with the warning color", () => {
    render(
      <AtAGlance
        {...baseProps}
        lastChecked={{ label: "9d ago", kind: "web", sub: null, stale: true }}
      />,
    );
    const scoreTile = screen.getByText("SiteCMD Score").closest("button")!;
    expect(scoreTile).toHaveTextContent("Updated 9d ago");
    expect(scoreTile.querySelector(".text-severity-high")).not.toBeNull();
  });
});
