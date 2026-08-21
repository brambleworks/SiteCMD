import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AlertRow } from "@/lib/types";
import { withQueryClient } from "@/test-utils/query-client";
import { useAlerts } from "./useAlerts";

const {
  countUnreadAlertsMock,
  dismissAlertMock,
  getAlertsMock,
  markAlertUnreadMock,
  markAlertViewedMock,
  markAlertsViewedBulkMock,
} = vi.hoisted(() => ({
  countUnreadAlertsMock: vi.fn(),
  dismissAlertMock: vi.fn(),
  getAlertsMock: vi.fn(),
  markAlertUnreadMock: vi.fn(),
  markAlertViewedMock: vi.fn(),
  markAlertsViewedBulkMock: vi.fn(),
}));

vi.mock("@/lib/alerts", () => ({
  countUnreadAlerts: countUnreadAlertsMock,
  dismissAlert: dismissAlertMock,
  getAlerts: getAlertsMock,
  markAlertUnread: markAlertUnreadMock,
  markAlertViewed: markAlertViewedMock,
  markAlertsViewedBulk: markAlertsViewedBulkMock,
}));

const baseAlert: AlertRow = {
  id: 1,
  projectId: 1,
  envUrl: "https://example.com",
  source: "uptimerobot",
  alertId: "outage:a",
  severity: "critical",
  title: "Site down",
  description: "Monitor flagged.",
  detailJson: null,
  occurredAt: Date.now() - 60_000,
  firstSeenAt: Date.now() - 60_000,
  lastSeenAt: Date.now() - 60_000,
  viewedAt: null,
  dismissedAt: null,
};

function AlertPageConsumer() {
  const { alerts, markAllRead, markUnread, markViewed, unreadCount } = useAlerts(1, "all");
  const firstAlert = alerts[0] ?? null;

  return (
    <div>
      <div data-testid="page-count">{unreadCount}</div>
      {firstAlert ? (
        <>
          <button type="button" onClick={() => void markViewed(firstAlert.id)}>
            Mark first viewed
          </button>
          <button type="button" onClick={() => void markUnread(firstAlert.id)}>
            Mark first unread
          </button>
        </>
      ) : null}
      <button type="button" onClick={() => void markAllRead()}>
        Mark all read
      </button>
    </div>
  );
}

function SidebarBadgeConsumer() {
  const { unreadCount } = useAlerts(1, "unread", { includeRows: false });
  return <div data-testid="sidebar-count">{unreadCount}</div>;
}

describe("useAlerts", () => {
  beforeEach(() => {
    countUnreadAlertsMock.mockReset();
    dismissAlertMock.mockReset();
    getAlertsMock.mockReset();
    markAlertUnreadMock.mockReset();
    markAlertViewedMock.mockReset();
    markAlertsViewedBulkMock.mockReset();
  });

  it("refreshes other mounted alert consumers after marking an alert viewed", async () => {
    let unreadCount = 1;
    let rows: AlertRow[] = [baseAlert];
    countUnreadAlertsMock.mockImplementation(() =>
      Promise.resolve({ total: unreadCount, critical: 0 }),
    );
    getAlertsMock.mockImplementation(() => Promise.resolve(rows));
    markAlertViewedMock.mockImplementation(async () => {
      unreadCount = 0;
      rows = [{ ...baseAlert, viewedAt: Date.now() }];
    });

    render(
      <>
        <AlertPageConsumer />
        <SidebarBadgeConsumer />
      </>,
      { wrapper: withQueryClient() },
    );

    await waitFor(() => expect(screen.getByTestId("sidebar-count")).toHaveTextContent("1"));

    fireEvent.click(screen.getByRole("button", { name: "Mark first viewed" }));

    await waitFor(() => expect(screen.getByTestId("sidebar-count")).toHaveTextContent("0"));
    expect(markAlertViewedMock).toHaveBeenCalledWith(1);
  });

  it("refreshes other mounted alert consumers after marking all alerts read", async () => {
    const secondAlert: AlertRow = { ...baseAlert, id: 2, alertId: "outage:b" };
    let unreadCount = 2;
    let rows: AlertRow[] = [baseAlert, secondAlert];
    countUnreadAlertsMock.mockImplementation(() =>
      Promise.resolve({ total: unreadCount, critical: 0 }),
    );
    getAlertsMock.mockImplementation(() => Promise.resolve(rows));
    markAlertsViewedBulkMock.mockImplementation(async () => {
      unreadCount = 0;
      rows = rows.map((alert) => ({ ...alert, viewedAt: Date.now() }));
    });

    render(
      <>
        <AlertPageConsumer />
        <SidebarBadgeConsumer />
      </>,
      { wrapper: withQueryClient() },
    );

    await waitFor(() => expect(screen.getByTestId("sidebar-count")).toHaveTextContent("2"));

    fireEvent.click(screen.getByRole("button", { name: "Mark all read" }));

    await waitFor(() => expect(screen.getByTestId("sidebar-count")).toHaveTextContent("0"));
    expect(markAlertsViewedBulkMock).toHaveBeenCalledWith([1, 2]);
  });

  it("refreshes other mounted alert consumers after marking an alert unread", async () => {
    let unreadCount = 0;
    let rows: AlertRow[] = [{ ...baseAlert, viewedAt: Date.now() }];
    countUnreadAlertsMock.mockImplementation(() =>
      Promise.resolve({ total: unreadCount, critical: 0 }),
    );
    getAlertsMock.mockImplementation(() => Promise.resolve(rows));
    markAlertUnreadMock.mockImplementation(async () => {
      unreadCount = 1;
      rows = [{ ...baseAlert, viewedAt: null }];
    });

    render(
      <>
        <AlertPageConsumer />
        <SidebarBadgeConsumer />
      </>,
      { wrapper: withQueryClient() },
    );

    await waitFor(() => expect(screen.getByTestId("sidebar-count")).toHaveTextContent("0"));

    fireEvent.click(await screen.findByRole("button", { name: "Mark first unread" }));

    await waitFor(() => expect(screen.getByTestId("sidebar-count")).toHaveTextContent("1"));
    expect(markAlertUnreadMock).toHaveBeenCalledWith(1);
  });
});
