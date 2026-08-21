import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// This suite pins exact update-focused follow-up behavior. Broader page
// rendering and retry states live in UpdatesPage.behavior.test.tsx.

const {
  invokeMock,
  usePendingVerificationCenterMock,
  useDesktopPromptCenterMock,
  addJobMock,
  completeJobMock,
  failJobMock,
  sendActionableDesktopNotificationMock,
  hasFeatureMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  usePendingVerificationCenterMock: vi.fn(),
  useDesktopPromptCenterMock: vi.fn(),
  addJobMock: vi.fn(),
  completeJobMock: vi.fn(),
  failJobMock: vi.fn(),
  sendActionableDesktopNotificationMock: vi.fn(() => Promise.resolve()),
  hasFeatureMock: vi.fn((_: string) => true),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@/app/ShellHeader", () => ({
  HeaderActions: ({ children }: { children: React.ReactNode }) =>
    React.createElement("div", null, children),
}));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => ({
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
  }),
}));
vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    hasFeature: hasFeatureMock,
  }),
}));
vi.mock("@/components/ui/external-link", () => ({
  ExtLink: ({ children, href }: { children: React.ReactNode; href: string }) =>
    React.createElement("a", { href }, children),
}));
vi.mock("@/components/ui/markdown", () => ({
  Markdown: ({ children }: { children: React.ReactNode }) =>
    React.createElement("div", null, children),
}));
vi.mock("@/components/issues/IssueActionBar", () => ({
  IssueActionBar: ({
    verifyAction,
  }: {
    verifyAction?: {
      label?: string;
      onClick: () => void | Promise<void>;
      disabled?: boolean;
    };
  }) =>
    React.createElement(
      "div",
      null,
      "IssueActionBar",
      verifyAction
        ? React.createElement(
            "button",
            {
              type: "button",
              disabled: verifyAction.disabled,
              onClick: () => void verifyAction.onClick(),
            },
            verifyAction.label ?? "Verify now",
          )
        : null,
    ),
}));
vi.mock("@/components/issues/FixWithAgentAction", () => ({
  FixWithAgentAction: () => React.createElement("div", null, "FixWithAgentAction"),
}));
vi.mock("@/components/issues/IssueDossierPanel", async () => {
  const { buildDossierPanelMock } = await import("@/test-utils/dossier-panel-mock");
  return buildDossierPanelMock();
});
vi.mock("@/components/issues/CommandExecutionPanel", () => ({
  CommandExecutionPanel: () => null,
}));
vi.mock("@/components/issues/RecentWatchedFileSection", () => ({
  RecentWatchedFileSection: ({ prompt }: { prompt: { relativePath: string } }) =>
    React.createElement("div", { "data-testid": "recent-watched-file" }, prompt.relativePath),
}));
vi.mock("@/components/issues/WatchedFileArrivalBanner", () => ({
  WatchedFileArrivalBanner: ({
    prompt,
    onOpenFile,
    onReview,
    reviewLabel,
  }: {
    prompt: { relativePath: string; detail: string };
    onOpenFile?: (() => void) | null;
    onReview?: (() => void) | null;
    reviewLabel?: string;
  }) =>
    React.createElement("div", { "data-testid": "arrival-banner" }, [
      React.createElement("div", { key: "path" }, prompt.relativePath),
      React.createElement("div", { key: "detail" }, prompt.detail),
      onOpenFile
        ? React.createElement(
            "button",
            { key: "open", type: "button", onClick: onOpenFile },
            "Open changed file",
          )
        : null,
      onReview && reviewLabel
        ? React.createElement(
            "button",
            { key: "review", type: "button", onClick: onReview },
            reviewLabel,
          )
        : null,
    ]),
}));
vi.mock("@/lib/desktop-prefs", () => ({
  useDesktopPrefs: () => ({
    prefs: {
      desktopNotifications: true,
    },
  }),
}));
vi.mock("@/lib/desktop-actions", () => ({
  openPathInEditor: vi.fn(() => Promise.resolve()),
  revealPath: vi.fn(() => Promise.resolve()),
  runProjectCommand: vi.fn(() => Promise.resolve({ success: true, stdout: "", stderr: "" })),
}));
vi.mock("@/lib/jobs", () => ({
  addJob: addJobMock,
  completeJob: completeJobMock,
  failJob: failJobMock,
}));
vi.mock("@/lib/actionable-notifications", () => ({
  sendActionableDesktopNotification: sendActionableDesktopNotificationMock,
}));
vi.mock("@/lib/desktop-prompts", () => ({
  useDesktopPromptCenter: () => useDesktopPromptCenterMock(),
}));
vi.mock("@/lib/update-memory", () => ({
  getUpdateMemory: vi.fn(() => null),
  getRecentPendingProjectUpdates: vi.fn(() => []),
  markUpdateStillPending: vi.fn(),
  markUpdateVerified: vi.fn(),
  readUpdateSnapshot: vi.fn(() => null),
  recordSeenUpdates: vi.fn(),
  writeUpdateSnapshot: vi.fn(),
}));
vi.mock("@/lib/pending-verification", () => ({
  buildPendingVerificationId: vi.fn(() => "pending-id"),
  queuePendingVerification: vi.fn(),
  resolvePendingVerification: vi.fn(),
  usePendingVerificationCenter: () => usePendingVerificationCenterMock(),
}));
vi.mock("@/lib/action-language", () => ({
  getCopyActionLabel: vi.fn(() => "Copy"),
  getVerificationActionLabel: vi.fn(() => "Verify now"),
  getOpenTargetLabel: vi.fn(
    (target: { page?: string; itemId?: string | null; focus?: string | null }) => {
      if (target.page === "updates" && target.itemId) return "Open Package Update";
      if (target.page === "issues" && target.focus === "security") return "Open security issues";
      return "Open";
    },
  ),
}));
import { UpdatesPage } from "./UpdatesPage";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

function renderUpdatesPage(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient(createTestQueryClient()) });
}

describe("UpdatesPage watched-file arrival", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
    usePendingVerificationCenterMock.mockReturnValue([]);
    useDesktopPromptCenterMock.mockReturnValue([]);
    addJobMock.mockReset();
    completeJobMock.mockReset();
    failJobMock.mockReset();
    sendActionableDesktopNotificationMock.mockReset();
    sendActionableDesktopNotificationMock.mockResolvedValue(undefined);
    Object.defineProperty(window.HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });
  });

  it("frames Updates as the dependency risk surface", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "detect_updates":
          return {
            packages: ["package.json"],
            updates: [
              {
                ecosystem: "npm",
                name: "react",
                currentVersion: "18.2.0",
                latestVersion: "19.0.0",
                updateType: "major",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "npm",
              },
            ],
          };
        default:
          return null;
      }
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-intro"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    // Wait for update data to land so the page settles.
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("detect_updates", expect.any(Object)),
    );
    expect(screen.queryByText("See what changed in dependency risk")).not.toBeInTheDocument();
  });

  it("renders the arrival banner and keeps watched-file context in the opened dossier", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "detect_updates":
          return {
            packages: ["package.json", "package-lock.json"],
            updates: [
              {
                ecosystem: "npm",
                name: "next",
                currentVersion: "14.2.0",
                latestVersion: "14.2.3",
                updateType: "patch",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "npm",
              },
            ],
          };
        default:
          return null;
      }
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example"
        projectName="Example"
        onAddFolder={vi.fn()}
        arrivalPrompt={{
          id: "prompt-2",
          projectId: 7,
          url: "https://example.com",
          page: "updates",
          title: "Dependencies changed",
          relativePath: "package.json",
          absolutePath: "/tmp/example/package.json",
          detail: "Dependencies changed and should be rechecked.",
          kind: "changed-dependencies",
          createdAt: Date.now(),
          updatedAt: Date.now(),
        }}
      />,
    );

    expect(await screen.findByTestId("arrival-banner")).toHaveTextContent("package.json");
    expect(screen.getByRole("button", { name: "Review package changes" })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Open changed file" }).length).toBeGreaterThan(0);
    expect(await screen.findByTestId("issue-dossier")).toHaveTextContent("next");
    expect(screen.getByTestId("recent-watched-file")).toHaveTextContent("package.json");
  });

  it("does not auto-open the dossier when the watched-file arrival still leaves multiple updates", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "detect_updates":
          return {
            packages: ["package.json", "package-lock.json"],
            updates: [
              {
                ecosystem: "npm",
                name: "next",
                currentVersion: "14.2.0",
                latestVersion: "14.2.3",
                updateType: "patch",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "npm",
              },
              {
                ecosystem: "npm",
                name: "react",
                currentVersion: "18.2.0",
                latestVersion: "18.3.0",
                updateType: "minor",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "npm",
              },
            ],
          };
        default:
          return null;
      }
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-many"
        projectName="Example"
        onAddFolder={vi.fn()}
        arrivalPrompt={{
          id: "prompt-4",
          projectId: 7,
          url: "https://example.com",
          page: "updates",
          title: "Dependencies changed",
          relativePath: "package.json",
          absolutePath: "/tmp/example/package.json",
          detail: "Dependencies changed and should be rechecked.",
          kind: "changed-dependencies",
          createdAt: Date.now(),
          updatedAt: Date.now(),
        }}
      />,
    );

    expect(await screen.findByText("next")).toBeInTheDocument();
    expect(screen.queryByTestId("issue-dossier")).not.toBeInTheDocument();
  });

  it("opens the exact package dossier when an initial target item id is provided", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "detect_updates":
          return {
            packages: ["package.json", "package-lock.json"],
            updates: [
              {
                ecosystem: "npm",
                name: "next",
                currentVersion: "14.2.0",
                latestVersion: "14.2.3",
                updateType: "patch",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "npm",
              },
              {
                ecosystem: "npm",
                name: "react",
                currentVersion: "18.2.0",
                latestVersion: "19.0.0",
                updateType: "major",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "npm",
              },
            ],
          };
        default:
          return null;
      }
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-targeted"
        projectName="Example"
        onAddFolder={vi.fn()}
        initialTarget={{
          itemId: "npm:react",
        }}
      />,
    );

    expect(await screen.findByTestId("issue-dossier")).toHaveTextContent("react");
  });

  it("records exact package verify jobs and actionable notifications", async () => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => "hidden",
    });
    expect(document.visibilityState).toBe("hidden");

    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_updates") {
        const detectCount = invokeMock.mock.calls.filter(
          ([name]) => name === "detect_updates",
        ).length;
        if (detectCount === 1) {
          return {
            packages: ["package.json", "package-lock.json"],
            updates: [
              {
                ecosystem: "npm",
                name: "react",
                currentVersion: "18.2.0",
                latestVersion: "19.0.0",
                updateType: "major",
                isSecurity: true,
                isDev: false,
                advisorySeverity: "high",
                advisoryUrl: "https://example.com/advisory",
                source: "npm",
              },
            ],
          };
        }
        return {
          packages: ["package.json", "package-lock.json"],
          updates: [],
        };
      }
      return null;
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-notify"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: /^react, / }));
    fireEvent.click(await screen.findByRole("button", { name: "Verify" }));

    await waitFor(() => {
      expect(addJobMock).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "updates-verify:7:npm:react",
          target: {
            page: "updates",
            projectId: 7,
            url: "https://example.com",
            itemId: "npm:react",
          },
        }),
      );
    });

    await waitFor(() => {
      expect(completeJobMock).toHaveBeenCalledWith(
        "updates-verify:7:npm:react",
        expect.objectContaining({
          label: "Update verified",
          target: {
            page: "updates",
            projectId: 7,
            url: "https://example.com",
            itemId: "npm:react",
          },
        }),
      );
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "record_update_event",
        expect.objectContaining({
          projectId: 7,
          title: "1 Update Applied",
          sourceId: expect.stringMatching(/^updates-verify:7:npm:react:verified:/),
          severity: "info",
        }),
      );
    });

    expect(failJobMock).not.toHaveBeenCalled();
  });

  it("points verified package follow-up jobs and notifications at the next related package update", async () => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => "hidden",
    });
    expect(document.visibilityState).toBe("hidden");

    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_updates") {
        const detectCount = invokeMock.mock.calls.filter(
          ([name]) => name === "detect_updates",
        ).length;
        if (detectCount === 1) {
          return {
            packages: ["package.json", "package-lock.json"],
            updates: [
              {
                ecosystem: "npm",
                name: "react",
                currentVersion: "18.2.0",
                latestVersion: "19.0.0",
                updateType: "major",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "package.json",
              },
              {
                ecosystem: "npm",
                name: "react-dom",
                currentVersion: "18.2.0",
                latestVersion: "19.0.0",
                updateType: "major",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "package.json",
              },
            ],
          };
        }
        return {
          packages: ["package.json", "package-lock.json"],
          updates: [
            {
              ecosystem: "npm",
              name: "react-dom",
              currentVersion: "18.2.0",
              latestVersion: "19.0.0",
              updateType: "major",
              isSecurity: false,
              isDev: false,
              advisorySeverity: null,
              advisoryUrl: null,
              source: "package.json",
            },
          ],
        };
      }
      return null;
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-next-related"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: /^react, / }));
    fireEvent.click(await screen.findByRole("button", { name: "Verify" }));

    await waitFor(() => {
      expect(completeJobMock).toHaveBeenCalledWith(
        "updates-verify:7:npm:react",
        expect.objectContaining({
          label: "Update verified",
          target: {
            page: "updates",
            projectId: 7,
            url: "https://example.com",
            itemId: "npm:react-dom",
          },
        }),
      );
    });

    await waitFor(() => {
      expect(sendActionableDesktopNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "updates-verify:7:npm:react:verified",
          clickTarget: {
            page: "updates",
            projectId: 7,
            url: "https://example.com",
            itemId: "npm:react-dom",
          },
        }),
      );
    });

    const updateEventCall = invokeMock.mock.calls.find(
      ([command, payload]) =>
        command === "record_update_event" && payload?.title === "1 Update Applied",
    );
    expect(updateEventCall).toBeDefined();
    const updateEventDetail = JSON.parse(String(updateEventCall?.[1]?.detail ?? "{}"));
    expect(updateEventDetail).toMatchObject({
      verified_label: "react 18.2.0 -> 19.0.0 • major",
      next_item_label: "react-dom 18.2.0 -> 19.0.0 • major",
      status_before: "Pending",
      status_after: "Verified",
      applied_updates: [
        {
          name: "react",
          from_version: "18.2.0",
          to_version: "19.0.0",
        },
      ],
    });
  });

  it("records grouped dependency verification jobs for verify-all pending flows", async () => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => "hidden",
    });
    expect(document.visibilityState).toBe("hidden");

    usePendingVerificationCenterMock.mockReturnValue([
      {
        id: "7:https://example.com:updates:npm:axios",
        projectId: 7,
        url: "https://example.com",
        itemId: "npm:axios",
        label: "axios 1.6.0 -> 1.7.0 • security (critical)",
        reason:
          "Dependency files changed on this site. Re-check package and vulnerability risk before moving on.",
        page: "updates",
        focus: null,
        filePath: null,
        createdAt: 3,
        updatedAt: 3,
      },
      {
        id: "7:https://example.com:updates:npm:react-dom",
        projectId: 7,
        url: "https://example.com",
        itemId: "npm:react-dom",
        label: "react-dom 18.2.0 -> 19.0.0",
        reason:
          "Dependency files changed on this site. Re-check package and vulnerability risk before moving on.",
        page: "updates",
        focus: null,
        filePath: null,
        createdAt: 2,
        updatedAt: 2,
      },
      {
        id: "7:https://example.com:updates:npm:next",
        projectId: 7,
        url: "https://example.com",
        itemId: "npm:next",
        label: "next 14.1.0 -> 15.0.0",
        reason:
          "Dependency files changed on this site. Re-check package and vulnerability risk before moving on.",
        page: "updates",
        focus: null,
        filePath: null,
        createdAt: 1,
        updatedAt: 1,
      },
    ]);

    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_updates") {
        const detectCount = invokeMock.mock.calls.filter(
          ([name]) => name === "detect_updates",
        ).length;
        if (detectCount === 1) {
          return {
            packages: ["package.json", "package-lock.json"],
            updates: [
              {
                ecosystem: "npm",
                name: "axios",
                currentVersion: "1.6.0",
                latestVersion: "1.7.0",
                updateType: "minor",
                isSecurity: true,
                isDev: false,
                advisorySeverity: "critical",
                advisoryFixedVersion: "1.7.0",
                advisoryUrl: "https://example.com/advisory",
                source: "package.json",
              },
              {
                ecosystem: "npm",
                name: "react-dom",
                currentVersion: "18.2.0",
                latestVersion: "19.0.0",
                updateType: "major",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "package.json",
              },
              {
                ecosystem: "npm",
                name: "next",
                currentVersion: "14.1.0",
                latestVersion: "15.0.0",
                updateType: "major",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "package.json",
              },
            ],
          };
        }
        return {
          packages: ["package.json", "package-lock.json"],
          updates: [
            {
              ecosystem: "npm",
              name: "axios",
              currentVersion: "1.6.0",
              latestVersion: "1.7.0",
              updateType: "minor",
              isSecurity: true,
              isDev: false,
              advisorySeverity: "critical",
              advisoryFixedVersion: "1.7.0",
              advisoryUrl: "https://example.com/advisory",
              source: "package.json",
            },
            {
              ecosystem: "npm",
              name: "next",
              currentVersion: "14.1.0",
              latestVersion: "15.0.0",
              updateType: "major",
              isSecurity: false,
              isDev: false,
              advisorySeverity: null,
              advisoryUrl: null,
              source: "package.json",
            },
          ],
        };
      }
      return null;
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-verify-all"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Verify all" }));

    await waitFor(() => {
      expect(addJobMock).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "updates-pending-all:7",
          label: "Verify package updates",
          target: {
            page: "updates",
            projectId: 7,
            url: "https://example.com",
            itemId: "npm:axios",
          },
        }),
      );
    });

    await waitFor(() => {
      expect(completeJobMock).toHaveBeenCalledWith(
        "updates-pending-all:7",
        expect.objectContaining({
          label: "Continue dependency cleanup",
          target: {
            page: "updates",
            projectId: 7,
            url: "https://example.com",
            itemId: "npm:axios",
          },
        }),
      );
    });

    await waitFor(() => {
      expect(sendActionableDesktopNotificationMock).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "updates-pending-all:7:complete",
          clickTarget: {
            page: "updates",
            projectId: 7,
            url: "https://example.com",
            itemId: "npm:axios",
          },
        }),
      );
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "record_update_event",
        expect.objectContaining({
          projectId: 7,
          title: "1 Update Applied",
          sourceId: expect.stringMatching(/^updates-pending-all:7:/),
          severity: "warning",
        }),
      );
    });

    const updateEventCall = invokeMock.mock.calls.find(
      ([command, payload]) =>
        command === "record_update_event" && payload?.title === "1 Update Applied",
    );
    expect(updateEventCall).toBeDefined();
    const updateEventDetail = JSON.parse(String(updateEventCall?.[1]?.detail ?? "{}"));
    expect(updateEventDetail).toMatchObject({
      next_item_label: "axios 1.6.0 -> 1.7.0 • security (critical)",
      applied_updates: [
        {
          name: "react-dom",
          from_version: "18.2.0",
          to_version: "19.0.0",
        },
      ],
    });
  });

  it("records exact update events for single pending-entry verification", async () => {
    usePendingVerificationCenterMock.mockReturnValue([
      {
        id: "7:https://example.com:updates:npm:react",
        projectId: 7,
        url: "https://example.com",
        itemId: "npm:react",
        label: "react 18.2.0 -> 19.0.0",
        reason:
          "Dependency files changed on this site. Re-check package and vulnerability risk before moving on.",
        page: "updates",
        focus: null,
        filePath: null,
        createdAt: 1,
        updatedAt: 1,
      },
    ]);

    invokeMock.mockImplementation(async (command: string) => {
      if (command === "detect_updates") {
        const detectCount = invokeMock.mock.calls.filter(
          ([name]) => name === "detect_updates",
        ).length;
        if (detectCount === 1) {
          return {
            packages: ["package.json", "package-lock.json"],
            updates: [
              {
                ecosystem: "npm",
                name: "react",
                currentVersion: "18.2.0",
                latestVersion: "19.0.0",
                updateType: "major",
                isSecurity: false,
                isDev: false,
                advisorySeverity: null,
                advisoryUrl: null,
                source: "package.json",
              },
            ],
          };
        }
        return {
          packages: ["package.json", "package-lock.json"],
          updates: [],
        };
      }
      return null;
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-pending-single"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Verify now" }));

    await waitFor(() => {
      expect(completeJobMock).toHaveBeenCalledWith(
        "updates-pending:7:npm:react",
        expect.objectContaining({
          label: "Update verified",
        }),
      );
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "record_update_event",
        expect.objectContaining({
          projectId: 7,
          title: "1 Update Applied",
          sourceId: expect.stringMatching(/^updates-pending:7:npm:react:verified:/),
          severity: "info",
        }),
      );
    });

    const updateEventCall = invokeMock.mock.calls.find(
      ([command, payload]) =>
        command === "record_update_event" && payload?.title === "1 Update Applied",
    );
    expect(updateEventCall).toBeDefined();
    const updateEventDetail = JSON.parse(String(updateEventCall?.[1]?.detail ?? "{}"));
    expect(updateEventDetail).toMatchObject({
      status_before: "Pending",
      status_after: "Verified",
      verified_label: "react 18.2.0 -> 19.0.0",
      applied_updates: [
        {
          name: "react",
          from_version: "18.2.0",
          to_version: "19.0.0",
        },
      ],
    });
  });
});
