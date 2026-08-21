import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const {
  invokeMock,
  emitMock,
  hasFeatureMock,
  getProjectNavBadgeSnapshotMock,
  publishUpdatesBadgeForReportMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  emitMock: vi.fn((_event: string, _payload?: unknown) => Promise.resolve()),
  hasFeatureMock: vi.fn(() => false),
  getProjectNavBadgeSnapshotMock: vi.fn(),
  publishUpdatesBadgeForReportMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: (event: string, payload?: unknown) => emitMock(event, payload),
}));

vi.mock("@/app/ShellHeader", () => ({
  HeaderActions: ({ children }: { children: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
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

vi.mock("@/lib/desktop-prefs", () => ({
  useDesktopPrefs: () => ({
    prefs: {
      desktopNotifications: false,
    },
  }),
}));

vi.mock("@/lib/desktop-prompts", () => ({
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

vi.mock("@/lib/project-summary-signals", () => ({
  getProjectNavBadgeSnapshot: (...args: unknown[]) => getProjectNavBadgeSnapshotMock(...args),
}));

vi.mock("@/lib/project-nav-badges", () => ({
  publishUpdatesBadgeForReport: (...args: unknown[]) => publishUpdatesBadgeForReportMock(...args),
}));

import { UpdatesPage } from "./UpdatesPage";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

function renderUpdatesPage(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient(createTestQueryClient()) });
}

function buildNavBadgeSnapshot(overrides?: {
  updates?: Record<string, unknown> | null;
  updatesRefreshedAt?: string | null;
}) {
  return {
    projectId: 7,
    environmentUrl: "https://example.com",
    aggregatedFailedIssues: [],
    inactiveCheckIds: [],
    signals: {
      updates: overrides?.updates ?? null,
      updatesRefreshedAt: overrides?.updatesRefreshedAt ?? null,
    },
  };
}

describe("UpdatesPage behavior", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    emitMock.mockClear();
    hasFeatureMock.mockReset();
    getProjectNavBadgeSnapshotMock.mockReset();
    publishUpdatesBadgeForReportMock.mockReset();
    hasFeatureMock.mockReturnValue(false);
    getProjectNavBadgeSnapshotMock.mockResolvedValue(buildNavBadgeSnapshot());
    window.localStorage.clear();
  });

  it("shows the real no-folder state before any update scan can run", async () => {
    const onAddFolder = vi.fn();

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath={null}
        projectName="Example"
        onAddFolder={onAddFolder}
      />,
    );

    expect(screen.getByText("No project folder linked")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Add Folder" }));
    expect(onAddFolder).toHaveBeenCalled();
  });

  it("shows a page-shaped loading skeleton while dependency data is loading", () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return new Promise(() => {});
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-updates-loading"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Updates loading state")).toBeInTheDocument();
  });

  it("shows the loading skeleton instead of a persisted signal snapshot", async () => {
    getProjectNavBadgeSnapshotMock.mockResolvedValue(
      buildNavBadgeSnapshot({
        updates: {
          packages: [],
          updates: [
            {
              name: "react",
              currentVersion: "18.2.0",
              latestVersion: "19.0.0",
              ecosystem: "npm",
              updateType: "major",
              isSecurity: false,
              advisorySeverity: null,
              advisoryUrl: null,
              source: "package.json",
              isDev: false,
            },
          ],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 1234,
        },
        updatesRefreshedAt: "2026-04-20T16:40:00Z",
      }),
    );
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return new Promise(() => {});
      }
      if (command === "get_events") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-updates-snapshot"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    expect(await screen.findByLabelText("Updates loading state")).toBeInTheDocument();
    expect(screen.queryByText("All (1)")).not.toBeInTheDocument();
    expect(screen.queryByText("react")).not.toBeInTheDocument();
  });

  it("shows the loading skeleton instead of the local update snapshot", async () => {
    const { writeUpdateSnapshot } = await import("@/lib/update-memory");
    writeUpdateSnapshot("/tmp/example-updates-local-snapshot", [
      {
        name: "react",
        currentVersion: "18.2.0",
        latestVersion: "19.0.0",
        ecosystem: "npm",
        updateType: "major",
        isSecurity: false,
        advisorySeverity: null,
        advisoryUrl: null,
        source: "package.json",
        isDev: false,
        isDeprecated: false,
        deprecationMessage: null,
        currentVersionDeprecated: false,
        isStale: false,
        lastPublished: null,
        workspaceMembers: [],
      },
    ]);
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return new Promise(() => {});
      }
      if (command === "get_events") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-updates-local-snapshot"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    expect(await screen.findByLabelText("Updates loading state")).toBeInTheDocument();
    expect(screen.queryByText("react")).not.toBeInTheDocument();
    expect(publishUpdatesBadgeForReportMock).not.toHaveBeenCalledWith(
      7,
      expect.objectContaining({ updates: expect.anything() }),
    );
  });

  it("recovers from a failed dependency load and renders the real update content", async () => {
    let shouldFail = true;
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        if (shouldFail) {
          shouldFail = false;
          return Promise.reject(new Error("scanner offline"));
        }
        return Promise.resolve({
          packages: [
            {
              name: "react",
              version: "18.2.0",
              ecosystem: "npm",
              source: "package.json",
              isDev: false,
            },
          ],
          updates: [
            {
              ecosystem: "npm",
              name: "react",
              currentVersion: "18.2.0",
              latestVersion: "19.0.0",
              updateType: "major",
              isSecurity: false,
              advisorySeverity: null,
              advisoryUrl: null,
              source: "npm",
              isDev: false,
            },
          ],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-updates-retry"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(
        screen.queryByRole("button", { name: "Retry" }) ??
          screen.queryByRole("button", { name: "Refresh" }),
      ).toBeInTheDocument();
    });

    fireEvent.click(
      screen.queryByRole("button", { name: "Retry" }) ??
        screen.getByRole("button", { name: "Refresh" }),
    );

    await waitFor(() => {
      expect(screen.getByText("react")).toBeInTheDocument();
    });
  });

  it("publishes the updates badge once the dependency report is loaded", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return Promise.resolve({
          packages: [
            {
              name: "react",
              version: "18.2.0",
              ecosystem: "npm",
              source: "package.json",
              isDev: false,
            },
          ],
          updates: [
            {
              ecosystem: "npm",
              name: "react",
              currentVersion: "18.2.0",
              latestVersion: "19.0.0",
              updateType: "major",
              isSecurity: false,
              advisorySeverity: null,
              advisoryUrl: null,
              source: "npm",
              isDev: false,
            },
          ],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      if (command === "get_events") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-updates-badge"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("react")).toBeInTheDocument();
    });

    expect(screen.getByText("react")).toBeInTheDocument();
    await waitFor(() => {
      expect(publishUpdatesBadgeForReportMock).toHaveBeenCalledWith(
        7,
        expect.objectContaining({
          updates: [expect.objectContaining({ name: "react", isSecurity: false })],
        }),
      );
      expect(emitMock).toHaveBeenCalledWith(
        "project-signals-changed",
        expect.objectContaining({
          projectId: 7,
          source: "updates",
          updates: expect.objectContaining({
            updates: [expect.objectContaining({ name: "react", isSecurity: false })],
          }),
        }),
      );
    });
  });

  it("shows recent dependency history when update events exist", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return Promise.resolve({
          packages: [
            {
              name: "astro",
              version: "6.1.8",
              ecosystem: "npm",
              source: "package.json",
              isDev: false,
            },
            {
              name: "@astrojs/cloudflare",
              version: "13.1.10",
              ecosystem: "npm",
              source: "package.json",
              isDev: false,
            },
          ],
          updates: [],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      if (command === "get_events") {
        return Promise.resolve([
          {
            id: 77,
            project_id: 7,
            eventType: "update",
            severity: "info",
            occurredAtMs: Date.parse("2026-04-19T15:00:00Z"),
            title: "2 Updates Applied",
            summary: "astro 5.18.1 -> 6.1.8 and 3 more left the list. No pending updates remain.",
            detail: JSON.stringify({
              page: "updates",
              verified_count: 2,
              remaining_updates: 0,
              security_updates: 0,
              applied_updates: [
                {
                  name: "astro",
                  from_version: "5.18.1",
                  to_version: "6.1.8",
                },
                {
                  name: "@astrojs/cloudflare",
                  from_version: "12.6.13",
                  to_version: "13.1.10",
                },
              ],
            }),
            source: "internal",
            sourceId: "updates-refresh:7:1",
          },
        ]);
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-updates-history"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("History")).toBeInTheDocument();
    });

    expect(screen.getByText("2 Updates Applied")).toBeInTheDocument();
    expect(screen.getByText("astro")).toBeInTheDocument();
    expect(screen.getByText("5.18.1")).toBeInTheDocument();
    expect(screen.getByText("6.1.8")).toBeInTheDocument();
  });

  it("keeps dependency history scoped to the active project when switching projects", async () => {
    let resolveProjectAEvents: ((events: Array<Record<string, unknown>>) => void) | undefined;

    invokeMock.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "detect_updates") {
        return Promise.resolve({
          packages: [],
          updates: [],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      if (command === "get_events") {
        if (payload?.projectId === 7) {
          return new Promise((resolve) => {
            resolveProjectAEvents = resolve as (events: Array<Record<string, unknown>>) => void;
          });
        }
        if (payload?.projectId === 8) {
          return Promise.resolve([
            {
              id: 88,
              project_id: 8,
              eventType: "update",
              severity: "info",
              occurredAtMs: Date.parse("2026-04-19T15:05:00Z"),
              title: "1 Update Applied",
              summary: "tailwindcss 4.1.13 -> 4.2.2 left the list.",
              detail: JSON.stringify({
                page: "updates",
                verified_count: 1,
                remaining_updates: 0,
                security_updates: 0,
                applied_updates: [
                  {
                    name: "tailwindcss",
                    from_version: "4.1.13",
                    to_version: "4.2.2",
                  },
                ],
              }),
              source: "internal",
              sourceId: "updates-refresh:8:0:0:npm:tailwindcss:4.1.13->4.2.2",
            },
          ]);
        }
      }
      return Promise.resolve(null);
    });

    const view = renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://site-a.test"
        projectPath="/tmp/example-updates-project-a"
        projectName="Site A"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("detect_updates", {
        projectId: 7,
        projectPath: "/tmp/example-updates-project-a",
      });
    });

    view.rerender(
      <UpdatesPage
        projectId={8}
        url="https://site-b.test"
        projectPath="/tmp/example-updates-project-b"
        projectName="Site B"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("tailwindcss")).toBeInTheDocument();
    });

    if (resolveProjectAEvents) {
      resolveProjectAEvents([
        {
          id: 77,
          project_id: 7,
          eventType: "update",
          severity: "info",
          occurredAtMs: Date.parse("2026-04-19T15:00:00Z"),
          title: "1 Update Applied",
          summary: "astro 5.18.1 -> 6.1.8 left the list.",
          detail: JSON.stringify({
            page: "updates",
            verified_count: 1,
            remaining_updates: 0,
            security_updates: 0,
            applied_updates: [
              {
                name: "astro",
                from_version: "5.18.1",
                to_version: "6.1.8",
              },
            ],
          }),
          source: "internal",
          sourceId: "updates-refresh:7:0:0:npm:astro:5.18.1->6.1.8",
        },
      ]);
    }

    await waitFor(() => {
      expect(screen.getByText("tailwindcss")).toBeInTheDocument();
      expect(screen.queryByText("astro")).not.toBeInTheDocument();
    });
  });

  it("collapses duplicate applied update history entries into one clean record", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return Promise.resolve({
          packages: [],
          updates: [],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      if (command === "get_events") {
        return Promise.resolve([
          {
            id: 103,
            project_id: 7,
            eventType: "update",
            severity: "info",
            occurredAtMs: Date.parse("2026-04-19T15:06:00Z"),
            title: "2 Updates Applied",
            summary:
              "@tailwindcss/vite 4.1.13 -> 4.2.2 and tailwindcss 4.1.13 -> 4.2.2 left the list.",
            detail: JSON.stringify({
              page: "updates",
              verified_count: 2,
              remaining_updates: 0,
              security_updates: 0,
              applied_updates: [
                {
                  name: "@tailwindcss/vite",
                  from_version: "4.1.13",
                  to_version: "4.2.2",
                },
                {
                  name: "tailwindcss",
                  from_version: "4.1.13",
                  to_version: "4.2.2",
                },
              ],
            }),
            source: "internal",
            sourceId: "updates-refresh:7:0:0:latest",
          },
          {
            id: 102,
            project_id: 7,
            eventType: "update",
            severity: "info",
            occurredAtMs: Date.parse("2026-04-19T15:03:00Z"),
            title: "2 Updates Applied",
            summary:
              "@tailwindcss/vite 4.1.13 -> 4.2.2 and tailwindcss 4.1.13 -> 4.2.2 left the list.",
            detail: JSON.stringify({
              page: "updates",
              verified_count: 2,
              remaining_updates: 0,
              security_updates: 0,
              applied_updates: [
                {
                  name: "@tailwindcss/vite",
                  from_version: "4.1.13",
                  to_version: "4.2.2",
                },
                {
                  name: "tailwindcss",
                  from_version: "4.1.13",
                  to_version: "4.2.2",
                },
              ],
            }),
            source: "internal",
            sourceId: "updates-refresh:7:0:0:older",
          },
        ]);
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-updates-dedupe"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("History")).toBeInTheDocument();
    });

    expect(screen.getAllByText("2 Updates Applied")).toHaveLength(1);
    expect(screen.getAllByText("@tailwindcss/vite")).toHaveLength(1);
    expect(screen.getAllByText("tailwindcss")).toHaveLength(1);
  });

  it("hides update history rows that do not belong to the current project's package set", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return Promise.resolve({
          packages: [
            {
              name: "astro",
              version: "6.1.8",
              ecosystem: "npm",
              source: "package.json",
              isDev: false,
            },
            {
              name: "tailwindcss",
              version: "4.2.2",
              ecosystem: "npm",
              source: "package.json",
              isDev: false,
            },
          ],
          updates: [],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      if (command === "get_events") {
        return Promise.resolve([
          {
            id: 201,
            project_id: 2,
            eventType: "update",
            severity: "info",
            occurredAtMs: Date.parse("2026-04-19T15:06:00Z"),
            title: "2 Updates Applied",
            summary: "tailwindcss 4.1.13 -> 4.2.2 and astro 5.18.1 -> 6.1.8 left the list.",
            detail: JSON.stringify({
              page: "updates",
              verified_count: 2,
              remaining_updates: 0,
              security_updates: 0,
              applied_updates: [
                { name: "tailwindcss", from_version: "4.1.13", to_version: "4.2.2" },
                { name: "astro", from_version: "5.18.1", to_version: "6.1.8" },
              ],
            }),
            source: "internal",
            sourceId: "updates-refresh:2:0:0:npm:astro:5.18.1->6.1.8|npm:tailwindcss:4.1.13->4.2.2",
          },
          {
            id: 200,
            project_id: 2,
            eventType: "update",
            severity: "info",
            occurredAtMs: Date.parse("2026-04-19T15:05:00Z"),
            title: "20 Updates Applied",
            summary: "drupal/core 11.3.5 -> 11.3.7 and 19 more left the list.",
            detail: JSON.stringify({
              page: "updates",
              verified_count: 20,
              remaining_updates: 0,
              security_updates: 0,
              applied_updates: [
                { name: "drupal/core", from_version: "11.3.5", to_version: "11.3.7" },
                { name: "phpunit/phpunit", from_version: "11.5.55", to_version: "13.1.7" },
              ],
            }),
            source: "internal",
            sourceId: "updates-refresh:2:0:0:composer:drupal/core:11.3.5->11.3.7",
          },
        ]);
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={2}
        url="https://example.com"
        projectPath="/tmp/example-updates-filter-bogus"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("History")).toBeInTheDocument();
    });

    expect(screen.getByText("tailwindcss")).toBeInTheDocument();
    expect(screen.queryByText("drupal/core")).not.toBeInTheDocument();
    expect(screen.queryByText("phpunit/phpunit")).not.toBeInTheDocument();
    expect(screen.getAllByText("2 Updates Applied")).toHaveLength(1);
  });

  it("does not credit remembered pending updates as applied when a scan observed no packages", async () => {
    const { recordSeenUpdates } = await import("@/lib/update-memory");
    recordSeenUpdates("/tmp/example-updates-history-fallback", [
      {
        ecosystem: "npm",
        name: "@tailwindcss/vite",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
        isSecurity: false,
        advisorySeverity: null,
        advisoryUrl: null,
        source: "package.json",
        isDev: false,
        isDeprecated: false,
        deprecationMessage: null,
        currentVersionDeprecated: false,
        isStale: false,
        lastPublished: null,
        workspaceMembers: [],
      },
      {
        ecosystem: "npm",
        name: "tailwindcss",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
        isSecurity: false,
        advisorySeverity: null,
        advisoryUrl: null,
        source: "package.json",
        isDev: false,
        isDeprecated: false,
        deprecationMessage: null,
        currentVersionDeprecated: false,
        isStale: false,
        lastPublished: null,
        workspaceMembers: [],
      },
    ]);

    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return Promise.resolve({
          packages: [],
          updates: [],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      if (command === "get_events") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-updates-history-fallback"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("detect_updates", {
        projectId: 7,
        projectPath: "/tmp/example-updates-history-fallback",
      });
    });
    await waitFor(() => {
      expect(invokeMock.mock.calls.some(([command]) => command === "get_events")).toBe(true);
    });

    expect(invokeMock.mock.calls.some(([command]) => command === "record_update_event")).toBe(
      false,
    );
  });

  it("does not record fake applied-update history from a polluted previous snapshot", async () => {
    const { writeUpdateSnapshot } = await import("@/lib/update-memory");
    writeUpdateSnapshot("/tmp/example-updates-snapshot-sanitize", [
      {
        ecosystem: "composer",
        name: "drupal/core",
        currentVersion: "11.3.5",
        latestVersion: "11.3.7",
        updateType: "patch",
        isSecurity: false,
        advisorySeverity: null,
        advisoryUrl: null,
        source: "composer.lock",
        isDev: false,
        isDeprecated: false,
        deprecationMessage: null,
        currentVersionDeprecated: false,
        isStale: false,
        lastPublished: null,
        workspaceMembers: [],
      },
    ]);

    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return Promise.resolve({
          packages: [
            {
              name: "astro",
              version: "6.1.8",
              ecosystem: "npm",
              source: "package.json",
              isDev: false,
            },
          ],
          updates: [],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      if (command === "get_events") {
        return Promise.resolve([]);
      }
      if (command === "record_update_event") {
        throw new Error("should not record polluted history");
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={2}
        url="https://example.com"
        projectPath="/tmp/example-updates-snapshot-sanitize"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("detect_updates", {
        projectId: 2,
        projectPath: "/tmp/example-updates-snapshot-sanitize",
      });
    });

    expect(invokeMock.mock.calls.some(([command]) => command === "record_update_event")).toBe(
      false,
    );
  });

  it("never synthesizes applied-update history when History and the report are both empty", async () => {
    const projectPath = "/tmp/example-updates-history-cache";

    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return Promise.resolve({
          packages: [],
          updates: [],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      if (command === "get_events") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    const firstRender = renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath={projectPath}
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("detect_updates", { projectId: 7, projectPath });
    });

    firstRender.unmount();

    const { recordSeenUpdates } = await import("@/lib/update-memory");
    recordSeenUpdates(projectPath, [
      {
        ecosystem: "npm",
        name: "@tailwindcss/vite",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
        isSecurity: false,
        advisorySeverity: null,
        advisoryUrl: null,
        source: "package.json",
        isDev: false,
        isDeprecated: false,
        deprecationMessage: null,
        currentVersionDeprecated: false,
        isStale: false,
        lastPublished: null,
        workspaceMembers: [],
      },
      {
        ecosystem: "npm",
        name: "tailwindcss",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
        isSecurity: false,
        advisorySeverity: null,
        advisoryUrl: null,
        source: "package.json",
        isDev: false,
        isDeprecated: false,
        deprecationMessage: null,
        currentVersionDeprecated: false,
        isStale: false,
        lastPublished: null,
        workspaceMembers: [],
      },
    ]);

    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_events") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath={projectPath}
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(invokeMock.mock.calls.some(([command]) => command === "get_events")).toBe(true);
    });

    expect(invokeMock.mock.calls.some(([command]) => command === "record_update_event")).toBe(
      false,
    );
  });

  it("hides stale 'update still pending' history entries from the Updates page", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "detect_updates") {
        return Promise.resolve({
          packages: [],
          updates: [],
          ecosystemsDetected: ["npm"],
          scanDurationMs: 120,
        });
      }
      if (command === "get_events") {
        return Promise.resolve([
          {
            id: 78,
            project_id: 7,
            eventType: "update",
            severity: "warning",
            occurredAtMs: Date.parse("2026-04-19T15:00:00Z"),
            title: "Update still pending: @tailwindcss/vite",
            summary: "@tailwindcss/vite is still waiting in Updates.",
            detail: JSON.stringify({
              page: "updates",
              status_after: "Still pending",
            }),
            source: "internal",
            sourceId: "updates-verify:7:npm:@tailwindcss/vite:pending:1",
          },
        ]);
      }
      return Promise.resolve(null);
    });

    renderUpdatesPage(
      <UpdatesPage
        projectId={7}
        url="https://example.com"
        projectPath="/tmp/example-updates-hide-pending"
        projectName="Example"
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.queryByText("History")).not.toBeInTheDocument();
    });
    expect(screen.queryByText("Update still pending: @tailwindcss/vite")).not.toBeInTheDocument();
  });
});
