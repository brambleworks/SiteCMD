import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, openDialogMock } = vi.hoisted(() => ({
  invokeMock: vi.fn<(...args: unknown[]) => Promise<unknown>>(() => Promise.resolve(null)),
  openDialogMock: vi.fn<(...args: unknown[]) => Promise<string | null>>(() =>
    Promise.resolve(null),
  ),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(() => Promise.resolve(null)),
  open: openDialogMock,
}));
vi.mock("@tauri-apps/plugin-autostart", () => ({
  disable: vi.fn(() => Promise.resolve()),
  enable: vi.fn(() => Promise.resolve()),
  isEnabled: vi.fn(() => Promise.resolve(false)),
}));
vi.mock("@tauri-apps/plugin-os", () => ({
  platform: vi.fn(() => "macOS"),
  version: vi.fn(() => "15.0"),
  arch: vi.fn(() => "arm64"),
}));

vi.mock("@/lib/clipboard", () => ({
  copyToClipboard: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/hooks/useTheme", () => ({
  useTheme: () => ({
    theme: "dark",
    setTheme: vi.fn(),
  }),
}));

vi.mock("@/hooks/useScanPrefs", () => ({
  useScanPrefs: () => ({
    prefs: {
      timeout: 30,
      retentionLimit: 30,
      categories: {
        security: true,
        seo: true,
        performance: true,
        accessibility: true,
        polish: true,
      },
    },
    setPrefs: vi.fn(),
  }),
}));

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    hasFeature: vi.fn(() => false),
  }),
}));

vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  }),
}));

vi.mock("@/lib/desktop-prefs", () => ({
  useDesktopPrefs: () => ({
    prefs: {
      backgroundMonitoring: false,
      fileWatchSuggestions: false,
      desktopNotifications: false,
      refreshOnFocus: false,
    },
    updatePrefs: vi.fn(),
  }),
}));

vi.mock("@/lib/observability", () => ({
  buildObservabilitySnapshotText: vi.fn(() => "observability"),
}));

vi.mock("@/lib/performance-metrics", () => ({
  buildPerformanceSnapshotText: vi.fn(() => "performance"),
}));

vi.mock("./SitemapSection", () => ({
  SitemapSection: ({ siteUrl }: { siteUrl?: string }) =>
    React.createElement("div", null, `SitemapSection:${siteUrl ?? "none"}`),
}));

vi.mock("./AccountSettings", () => ({
  AccountSection: () => React.createElement("div", null, "AccountSection"),
}));

vi.mock("@/components/scan/ScanScheduleCard", () => ({
  ScanScheduleCard: ({ projectId }: { projectId?: number }) =>
    React.createElement("div", null, `ScanScheduleCard:${projectId ?? "none"}`),
}));

vi.mock("./CICDSection", () => ({
  CICDSection: ({ siteUrl }: { siteUrl?: string }) =>
    React.createElement("div", null, `CICDSection:${siteUrl ?? "none"}`),
}));

vi.mock("./WebhooksSection", () => ({
  WebhooksSection: ({ projectId }: { projectId?: number }) =>
    React.createElement("div", null, `WebhooksSection:${projectId ?? "none"}`),
}));

vi.mock("./ConnectedServiceSection", () => ({
  ConnectedServiceSection: ({
    projectId,
    environmentScopeKey,
  }: {
    projectId?: number;
    environmentScopeKey?: string;
  }) => {
    const [draft, setDraft] = React.useState("");
    return React.createElement(
      "div",
      null,
      `ConnectedServiceSection:${projectId ?? "none"}:${environmentScopeKey ?? "none"}`,
      React.createElement("input", {
        "aria-label": "Connected site draft",
        onChange: (event: React.ChangeEvent<HTMLInputElement>) => setDraft(event.target.value),
        value: draft,
      }),
    );
  },
}));

import { SettingsPage } from "./SettingsPage";
import { withQueryClient } from "@/test-utils/query-client";

function renderSettings(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient() });
}

describe("SettingsPage behavior", () => {
  beforeEach(() => {
    localStorage.clear();
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
    openDialogMock.mockReset();
    openDialogMock.mockResolvedValue(null);
  });

  it("renders the requested initial tab and lets the user move between real settings sections", () => {
    renderSettings(<SettingsPage projectId={7} url="https://example.com" initialTab="cicd" />);

    expect(screen.getByRole("heading", { name: "Automation" })).toBeInTheDocument();
    expect(screen.getByText("CICDSection:https://example.com")).toBeInTheDocument();
    expect(screen.getByText("WebhooksSection:7")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^Connected\b/i }));
    expect(screen.getByRole("heading", { name: "Connected" })).toBeInTheDocument();
    expect(screen.getByText("ConnectedServiceSection:7:https://example.com")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^Account & Billing\b/i }));
    expect(screen.getByRole("heading", { name: "Account & Billing" })).toBeInTheDocument();
    expect(screen.getByText("AccountSection")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^Scanning\b/i }));
    expect(screen.getByRole("heading", { name: "Scanning" })).toBeInTheDocument();
    expect(screen.getByText("Per-check timeout")).toBeInTheDocument();
    expect(screen.getByText("Scan history to keep")).toBeInTheDocument();
    expect(screen.getByText("ScanScheduleCard:7")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^Site Setup\b/i }));
    expect(screen.getByRole("heading", { name: "Site Setup" })).toBeInTheDocument();
    expect(screen.getByText("SitemapSection:https://example.com")).toBeInTheDocument();
    // Project deletion lives at the bottom of Site Setup, after the sitemap.
    expect(screen.getByRole("heading", { name: "Remove This Project" })).toBeInTheDocument();
  });

  it("keeps project-specific settings disabled when no project is selected", () => {
    renderSettings(<SettingsPage initialTab="account" />);

    const automationButton = screen.getByRole("button", { name: /^Automation\b/i });
    const scanningButton = screen.getByRole("button", { name: /^Scanning\b/i });
    const projectButton = screen.getByRole("button", { name: /^Site Setup\b/i });
    const connectedButton = screen.getByRole("button", { name: /^Connected\b/i });
    expect(automationButton).toBeDisabled();
    expect(scanningButton).toBeDisabled();
    expect(projectButton).toBeDisabled();
    expect(connectedButton).toBeDisabled();

    fireEvent.click(automationButton);
    expect(screen.getByRole("heading", { name: "Account & Billing" })).toBeInTheDocument();
    expect(screen.getByText("AccountSection")).toBeInTheDocument();
    expect(screen.queryByText(/WebhooksSection:/)).not.toBeInTheDocument();
  });

  it.each([
    ["environment", 7, "https://alpha.example", 7, "https://beta.example"],
    ["project", 7, "https://shared.example", 8, "https://shared.example"],
  ])(
    "discards connected-site state when the selected %s changes",
    (_change, initialProjectId, initialUrl, nextProjectId, nextUrl) => {
      const { rerender } = renderSettings(
        <SettingsPage projectId={initialProjectId} url={initialUrl} initialTab="connected" />,
      );

      const draft = screen.getByLabelText("Connected site draft");
      fireEvent.change(draft, { target: { value: "site-a-secret" } });
      expect(draft).toHaveValue("site-a-secret");

      rerender(<SettingsPage projectId={nextProjectId} url={nextUrl} initialTab="connected" />);

      expect(
        screen.getByText(`ConnectedServiceSection:${nextProjectId}:${nextUrl}`),
      ).toBeInTheDocument();
      expect(screen.getByLabelText("Connected site draft")).toHaveValue("");
    },
  );

  it("shows project URLs on Site Setup and lets the user add another environment", async () => {
    invokeMock.mockImplementation((...args: unknown[]) => {
      const command = String(args[0]);
      if (command === "get_db_path") return Promise.resolve("/tmp/sitecmd.db");
      if (command === "get_db_size") return Promise.resolve(1024);
      if (command === "add_environment_url") return Promise.resolve(88);
      return Promise.resolve(null);
    });

    const onProjectChanged = vi.fn(() => Promise.resolve());

    renderSettings(
      <SettingsPage
        projectId={7}
        projectName="Alpha"
        initialTab="site-setup"
        projectPath="/tmp/alpha"
        projectEnvironments={[
          {
            id: 77,
            url: "https://alpha.test",
            label: "Alpha (Production)",
            environment: "production",
            source: "manual",
            lastScannedAt: null,
            latestScore: 92,
          },
        ]}
        onProjectChanged={onProjectChanged}
      />,
    );

    expect(screen.getByRole("heading", { name: "Site Setup" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Project Folder" })).toBeInTheDocument();
    expect(screen.getByText("/tmp/alpha")).toBeInTheDocument();
    expect(screen.getByText("Site Environments")).toBeInTheDocument();
    expect(screen.getByText("https://alpha.test")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("https://staging.example.com"), {
      target: { value: "staging.alpha.test" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add URL" }));

    expect(invokeMock).toHaveBeenCalledWith("add_environment_url", {
      projectId: 7,
      url: "https://staging.alpha.test",
      label: "Alpha (Staging)",
      environment: "staging",
    });
  });

  it("shows database, backup, and workspace-wide cleanup on the Data tab", async () => {
    invokeMock.mockImplementation((...args: unknown[]) => {
      const command = String(args[0]);
      if (command === "get_db_path") return Promise.resolve("/tmp/sitecmd.db");
      if (command === "get_db_size") return Promise.resolve(1024);
      return Promise.resolve(null);
    });

    renderSettings(<SettingsPage projectId={7} projectName="Alpha" initialTab="data-support" />);

    expect(screen.getByRole("heading", { name: "Data" })).toBeInTheDocument();
    expect(await screen.findByText("/tmp/sitecmd.db")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Export/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Import/ })).toBeInTheDocument();
    // Data cleanup applies to the full workspace.
    expect(screen.getByText(/every project in this workspace/)).toBeInTheDocument();
    expect(screen.queryByText("Scan history to keep")).not.toBeInTheDocument();
    expect(screen.queryByText("Remove This Project")).not.toBeInTheDocument();
  });

  it("lets the user change the linked project folder from Site Setup", async () => {
    openDialogMock.mockResolvedValue("/tmp/alpha-new");
    invokeMock.mockImplementation((...args: unknown[]) => {
      const command = String(args[0]);
      if (command === "get_db_path") return Promise.resolve("/tmp/sitecmd.db");
      if (command === "get_db_size") return Promise.resolve(1024);
      return Promise.resolve(null);
    });
    const onProjectChanged = vi.fn(() => Promise.resolve());

    renderSettings(
      <SettingsPage
        projectId={7}
        projectName="Alpha"
        initialTab="site-setup"
        projectPath="/tmp/alpha"
        projectEnvironments={[]}
        onProjectChanged={onProjectChanged}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Change Folder" }));

    await waitFor(() => {
      expect(openDialogMock).toHaveBeenCalledWith({
        directory: true,
        multiple: false,
        title: "Select project folder",
      });
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("update_project_path", {
        projectId: 7,
        path: "/tmp/alpha-new",
      });
    });
    await waitFor(() => expect(onProjectChanged).toHaveBeenCalledTimes(1));
  });
});
