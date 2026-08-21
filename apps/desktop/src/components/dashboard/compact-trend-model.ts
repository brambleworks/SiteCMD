import { parseJsonRecord } from "@/lib/json-record";
import type { DashboardCodeTrendPoint } from "@/lib/project-summary-types";
import type { PackageUpdate, SiteEvent } from "@/lib/types";
import { buildUpdateQueueSummary } from "@/lib/update-summary";
import type { ScoreTrendPoint } from "./DashboardTrendComponents";

const DEFAULT_TREND_WINDOW = 10;

export type CompactTrendTone = "improving" | "worsening" | "stable" | "empty";

export interface CompactTrendModel {
  key: string;
  label: string;
  currentValue: string;
  detail: string;
  deltaLabel: string;
  tone: CompactTrendTone;
  series: number[];
}

interface BuildIssuesTrendModelArgs {
  webTrend: ScoreTrendPoint[];
  codeTrend: DashboardCodeTrendPoint[];
  currentIssueCount: number;
  criticalCount: number;
  windowSize?: number;
}

interface BuildUpdatesTrendModelArgs {
  events?: SiteEvent[];
  updates?: PackageUpdate[];
  windowSize?: number;
}

type TrendEvent = {
  timestamp: string;
  source: "web" | "code";
  value: number;
};

export function buildIssuesTrendModel({
  webTrend,
  codeTrend,
  currentIssueCount,
  criticalCount,
  windowSize = DEFAULT_TREND_WINDOW,
}: BuildIssuesTrendModelArgs): CompactTrendModel {
  const events: TrendEvent[] = [
    ...webTrend.map((point) => ({
      source: "web" as const,
      timestamp: point.timestamp,
      value: normalizeCount(point.issues),
    })),
    ...codeTrend
      .filter(
        (point): point is DashboardCodeTrendPoint & { issueCount: number } =>
          typeof point.issueCount === "number" && Number.isFinite(point.issueCount),
      )
      .map((point) => ({
        source: "code" as const,
        timestamp: point.timestamp,
        value: normalizeCount(point.issueCount),
      })),
  ].sort(sortByTimestamp);

  let latestWebCount: number | null = null;
  let latestCodeCount: number | null = null;
  const combinedSeries: number[] = [];

  for (const event of events) {
    if (event.source === "web") latestWebCount = event.value;
    if (event.source === "code") latestCodeCount = event.value;
    combinedSeries.push((latestWebCount ?? 0) + (latestCodeCount ?? 0));
  }

  const series = withCurrentPoint(
    takeLast(combinedSeries, windowSize),
    currentIssueCount,
    windowSize,
  );
  const delta = getLatestDelta(series);

  return {
    key: "issues-trend",
    label: "Issues trend",
    currentValue: formatCount(currentIssueCount),
    detail:
      criticalCount > 0
        ? `${criticalCount} critical issue${criticalCount === 1 ? "" : "s"}`
        : "No critical issues",
    deltaLabel: formatCountDelta(delta),
    tone: getLowerIsBetterTone(delta, series),
    series,
  };
}

export function buildUpdatesTrendModel({
  events = [],
  updates = [],
  windowSize = DEFAULT_TREND_WINDOW,
}: BuildUpdatesTrendModelArgs): CompactTrendModel {
  const updateSummary = buildUpdateQueueSummary(updates);
  const currentCount = updateSummary.total;
  const historySeries = events
    .filter((event) => event.eventType === "update")
    .sort(sortSiteEventsByTimestamp)
    .map(readPendingUpdatesFromEvent)
    .filter((value): value is number => value !== null);
  const series = withCurrentPoint(takeLast(historySeries, windowSize), currentCount, windowSize);
  const delta = getLatestDelta(series);

  return {
    key: "updates-trend",
    label: "Updates trend",
    currentValue: formatCount(currentCount),
    detail:
      updateSummary.security > 0
        ? `${updateSummary.security} security · ${updateSummary.major} major`
        : `${updateSummary.major} major · ${updateSummary.minor} minor`,
    deltaLabel: formatCountDelta(delta),
    tone: getLowerIsBetterTone(delta, series),
    series,
  };
}

function getLatestDelta(series: number[]): number | null {
  if (series.length < 2) return null;
  return series[series.length - 1] - series[series.length - 2];
}

function getLowerIsBetterTone(delta: number | null, series: number[]): CompactTrendTone {
  if (series.length < 2 || delta === null) return "empty";
  if (delta < 0) return "improving";
  if (delta > 0) return "worsening";
  return "stable";
}

function formatCountDelta(delta: number | null): string {
  if (delta === null) return "No trend yet";
  if (delta === 0) return "No change since last checked";
  const prefix = delta > 0 ? "+" : "";
  return `${prefix}${delta} since last checked`;
}

function withCurrentPoint(series: number[], currentValue: number, windowSize: number): number[] {
  const normalizedCurrent = normalizeCount(currentValue);
  if (series.length === 0) return [normalizedCurrent];
  if (series.length >= 2 && series[series.length - 1] === normalizedCurrent) return series;
  return [...series.slice(-(windowSize - 1)), normalizedCurrent];
}

function takeLast<T>(items: T[], count: number): T[] {
  return items.slice(Math.max(0, items.length - count));
}

function readPendingUpdatesFromEvent(event: SiteEvent): number | null {
  const detail = event.parsedDetail ?? parseEventDetail(event.detail);
  return (
    readNumber(detail, "remaining_updates") ??
    readNumber(detail, "pending_updates_after") ??
    readNumber(detail, "pending_updates")
  );
}

function readNumber(detail: Record<string, unknown> | null | undefined, key: string) {
  const value = detail?.[key];
  return typeof value === "number" && Number.isFinite(value) ? normalizeCount(value) : null;
}

function parseEventDetail(detail: string | null): Record<string, unknown> | null {
  if (!detail) return null;
  return parseJsonRecord(detail);
}

function sortByTimestamp(a: TrendEvent, b: TrendEvent): number {
  return timestampMs(a.timestamp) - timestampMs(b.timestamp);
}

function sortSiteEventsByTimestamp(a: SiteEvent, b: SiteEvent): number {
  return a.occurredAtMs - b.occurredAtMs;
}

function timestampMs(timestamp: string): number {
  const ms = new Date(timestamp).getTime();
  return Number.isFinite(ms) ? ms : 0;
}

function normalizeCount(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.round(value));
}

function formatCount(value: number): string {
  return normalizeCount(value).toLocaleString();
}
