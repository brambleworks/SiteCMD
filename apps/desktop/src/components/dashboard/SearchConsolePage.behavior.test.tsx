import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock, hasFeatureMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  hasFeatureMock: vi.fn(() => false),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));
vi.mock("@/lib/scan-execution-adapters", () => ({
  getScanHistory: (args: unknown) => invokeMock("get_scan_executions", args),
  getScanDetail: (args: unknown) => invokeMock("get_scan_execution_detail", args),
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

vi.mock("@/lib/desktop-prefs", () => ({
  useDesktopPrefs: () => ({
    prefs: {
      desktopNotifications: false,
    },
  }),
}));

vi.mock("@/lib/desktop-prompts", () => ({
  getLatestDesktopPrompt: vi.fn(() => null),
  useDesktopPromptCenter: () => [],
}));

vi.mock("@/lib/pending-verification", () => ({
  buildPendingVerificationId: vi.fn(() => "pending-id"),
  queuePendingVerification: vi.fn(),
  resolvePendingVerification: vi.fn(),
  usePendingVerificationCenter: () => [],
}));

vi.mock("@/lib/jobs", () => ({
  addJob: vi.fn(),
  completeJob: vi.fn(),
  failJob: vi.fn(),
}));

vi.mock("@/lib/actionable-notifications", () => ({
  sendActionableDesktopNotification: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/components/settings/InlineIntegrationSetup", () => ({
  InlineIntegrationSetup: () => React.createElement("div", null, "Integration setup"),
}));

import { SearchConsolePage } from "./SearchConsolePage";
import { __resetAnalyticsSnapshotCacheForTests } from "@/lib/analytics-snapshot-cache";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

function renderSearchConsolePage(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient(createTestQueryClient()) });
}

describe("SearchConsolePage behavior", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    hasFeatureMock.mockReset();
    hasFeatureMock.mockReturnValue(false);
    window.localStorage.clear();
    __resetAnalyticsSnapshotCacheForTests();
  });

  it("shows a real empty state when there is no SEO data yet and routes to Issues", async () => {
    const onNavigate = vi.fn();

    invokeMock.mockImplementation((command: string) => {
      if (command === "get_scan_executions") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    renderSearchConsolePage(
      <SearchConsolePage projectId={7} url="https://example.com" onNavigate={onNavigate} />,
    );

    await waitFor(() => {
      expect(screen.getByText("No search data yet")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Open Issues" }));
    expect(onNavigate).toHaveBeenCalledWith("issues");
  });

  it("shows a page-shaped loading skeleton while search data is still loading", () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_scan_executions") {
        return new Promise(() => {});
      }
      return Promise.resolve(null);
    });

    renderSearchConsolePage(
      <SearchConsolePage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    expect(screen.getByLabelText("Search loading state")).toBeInTheDocument();
  });

  it("retries a failed search load instead of leaving the page stranded", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_scan_executions") {
        const count = invokeMock.mock.calls.filter(
          ([name]) => name === "get_scan_executions",
        ).length;
        return count === 1 ? Promise.reject(new Error("offline")) : Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    renderSearchConsolePage(
      <SearchConsolePage projectId={7} url="https://example.com" onNavigate={vi.fn()} />,
    );

    await waitFor(() => {
      expect(screen.getByText("Search could not load")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() => {
      expect(screen.getByText("No search data yet")).toBeInTheDocument();
    });
  });
});
