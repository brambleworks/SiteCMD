import type { SiteEvent } from "@/lib/types";
import { Button } from "@/components/ui/button";

interface MonthViewProps {
  cursor: Date;
  events: SiteEvent[];
  onDayClick: (date: Date) => void;
}

const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

// Ordered so most important types render first
const TYPE_ORDER = [
  "verification",
  "security",
  "uptime",
  "scan",
  "deploy",
  "analytics",
  "performance",
  "accessibility",
  "compliance",
];
const TYPE_COLOR: Record<string, string> = {
  scan: "event-bar--scan",
  verification: "event-bar--verification",
  deploy: "event-bar--deploy",
  uptime: "event-bar--uptime",
  analytics: "event-bar--analytics",
  security: "event-bar--security",
  performance: "event-bar--performance",
  accessibility: "event-bar--accessibility",
  compliance: "event-bar--compliance",
};

function getMonthGrid(cursor: Date): Date[] {
  const year = cursor.getFullYear();
  const month = cursor.getMonth();
  const first = new Date(year, month, 1);
  const offset = first.getDay();
  return Array.from({ length: 42 }, (_, i) => new Date(year, month, 1 - offset + i));
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

export function MonthView({ cursor, events, onDayClick }: MonthViewProps) {
  const byDate = new Map<string, SiteEvent[]>();
  for (const e of events) {
    const k = dateKey(new Date(e.occurredAtMs));
    (byDate.get(k) ?? (byDate.set(k, []), byDate.get(k)!)).push(e);
  }

  const days = getMonthGrid(cursor);
  const month = cursor.getMonth();
  const lastUsedRow = Math.ceil(
    days.findLastIndex((d) => d.getMonth() === month || byDate.has(dateKey(d))) / 7,
  );
  const rows = Math.max(5, lastUsedRow + 1);
  const visibleDays = days.slice(0, rows * 7);

  return (
    <div className="month-grid-wrap">
      <div className="month-weekday-row">
        {DAYS.map((d) => (
          <div key={d} className="month-weekday">
            {d}
          </div>
        ))}
      </div>

      <div className="month-grid">
        {visibleDays.map((day, i) => {
          const k = dateKey(day);
          const dayEvts = byDate.get(k) || [];
          const inMonth = day.getMonth() === month;
          const today = isToday(day);

          const typeCounts = new Map<string, number>();
          for (const e of dayEvts) {
            typeCounts.set(e.eventType, (typeCounts.get(e.eventType) || 0) + 1);
          }

          // Build bars: ordered by TYPE_ORDER, only types present
          const bars = TYPE_ORDER.filter((t) => typeCounts.has(t)).map((t) => ({
            type: t,
            count: typeCounts.get(t)!,
            color: TYPE_COLOR[t] || "event-bar--muted",
          }));

          return (
            <Button
              unstyled
              type="button"
              key={i}
              onClick={() => onDayClick(day)}
              className={`month-cell ${!inMonth ? "month-cell--outside" : ""} ${
                today ? "month-cell--today" : ""
              }`}>
              <div className="month-daynum">
                {today ? (
                  <span className="calendar-day-current">{day.getDate()}</span>
                ) : (
                  <span
                    className={`month-daynum-text ${
                      inMonth ? "text-foreground" : "month-daynum-text--outside"
                    }`}>
                    {day.getDate()}
                  </span>
                )}
              </div>

              {bars.length > 0 && (
                <div className="month-bars">
                  {bars.map(({ type, count, color }) => {
                    return (
                      <div
                        key={type}
                        className={`event-bar ${color} ${eventBarHeightClass(count)}`}
                        title={`${count} ${type} event${count !== 1 ? "s" : ""}`}
                      />
                    );
                  })}
                </div>
              )}
            </Button>
          );
        })}
      </div>
    </div>
  );
}

function eventBarHeightClass(count: number): string {
  if (count <= 1) return "event-bar--h1";
  if (count === 2) return "event-bar--h2";
  if (count === 3) return "event-bar--h3";
  return "event-bar--h4";
}
