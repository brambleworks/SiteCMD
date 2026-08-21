import {
  BarChart3 as BarIcon,
  Eye,
  GitBranch,
  RefreshCw,
  Search,
  Shield as ShieldIcon,
  Wifi as WifiIcon,
  type LucideIcon,
} from "lucide-react";
import type { ActivityFeedEvent } from "@/lib/activity-feed";
import { formatRelativeTime } from "@/lib/format";
import { CATEGORY_LABELS } from "@/lib/tokens";
import type { SiteEvent } from "@/lib/types";

export type CalendarView = "feed" | "month" | "week" | "day";

export const EVENT_VIEW_OPTIONS: CalendarView[] = ["feed", "day", "week", "month"];

const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

const DAYS_FULL = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** Logical filter groups - each maps to one or more backend EventType values. */
export const EVENT_FILTER_GROUPS = [
  {
    key: "scans",
    label: "Scans & verification",
    types: ["scan", "verification", "security", "accessibility", "performance", "compliance"],
  },
  { key: "changes", label: "Changes", types: ["deploy", "update", "launch"] },
  { key: "monitoring", label: "Monitoring", types: ["uptime", "analytics", "search"] },
] as const;

export type FilterGroupKey = (typeof EVENT_FILTER_GROUPS)[number]["key"];

// `cls` is a single semantic class (events.css) that sets the badge background
// tint and the icon color; the Lucide icon inherits the color via currentColor.
export const ICON_MAP: Record<string, { icon: LucideIcon; cls: string }> = {
  scan: { icon: Search, cls: "event-icon--scan" },
  verification: { icon: RefreshCw, cls: "event-icon--verification" },
  search: { icon: Search, cls: "event-icon--search" },
  update: { icon: RefreshCw, cls: "event-icon--update" },
  launch: { icon: ShieldIcon, cls: "event-icon--launch" },
  deploy: { icon: GitBranch, cls: "event-icon--deploy" },
  uptime: { icon: WifiIcon, cls: "event-icon--uptime" },
  analytics: { icon: BarIcon, cls: "event-icon--analytics" },
  security: { icon: ShieldIcon, cls: "event-icon--security" },
  accessibility: { icon: Eye, cls: "event-icon--accessibility" },
};

/** Category label + color for the trailing tag on each row. */
export const CATEGORY_TAG: Record<string, { label: string; cls: string }> = {
  scan: { label: "Scan", cls: "event-tag--scan" },
  verification: { label: "Verification", cls: "event-tag--verification" },
  search: { label: "Search", cls: "event-tag--search" },
  update: { label: "Update", cls: "event-tag--update" },
  launch: { label: "Launch", cls: "event-tag--launch" },
  deploy: { label: "Deploy", cls: "event-tag--deploy" },
  uptime: { label: "Uptime", cls: "event-tag--uptime" },
  analytics: { label: "Analytics", cls: "event-tag--analytics" },
  security: { label: CATEGORY_LABELS.security, cls: "event-tag--security" },
  accessibility: { label: CATEGORY_LABELS.accessibility, cls: "event-tag--accessibility" },
  compliance: { label: CATEGORY_LABELS.compliance, cls: "event-tag--compliance" },
  performance: { label: CATEGORY_LABELS.performance, cls: "event-tag--performance" },
};

type FeedGroup = { label: string; events: ActivityFeedEvent[] };

export function startOfWeek(d: Date): Date {
  const day = d.getDay();
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() - day);
}

export function endOfWeek(d: Date): Date {
  const s = startOfWeek(d);
  return new Date(s.getFullYear(), s.getMonth(), s.getDate() + 6);
}

export function formatDateRange(view: CalendarView, cursor: Date): string {
  if (view === "feed") return "";
  if (view === "month") return `${MONTHS[cursor.getMonth()]} ${cursor.getFullYear()}`;
  if (view === "week") {
    const s = startOfWeek(cursor);
    const e = endOfWeek(cursor);
    const sM = MONTHS[s.getMonth()].slice(0, 3);
    const eM = MONTHS[e.getMonth()].slice(0, 3);
    return s.getMonth() === e.getMonth()
      ? `${sM} ${s.getDate()} – ${e.getDate()}, ${s.getFullYear()}`
      : `${sM} ${s.getDate()} – ${eM} ${e.getDate()}, ${e.getFullYear()}`;
  }
  return `${DAYS_FULL[cursor.getDay()]}, ${MONTHS[cursor.getMonth()]} ${cursor.getDate()}, ${cursor.getFullYear()}`;
}

export function dateRangeForView(view: CalendarView, cursor: Date): { start: string; end: string } {
  const fmt = (d: Date) => d.toISOString().split("T")[0];
  if (view === "feed") {
    const end = new Date(cursor);
    const start = new Date(cursor.getFullYear(), cursor.getMonth(), cursor.getDate() - 30);
    return { start: fmt(start) + "T00:00:00Z", end: fmt(end) + "T23:59:59Z" };
  }
  if (view === "month") {
    const start = new Date(cursor.getFullYear(), cursor.getMonth(), 1);
    const dayOffset = start.getDay();
    const gridStart = new Date(start.getFullYear(), start.getMonth(), start.getDate() - dayOffset);
    const gridEnd = new Date(
      gridStart.getFullYear(),
      gridStart.getMonth(),
      gridStart.getDate() + 41,
    );
    return { start: fmt(gridStart) + "T00:00:00Z", end: fmt(gridEnd) + "T23:59:59Z" };
  }
  if (view === "week") {
    const s = startOfWeek(cursor);
    const e = endOfWeek(cursor);
    return { start: fmt(s) + "T00:00:00Z", end: fmt(e) + "T23:59:59Z" };
  }
  return { start: fmt(cursor) + "T00:00:00Z", end: fmt(cursor) + "T23:59:59Z" };
}

export function navigate(view: CalendarView, cursor: Date, direction: -1 | 1): Date {
  if (view === "feed")
    return new Date(cursor.getFullYear(), cursor.getMonth(), cursor.getDate() + 30 * direction);
  if (view === "month") return new Date(cursor.getFullYear(), cursor.getMonth() + direction, 1);
  if (view === "week")
    return new Date(cursor.getFullYear(), cursor.getMonth(), cursor.getDate() + 7 * direction);
  return new Date(cursor.getFullYear(), cursor.getMonth(), cursor.getDate() + direction);
}

export function escapeCsvCell(value: string): string {
  const safeValue = /^[=+\-@\t\r]/.test(value) ? `'${value}` : value;
  return `"${safeValue.replace(/"/g, '""')}"`;
}

export function buildEventsCsvContent(events: SiteEvent[]): string {
  const headers = ["date,time,event_type,severity,title,summary,source"];
  const rows = events.map((event) => {
    const date = new Date(event.occurredAtMs);
    return [
      date.toLocaleDateString(),
      date.toLocaleTimeString(),
      escapeCsvCell(event.eventType),
      escapeCsvCell(event.severity),
      escapeCsvCell(event.title),
      escapeCsvCell(event.summary),
      escapeCsvCell(event.source),
    ].join(",");
  });
  return [...headers, ...rows].join("\n");
}

export function buildFeedGroups(events: ActivityFeedEvent[]): FeedGroup[] {
  const sorted = [...events].sort((a, b) => b.occurredAtMs - a.occurredAtMs);
  const groups: FeedGroup[] = [];
  const today = new Date();
  const yesterday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1);
  let currentLabel = "";
  let currentGroup: FeedGroup | null = null;

  for (const evt of sorted) {
    const d = new Date(evt.occurredAtMs);
    let label: string;
    if (d.toDateString() === today.toDateString()) {
      label = `Today - ${d.toLocaleDateString("en-US", { month: "short", day: "numeric" })}`;
    } else if (d.toDateString() === yesterday.toDateString()) {
      label = `Yesterday - ${d.toLocaleDateString("en-US", { month: "short", day: "numeric" })}`;
    } else {
      label = d.toLocaleDateString("en-US", { weekday: "short", month: "short", day: "numeric" });
    }
    if (label !== currentLabel) {
      currentGroup = { label, events: [] };
      groups.push(currentGroup);
      currentLabel = label;
    }
    currentGroup!.events.push(evt);
  }
  return groups;
}

export function getRelativeTime(date: Date, nowMs: number): string {
  return formatRelativeTime(date, nowMs);
}
