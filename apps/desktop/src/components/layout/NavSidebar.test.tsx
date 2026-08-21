import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { useIssuesBadgeMock, useNavBadgesMock, useNavIntegrationsMock, useTierMock, useThemeMock } =
  vi.hoisted(() => ({
    useIssuesBadgeMock: vi.fn(),
    useNavBadgesMock: vi.fn(),
    useNavIntegrationsMock: vi.fn(),
    useTierMock: vi.fn(),
    useThemeMock: vi.fn(),
  }));

vi.mock("@/lib/nav-badges", () => ({
  useNavBadges: () => useNavBadgesMock(),
  useNavIntegrations: () => useNavIntegrationsMock(),
}));

vi.mock("@/lib/issues-badge", () => ({
  useIssuesBadge: () => useIssuesBadgeMock(),
}));

vi.mock("@/hooks/useTier", () => ({
  useTier: () => useTierMock(),
}));

vi.mock("@/hooks/useTheme", () => ({
  useTheme: () => useThemeMock(),
}));

import { NavSidebar } from "./NavSidebar";

describe("NavSidebar", () => {
  beforeEach(() => {
    localStorage.clear();
    useIssuesBadgeMock.mockReturnValue(null);
    useNavBadgesMock.mockReturnValue({ updates: null, launch: null });
    useNavIntegrationsMock.mockReturnValue(new Set<string>());
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });
    useThemeMock.mockReturnValue({ theme: "dark", resolved: "dark", setTheme: vi.fn() });
  });

  it("renders the brand logo image, swapping artwork to match the theme", () => {
    // Dark mode: the white wordmark reads on the dark sidebar.
    useThemeMock.mockReturnValue({ theme: "dark", resolved: "dark", setTheme: vi.fn() });
    const { unmount } = render(
      <NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />,
    );
    const darkLogo = screen.getByRole("img", { name: "SiteCMD" });
    // Must be the image asset, not a text/SVG wordmark.
    expect(darkLogo.tagName.toLowerCase()).toBe("img");
    expect(darkLogo).toHaveAttribute("src", "/images/logo.png");
    unmount();

    // Light mode: the dark-text wordmark stays legible on the light sidebar.
    useThemeMock.mockReturnValue({ theme: "light", resolved: "light", setTheme: vi.fn() });
    render(<NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />);
    expect(screen.getByRole("img", { name: "SiteCMD" })).toHaveAttribute(
      "src",
      "/images/logo-dark.png",
    );
  });

  it("uses the favicon instead of squishing the full logo when collapsed", () => {
    render(<NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));

    expect(screen.getByRole("img", { name: "SiteCMD" })).toHaveAttribute("src", "/favicon.svg");
  });

  it("always shows the core section labels, and Monitor only once something is connected", () => {
    useNavBadgesMock.mockReturnValue({ updates: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    const { rerender } = render(
      <NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />,
    );

    // Core groups are always present; Monitor is absent with no integrations.
    expect(screen.getByText("Manage")).toHaveClass("nav-group-label");
    expect(screen.getByText("History")).toHaveClass("nav-group-label");
    expect(screen.queryByText("Monitor")).not.toBeInTheDocument();

    // Connecting a source that feeds a progressive page reveals the group.
    useNavIntegrationsMock.mockReturnValue(new Set(["plausible"]));
    rerender(<NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />);
    expect(screen.getByText("Monitor")).toHaveClass("nav-group-label");
  });

  it("keeps the cross-site Overview link aligned with the other sidebar links", () => {
    useNavBadgesMock.mockReturnValue({ updates: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    render(<NavSidebar activePage="dashboard" projectCount={2} onNavigate={vi.fn()} />);

    const overview = screen.getByRole("button", { name: "Overview" });
    expect(overview).toHaveClass("nav-item");
    expect(overview.parentElement).not.toHaveClass("px-2");
  });

  it("shows integrations as a primary workspace destination", () => {
    useNavBadgesMock.mockReturnValue({ updates: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    render(<NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />);

    expect(screen.getByRole("button", { name: "Integrations" })).toBeInTheDocument();
  });

  it("puts Settings in the sidebar utility row", () => {
    const onNavigate = vi.fn();
    useNavBadgesMock.mockReturnValue({ updates: null, launch: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    render(<NavSidebar activePage="dashboard" projectCount={1} onNavigate={onNavigate} />);

    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(onNavigate).toHaveBeenCalledWith("settings");
  });

  function navLabels() {
    return Array.from(document.querySelectorAll(".nav-item"))
      .map((button) => button.textContent?.trim())
      .filter(Boolean);
  }

  it("shows only the core loop when the project has no integrations connected", () => {
    useNavBadgesMock.mockReturnValue({ updates: null, launch: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    render(<NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />);

    expect(navLabels()).toEqual([
      "Dashboard",
      "Issues",
      "Alerts",
      "Updates",
      "Integrations",
      "Activity",
      "Reports",
    ]);
  });

  it("reveals each integration-fed page when its source is connected", () => {
    useNavBadgesMock.mockReturnValue({ updates: null, launch: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    // Plausible feeds Traffic, Search Console feeds Search & SEO, GitHub feeds Deploys.
    useNavIntegrationsMock.mockReturnValue(new Set(["plausible", "googlesearchconsole", "github"]));
    render(<NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />);

    expect(navLabels()).toEqual([
      "Dashboard",
      "Issues",
      "Alerts",
      "Updates",
      "Integrations",
      "Traffic",
      "Search & SEO",
      "Activity",
      "Deploys",
      "Reports",
    ]);
  });

  it("reveals only the pages whose specific source is connected", () => {
    useNavBadgesMock.mockReturnValue({ updates: null, launch: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    // GitHub alone should surface Deploys but not Traffic or Search & SEO.
    useNavIntegrationsMock.mockReturnValue(new Set(["github"]));
    render(<NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />);

    const labels = navLabels();
    expect(labels).toContain("Deploys");
    expect(labels).not.toContain("Traffic");
    expect(labels).not.toContain("Search & SEO");
  });

  it("shows Deploys once a local folder is linked, even with no GitHub connected", () => {
    useNavBadgesMock.mockReturnValue({ updates: null, launch: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    useNavIntegrationsMock.mockReturnValue(new Set<string>());
    render(
      <NavSidebar activePage="dashboard" projectCount={1} hasLinkedFolder onNavigate={vi.fn()} />,
    );

    const labels = navLabels();
    expect(labels).toContain("Deploys");
    expect(labels).not.toContain("Traffic");
    expect(labels).not.toContain("Search & SEO");
  });

  it("keeps Deploys hidden when there is neither a linked folder nor GitHub", () => {
    useNavBadgesMock.mockReturnValue({ updates: null, launch: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    useNavIntegrationsMock.mockReturnValue(new Set<string>());
    render(
      <NavSidebar
        activePage="dashboard"
        projectCount={1}
        hasLinkedFolder={false}
        onNavigate={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Deploys" })).not.toBeInTheDocument();
  });

  it("keeps the active page in the sidebar even when its source is not connected", () => {
    useNavBadgesMock.mockReturnValue({ updates: null, launch: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    render(<NavSidebar activePage="analytics" projectCount={1} onNavigate={vi.fn()} />);

    const traffic = screen.getByRole("button", { name: "Traffic" });
    expect(traffic).toHaveClass("nav-item-active");
    expect(screen.queryByRole("button", { name: "Deploys" })).not.toBeInTheDocument();
  });

  it("shows one shared hover label for the sidebar utility buttons", () => {
    useNavBadgesMock.mockReturnValue({ updates: null, launch: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    render(<NavSidebar activePage="settings" projectCount={1} onNavigate={vi.fn()} />);

    const settingsButton = screen.getByRole("button", { name: "Settings" });
    expect(settingsButton).not.toHaveAttribute("title");

    fireEvent.mouseEnter(settingsButton);
    expect(screen.getByText("Settings")).toHaveClass("nav-utility-tooltip-visible");

    fireEvent.mouseEnter(screen.getByRole("button", { name: "Collapse sidebar" }));
    expect(screen.getByText("Collapse sidebar")).toHaveClass("nav-utility-tooltip-visible");
  });

  it("keeps Launch out of the main sidebar nav", () => {
    useNavBadgesMock.mockReturnValue({
      updates: null,
      launch: { projectId: 7, total: 3 },
    });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    render(
      <NavSidebar
        activePage="dashboard"
        activeProjectId={7}
        projectCount={1}
        onNavigate={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Launch" })).not.toBeInTheDocument();
    expect(screen.queryByTitle("3 launch blockers")).not.toBeInTheDocument();
  });

  it("keeps sidebar count badges neutral unless the page has critical items", () => {
    useNavBadgesMock.mockReturnValue({
      updates: { projectId: 7, total: 5, critical: 2 },
      launch: { projectId: 7, total: 3 },
    });
    useIssuesBadgeMock.mockReturnValue({ projectId: 7, total: 12, critical: 1 });

    render(
      <NavSidebar
        activePage="dashboard"
        activeProjectId={7}
        projectCount={1}
        onNavigate={vi.fn()}
        alertsBadge={4}
        alertsCriticalBadge={1}
      />,
    );

    const updatesBadge = screen.getByTitle(
      "2 critical security updates out of 5 total package updates",
    );
    expect(updatesBadge).toHaveTextContent("5");
    expect(updatesBadge).toHaveClass("nav-count-badge", "nav-count-critical");

    const issuesBadge = screen.getByTitle("12 active issues");
    expect(issuesBadge).toHaveClass("nav-count-badge", "nav-count-critical");

    const alertsBadge = screen.getByTitle("1 critical unread alert out of 4 unread alerts");
    expect(alertsBadge).toHaveTextContent("4");
    expect(alertsBadge).toHaveClass("nav-count-badge", "nav-count-critical");
  });

  it("keeps the primary scan action out of the sidebar", () => {
    useNavBadgesMock.mockReturnValue({ updates: null });
    useTierMock.mockReturnValue({ hasFeature: vi.fn(() => true) });

    render(<NavSidebar activePage="dashboard" projectCount={1} onNavigate={vi.fn()} />);

    expect(screen.queryByRole("button", { name: "Run Scan" })).not.toBeInTheDocument();
  });
});
