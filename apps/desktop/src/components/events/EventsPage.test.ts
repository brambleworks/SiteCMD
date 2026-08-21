import React from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { withQueryClient } from "@/test-utils/query-client";

const { useEventsMock, useProjectMock, useToastMock, getProjectSignalSnapshotMock } = vi.hoisted(
  () => ({
    useEventsMock: vi.fn(),
    useProjectMock: vi.fn(),
    useToastMock: vi.fn(),
    getProjectSignalSnapshotMock: vi.fn(),
  }),
);

vi.mock("@/lib/tauri-invoke", () => ({ invoke: vi.fn(() => Promise.resolve(null)) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@/hooks/useEvents", () => ({
  useEvents: (...args: unknown[]) => useEventsMock(...args),
}));
vi.mock("@/hooks/useProject", () => ({
  useProject: () => useProjectMock(),
}));
vi.mock("@/hooks/useToast", () => ({
  useToast: () => useToastMock(),
}));
vi.mock("@/app/ShellHeader", () => ({
  HeaderActions: ({ children }: { children: React.ReactNode }) =>
    React.createElement("div", null, children),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("react-virtuoso", () => ({
  Virtuoso: ({
    data,
    itemContent,
  }: {
    data: unknown[];
    itemContent: (index: number, item: unknown) => React.ReactNode;
  }) =>
    React.createElement(
      "div",
      null,
      data.map((item, index) =>
        React.createElement("div", { key: index }, itemContent(index, item)),
      ),
    ),
}));
vi.mock("@/lib/project-summary-signals", () => ({
  getProjectSignalSnapshot: (...args: unknown[]) => getProjectSignalSnapshotMock(...args),
}));

import {
  EventsPage,
  buildEventScanTarget,
  buildEventsCsvContent,
  dateRangeForView,
  endOfWeek,
  formatDateRange,
  humanizeEventDetail,
  navigate,
  startOfWeek,
} from "./EventsPage";

function renderEventsPage(props: React.ComponentProps<typeof EventsPage>) {
  return render(React.createElement(EventsPage, props), { wrapper: withQueryClient() });
}

beforeEach(() => {
  useEventsMock.mockReset();
  useProjectMock.mockReset();
  useToastMock.mockReset();
  getProjectSignalSnapshotMock.mockReset();

  useProjectMock.mockReturnValue({
    activeEnv: { url: "https://example.com" },
  });
  useToastMock.mockReturnValue({
    success: vi.fn(),
    info: vi.fn(),
    error: vi.fn(),
  });
  useEventsMock.mockReturnValue({
    events: [],
    loading: false,
    error: null,
    loadEvents: vi.fn(),
    refreshIntegrations: vi.fn(() => Promise.resolve(0)),
  });
  getProjectSignalSnapshotMock.mockResolvedValue({
    workSummary: {
      unresolvedCount: 0,
      newCount: 0,
      workingCount: 0,
      regressedCount: 0,
      ignoredCount: 0,
      blockedCount: 0,
      launchBlockerCount: 0,
      maintenanceCount: 0,
      primaryAction: null,
      regressedAction: null,
      workingAction: null,
      blockedAction: null,
      ignoredAction: null,
      launchBlockerAction: null,
      weeklySummary: null,
    },
  });
});

describe("startOfWeek / endOfWeek", () => {
  it("startOfWeek returns the previous Sunday (local time)", () => {
    const start = startOfWeek(new Date(2026, 3, 10));
    expect(start.getDay()).toBe(0);
    expect(start.getDate()).toBe(5);
  });

  it("endOfWeek returns the Saturday 6 days after startOfWeek", () => {
    const end = endOfWeek(new Date(2026, 3, 10));
    expect(end.getDay()).toBe(6);
    expect(end.getDate()).toBe(11);
  });

  it("is idempotent on a Sunday", () => {
    const sunday = new Date(2026, 3, 5);
    const s = startOfWeek(sunday);
    expect(s.getDate()).toBe(5);
  });

  it("handles end-of-month wraparound", () => {
    const s = startOfWeek(new Date(2026, 3, 1));
    expect(s.getMonth()).toBe(2);
    expect(s.getDate()).toBe(29);
  });
});

describe("formatDateRange", () => {
  const cursor = new Date(2026, 3, 10); // 2026-04-10, Friday

  it("returns '' for feed view", () => {
    expect(formatDateRange("feed", cursor)).toBe("");
  });

  it("returns 'Month YYYY' for month view", () => {
    expect(formatDateRange("month", cursor)).toBe("April 2026");
  });

  it("returns the same-month week range", () => {
    expect(formatDateRange("week", cursor)).toBe("Apr 5 \u2013 11, 2026");
  });

  it("returns cross-month week range when the week spans two months", () => {
    const out = formatDateRange("week", new Date(2026, 2, 31));
    expect(out).toBe("Mar 29 \u2013 Apr 4, 2026");
  });

  it("returns 'Weekday, Month D, YYYY' for day view", () => {
    expect(formatDateRange("day", cursor)).toBe("Friday, April 10, 2026");
  });
});

describe("dateRangeForView", () => {
  const cursor = new Date(2026, 3, 10);

  it("day view produces a single-day start/end", () => {
    const { start, end } = dateRangeForView("day", cursor);
    expect(start.startsWith("2026-04-10")).toBe(true);
    expect(end.startsWith("2026-04-10")).toBe(true);
    expect(start.endsWith("T00:00:00Z")).toBe(true);
    expect(end.endsWith("T23:59:59Z")).toBe(true);
  });

  it("week view spans 7 days (Sun -> Sat)", () => {
    const { start, end } = dateRangeForView("week", cursor);
    expect(start.startsWith("2026-04-05")).toBe(true);
    expect(end.startsWith("2026-04-11")).toBe(true);
  });

  it("month view spans 42 days from the grid start", () => {
    const { start, end } = dateRangeForView("month", cursor);
    // April 2026's six-week grid runs March 29 through May 9.
    expect(start.startsWith("2026-03-29")).toBe(true);
    expect(end.startsWith("2026-05-09")).toBe(true);
  });

  it("feed view looks back 30 days", () => {
    const { start, end } = dateRangeForView("feed", cursor);
    expect(start.startsWith("2026-03-11")).toBe(true);
    expect(end.startsWith("2026-04-10")).toBe(true);
  });
});

describe("navigate", () => {
  const cursor = new Date(2026, 3, 10);

  it("day view advances by one day", () => {
    const next = navigate("day", cursor, 1);
    expect(next.getDate()).toBe(11);
  });

  it("day view goes back one day", () => {
    const prev = navigate("day", cursor, -1);
    expect(prev.getDate()).toBe(9);
  });

  it("week view jumps by 7 days", () => {
    expect(navigate("week", cursor, 1).getDate()).toBe(17);
  });

  it("month view jumps to the 1st of the next month", () => {
    const next = navigate("month", cursor, 1);
    expect(next.getMonth()).toBe(4);
    expect(next.getDate()).toBe(1);
  });

  it("feed view jumps 30 days", () => {
    expect(navigate("feed", cursor, 1).getDate()).toBe(10);
    expect(navigate("feed", cursor, 1).getMonth()).toBe(4);
  });
});

describe("humanizeEventDetail", () => {
  it("extracts hostname from URL pills, appending path when non-root", () => {
    const pills = humanizeEventDetail({ url: "https://example.com/api/health" }, "scan");
    expect(pills[0]).toBe("example.com/api/health");
  });

  it("omits path for root URLs", () => {
    const pills = humanizeEventDetail({ url: "https://example.com/" }, "scan");
    expect(pills[0]).toBe("example.com");
  });

  it("falls back to raw url on invalid URLs", () => {
    const pills = humanizeEventDetail({ url: "not-a-url" }, "scan");
    expect(pills[0]).toBe("not-a-url");
  });

  it("strips query and hash parts from invalid URL labels", () => {
    const pills = humanizeEventDetail({ url: "not-a-url?token=secret#hash" }, "scan");
    expect(pills[0]).toBe("not-a-url");
  });

  it("emits score/issues/severity counts for scan events", () => {
    const pills = humanizeEventDetail(
      {
        overall_score: 87,
        issues_total: 12,
        critical_issues: 1,
        high_issues: 3,
        duration_ms: 5_400,
      },
      "scan_completed",
    );
    expect(pills).toContain("Score: 87");
    expect(pills).toContain("12 issues");
    expect(pills).toContain("1 critical");
    expect(pills).toContain("3 high");
    expect(pills).toContain("5.4s");
  });

  it("translates scan_type and top_domain into human labels", () => {
    const pills = humanizeEventDetail(
      {
        scan_type: "code",
        top_domain: "ai-safety",
        top_domain_count: 4,
        workflow_label: "2 regressed",
      },
      "scan_completed",
    );
    expect(pills).toContain("Code Scan");
    expect(pills).toContain("AI Safety 4");
    expect(pills).toContain("2 regressed");
  });

  it("scan_type=health maps to 'Web Scan · Full'", () => {
    const pills = humanizeEventDetail({ scan_type: "health" }, "scan");
    expect(pills).toContain("Web Scan \u00b7 Full");
  });

  it("slices commit_sha to 7 chars", () => {
    const pills = humanizeEventDetail(
      { commit_sha: "abcdef0123456789", branch: "main", status: "success" },
      "deploy_succeeded",
    );
    expect(pills).toContain("abcdef0");
    expect(pills).toContain("main");
    expect(pills).toContain("success");
  });

  it("analytics change_pct keeps sign formatting", () => {
    const positive = humanizeEventDetail({ metric: "visitors", change_pct: 25 }, "analytics");
    const negative = humanizeEventDetail({ metric: "visitors", change_pct: -15 }, "analytics");
    expect(positive).toContain("+25%");
    expect(negative).toContain("-15%");
  });

  it("skips non-finite numeric event detail labels", () => {
    const pills = humanizeEventDetail(
      {
        overall_score: Infinity,
        issues_total: Infinity,
        critical_issues: -1,
        high_issues: Number.NaN,
        completed_pages: Infinity,
        duration_ms: Infinity,
        change_pct: "not-a-number",
        downtime_minutes: Number("1e999"),
      },
      "analytics",
    );

    expect(pills).not.toContain("Score: Infinity");
    expect(pills).not.toContain("Infinity issues");
    expect(pills).not.toContain("-1 critical");
    expect(pills).not.toContain("NaN high");
    expect(pills).not.toContain("Infinity pages");
    expect(pills).not.toContain("Infinitys");
    expect(pills).not.toContain("NaN%");
    expect(pills).not.toContain("Infinitym down");
  });

  it("rounds counts and clamps score labels from persisted detail", () => {
    const pills = humanizeEventDetail(
      {
        overall_score: 145.7,
        issues_total: "2.4",
        critical_issues: 0.2,
        high_issues: "3.8",
        completed_pages: 2.5,
      },
      "scan_completed",
    );

    expect(pills).toContain("Score: 100");
    expect(pills).toContain("2 issues");
    expect(pills).toContain("0 critical");
    expect(pills).toContain("4 high");
    expect(pills).toContain("3 pages");
  });

  it("caps total pills at 5", () => {
    const pills = humanizeEventDetail(
      {
        url: "https://example.com/",
        overall_score: 80,
        issues_total: 1,
        critical_issues: 0,
        high_issues: 0,
        scan_type: "health",
        completed_pages: 5,
        duration_ms: 1000,
      },
      "scan",
    );
    expect(pills.length).toBeLessThanOrEqual(5);
  });

  it("drops null and empty values from fallback key dump", () => {
    const pills = humanizeEventDetail({ custom: "value", nullable: null, empty: "" }, "custom");
    expect(pills).toContain("custom: value");
    expect(pills.some((p) => p.startsWith("nullable"))).toBe(false);
    expect(pills.some((p) => p.startsWith("empty"))).toBe(false);
  });

  it("prioritizes compact update severity buckets for update events", () => {
    const pills = humanizeEventDetail(
      {
        url: "https://example.com",
        critical_updates: 1,
        major_updates: 2,
        minor_updates: 0,
        patch_updates: 3,
      },
      "update",
    );

    expect(pills[0]).toBe("1 Critical, 2 Major, 0 Minor, 3 Patch");
  });
});

describe("buildEventScanTarget", () => {
  it("builds a code scan target when code_scan_id is present", () => {
    expect(
      buildEventScanTarget(7, {
        code_scan_id: 12,
        url: "https://example.com",
      }),
    ).toEqual({
      page: "issues",
      projectId: 7,
      url: "https://example.com",
      scanId: 12,
      scanKind: "code",
    });
  });

  it("builds a site scan target when scan_id is present", () => {
    expect(
      buildEventScanTarget(7, {
        scan_id: 9,
        url: "https://example.com",
      }),
    ).toEqual({
      page: "issues",
      projectId: 7,
      url: "https://example.com",
      scanId: 9,
      scanKind: "site",
    });
  });

  it("builds a multi-page session target when session_id is present", () => {
    expect(
      buildEventScanTarget(7, {
        session_id: 44,
        url: "https://example.com",
      }),
    ).toEqual({
      page: "issues",
      projectId: 7,
      url: "https://example.com",
      sessionId: 44,
    });
  });

  it("builds an updates target when dependency event detail is present", () => {
    expect(
      buildEventScanTarget(7, {
        page: "updates",
        url: "https://example.com",
        item_id: "npm:react",
        reason: "dependency-verification",
      }),
    ).toEqual({
      page: "updates",
      projectId: 7,
      url: "https://example.com",
      itemId: "npm:react",
      reason: "dependency-verification",
    });
  });

  it("builds a search target when search verification detail is present", () => {
    expect(
      buildEventScanTarget(7, {
        page: "search-console",
        url: "https://example.com",
        item_id: "seo.title",
        focus: "seo.titles",
        reason: "search-verification",
      }),
    ).toEqual({
      page: "search-console",
      projectId: 7,
      url: "https://example.com",
      itemId: "seo.title",
      focus: "seo.titles",
      reason: "search-verification",
    });
  });

  it("builds a security target when security verification detail is present", () => {
    expect(
      buildEventScanTarget(7, {
        page: "issues",
        url: "https://example.com",
        item_id: "code-2",
        focus: "code_scan",
        reason: "security-verification",
      }),
    ).toEqual({
      page: "issues",
      projectId: 7,
      url: "https://example.com",
      itemId: "code-2",
      focus: "code_scan",
      reason: "security-verification",
    });
  });

  it("returns null when no scan identifiers are present", () => {
    expect(buildEventScanTarget(7, { url: "https://example.com" })).toBeNull();
    expect(buildEventScanTarget(7, null)).toBeNull();
  });

  it("rejects malformed scan identifiers from event detail", () => {
    expect(
      buildEventScanTarget(7, {
        scan_id: Number.POSITIVE_INFINITY,
        url: "https://example.com",
      }),
    ).toBeNull();
    expect(
      buildEventScanTarget(7, {
        code_scan_id: -1,
        url: "https://example.com",
      }),
    ).toBeNull();
    expect(
      buildEventScanTarget(7, {
        session_id: 1.5,
        url: "https://example.com",
      }),
    ).toBeNull();
  });

  it("drops unsafe URLs from event navigation targets", () => {
    expect(
      buildEventScanTarget(7, {
        scan_id: 9,
        url: "https://user:token@example.com/private",
      }),
    ).toEqual({
      page: "issues",
      projectId: 7,
      url: null,
      scanId: 9,
      scanKind: "site",
    });
  });
});

describe("EventsPage", () => {
  it("prefixes formula-like cells when building CSV exports", () => {
    const csv = buildEventsCsvContent([
      {
        id: 1,
        projectId: 7,
        eventType: "update",
        severity: "warning",
        occurredAtMs: Date.parse("2026-04-12T12:00:00Z"),
        title: "=CMD()",
        summary: "@SUM(A1:A2)",
        detail: null,
        source: "internal",
        sourceId: null,
      } as never,
    ]);

    expect(csv).toContain(`"'=CMD()"`);
    expect(csv).toContain(`"'@SUM(A1:A2)"`);
  });

  it("opens the exact saved multi-page session target from the timeline", async () => {
    const onOpenTarget = vi.fn();
    useEventsMock.mockReturnValue({
      events: [
        {
          id: 44,
          projectId: 7,
          eventType: "scan",
          severity: "info",
          occurredAtMs: Date.parse("2026-04-11T12:00:00Z"),
          title: "Multi-page scan finished",
          summary: "Scanned 5 pages.",
          detail: JSON.stringify({
            session_id: 88,
            url: "https://example.com",
            overall_score: 82,
            completed_pages: 5,
          }),
          source: "scanner",
          sourceId: "session-88",
        },
      ],
      loading: false,
      loadEvents: vi.fn(),
      refreshIntegrations: vi.fn(() => Promise.resolve(0)),
    });

    renderEventsPage({ projectId: 7, onOpenTarget });

    fireEvent.click(await screen.findByRole("button", { name: /Multi-page scan finished/i }));
    expect(onOpenTarget).toHaveBeenCalledWith({
      page: "issues",
      projectId: 7,
      url: "https://example.com",
      sessionId: 88,
    });
  });

  it("opens the exact saved package update target from the timeline", async () => {
    const onOpenTarget = vi.fn();
    useEventsMock.mockReturnValue({
      events: [
        {
          id: 45,
          projectId: 7,
          eventType: "update",
          severity: "info",
          occurredAtMs: Date.parse("2026-04-11T12:30:00Z"),
          title: "Update verified: react",
          summary:
            "react 18.2.0 -> 19.0.0 cleared from Updates. Next up: react-dom 18.2.0 -> 19.0.0",
          detail: JSON.stringify({
            page: "updates",
            url: "https://example.com",
            item_id: "npm:react-dom",
            item_label: "react 18.2.0 -> 19.0.0",
            verified_label: "react 18.2.0 -> 19.0.0",
            next_item_label: "react-dom 18.2.0 -> 19.0.0",
            status_before: "Pending",
            status_after: "Verified",
            remaining_updates: 1,
            workflow_label: "Exact package verified",
            reason: "dependency-verification",
          }),
          source: "internal",
          sourceId: "updates-verify:7:npm:react:verified:1",
        },
      ],
      loading: false,
      loadEvents: vi.fn(),
      refreshIntegrations: vi.fn(() => Promise.resolve(0)),
    });

    renderEventsPage({ projectId: 7, onOpenTarget });

    fireEvent.click(await screen.findByRole("button", { name: /Update verified: react/i }));
    expect(onOpenTarget).toHaveBeenCalledWith({
      page: "updates",
      projectId: 7,
      url: "https://example.com",
      itemId: "npm:react-dom",
      reason: "dependency-verification",
    });
  });

  it("collapses adjacent web and code scans into one Full Scan row in feed view", async () => {
    useEventsMock.mockReturnValue({
      events: [
        {
          id: 44,
          projectId: 7,
          eventType: "scan",
          severity: "warning",
          occurredAtMs: Date.parse("2026-04-11T12:01:00Z"),
          title: "SiteCMD Score: 77/100",
          summary: "4 code issues (1 critical, 1 high)",
          detail: JSON.stringify({
            code_scan_id: 88,
            scan_type: "code",
            overall_score: 77,
            issues_total: 4,
            url: "https://example.com",
          }),
          source: "internal",
          sourceId: "code-scan-88",
        },
        {
          id: 43,
          projectId: 7,
          eventType: "scan",
          severity: "warning",
          occurredAtMs: Date.parse("2026-04-11T12:00:00Z"),
          title: "SiteCMD Score: 82/100",
          summary: "3 issues (1 critical, 1 high)",
          detail: JSON.stringify({
            scan_id: 87,
            scan_type: "health",
            overall_score: 82,
            issues_total: 3,
            url: "https://example.com",
          }),
          source: "internal",
          sourceId: "scan-87",
        },
      ],
      loading: false,
      loadEvents: vi.fn(),
      refreshIntegrations: vi.fn(() => Promise.resolve(0)),
    });

    renderEventsPage({ projectId: 7 });

    expect(await screen.findByText("Full Scan")).toBeInTheDocument();
    expect(screen.queryByText("SiteCMD Score: 82/100")).not.toBeInTheDocument();
    expect(screen.queryByText("SiteCMD Score: 77/100")).not.toBeInTheDocument();
  });

  it("shows compact update counts on the feed for update events", async () => {
    useEventsMock.mockReturnValue({
      events: [
        {
          id: 45,
          projectId: 7,
          eventType: "update",
          severity: "warning",
          occurredAtMs: Date.parse("2026-04-11T12:30:00Z"),
          title: "3 Updates Applied",
          summary: "react, vite, and lucide-react were updated.",
          detail: JSON.stringify({
            page: "updates",
            url: "https://example.com",
            critical_updates: 1,
            major_updates: 1,
            minor_updates: 0,
            patch_updates: 2,
          }),
          source: "internal",
          sourceId: "updates-45",
        },
      ],
      loading: false,
      loadEvents: vi.fn(),
      refreshIntegrations: vi.fn(() => Promise.resolve(0)),
    });

    renderEventsPage({ projectId: 7 });

    expect(await screen.findByText("3 Updates Applied")).toBeInTheDocument();
    expect(screen.getByText("1 Critical, 1 Major, 0 Minor, 2 Patch")).toBeInTheDocument();
  });

  it("opens the strongest saved target from a Today verification campaign", async () => {
    const onOpenTarget = vi.fn();
    useEventsMock.mockReturnValue({
      events: [
        {
          id: 44,
          projectId: 31,
          eventType: "verification",
          severity: "warning",
          occurredAtMs: Date.parse("2026-04-11T12:20:00Z"),
          title: "Today verify sweep: 2 issues still open",
          summary:
            "Checked 3 items again on Dependency Queue. Cleared 3 reminders. 2 issues are still open. Next up: axios 1.6.0 -> 1.7.0 • security (critical).",
          detail: JSON.stringify({
            page: "updates",
            url: "https://deps-verify.test",
            item_id: "npm:axios",
            lane: "pending-verification",
            reason: "today-verification",
            rechecked_count: 3,
            cleared_count: 3,
            still_failing_count: 2,
            recurring_source_count: 0,
            recurring_source_cleared_count: 0,
            recurring_source_still_failing_count: 0,
            next_item_label: "axios 1.6.0 -> 1.7.0 • security (critical)",
            workflow_label: "Today verification continues",
          }),
          source: "internal",
          sourceId: "today-verify-31:1",
        },
      ],
      loading: false,
      loadEvents: vi.fn(),
      refreshIntegrations: vi.fn(() => Promise.resolve(0)),
    });

    renderEventsPage({ projectId: 31, onOpenTarget });

    fireEvent.click(await screen.findByRole("button", { name: /Today verify sweep/i }));
    expect(onOpenTarget).toHaveBeenCalledWith({
      page: "updates",
      projectId: 31,
      url: "https://deps-verify.test",
      itemId: "npm:axios",
      lane: "pending-verification",
      reason: "today-verification",
    });
  });

  it("opens the exact saved Search & SEO target from the timeline", async () => {
    const onOpenTarget = vi.fn();
    useEventsMock.mockReturnValue({
      events: [
        {
          id: 47,
          projectId: 7,
          eventType: "search",
          severity: "warning",
          occurredAtMs: Date.parse("2026-04-11T12:50:00Z"),
          title: "Search & SEO checks still open",
          summary: "Verified 2 Search & SEO focus areas. Next issue: Title tag is missing.",
          detail: JSON.stringify({
            page: "search-console",
            url: "https://example.com",
            item_id: "seo.title",
            item_label: "Title tag is missing",
            verified_label: "Robots.txt blocks crawling",
            next_item_label: "Title tag is missing",
            status_before: "Fail",
            status_after: "Pass",
            focus: "seo.titles",
            focus_label: "Titles",
            checked_count: 2,
            open_checks: 1,
            passed_checks: 1,
            workflow_label: "Search verification continues",
            reason: "search-verification",
          }),
          source: "internal",
          sourceId: "search-verify-all:7:https://example.com:complete",
        },
      ],
      loading: false,
      loadEvents: vi.fn(),
      refreshIntegrations: vi.fn(() => Promise.resolve(0)),
    });

    renderEventsPage({ projectId: 7, onOpenTarget });

    fireEvent.click(await screen.findByRole("button", { name: /Search & SEO checks still open/i }));
    expect(onOpenTarget).toHaveBeenCalledWith({
      page: "search-console",
      projectId: 7,
      url: "https://example.com",
      itemId: "seo.title",
      focus: "seo.titles",
      reason: "search-verification",
    });
  });

  it("opens the exact saved Security target from the timeline", async () => {
    const onOpenTarget = vi.fn();
    useEventsMock.mockReturnValue({
      events: [
        {
          id: 48,
          projectId: 7,
          eventType: "security",
          severity: "warning",
          occurredAtMs: Date.parse("2026-04-11T12:55:00Z"),
          title: "Code issues still open",
          summary: "Verified 1 queued Code Scan fix. Next guardrail: Missing CSP middleware.",
          detail: JSON.stringify({
            page: "issues",
            url: "https://example.com",
            item_id: "code-2",
            item_label: "Missing CSP middleware",
            verified_label: "Unsafe auth helper",
            next_item_label: "Missing CSP middleware",
            status_before: "Fail",
            status_after: "Pass",
            focus: "code_scan",
            focus_label: "Code Scan",
            priority_before: 2,
            priority_after: 1,
            code_issues_before: 2,
            code_issues_after: 1,
            workflow_label: "Code verification continues",
            reason: "security-verification",
          }),
          source: "internal",
          sourceId: "source-verify:7:all:complete",
        },
      ],
      loading: false,
      loadEvents: vi.fn(),
      refreshIntegrations: vi.fn(() => Promise.resolve(0)),
    });

    renderEventsPage({ projectId: 7, onOpenTarget });

    fireEvent.click(await screen.findByRole("button", { name: /Code issues still open/i }));
    expect(onOpenTarget).toHaveBeenCalledWith({
      page: "issues",
      projectId: 7,
      url: "https://example.com",
      itemId: "code-2",
      focus: "code_scan",
      reason: "security-verification",
    });
  });

  it("shows a retry state when activity fails to load", async () => {
    const loadEvents = vi.fn();
    useEventsMock.mockReturnValue({
      events: [],
      loading: false,
      error: "Activity could not load right now.",
      loadEvents,
      refreshIntegrations: vi.fn(() => Promise.resolve(0)),
    });

    renderEventsPage({ projectId: 7 });

    expect(await screen.findByText("Activity could not load")).toBeInTheDocument();
    const initialCalls = loadEvents.mock.calls.length;
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(loadEvents).toHaveBeenCalledTimes(initialCalls + 1);
  });
});
