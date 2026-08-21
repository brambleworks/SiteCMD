import { useMemo } from "react";
import { ChevronRight } from "lucide-react";
import type { AppTarget } from "@/lib/app-targets";
import { normalizeActivityFeedEvents, type ActivityFeedEvent } from "@/lib/activity-feed";
import type { SiteEvent } from "@/lib/types";
import { buildEventScanTarget, humanizeEventDetail } from "./event-presentation";
import { CATEGORY_TAG, ICON_MAP, buildFeedGroups, getRelativeTime } from "./events-page-model";
import { Button } from "@/components/ui/button";
import { useCurrentTime } from "@/lib/useCurrentTime";

interface EventsFeedViewProps {
  events: SiteEvent[];
  onOpenTarget?: (target: AppTarget) => void;
}

export function EventsFeedView({ events, onOpenTarget }: EventsFeedViewProps) {
  const nowMs = useCurrentTime();
  const normalizedEvents = useMemo(() => normalizeActivityFeedEvents(events), [events]);
  const groups = useMemo(() => buildFeedGroups(normalizedEvents), [normalizedEvents]);

  return (
    <div className="events-feed-list">
      {groups.map((group) => (
        <section key={group.label}>
          <div className="events-feed-group-head">
            <span className="eyebrow--alt text-muted-foreground no-shrink">{group.label}</span>
            <div className="events-feed-rule" />
          </div>
          <div className="activity-group">
            {group.events.map((evt) => (
              <ActivityRow key={evt.id} event={evt} nowMs={nowMs} onOpenTarget={onOpenTarget} />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

function ActivityRow({
  event,
  nowMs,
  onOpenTarget,
}: {
  event: ActivityFeedEvent;
  nowMs: number;
  onOpenTarget?: (target: AppTarget) => void;
}) {
  const meta = ICON_MAP[event.eventType] ?? ICON_MAP.scan;
  const Icon = meta.icon;
  const tag = CATEGORY_TAG[event.eventType];
  const relTime = getRelativeTime(new Date(event.occurredAtMs), nowMs);
  const parsedDetail = event.parsedDetail;

  const eventTarget = buildEventScanTarget(event.projectId, parsedDetail);
  const isClickable = Boolean(eventTarget && onOpenTarget);

  const row = (
    <div className="events-feed-row">
      <div className={`activity-icon ${meta.cls}`}>
        <Icon className="icon-18" />
      </div>
      <div className="flex-fill">
        <h3 className="text-body text-truncate events-feed-title">{event.title}</h3>
        <div className="row-loose events-feed-meta">
          <span className="text-micro events-feed-source">{event.source}</span>
          {parsedDetail &&
            (() => {
              const pills = humanizeEventDetail(parsedDetail, event.eventType);
              const first = pills[0];
              if (!first) return null;
              return <span className="activity-meta-tag">{first}</span>;
            })()}
          {tag && <span className={`text-micro events-feed-tag ${tag.cls}`}>{tag.label}</span>}
        </div>
      </div>
      <div className="row-loose no-shrink">
        <span className="text-meta">{relTime}</span>
        {isClickable && (
          <ChevronRight
            className={`icon-md ${tag?.cls ?? "text-muted-foreground"}`}
            aria-hidden="true"
          />
        )}
      </div>
    </div>
  );

  if (isClickable) {
    return (
      <Button
        unstyled
        onClick={() => onOpenTarget!(eventTarget!)}
        className="activity-row activity-row--button">
        {row}
      </Button>
    );
  }

  return <div className="activity-row">{row}</div>;
}
