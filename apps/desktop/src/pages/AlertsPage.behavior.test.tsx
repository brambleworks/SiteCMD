import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { AlertsPage } from "./AlertsPage";
import { withQueryClient } from "@/test-utils/query-client";

function renderAlerts(ui: React.ReactElement) {
  return render(ui, { wrapper: withQueryClient() });
}

const { publishAlertsChangedMock, publishEventsRecordedMock, refreshEventsMock } = vi.hoisted(
  () => ({
    publishAlertsChangedMock: vi.fn(),
    publishEventsRecordedMock: vi.fn(),
    refreshEventsMock: vi.fn(async () => {}),
  }),
);

const mockHook = vi.fn();
const mockConnectedHook = vi.fn();
vi.mock("@/hooks/useAlerts", () => ({
  useAlerts: (...args: unknown[]) => mockHook(...args),
  publishAlertsChanged: publishAlertsChangedMock,
}));
vi.mock("@/hooks/useConnectedAlerts", () => ({
  useConnectedAlerts: (...args: unknown[]) => mockConnectedHook(...args),
}));
vi.mock("@/lib/event-writes", () => ({ publishEventsRecorded: publishEventsRecordedMock }));
vi.mock("@/lib/commands", async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  refreshEvents: refreshEventsMock,
}));

/** Connected-service-unconfigured fixture. */
function noConnectedService() {
  return {
    failed: false,
    feed: { alerts: [], availability: "service_unconfigured", elsewhere: [], truncated: false },
    loading: false,
  };
}

function connectedState(
  feed: Partial<{
    alerts: unknown[];
    availability: string;
    elsewhere: unknown[];
    truncated: boolean;
  }>,
  state: { failed?: boolean; loading?: boolean } = {},
) {
  return {
    failed: state.failed ?? false,
    feed: { alerts: [], availability: "ready", elsewhere: [], truncated: false, ...feed },
    loading: state.loading ?? false,
  };
}

const connectedAlert = {
  alertId: "alr_0123456789abcdef01234567",
  causes: [{ count: 2, kind: "regression", severity: "critical" }],
  contentMode: "private",
  delivery: [{ outcome: "sent", targetId: "dst_1", targetKind: "destination" }],
  deploymentId: null,
  raisedAt: "2026-08-10T12:00:00.000Z",
  sequence: 12,
  severity: "critical",
  updatedAt: null,
};

const hasFeatureMock = vi.fn();

vi.mock("@/hooks/useTier", () => ({
  useTier: () => ({
    hasFeature: hasFeatureMock,
    licenseInfo: {
      checkout_urls: {
        core_monthly: "",
        pro_monthly: "",
      },
    },
  }),
}));

beforeEach(() => {
  hasFeatureMock.mockReset();
  hasFeatureMock.mockReturnValue(false);
  mockConnectedHook.mockReset();
  mockConnectedHook.mockReturnValue(noConnectedService());
});

const sampleAlert = {
  id: 1,
  projectId: 1,
  envUrl: "https://example.com",
  source: "uptimerobot",
  alertId: "outage:a",
  severity: "critical" as const,
  title: "Site down",
  description: "Monitor flagged.",
  detailJson: null,
  occurredAt: Date.now() - 60_000,
  firstSeenAt: 0,
  lastSeenAt: 0,
  viewedAt: null,
  dismissedAt: null,
};

describe("AlertsPage", () => {
  beforeEach(() => mockHook.mockReset());

  it("renders the feed full width and marks unread alerts read when details open", async () => {
    const markViewed = vi.fn(() => Promise.resolve());
    mockHook.mockReturnValue({
      alerts: [sampleAlert],
      unreadCount: 1,
      loading: false,
      error: null,
      refresh: vi.fn(),
      dismiss: vi.fn(),
      markViewed,
      markUnread: vi.fn(),
      markAllRead: vi.fn(),
    });
    renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);
    expect(screen.queryByText("Alerts", { selector: ".card__title span" })).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        "SiteCMD and connected-service events that changed enough to deserve attention.",
      ),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Status")).not.toBeInTheDocument();
    expect(screen.getAllByText("Site down").length).toBeGreaterThan(0);
    const alertRow = screen.getByRole("button", { name: /Site down/i });
    expect(alertRow.className).toContain("alert-list-row-unread");
    expect(alertRow.className).not.toMatch(/\bborder-l/);
    expect(alertRow.querySelector(".rounded-full")).toBeNull();
    expect(screen.getByText("Critical").className).not.toMatch(/\brounded|bg-/);
    expect(screen.queryByRole("button", { name: "Mark read" })).not.toBeInTheDocument();
    fireEvent.click(alertRow);
    expect(screen.getByRole("dialog", { name: "Site down" })).toBeInTheDocument();
    await waitFor(() => expect(markViewed).toHaveBeenCalledWith(1));
    // Opening an unread alert marks it read.
    expect(screen.queryByRole("button", { name: "Mark read" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Mark unread" })).toBeInTheDocument();
  });

  it("does not render the old redundant alert hero metrics", () => {
    mockHook.mockReturnValue({
      alerts: [sampleAlert],
      unreadCount: 1,
      loading: false,
      error: null,
      refresh: vi.fn(),
      dismiss: vi.fn(),
      markViewed: vi.fn(),
      markUnread: vi.fn(),
      markAllRead: vi.fn(),
    });

    renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);

    expect(screen.queryByText("Alert Center")).not.toBeInTheDocument();
    expect(screen.queryByText("Visible")).not.toBeInTheDocument();
    expect(screen.queryByText("Latest")).not.toBeInTheDocument();
    expect(screen.queryByText(/Showing \d+ alerts?:/)).not.toBeInTheDocument();
    expect(screen.getByText("Sources", { selector: ".card__title span" })).toBeInTheDocument();
  });

  it("does not mark already viewed alerts viewed again", () => {
    const markViewed = vi.fn();
    mockHook.mockReturnValue({
      alerts: [{ ...sampleAlert, viewedAt: Date.now() - 10_000 }],
      unreadCount: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
      dismiss: vi.fn(),
      markViewed,
      markUnread: vi.fn(),
      markAllRead: vi.fn(),
    });
    renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);
    fireEvent.click(screen.getByRole("button", { name: /Site down/i }));
    expect(markViewed).not.toHaveBeenCalled();
  });

  it("marks a read alert unread from the details dossier", async () => {
    const markUnread = vi.fn(() => Promise.resolve());
    mockHook.mockReturnValue({
      alerts: [{ ...sampleAlert, viewedAt: Date.now() - 10_000 }],
      unreadCount: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
      dismiss: vi.fn(),
      markViewed: vi.fn(),
      markUnread,
      markAllRead: vi.fn(),
    });

    renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);
    fireEvent.click(screen.getByRole("button", { name: /Site down/i }));
    fireEvent.click(screen.getByRole("button", { name: "Mark unread" }));

    await waitFor(() => expect(markUnread).toHaveBeenCalledWith(1));
    expect(screen.getByRole("button", { name: "Mark read" })).toBeInTheDocument();
  });

  it("opens the owning surface from alert details when a destination is present", () => {
    const onNavigate = vi.fn();
    mockHook.mockReturnValue({
      alerts: [
        {
          ...sampleAlert,
          source: "updates",
          detailJson: JSON.stringify({ destination: "updates" }),
        },
      ],
      unreadCount: 1,
      loading: false,
      error: null,
      refresh: vi.fn(),
      dismiss: vi.fn(),
      markViewed: vi.fn(),
      markUnread: vi.fn(),
      markAllRead: vi.fn(),
    });

    renderAlerts(
      <AlertsPage
        projectId={1}
        environmentScopeKey="https://example.com"
        onNavigate={onNavigate}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Site down/i }));
    fireEvent.click(screen.getByRole("button", { name: "Open Updates" }));

    expect(onNavigate).toHaveBeenCalledWith("updates");
  });

  it("shows a useful empty state when no alerts exist", () => {
    mockHook.mockReturnValue({
      alerts: [],
      unreadCount: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
      dismiss: vi.fn(),
      markViewed: vi.fn(),
      markUnread: vi.fn(),
      markAllRead: vi.fn(),
    });
    renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);
    expect(screen.queryByText(/No active alerts right now/)).not.toBeInTheDocument();
    expect(screen.getAllByText(/No alerts yet/).length).toBeGreaterThan(0);
    expect(screen.getByText("Sources", { selector: ".card__title span" })).toBeInTheDocument();
  });

  it("announces recorded events after checking sources, not just changed alerts", async () => {
    mockHook.mockReturnValue({
      alerts: [],
      unreadCount: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
      dismiss: vi.fn(),
      markViewed: vi.fn(),
      markUnread: vi.fn(),
      markAllRead: vi.fn(),
    });

    renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);
    fireEvent.click(screen.getByRole("button", { name: /Check sources/i }));

    // handleCheckSources sleeps 1500ms waiting for the queued polls to land.
    await waitFor(() => expect(publishEventsRecordedMock).toHaveBeenCalledWith(1), {
      timeout: 4000,
    });
    expect(refreshEventsMock).toHaveBeenCalledWith({ projectId: 1 });
    expect(publishAlertsChangedMock).toHaveBeenCalledWith(1);
  });
  describe("a connected deep link's arrival", () => {
    function alertsHook(overrides: Record<string, unknown> = {}) {
      return {
        alerts: [sampleAlert],
        unreadCount: 1,
        loading: false,
        error: null,
        refresh: vi.fn(),
        dismiss: vi.fn(),
        markViewed: vi.fn(() => Promise.resolve()),
        markUnread: vi.fn(),
        markAllRead: vi.fn(),
        ...overrides,
      };
    }

    it("opens the dossier for an alert id the timeline already holds", async () => {
      const markViewed = vi.fn(() => Promise.resolve());
      mockHook.mockReturnValue(alertsHook({ markViewed }));

      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: "outage:a", reason: null }}
        />,
      );

      await waitFor(() => expect(markViewed).toHaveBeenCalledWith(1));
      expect(screen.queryByText("Alert not available")).not.toBeInTheDocument();
    });

    it("reopens the same dossier when the same deep link arrives later", async () => {
      const markViewed = vi.fn(() => Promise.resolve());
      mockHook.mockReturnValue(alertsHook({ markViewed }));

      const view = renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: "outage:a", reason: null, arrival: 1 }}
        />,
      );
      await screen.findByRole("dialog");
      fireEvent.click(screen.getByRole("button", { name: "Close details panel" }));
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

      view.rerender(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: "outage:a", reason: null, arrival: 2 }}
        />,
      );

      await screen.findByRole("dialog");
      expect(markViewed).toHaveBeenCalledTimes(2);
    });

    it("lands an id the service answered for and does not hold on the not-found state", () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(connectedState({}));

      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: "alr_0123456789abcdef01234567", reason: null }}
        />,
      );

      expect(screen.getByText("Alert not available")).toBeInTheDocument();
      // The timeline is still underneath it: a notice, not a replacement.
      expect(screen.getAllByText("Site down").length).toBeGreaterThan(0);
    });

    it("never claims an alert aged out when this build has no connected service", () => {
      mockHook.mockReturnValue(alertsHook());

      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: "alr_0123456789abcdef01234567", reason: null }}
        />,
      );

      expect(screen.getByText("This build has no connected service")).toBeInTheDocument();
      expect(screen.getByText(/nothing was ever asked about it/)).toBeInTheDocument();
      expect(screen.queryByText("Alert not available")).not.toBeInTheDocument();
      expect(screen.queryByText(/age out of the connected service after 90 days/)).toBeNull();
    });

    it("waits for the rows before deciding an id is unknown", () => {
      mockHook.mockReturnValue(alertsHook({ alerts: [], loading: true }));

      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: "alr_0123456789abcdef01234567", reason: null }}
        />,
      );

      expect(screen.queryByText("Alert not available")).not.toBeInTheDocument();
      expect(screen.queryByText("This build has no connected service")).not.toBeInTheDocument();
    });

    it("names an unrecognized link for what it is, without quoting it", () => {
      mockHook.mockReturnValue(alertsHook());

      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ reason: "connected-link-unknown" }}
        />,
      );

      expect(screen.getByText("Link not recognized")).toBeInTheDocument();
    });

    it("renders nothing extra for an ordinary visit", () => {
      mockHook.mockReturnValue(alertsHook());

      renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);

      expect(screen.queryByText("Alert not available")).not.toBeInTheDocument();
      expect(screen.queryByText("Link not recognized")).not.toBeInTheDocument();
      expect(screen.queryByText("This build has no connected service")).not.toBeInTheDocument();
    });

    it("opens the connected dossier for an id only the service holds", async () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(connectedState({ alerts: [connectedAlert] }));

      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: connectedAlert.alertId, reason: null }}
        />,
      );

      await waitFor(() =>
        expect(
          screen.getByRole("dialog", { name: /Regression of a verified fix/ }),
        ).toBeInTheDocument(),
      );
      expect(screen.queryByText("Alert not available")).not.toBeInTheDocument();
    });

    it("says which project an alert for another site belongs to", () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(
        connectedState({
          elsewhere: [
            {
              alertId: connectedAlert.alertId,
              environmentUrl: "https://other.example.com",
              projectId: 9,
              projectName: "Other Project",
            },
          ],
        }),
      );

      const onNavigate = vi.fn();
      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          onNavigate={onNavigate}
          deepLinkTarget={{ alertId: connectedAlert.alertId, reason: null }}
        />,
      );

      expect(screen.getByText("That alert is on another site")).toBeInTheDocument();
      expect(screen.getByText(/Other Project/)).toBeInTheDocument();
      // A stated outcome with a way out of it, not a dead notice.
      fireEvent.click(screen.getByRole("button", { name: "Open Sites" }));
      expect(onNavigate).toHaveBeenCalledWith("sites");
    });

    it("never calls an alert gone when the connected read failed", () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(connectedState({}, { failed: true }));

      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: connectedAlert.alertId, reason: null }}
        />,
      );

      expect(screen.getByText("Could not check the connected service")).toBeInTheDocument();
      expect(screen.queryByText("Alert not available")).not.toBeInTheDocument();
    });

    it("waits for the connected read too before deciding an id is unknown", () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(connectedState({}, { loading: true }));

      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: connectedAlert.alertId, reason: null }}
        />,
      );

      expect(screen.queryByText("Alert not available")).not.toBeInTheDocument();
    });

    it("points an unconnected project at the one that is", () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(connectedState({ availability: "site_not_connected" }));

      renderAlerts(
        <AlertsPage
          projectId={1}
          environmentScopeKey="https://example.com"
          deepLinkTarget={{ alertId: connectedAlert.alertId, reason: null }}
        />,
      );

      expect(screen.getByText("This project is not connected")).toBeInTheDocument();
    });
  });

  describe("the connected timeline", () => {
    function alertsHook() {
      return {
        alerts: [],
        unreadCount: 0,
        loading: false,
        error: null,
        refresh: vi.fn(),
        dismiss: vi.fn(),
        markViewed: vi.fn(),
        markUnread: vi.fn(),
        markAllRead: vi.fn(),
      };
    }

    it("stays off the page entirely when no site is connected", () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(connectedState({ availability: "site_not_connected" }));

      renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);

      expect(screen.queryByText("From the connected service")).not.toBeInTheDocument();
    });

    it("distinguishes a service that raised nothing from one it could not reach", () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(connectedState({}));

      const { unmount } = renderAlerts(
        <AlertsPage projectId={1} environmentScopeKey="https://example.com" />,
      );
      expect(screen.getByText(/has raised nothing for this site/)).toBeInTheDocument();
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
      unmount();

      mockConnectedHook.mockReturnValue(connectedState({}, { failed: true }));
      renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);
      expect(screen.getByRole("alert")).toHaveTextContent(/could not be reached/);
      expect(screen.queryByText(/has raised nothing for this site/)).not.toBeInTheDocument();
    });

    it("says who a connected alert reached without opening it", () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(connectedState({ alerts: [connectedAlert] }));

      renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);

      const row = screen.getByRole("button", { name: /Regression of a verified fix/ });
      expect(row).toHaveTextContent("1 delivered");
      // Flat rows, and none of the local feed's read-state chrome: a connected
      // alert has no unread state anywhere in the protocol.
      expect(row.className).toContain("list-row");
      expect(row.className).not.toContain("alert-list-row-unread");
      expect(screen.queryByRole("button", { name: "Mark read" })).not.toBeInTheDocument();
    });

    it("states that an alert reached nobody rather than leaving it blank", () => {
      mockHook.mockReturnValue(alertsHook());
      mockConnectedHook.mockReturnValue(
        connectedState({ alerts: [{ ...connectedAlert, delivery: [] }] }),
      );

      renderAlerts(<AlertsPage projectId={1} environmentScopeKey="https://example.com" />);

      expect(
        screen.getByText(/Sent to nobody: this site has no alert destination/),
      ).toBeInTheDocument();
    });
  });
});
