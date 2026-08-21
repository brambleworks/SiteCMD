import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ReferenceSignals } from "./ReferenceSignals";

const noop = vi.fn();
const baseHandlers = {
  onOpenWebVitals: noop,
  onOpenSearchConsole: noop,
  onOpenDelivery: noop,
  onOpenDeploys: noop,
  onOpenIntegrations: noop,
};

describe("ReferenceSignals", () => {
  it("renders all four tile labels", () => {
    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={null}
        delivery={null}
        deployRelease={null}
        {...baseHandlers}
      />,
    );
    expect(screen.getByText("Web Vitals")).toBeInTheDocument();
    expect(screen.getByText("Search & Index")).toBeInTheDocument();
    expect(screen.getByText("Delivery")).toBeInTheDocument();
    expect(screen.getByText("Deploy & Release")).toBeInTheDocument();
  });

  it("shows empty-state CTAs when all data is null", () => {
    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={null}
        delivery={null}
        deployRelease={null}
        {...baseHandlers}
      />,
    );
    expect(screen.getByText("Run PageSpeed")).toBeInTheDocument();
    expect(screen.getAllByText("Connect")).toHaveLength(3);
  });

  it("routes the deploy tile to Deploys (not a GitHub connect) when a folder is linked", () => {
    const onOpenDeploys = vi.fn();
    const onOpenIntegrations = vi.fn();
    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={null}
        delivery={null}
        deployRelease={null}
        deploysFolderLinked
        {...baseHandlers}
        onOpenDeploys={onOpenDeploys}
        onOpenIntegrations={onOpenIntegrations}
      />,
    );

    // A linked folder routes the empty tile to local deploy history.
    const deployTile = screen.getByText("Deploy & Release").closest("button")!;
    expect(deployTile).toHaveTextContent("View deploys");
    fireEvent.click(deployTile);
    expect(onOpenDeploys).toHaveBeenCalled();
    expect(onOpenIntegrations).not.toHaveBeenCalled();
    // Only Search and Delivery remain as GitHub-style connect prompts.
    expect(screen.getAllByText("Connect")).toHaveLength(2);
  });

  it("shows a loading state for Web Vitals while PageSpeed is still resolving", () => {
    render(
      <ReferenceSignals
        webVitals={null}
        webVitalsLoading
        searchIndex={null}
        delivery={null}
        deployRelease={null}
        {...baseHandlers}
      />,
    );
    expect(screen.getByText(/Loading PageSpeed/i)).toBeInTheDocument();
    expect(screen.queryByText("Run PageSpeed")).not.toBeInTheDocument();
  });

  it("renders web vitals data when provided", () => {
    render(
      <ReferenceSignals
        webVitals={{
          score: 91,
          lcpMs: 1800,
          cls: 0.04,
          tbtMs: 180,
        }}
        searchIndex={null}
        delivery={null}
        deployRelease={null}
        {...baseHandlers}
      />,
    );
    expect(screen.getByText(/LCP 1\.8s/)).toBeInTheDocument();
    expect(screen.getByText(/CLS 0\.04 · TBT 180ms · Score 91\/100/i)).toBeInTheDocument();
  });

  it("renders real Search Console search visibility data when provided", () => {
    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={{
          sourceLabel: "Search Console",
          visiblePageCount: 2,
          totalClicks: 123,
          totalImpressions: 4567,
        }}
        delivery={null}
        deployRelease={null}
        {...baseHandlers}
      />,
    );

    expect(screen.getByText("2 visible pages")).toBeInTheDocument();
    expect(screen.getByText(/Search Console · 5k impressions/i)).toBeInTheDocument();
  });

  it("shows a score-only summary when PSI returns no LCP/CLS", () => {
    render(
      <ReferenceSignals
        webVitals={{
          score: 74,
          lcpMs: null,
          cls: null,
          tbtMs: null,
        }}
        searchIndex={null}
        delivery={null}
        deployRelease={null}
        {...baseHandlers}
      />,
    );
    expect(screen.getByText("Performance 74/100")).toBeInTheDocument();
    expect(screen.getByText("PageSpeed (mobile)")).toBeInTheDocument();
  });

  it("routes clicks to correct handlers", () => {
    const onOpenDeploys = vi.fn();
    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={null}
        delivery={null}
        deployRelease={{
          tagName: "v2.14",
          conclusion: "success",
          ageLabel: "1h ago",
          commitsSince: 3,
        }}
        {...baseHandlers}
        onOpenDeploys={onOpenDeploys}
      />,
    );
    fireEvent.click(screen.getByText("Deploy & Release").closest("button")!);
    expect(onOpenDeploys).toHaveBeenCalled();
  });

  it("routes connect-state cards to Integrations", () => {
    const onOpenIntegrations = vi.fn();
    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={null}
        delivery={null}
        deployRelease={null}
        {...baseHandlers}
        onOpenIntegrations={onOpenIntegrations}
      />,
    );

    const connectButtons = screen.getAllByRole("button", { name: /connect/i });
    fireEvent.click(connectButtons[0]!);
    expect(onOpenIntegrations).toHaveBeenCalled();
  });

  it("routes configured search without loaded data to Search instead of Integrations", () => {
    const onOpenSearchConsole = vi.fn();
    const onOpenIntegrations = vi.fn();

    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={null}
        searchConfigured
        delivery={null}
        deployRelease={null}
        {...baseHandlers}
        onOpenSearchConsole={onOpenSearchConsole}
        onOpenIntegrations={onOpenIntegrations}
      />,
    );

    const searchTile = screen.getByText("Search & Index").closest("button")!;
    expect(searchTile).toHaveTextContent("View Search");

    fireEvent.click(searchTile);
    expect(onOpenSearchConsole).toHaveBeenCalled();
    expect(onOpenIntegrations).not.toHaveBeenCalled();
  });

  it("shows a loading state on the Delivery tile while configured CDN data is loading", () => {
    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={null}
        delivery={null}
        deliveryConfigured
        deliveryLoading
        deployRelease={null}
        {...baseHandlers}
      />,
    );

    const deliveryTile = screen.getByText("Delivery").closest("button")!;
    expect(deliveryTile).toHaveTextContent("Loading delivery...");
    expect(deliveryTile).not.toHaveTextContent("Connect");
  });

  it("routes configured delivery without loaded data to Delivery instead of Integrations", () => {
    const onOpenDelivery = vi.fn();
    const onOpenIntegrations = vi.fn();

    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={null}
        delivery={null}
        deliveryConfigured
        deployRelease={null}
        {...baseHandlers}
        onOpenDelivery={onOpenDelivery}
        onOpenIntegrations={onOpenIntegrations}
      />,
    );

    const deliveryTile = screen.getByText("Delivery").closest("button")!;
    expect(deliveryTile).toHaveTextContent("View Delivery");

    fireEvent.click(deliveryTile);
    expect(onOpenDelivery).toHaveBeenCalled();
    expect(onOpenIntegrations).not.toHaveBeenCalled();
  });

  it("shows deploy conclusion with color for passed release", () => {
    render(
      <ReferenceSignals
        webVitals={null}
        searchIndex={null}
        delivery={null}
        deployRelease={{
          tagName: "v2.14",
          conclusion: "success",
          ageLabel: "1h ago",
          commitsSince: null,
        }}
        {...baseHandlers}
      />,
    );
    expect(screen.getByText(/v2\.14 passed/i)).toBeInTheDocument();
  });
});
