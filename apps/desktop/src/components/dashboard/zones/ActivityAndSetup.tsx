import { Activity, Rocket } from "lucide-react";
import type { ActivityRow, SetupRow } from "@/lib/dashboard/types";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { CATEGORY_TAG, ICON_MAP } from "@/components/events/events-page-model";

interface RecentActivityCardProps {
  activity: ActivityRow[];
  activityLoading?: boolean;
  onOpenEmptyActivity: () => void;
  onOpenAllActivity: () => void;
}

function getDashboardActivityEventType(item: ActivityRow) {
  if (item.eventType) return item.eventType;
  const label = item.label.toLowerCase();
  if (label.includes("deploy") || label.includes("commit")) return "deploy";
  if (label.includes("update")) return "update";
  return "scan";
}

export function RecentActivityCard({
  activity,
  activityLoading = false,
  onOpenEmptyActivity,
  onOpenAllActivity,
}: RecentActivityCardProps) {
  return (
    <div className="card card-column">
      <div className="card__title-rule">
        <span className="card__title">
          <Activity className="card__icon icon-md" aria-hidden="true" />
          <span>Recent Activity</span>
        </span>
      </div>

      {activityLoading ? (
        <div className="dashboard-activity-skeleton-list">
          {Array.from({ length: 5 }, (_, index) => (
            <div key={`activity-skeleton-${index}`} className="activity-skeleton-row">
              <div className="activity-skeleton-rail">
                <span className="activity-skeleton-line" />
                <span className="timeline-dot" />
              </div>
              <div className="activity-skeleton-content">
                <div className="activity-skeleton-head">
                  <Skeleton className="activity-skeleton-label" />
                  <Skeleton className="activity-skeleton-time" />
                </div>
                <Skeleton className="activity-skeleton-sub" />
              </div>
            </div>
          ))}
        </div>
      ) : activity.length === 0 ? (
        <div className="activity-empty-body">
          <p className="text-body-muted activity-empty-text">No recent activity yet</p>
          <Button
            variant="ghost"
            size="sm"
            type="button"
            onClick={onOpenEmptyActivity}
            className="text-primary">
            Run your first scan
          </Button>
        </div>
      ) : (
        <div className="activity-list-wrap">
          <div className="activity-list">
            {activity.map((item) => {
              const eventType = getDashboardActivityEventType(item);
              const meta = ICON_MAP[eventType] ?? ICON_MAP.scan;
              const Icon = meta.icon;
              const tag = CATEGORY_TAG[eventType];
              const showTag = tag && tag.label.toLowerCase() !== item.label.toLowerCase();
              const sourceLabel =
                item.source && item.source.toLowerCase() !== item.label.toLowerCase()
                  ? item.source
                  : null;
              return (
                <Button
                  unstyled
                  key={item.id}
                  type="button"
                  onClick={item.onOpen}
                  className="activity-row activity-row--button">
                  <div className="activity-row-body">
                    <div className={`activity-icon ${meta.cls}`}>
                      <Icon className="icon-18" />
                    </div>
                    <div className="flex-fill">
                      <h3 className="text-truncate text-body events-feed-title">{item.label}</h3>
                      <div className="row-loose events-feed-meta">
                        {sourceLabel ? (
                          <span className="text-micro events-feed-source">{sourceLabel}</span>
                        ) : null}
                        <span className="activity-meta-tag activity-value-tag text-truncate">
                          {item.value}
                        </span>
                        {showTag ? (
                          <span className={`text-micro events-feed-tag ${tag.cls}`}>
                            {tag.label}
                          </span>
                        ) : null}
                      </div>
                    </div>
                    <div className="activity-time-cell">
                      <span className="text-meta">{item.timeAgo}</span>
                    </div>
                  </div>
                </Button>
              );
            })}
          </div>
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={onOpenAllActivity}
            className="btn--block">
            View All Activity
          </Button>
        </div>
      )}
    </div>
  );
}

export function SetupCard({ rows }: { rows: SetupRow[] }) {
  // Once every setup task is done there is nothing left to prompt; the card
  // disappears and Recent Activity takes the full row.
  if (rows.length === 0) return null;

  return (
    <div className="card card-column">
      <div className="card__title-rule">
        <span className="card__title">
          <Rocket className="card__icon icon-md" aria-hidden="true" />
          <span>Finish Setup</span>
        </span>
      </div>

      <p className="text-body-muted setup-desc">
        One-time steps that give SiteCMD more to work with.
      </p>

      <div className="stack-tight">
        {rows.map((row) => (
          <Button
            unstyled
            key={row.id}
            type="button"
            onClick={row.onOpen}
            className="list-row list-row--dashboard">
            <span className="list-row__label">{row.label}</span>
            <span className="flex-fill text-body text-foreground">{row.value}</span>
            <span className="list-row__chevron">&#8250;</span>
          </Button>
        ))}
      </div>
    </div>
  );
}
