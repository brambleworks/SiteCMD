import { render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { IssueMemoryRail } from "./IssueMemorySection";
import { withQueryClient } from "@/test-utils/query-client";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@/lib/tauri-invoke", () => ({ invoke: invokeMock }));

function mockInvoke(handlers: Record<string, unknown>) {
  invokeMock.mockImplementation((command: string) =>
    Promise.resolve(command in handlers ? handlers[command] : null),
  );
}

const project = {
  id: 7,
  name: "Example Site",
  path: "/tmp/site",
  framework: "Next.js",
  createdAt: "2026-05-01T00:00:00Z",
  environments: [
    {
      id: 1,
      label: "Example Site (production)",
      environment: "production",
      url: "https://example.com",
      source: null,
      lastScannedAt: null,
      latestScore: null,
    },
    {
      id: 2,
      label: "Example Site (staging)",
      environment: "staging",
      url: "https://staging.example.com",
      source: null,
      lastScannedAt: null,
      latestScore: null,
    },
  ],
};

describe("IssueMemoryRail", () => {
  beforeEach(() => invokeMock.mockReset());

  it("reads work_items lifecycle in one query and maps active envs to labels", async () => {
    mockInvoke({
      get_projects: [project],
      get_issue_check_memory: {
        firstSeen: Date.UTC(2026, 4, 10, 12, 0, 0),
        lastFailed: Date.UTC(2026, 4, 12, 12, 0, 0),
        lastVerified: Date.UTC(2026, 4, 11, 12, 0, 0),
        // Prod still active; staging resolved (absent), so it must not show.
        affectedEnvUrls: ["https://example.com"],
      },
      get_events: [],
    });

    render(
      <IssueMemoryRail
        projectId={7}
        url="https://example.com"
        checkId="security.csp"
        currentStatus="fail"
      />,
      { wrapper: withQueryClient() },
    );

    await waitFor(() => expect(screen.getByText("First seen")).toBeInTheDocument());

    // One indexed query replaces the per-scan-detail N+1.
    expect(invokeMock).toHaveBeenCalledWith("get_issue_check_memory", {
      projectId: 7,
      checkId: "security.csp",
    });
    expect(invokeMock).not.toHaveBeenCalledWith("get_scan_execution_detail", expect.anything());

    expect(screen.getByText("Last failed")).toBeInTheDocument();
    expect(screen.getByText("Verified")).toBeInTheDocument();

    const environmentSection = screen.getByText("Environments").closest(".dossier-rail-row");
    expect(environmentSection).not.toBeNull();
    expect(
      within(environmentSection as HTMLElement).getByText("Example Site (production)"),
    ).toBeInTheDocument();
    expect(
      within(environmentSection as HTMLElement).queryByText("Example Site (staging)"),
    ).not.toBeInTheDocument();
  });

  it("does not claim a regression for an issue that never passed", async () => {
    mockInvoke({
      get_projects: [project],
      get_issue_check_memory: {
        firstSeen: Date.UTC(2026, 4, 10, 12, 0, 0),
        lastFailed: Date.UTC(2026, 4, 12, 12, 0, 0),
        // Never verified, so no deploy can have broken it.
        lastVerified: null,
        affectedEnvUrls: ["https://example.com"],
      },
      get_events: [
        {
          id: 1,
          occurredAtMs: Date.UTC(2026, 4, 11, 12, 0, 0),
          title: "Deploy abc123",
          eventType: "deploy",
        },
      ],
    });

    render(
      <IssueMemoryRail
        projectId={7}
        url="https://example.com"
        checkId="security.csp"
        currentStatus="fail"
      />,
      { wrapper: withQueryClient() },
    );

    await waitFor(() => expect(screen.getByText("First seen")).toBeInTheDocument());

    expect(screen.queryByText(/Regressed after/i)).not.toBeInTheDocument();
    // Nothing can regress, so the deploy history is never fetched.
    expect(invokeMock).not.toHaveBeenCalledWith("get_events", expect.anything());
  });

  it("names the deploy an issue regressed after once it had passed", async () => {
    mockInvoke({
      get_projects: [project],
      get_issue_check_memory: {
        firstSeen: Date.UTC(2026, 4, 10, 12, 0, 0),
        lastFailed: Date.UTC(2026, 4, 14, 12, 0, 0),
        lastVerified: Date.UTC(2026, 4, 11, 12, 0, 0),
        affectedEnvUrls: ["https://example.com"],
      },
      get_events: [
        {
          id: 1,
          occurredAtMs: Date.UTC(2026, 4, 10, 12, 0, 0),
          title: "Deploy before the fix",
          eventType: "deploy",
        },
        {
          id: 2,
          occurredAtMs: Date.UTC(2026, 4, 13, 12, 0, 0),
          title: "Deploy def456",
          eventType: "deploy",
        },
      ],
    });

    render(
      <IssueMemoryRail
        projectId={7}
        url="https://example.com"
        checkId="security.csp"
        currentStatus="fail"
      />,
      { wrapper: withQueryClient() },
    );

    expect(await screen.findByText("Regressed after Deploy def456.")).toBeInTheDocument();
  });

  it("labels storage failures as errors instead of no issue memory", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_projects") return Promise.resolve([project]);
      if (command === "get_issue_check_memory") {
        return Promise.reject(new Error("database unavailable"));
      }
      return Promise.resolve(null);
    });

    render(
      <IssueMemoryRail
        projectId={7}
        url="https://example.com"
        checkId="security.csp"
        currentStatus="fail"
      />,
      { wrapper: withQueryClient() },
    );

    expect(await screen.findByText("Issue history could not load.")).toBeInTheDocument();
    expect(screen.queryByText("No issue memory yet.")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});
