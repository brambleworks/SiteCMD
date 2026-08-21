import { CloudAlert, Send } from "lucide-react";
import type {
  ConnectedAlert,
  ConnectedAlertFeed,
  ConnectedDestination,
} from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import type { NavTarget } from "@/components/layout/nav-page";
import { formatRelativeTime } from "@/lib/format";
import { useCurrentTime } from "@/lib/useCurrentTime";
import {
  alertSeverityLabel,
  alertSeverityToneClass,
  alertTitle,
  deliverySummary,
  shouldRenderConnected,
  unavailableNotice,
} from "./connected-alert-display";
import { ConnectedAlertDossier } from "./ConnectedAlertDossier";

interface ConnectedAlertTimelineProps {
  feed: ConnectedAlertFeed;
  loading: boolean;
  failed: boolean;
  // Used only to label delivery targets.
  destinations: ConnectedDestination[];
  selectedAlertId: string | null;
  onSelect: (alertId: string | null) => void;
  onNavigate?: (page: NavTarget) => void;
}

/** Renders connected alerts separately because they have no local read/dismiss lifecycle. */
export function ConnectedAlertTimeline({
  feed,
  loading,
  failed,
  destinations,
  selectedAlertId,
  onSelect,
  onNavigate,
}: ConnectedAlertTimelineProps) {
  const nowMs = useCurrentTime();

  // Avoid a transient placeholder for projects with no connected service.
  if (loading) return null;
  if (!failed && !shouldRenderConnected(feed.availability)) return null;

  const notice = unavailableNotice(feed.availability);
  const selected = feed.alerts.find((alert) => alert.alertId === selectedAlertId) ?? null;

  return (
    <>
      <section className="card connected-alert-panel">
        <div className="card__title-rule">
          <span className="card__title">
            <CloudAlert className="card__icon icon-md" aria-hidden="true" />
            <span>From the connected service</span>
          </span>
          {onNavigate ? (
            <Button variant="outline" size="sm" onClick={() => onNavigate("settings:connected")}>
              Alert Settings
            </Button>
          ) : null}
        </div>
        <p className="text-body-muted">
          Raised by the service watching this site on its own schedule, and already sent to the
          addresses and endpoints this account nominated. They keep no read or dismissed state here:
          this is the record of what the service did, and it holds the last 90 days.
        </p>

        {failed ? (
          <p className="connected-alert-error" role="alert">
            The connected service could not be reached, so this list is not what it has. It is not
            an empty timeline.
          </p>
        ) : null}

        {notice && !failed ? (
          <div className="connected-alert-notice" role="status">
            <p className="text-body text-strong">{notice.headline}</p>
            <p className="text-body-muted">{notice.detail}</p>
            {onNavigate ? (
              <Button variant="outline" size="sm" onClick={() => onNavigate("settings:connected")}>
                Open Connected Settings
              </Button>
            ) : null}
          </div>
        ) : null}

        {!failed && feed.availability === "ready" && feed.alerts.length === 0 ? (
          <p className="text-body-muted">
            The service has raised nothing for this site. It reports what it finds on its own
            schedule, so an empty list here means it has found nothing worth waking anyone for.
          </p>
        ) : null}

        {feed.alerts.length > 0 ? (
          <ul className="connected-alert-list">
            {feed.alerts.map((alert) => (
              <li key={alert.alertId}>
                <ConnectedAlertRow
                  alert={alert}
                  nowMs={nowMs}
                  selected={alert.alertId === selectedAlertId}
                  onSelect={() => onSelect(alert.alertId)}
                />
              </li>
            ))}
          </ul>
        ) : null}

        {feed.truncated ? (
          <p className="text-meta">
            Showing the most recent alerts across this account. Older ones are in the service and
            not reachable from here.
          </p>
        ) : null}
      </section>

      {selected ? (
        <ConnectedAlertDossier
          key={selected.alertId}
          alert={selected}
          destinations={destinations}
          onClose={() => onSelect(null)}
          onNavigate={onNavigate}
        />
      ) : null}
    </>
  );
}

function ConnectedAlertRow({
  alert,
  nowMs,
  selected,
  onSelect,
}: {
  alert: ConnectedAlert;
  nowMs: number;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <Button
      unstyled
      type="button"
      className={`list-row list-row--interactive connected-alert-row${
        selected ? " is-selected" : ""
      }`}
      onClick={onSelect}>
      <span className="connected-alert-row__body">
        <span className="row-tight connected-alert-row__tags">
          <span className={`eyebrow ${alertSeverityToneClass(alert.severity)}`}>
            {alertSeverityLabel(alert.severity)}
          </span>
          <span className="text-micro">{formatRelativeTime(alert.raisedAt, nowMs)}</span>
        </span>
        <span className="list-row__title connected-alert-row__title">{alertTitle(alert)}</span>
        <span className="connected-alert-row__delivery">
          <Send className="icon-xs" aria-hidden="true" />
          {deliverySummary(alert.delivery)}
        </span>
      </span>
    </Button>
  );
}
