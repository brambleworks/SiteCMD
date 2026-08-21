import React from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));
vi.mock("@/lib/scan-execution-adapters", () => ({
  getScanHistory: (args: unknown) => invokeMock("get_scan_executions", args),
}));
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
vi.mock("@/components/ui/external-link", () => ({
  ExtLink: ({
    children,
    href,
    className,
  }: {
    children: React.ReactNode;
    href: string;
    className?: string;
  }) => React.createElement("a", { href, className }, children),
}));

import { DeploysPage } from "./DeploysPage";
import { createTestQueryClient, withQueryClient } from "@/test-utils/query-client";

function renderDeploys(ui: React.ReactElement, queryClient = createTestQueryClient()) {
  return render(ui, { wrapper: withQueryClient(queryClient) });
}

function makeCommit(index: number) {
  return {
    hash: `commit-${index.toString().padStart(2, "0")}-full-hash`,
    shortHash: `c${index.toString().padStart(2, "0")}`,
    message: `Commit ${index}`,
    author: "Kyle",
    date: `2026-04-${String(Math.min(index, 28)).padStart(2, "0")}T09:15:00Z`,
    relativeDate: `${index}h ago`,
  };
}

describe("DeploysPage", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("shows the deploy regression callout, highlights the matching commit, and opens the dropped scan", async () => {
    const onViewScan = vi.fn();
    const onScan = vi.fn();

    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_git_status":
          return {
            isGitRepo: true,
            branch: "main",
            commits: [
              {
                hash: "9f8e7d6c5b4a3210",
                shortHash: "9f8e7d6",
                message: "Ship robots header fix",
                author: "Kyle",
                date: "2026-04-10T09:15:00Z",
                relativeDate: "2h ago",
              },
            ],
            totalCommits: 1,
            hasUncommitted: false,
          };
        case "get_scan_executions":
          return [
            {
              id: 91,
              url: "https://example.com",
              mode: "live",
              overallScore: 72,
              issuesTotal: 6,
              issuesCritical: 1,
              issuesHigh: 2,
              durationMs: 2100,
              timestamp: "2026-04-10T10:00:00Z",
            },
          ];
        case "get_events":
          return [
            {
              id: 44,
              project_id: 7,
              eventType: "deploy",
              severity: "info",
              timestamp: "2026-04-10T09:20:00Z",
              title: "Deploy completed",
              summary: "Production deploy finished",
              detail: null,
              source: "github",
              sourceId: "9f8e7d6c5b4a3210",
            },
          ];
        case "get_correlations":
          return [
            {
              sourceEventId: 44,
              targetEventId: 91,
              correlationType: "deploy_to_regression",
              confidence: "high",
              description: "This deploy likely introduced the latest Web Scan score drop.",
              sourceTimestamp: "2026-04-10T09:20:00Z",
              targetTimestamp: "2026-04-10T10:00:00Z",
            },
          ];
        case "fetch_github_data":
          return {
            repo: "acme/site",
            workflow_runs: [],
            deployments: [],
            open_prs: [],
          };
        default:
          return null;
      }
    });

    renderDeploys(
      <DeploysPage
        projectPath="/tmp/example"
        projectId={7}
        url="https://example.com"
        onScan={onScan}
        scanning={false}
        onViewScan={onViewScan}
        onAddFolder={vi.fn()}
      />,
    );

    expect(await screen.findByText("Deploy Likely Caused Regression")).toBeInTheDocument();
    expect(
      screen.getByText("This deploy likely introduced the latest Web Scan score drop."),
    ).toBeInTheDocument();

    const commitTitle = await screen.findByText("Ship robots header fix");
    const commitRow = commitTitle.closest(".row-between");
    expect(commitRow).not.toBeNull();
    expect(within(commitRow as HTMLElement).getByText("Likely regression")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Open dropped scan" }));
    expect(onViewScan).toHaveBeenCalledWith(91);

    fireEvent.click(screen.getByRole("button", { name: "Scan after deploy" }));
    expect(onScan).toHaveBeenCalledTimes(1);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_correlations", { projectId: 7 });
    });
  });

  it("shows a page-shaped loading skeleton while deploy data is still loading", () => {
    invokeMock.mockImplementation(() => new Promise(() => {}));

    renderDeploys(
      <DeploysPage
        projectPath="/tmp/example"
        projectId={7}
        url="https://example.com"
        onScan={vi.fn()}
        scanning={false}
        onViewScan={vi.fn()}
        onAddFolder={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Deploys loading state")).toBeInTheDocument();
  });

  it("keeps cached deploy data visible while a stale remount revalidates", async () => {
    let nowMs = Date.parse("2026-04-10T12:00:00Z");
    const dateNow = vi.spyOn(Date, "now").mockImplementation(() => nowMs);
    let gitReads = 0;
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_git_status":
          gitReads += 1;
          return {
            isGitRepo: true,
            branch: "main",
            commits: [
              {
                ...makeCommit(gitReads),
                message: gitReads === 1 ? "Cached commit" : "Refreshed commit",
              },
            ],
            totalCommits: 1,
            hasUncommitted: false,
          };
        case "get_scan_executions":
        case "get_events":
        case "get_correlations":
        case "get_integrations":
          return [];
        case "fetch_github_data":
          return { repo: null, workflow_runs: [], deployments: [], open_prs: [] };
        default:
          return null;
      }
    });
    const queryClient = createTestQueryClient();
    const props = {
      projectPath: "/tmp/example",
      projectId: 7,
      url: "https://example.com",
      onScan: vi.fn(),
      scanning: false,
      onViewScan: vi.fn(),
      onAddFolder: vi.fn(),
    };

    const first = renderDeploys(<DeploysPage {...props} />, queryClient);
    expect(await screen.findByText("Cached commit")).toBeInTheDocument();
    first.unmount();
    nowMs += 60_001;

    renderDeploys(<DeploysPage {...props} />, queryClient);

    expect(screen.getByText("Cached commit")).toBeInTheDocument();
    expect(screen.queryByLabelText("Deploys loading state")).not.toBeInTheDocument();
    expect(await screen.findByText("Refreshed commit")).toBeInTheDocument();
    expect(gitReads).toBe(2);
    dateNow.mockRestore();
  });

  it("always fetches GitHub data now the integration gate is retired", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_git_status":
          return {
            isGitRepo: true,
            branch: "main",
            commits: [],
            totalCommits: 0,
            hasUncommitted: false,
          };
        case "get_scan_executions":
        case "get_events":
        case "get_correlations":
        case "get_integrations":
          return [];
        case "fetch_github_data":
          return { repo: "acme/site", workflow_runs: [], deployments: [], open_prs: [] };
        default:
          return null;
      }
    });

    renderDeploys(
      <DeploysPage
        projectPath="/tmp/example"
        projectId={7}
        url="https://example.com"
        onScan={vi.fn()}
        scanning={false}
        onViewScan={vi.fn()}
        onAddFolder={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("fetch_github_data", { projectId: 7 });
    });
  });

  it("shows the latest ten commits by default and paginates older commits", async () => {
    const commits = Array.from({ length: 12 }, (_, index) => makeCommit(index + 1));

    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_git_status":
          return {
            isGitRepo: true,
            branch: "main",
            commits,
            totalCommits: 12,
            hasUncommitted: false,
          };
        case "get_scan_executions":
        case "get_events":
        case "get_correlations":
          return [];
        case "fetch_github_data":
          return {
            repo: "acme/site",
            workflow_runs: [],
            deployments: [],
            open_prs: [],
          };
        default:
          return null;
      }
    });

    renderDeploys(
      <DeploysPage
        projectPath="/tmp/example"
        projectId={7}
        url="https://example.com"
        onScan={vi.fn()}
        scanning={false}
        onViewScan={vi.fn()}
        onAddFolder={vi.fn()}
      />,
    );

    expect(await screen.findByText("Latest Commits")).toBeInTheDocument();
    expect(screen.getByText("Showing 1-10 of 12")).toBeInTheDocument();
    expect(screen.getByText("Commit 1")).toBeInTheDocument();
    expect(screen.getByText("Commit 10")).toBeInTheDocument();
    expect(screen.queryByText("Commit 11")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next commits page" }));

    expect(screen.getByText("Showing 11-12 of 12")).toBeInTheDocument();
    expect(screen.queryByText("Commit 1")).not.toBeInTheDocument();
    expect(screen.getByText("Commit 11")).toBeInTheDocument();
    expect(screen.getByText("Commit 12")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Previous commits page" }));
    expect(screen.getByText("Showing 1-10 of 12")).toBeInTheDocument();
  });

  it("resets to the first page when a refreshed commit list arrives", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      switch (command) {
        case "get_git_status":
          return {
            isGitRepo: true,
            branch: "main",
            // Fresh array per call: a reload must produce a new list identity
            // exactly like the real invoke layer does.
            commits: Array.from({ length: 12 }, (_, index) => makeCommit(index + 1)),
            totalCommits: 12,
            hasUncommitted: false,
          };
        case "get_scan_executions":
        case "get_events":
        case "get_correlations":
          return [];
        case "fetch_github_data":
          return {
            repo: "acme/site",
            workflow_runs: [],
            deployments: [],
            open_prs: [],
          };
        default:
          return null;
      }
    });

    const baseProps = {
      projectPath: "/tmp/example",
      projectId: 7,
      onScan: vi.fn(),
      scanning: false,
      onViewScan: vi.fn(),
      onAddFolder: vi.fn(),
    };
    const { rerender } = renderDeploys(<DeploysPage {...baseProps} url="https://example.com" />);

    expect(await screen.findByText("Latest Commits")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Next commits page" }));
    expect(screen.getByText("Showing 11-12 of 12")).toBeInTheDocument();

    rerender(<DeploysPage {...baseProps} url="https://example.org" />);

    expect(await screen.findByText("Showing 1-10 of 12")).toBeInTheDocument();
    expect(screen.getByText("Commit 1")).toBeInTheDocument();
    expect(screen.queryByText("Commit 11")).not.toBeInTheDocument();
  });

  const emptyRepoInvoke =
    (githubHandler: (command: string) => unknown) => async (command: string) => {
      switch (command) {
        case "get_git_status":
          return {
            isGitRepo: true,
            branch: "main",
            commits: [],
            totalCommits: 0,
            hasUncommitted: false,
          };
        case "get_scan_executions":
        case "get_events":
        case "get_correlations":
          return [];
        default:
          return githubHandler(command);
      }
    };

  it("offers a reconnect prompt when GitHub is configured but the token expired", async () => {
    invokeMock.mockImplementation(
      emptyRepoInvoke((command) => {
        if (command === "get_integrations")
          return [{ integrationType: "github", repo: "acme/site" }];
        if (command === "fetch_github_data") throw new Error("token expired");
        return null;
      }),
    );

    render(
      <DeploysPage
        projectPath="/tmp/example"
        projectId={7}
        url="https://example.com"
        onScan={vi.fn()}
        scanning={false}
        onViewScan={vi.fn()}
        onAddFolder={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    // The configured-but-broken integration reads as "reconnect", never as a
    // hidden/absent section (the "connected = config, not data" rule).
    expect(await screen.findByText(/GitHub CI stopped syncing/i)).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: /connect with github/i })).toBeInTheDocument();
  });

  it("shows a connect card without a reconnect prompt when GitHub was never configured", async () => {
    invokeMock.mockImplementation(
      emptyRepoInvoke((command) => {
        if (command === "get_integrations") return [];
        if (command === "fetch_github_data") throw new Error("not configured");
        return null;
      }),
    );

    render(
      <DeploysPage
        projectPath="/tmp/example"
        projectId={7}
        url="https://example.com"
        onScan={vi.fn()}
        scanning={false}
        onViewScan={vi.fn()}
        onAddFolder={vi.fn()}
      />,
      { wrapper: withQueryClient() },
    );

    expect(await screen.findByRole("button", { name: /connect with github/i })).toBeInTheDocument();
    expect(screen.queryByText(/GitHub CI stopped syncing/i)).not.toBeInTheDocument();
  });
});
