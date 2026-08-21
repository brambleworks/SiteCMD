import { CheckCheck, Inbox } from "lucide-react";
import { Button } from "@/components/ui/button";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import type { AlertRow, AlertFilter } from "@/lib/types";
import { formatRelative, labelForSource, severityLabel, severityToneClass } from "./alert-display";
import { useCurrentTime } from "@/lib/useCurrentTime";

interface Props {
  alerts: AlertRow[];
  filter: AlertFilter;
  onFilterChange: (f: AlertFilter) => void;
  selectedId: number | null;
  onSelect: (a: AlertRow) => void;
  loading: boolean;
  unreadCount: number;
  onMarkAllRead: () => void;
}

const FILTERS: { id: AlertFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "unread", label: "Unread" },
  { id: "viewed", label: "Viewed" },
  { id: "dismissed", label: "Dismissed" },
];

export function AlertList({
  alerts,
  filter,
  onFilterChange,
  selectedId,
  onSelect,
  loading,
  unreadCount,
  onMarkAllRead,
}: Props) {
  const nowMs = useCurrentTime();
  const showEmpty = !loading && alerts.length === 0;

  return (
    <section className="card alert-panel">
      <div className="alert-list-header">
        <div className="alert-filter-tabs">
          {FILTERS.map((f) => (
            <Button
              unstyled
              key={f.id}
              type="button"
              className={`alert-filter-tab ${
                filter === f.id ? "alert-filter-tab--active" : "alert-filter-tab--inactive"
              }`}
              onClick={() => onFilterChange(f.id)}>
              {f.label}
            </Button>
          ))}
        </div>
        <Button
          variant="outline"
          size="sm"
          className="alert-toolbar-action"
          onClick={onMarkAllRead}
          disabled={unreadCount === 0}>
          <CheckCheck />
          Mark all read
        </Button>
      </div>

      {loading && alerts.length === 0 ? <AlertListSkeleton /> : null}

      {showEmpty ? <AlertEmptyState filter={filter} /> : null}

      {alerts.length > 0 ? (
        <ul className="alert-list-stack">
          {alerts.map((alert) => (
            <li key={alert.id}>
              <Button
                unstyled
                type="button"
                className={alertListRowClass(alert, selectedId === alert.id)}
                onClick={() => onSelect(alert)}>
                <div className="alert-row-content">
                  <div className="alert-row-body">
                    <div className="alert-row-tags">
                      <span className={`alert-severity-label ${severityToneClass(alert.severity)}`}>
                        {severityLabel(alert.severity)}
                      </span>
                      <span className="subtitle-xs">
                        {labelForSource(alert.source)} - {formatRelative(alert.occurredAt, nowMs)}
                      </span>
                    </div>
                    <p className="alert-item-title">{alert.title}</p>
                    <p className="alert-item-desc">{alert.description}</p>
                  </div>
                </div>
              </Button>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}

function alertListRowClass(alert: AlertRow, selected: boolean): string {
  const isUnread = alert.viewedAt === null && alert.dismissedAt === null;
  return [
    "alert-list-row",
    isUnread ? "alert-list-row-unread" : "",
    selected ? "alert-list-row-selected" : "",
  ]
    .filter(Boolean)
    .join(" ");
}

function AlertListSkeleton() {
  return (
    <LoadingRegion label="Alerts loading state" className="alert-skeleton-list">
      {[0, 1, 2].map((row) => (
        <div key={row} className="alert-skeleton-row">
          <div className="alert-skeleton-body">
            <Skeleton variant="dot" className="alert-skeleton-dot" />
            <div className="alert-skeleton-lines">
              <Skeleton variant="line" width="sm" />
              <Skeleton variant="line-lg" width="lg" />
              <Skeleton variant="line" width="full" />
              <Skeleton variant="line" width="wide" />
            </div>
          </div>
        </div>
      ))}
    </LoadingRegion>
  );
}

function AlertEmptyState({ filter }: { filter: AlertFilter }) {
  const title = filter === "all" ? "No alerts yet" : "No matching alerts";
  const detail =
    filter === "all"
      ? "Alerts appear when SiteCMD detects scan regressions or connected services report downtime, traffic anomalies, blocked threat traffic, or search impression drops."
      : "Nothing in this alert state right now. Switch to All to see older viewed alerts.";

  return (
    <div className="alert-empty">
      <Inbox className="alert-empty-icon" />
      <p className="alert-empty-title">{title}</p>
      <p className="alert-empty-desc">{detail}</p>
    </div>
  );
}
