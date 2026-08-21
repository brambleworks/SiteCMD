import type { SiteEvent } from "@/lib/types";
import { Button } from "@/components/ui/button";

interface WeekViewProps {
  cursor: Date;
  events: SiteEvent[];
  onDayClick: (date: Date) => void;
}

const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

const TYPE_ACCENT: Record<string, string> = {
  scan: "event-accent--scan",
  verification: "event-accent--verification",
  deploy: "event-accent--deploy",
  uptime: "event-accent--uptime",
  analytics: "event-accent--analytics",
  security: "event-accent--security",
  performance: "event-accent--performance",
  accessibility: "event-accent--accessibility",
  compliance: "event-accent--compliance",
};

function getWeekDays(cursor: Date): Date[] {
  const day = cursor.getDay();
  const start = new Date(cursor.getFullYear(), cursor.getMonth(), cursor.getDate() - day);
  return Array.from(
    { length: 7 },
    (_, i) => new Date(start.getFullYear(), start.getMonth(), start.getDate() + i),
  );
}

function dateKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function isToday(d: Date): boolean {
  const n = new Date();
  return (
    d.getFullYear() === n.getFullYear() &&
    d.getMonth() === n.getMonth() &&
    d.getDate() === n.getDate()
  );
}

function fmtTime(ms: number): string {
  try {
    return new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  } catch {
    return "";
  }
}

const MAX_VISIBLE = 5;

export function WeekView({ cursor, events, onDayClick }: WeekViewProps) {
  const days = getWeekDays(cursor);

  const byDate = new Map<string, SiteEvent[]>();
  for (const e of events) {
    const k = dateKey(new Date(e.occurredAtMs));
    (byDate.get(k) ?? (byDate.set(k, []), byDate.get(k)!)).push(e);
  }

  return (
    <div className="week-grid">
      {days.map((day) => {
        const k = dateKey(day);
        const dayEvts = (byDate.get(k) || []).sort((a, b) => a.occurredAtMs - b.occurredAtMs);
        const today = isToday(day);
        const visible = dayEvts.slice(0, MAX_VISIBLE);
        const overflow = dayEvts.length - MAX_VISIBLE;

        return (
          <Button
            unstyled
            type="button"
            key={k}
            onClick={() => onDayClick(day)}
            className={`week-col ${today ? "week-col--today" : ""}`}>
            <div className="week-col-head">
              <div className="section-label-mid">{DAYS[day.getDay()]}</div>
              <div className={`week-date ${today ? "week-date--today" : "week-date--plain"}`}>
                {day.getDate()}
              </div>
            </div>

            <div className="week-events">
              {visible.map((evt) => {
                const accent = TYPE_ACCENT[evt.eventType] || "event-accent--muted";

                return (
                  <div key={evt.id} className={`${accent} week-event`}>
                    <div className="min-w-0">
                      <div className="text-meta week-event-title">{evt.title}</div>
                      <div className="meta-num">{fmtTime(evt.occurredAtMs)}</div>
                    </div>
                  </div>
                );
              })}

              {overflow > 0 && <div className="week-overflow">+{overflow} more</div>}

              {dayEvts.length === 0 && (
                <div className="week-empty">
                  <span className="week-empty-mark">-</span>
                </div>
              )}
            </div>
          </Button>
        );
      })}
    </div>
  );
}
