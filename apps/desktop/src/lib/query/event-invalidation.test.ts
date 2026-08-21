import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient } from "@tanstack/react-query";

const { listenerRegistry, safeListenMock } = vi.hoisted(() => {
  const registry = new Map<string, (event: { payload: unknown }) => void>();
  return {
    listenerRegistry: registry,
    safeListenMock: vi.fn((event: string, handler: (event: { payload: unknown }) => void) => {
      registry.set(event, handler);
      return Promise.resolve(() => {});
    }),
  };
});

vi.mock("@/lib/tauri-events", () => ({ safeListen: safeListenMock }));

import { installQueryEventInvalidation, QUERY_INVALIDATION_RULES } from "./event-invalidation";
import { queryKeys } from "./query-keys";
import { ISSUE_LIFECYCLE_CHANGED_EVENT } from "@/lib/issues";

function fakeClient() {
  return { invalidateQueries: vi.fn() } as unknown as QueryClient;
}

function dispatch(event: string, payload?: unknown) {
  listenerRegistry.get(event)!({ payload });
}

describe("query event-invalidation registry", () => {
  beforeEach(() => {
    listenerRegistry.clear();
    safeListenMock.mockClear();
    window.sessionStorage.clear();
  });

  it("registers a listener for every rule", () => {
    installQueryEventInvalidation(fakeClient());
    for (const rule of QUERY_INVALIDATION_RULES) {
      expect(listenerRegistry.has(rule.event)).toBe(true);
    }
    expect(safeListenMock).toHaveBeenCalledTimes(QUERY_INVALIDATION_RULES.length);
  });

  it("scan-execution-completed invalidates execution detail and code audit families", () => {
    const client = fakeClient();
    installQueryEventInvalidation(client);
    dispatch("scan-execution-completed", { projectId: 3 });
    // Addressed by run id, so it stays global.
    expect(client.invalidateQueries).toHaveBeenCalledWith({
      queryKey: queryKeys.scanExecution.all,
    });
    expect(client.invalidateQueries).toHaveBeenCalledWith({
      queryKey: queryKeys.codeScanAudit.projectScope(3),
    });
  });

  it("lifecycle and score events both invalidate work items", () => {
    const client = fakeClient();
    installQueryEventInvalidation(client);
    dispatch(ISSUE_LIFECYCLE_CHANGED_EVENT, { projectId: 3 });
    dispatch("site-score-changed", { projectId: 3 });
    const workItemInvalidations = vi
      .mocked(client.invalidateQueries)
      .mock.calls.filter(
        ([filters]) =>
          JSON.stringify(filters?.queryKey) === JSON.stringify(queryKeys.workItems.projectScope(3)),
      );
    expect(workItemInvalidations).toHaveLength(2);
  });

  it("scopes a scan completion to the project that scanned", () => {
    const client = fakeClient();
    installQueryEventInvalidation(client);

    dispatch("scan-execution-completed", { projectId: 3 });

    const keys = vi
      .mocked(client.invalidateQueries)
      .mock.calls.map(([filters]) => JSON.stringify(filters?.queryKey));
    expect(keys).toContain(JSON.stringify(queryKeys.projectSummary.projectScope(3)));
    expect(keys).toContain(JSON.stringify(queryKeys.reports.projectScope(3)));
    // The unscoped family prefixes must not appear - those are what reached
    // across projects.
    expect(keys).not.toContain(JSON.stringify(queryKeys.projectSummary.all));
    expect(keys).not.toContain(JSON.stringify(queryKeys.reports.all));
    // Genuinely multi-project data still sweeps in full.
    expect(keys).toContain(JSON.stringify(queryKeys.sites.all));
  });

  it("falls back to a full sweep when the payload cannot name a project", () => {
    const client = fakeClient();
    installQueryEventInvalidation(client);

    dispatch("fix-attempt-updated");
    dispatch("site-score-changed", {});
    dispatch("alerts-changed", { projectId: null });

    const keys = vi
      .mocked(client.invalidateQueries)
      .mock.calls.map(([filters]) => JSON.stringify(filters?.queryKey));
    expect(keys).toContain(JSON.stringify(queryKeys.workItems.all));
    expect(keys).toContain(JSON.stringify(queryKeys.currentScore.all));
    expect(keys).toContain(JSON.stringify(queryKeys.alerts.all));
  });

  it("integration signal changes invalidate only that project's live data and analytics", () => {
    const client = fakeClient();
    installQueryEventInvalidation(client);

    dispatch("project-signals-changed", {
      projectId: 7,
      url: null,
      source: "integration",
    });

    expect(client.invalidateQueries).toHaveBeenCalledWith({
      queryKey: queryKeys.integrations.forProject(7),
    });
    expect(client.invalidateQueries).toHaveBeenCalledWith({
      queryKey: queryKeys.analytics.forProject(7),
    });
  });

  it("clears persisted project summaries even when no in-memory query exists", () => {
    const client = fakeClient();
    window.sessionStorage.setItem("sitecmd:dashboard-snapshot:7:https://example.com", "old");
    window.sessionStorage.setItem(
      "sitecmd:dashboard-reference-signals:7:https://example.com:base",
      "old",
    );
    window.sessionStorage.setItem("sitecmd:nav-badge-snapshot:8:https://other.test", "keep");
    installQueryEventInvalidation(client);

    dispatch("site-score-changed", { projectId: 7 });

    expect(window.sessionStorage.getItem("sitecmd:dashboard-snapshot:7:https://example.com")).toBe(
      null,
    );
    expect(
      window.sessionStorage.getItem(
        "sitecmd:dashboard-reference-signals:7:https://example.com:base",
      ),
    ).toBe(null);
    expect(window.sessionStorage.getItem("sitecmd:nav-badge-snapshot:8:https://other.test")).toBe(
      "keep",
    );
  });

  it("non-integration project signals do not sweep integration data", () => {
    const client = fakeClient();
    installQueryEventInvalidation(client);

    dispatch("project-signals-changed", {
      projectId: 7,
      url: "https://example.com",
      source: "updates",
    });

    expect(client.invalidateQueries).not.toHaveBeenCalledWith({
      queryKey: queryKeys.integrations.forProject(7),
    });
    expect(client.invalidateQueries).not.toHaveBeenCalledWith({
      queryKey: queryKeys.analytics.forProject(7),
    });
  });

  it("events-recorded refreshes the cached activity ranges", () => {
    const client = fakeClient();
    installQueryEventInvalidation(client);

    dispatch("events-recorded", { projectId: 3 });

    expect(client.invalidateQueries).toHaveBeenCalledWith({
      queryKey: queryKeys.events.projectScope(3),
    });
  });

  it("integration-hint-dismissed refreshes the issue-group families that carry the hint", () => {
    const client = fakeClient();
    installQueryEventInvalidation(client);

    dispatch("integration-hint-dismissed", {
      projectId: 3,
      checkId: "missing-og-tags",
      integration: "plausible",
    });

    expect(client.invalidateQueries).toHaveBeenCalledWith({
      queryKey: queryKeys.pageIssues.projectScope(3),
    });
    expect(client.invalidateQueries).toHaveBeenCalledWith({
      queryKey: queryKeys.workItems.projectScope(3),
    });
  });

  it("a completed refresh tick refetches the catalog status, whatever its outcome", () => {
    const client = fakeClient();
    installQueryEventInvalidation(client);

    dispatch("catalog-refresh-completed", undefined);

    expect(client.invalidateQueries).toHaveBeenCalledWith({
      queryKey: queryKeys.settings.catalogStatus(),
    });
  });
});

describe("project-scoped invalidation against a real client", () => {
  const MINE = 3;
  const OTHER = 9;

  // One real, in-use key per project-scoped family, for two different projects.
  function seededKeys() {
    return {
      workItems: [
        queryKeys.workItems.forEnv(MINE, "https://a.com"),
        queryKeys.workItems.forEnv(OTHER, "https://b.com"),
      ],
      pageIssues: [
        queryKeys.pageIssues.forPage(MINE, "https://a.com", "https://a.com/x"),
        queryKeys.pageIssues.forPage(OTHER, "https://b.com", "https://b.com/x"),
      ],
      issuePages: [
        queryKeys.issuePages.forEnv(MINE, "https://a.com"),
        queryKeys.issuePages.forEnv(OTHER, "https://b.com"),
      ],
      issueMemory: [
        queryKeys.issueMemory.forCheck(MINE, "seo.title", null),
        queryKeys.issueMemory.forCheck(OTHER, "seo.title", null),
      ],
      resolvedIssues: [
        queryKeys.resolvedIssues.forEnv(MINE, "https://a.com", 100),
        queryKeys.resolvedIssues.forEnv(OTHER, "https://b.com", 100),
      ],
      currentScore: [
        queryKeys.currentScore.forEnv(MINE, "https://a.com"),
        queryKeys.currentScore.forEnv(OTHER, "https://b.com"),
      ],
      codeScanAudit: [
        queryKeys.codeScanAudit.forProject(MINE, "/a"),
        queryKeys.codeScanAudit.forProject(OTHER, "/b"),
      ],
      reports: [
        queryKeys.reports.snapshot(MINE, "https://a.com", 30, "s"),
        queryKeys.reports.snapshot(OTHER, "https://b.com", 30, "s"),
      ],
      alerts: [queryKeys.alerts.rows(MINE, "all"), queryKeys.alerts.rows(OTHER, "all")],
      deploys: [
        queryKeys.deploys.overview(MINE, "https://a.com", "/a"),
        queryKeys.deploys.overview(OTHER, "https://b.com", "/b"),
      ],
      events: [
        queryKeys.events.range(MINE, "s", "e", ""),
        queryKeys.events.range(OTHER, "s", "e", ""),
      ],
      updates: [
        queryKeys.updates.report(MINE, "/a", "https://a.com"),
        queryKeys.updates.report(OTHER, "/b", "https://b.com"),
      ],
      projectSummary: [
        queryKeys.projectSummary.snapshot(MINE, "https://a.com"),
        queryKeys.projectSummary.snapshot(OTHER, "https://b.com"),
      ],
      integrations: [
        queryKeys.integrations.data(MINE, "github", "a.com"),
        queryKeys.integrations.data(OTHER, "github", "b.com"),
      ],
      analytics: [
        queryKeys.analytics.forQuery(MINE, "30d", null),
        queryKeys.analytics.forQuery(OTHER, "30d", null),
      ],
    };
  }

  function seed(client: QueryClient) {
    for (const pair of Object.values(seededKeys())) {
      for (const key of pair) client.setQueryData(key, { seeded: true });
    }
  }

  function invalidatedNames(client: QueryClient, index: 0 | 1) {
    return Object.entries(seededKeys())
      .filter(([, pair]) => client.getQueryState(pair[index])?.isInvalidated)
      .map(([name]) => name);
  }

  it("reaches every family it claims to, for the scanning project only", async () => {
    const client = new QueryClient();
    seed(client);
    installQueryEventInvalidation(client);

    dispatch("scan-execution-completed", { projectId: MINE });
    await vi.waitFor(() =>
      expect(invalidatedNames(client, 0)).toEqual(
        expect.arrayContaining([
          "workItems",
          "pageIssues",
          "issuePages",
          "issueMemory",
          "currentScore",
          "codeScanAudit",
          "reports",
          "alerts",
          "deploys",
          "events",
          "projectSummary",
        ]),
      ),
    );

    // The other project keeps its cache - that is the whole point.
    expect(invalidatedNames(client, 1)).toEqual([]);
  });

  it("still reaches both projects when the payload names none", async () => {
    const client = new QueryClient();
    seed(client);
    installQueryEventInvalidation(client);

    dispatch("fix-attempt-updated");

    await vi.waitFor(() =>
      expect(invalidatedNames(client, 1)).toEqual(
        expect.arrayContaining(["workItems", "pageIssues", "currentScore", "projectSummary"]),
      ),
    );
  });
});
