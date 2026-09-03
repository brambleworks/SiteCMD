import type { EventSeverity, SiteEvent } from "@/lib/types";
import { SCAN_LABELS } from "@/lib/scan-labels";
import { parseJsonRecord } from "./json-record";

export interface UpdateBreakdown {
  critical: number;
  major: number;
  minor: number;
  patch: number;
}

export interface ActivityFeedEvent {
  id: string;
  projectId: number;
  eventType: SiteEvent["eventType"];
  severity: EventSeverity;
  /** Epoch milliseconds (UTC) when the event occurred. */
  occurredAtMs: number;
  title: string;
  summary: string;
  source: SiteEvent["source"];
  sourceId: string | null;
  detail: string | null;
  parsedDetail: Record<string, unknown> | null;
}

export const FULL_SCAN_MERGE_WINDOW_MS = 5 * 60 * 1000;

export function toEpoch(value: string | null | undefined): number {
  if (!value) return Number.NEGATIVE_INFINITY;
  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? Number.NEGATIVE_INFINITY : parsed;
}

function parseEventDetail(detail: string | null): Record<string, unknown> | null {
  if (!detail) return null;
  return parseJsonRecord(detail);
}

function withParsedEventDetail(event: SiteEvent): SiteEvent {
  return {
    ...event,
    parsedDetail: event.parsedDetail ?? parseEventDetail(event.detail),
  };
}

function isCodeScanEvent(event: Pick<SiteEvent, "title" | "parsedDetail">): boolean {
  const scanType = event.parsedDetail?.scan_type;
  if (scanType === "code") return true;
  return event.title.toLowerCase().startsWith("code scan:");
}

function isWebScanEvent(event: Pick<SiteEvent, "title" | "parsedDetail">): boolean {
  const scanType = event.parsedDetail?.scan_type;
  if (scanType === "health") return true;
  const normalizedTitle = event.title.toLowerCase();
  return (
    normalizedTitle.startsWith("web scan:") || normalizedTitle.startsWith("multi-page web scan:")
  );
}

function extractIssueCount(event: Pick<SiteEvent, "parsedDetail">): number | null {
  const rawCount = parseFiniteNumber(event.parsedDetail?.issues_total);
  if (rawCount == null) return null;
  return Math.max(0, Math.round(rawCount));
}

function issueCountLabel(count: number, singular: string, plural: string): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

function parseVersionParts(value: unknown): [number, number, number] | null {
  if (typeof value !== "string" || !value.trim()) return null;
  const match = value.match(/(\d+)(?:\.(\d+))?(?:\.(\d+))?/);
  if (!match) return null;
  return [
    Number.parseInt(match[1] ?? "0", 10),
    Number.parseInt(match[2] ?? "0", 10),
    Number.parseInt(match[3] ?? "0", 10),
  ];
}

function classifyVersionBump(
  fromVersion: unknown,
  toVersion: unknown,
): "major" | "minor" | "patch" {
  const from = parseVersionParts(fromVersion);
  const to = parseVersionParts(toVersion);
  if (!from || !to) return "patch";
  if (to[0] !== from[0]) return "major";
  if (to[1] !== from[1]) return "minor";
  return "patch";
}

export function buildUpdateBreakdownFromEventDetail(
  detail: Record<string, unknown> | null,
): UpdateBreakdown | null {
  if (!detail) return null;

  const explicitCounts = [
    detail.critical_updates,
    detail.major_updates,
    detail.minor_updates,
    detail.patch_updates,
  ];
  if (explicitCounts.some((value) => parseFiniteNumber(value) != null)) {
    return {
      critical: parseCount(detail.critical_updates),
      major: parseCount(detail.major_updates),
      minor: parseCount(detail.minor_updates),
      patch: parseCount(detail.patch_updates),
    };
  }

  if (!Array.isArray(detail.applied_updates)) return null;

  const breakdown: UpdateBreakdown = {
    critical: 0,
    major: 0,
    minor: 0,
    patch: 0,
  };

  for (const update of detail.applied_updates) {
    if (!update || typeof update !== "object") continue;
    const typedUpdate = update as Record<string, unknown>;
    const bucket = classifyVersionBump(typedUpdate.from_version, typedUpdate.to_version);
    breakdown[bucket] += 1;
  }

  return breakdown;
}

function parseFiniteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function parseCount(value: unknown): number {
  const parsed = parseFiniteNumber(value);
  if (parsed == null) return 0;
  return Math.max(0, Math.round(parsed));
}

export function formatUpdateBreakdown(breakdown: UpdateBreakdown): string {
  return `${breakdown.critical} Critical, ${breakdown.major} Major, ${breakdown.minor} Minor, ${breakdown.patch} Patch`;
}

function mergeSeverity(a: EventSeverity, b: EventSeverity): EventSeverity {
  if (a === "critical" || b === "critical") return "critical";
  if (a === "warning" || b === "warning") return "warning";
  return "info";
}

function buildNormalizedEvent(event: SiteEvent): ActivityFeedEvent {
  const parsedDetail = event.parsedDetail ?? null;
  const updateBreakdown =
    event.eventType === "update" ? buildUpdateBreakdownFromEventDetail(parsedDetail) : null;

  return {
    id: `activity-event-${event.id}`,
    projectId: event.projectId,
    eventType: event.eventType,
    severity: event.severity,
    occurredAtMs: event.occurredAtMs,
    title: event.title,
    summary: updateBreakdown
      ? formatUpdateBreakdown(updateBreakdown)
      : event.summary || event.title,
    source: event.source,
    sourceId: event.sourceId,
    detail: event.detail,
    parsedDetail,
  };
}

function buildFullScanEvent(webScan: SiteEvent, codeScan: SiteEvent): ActivityFeedEvent {
  const newerEvent = webScan.occurredAtMs >= codeScan.occurredAtMs ? webScan : codeScan;
  const webIssues = extractIssueCount(webScan);
  const codeIssues = extractIssueCount(codeScan);
  const summary = [
    webIssues !== null ? issueCountLabel(webIssues, "web issue", "web issues") : null,
    codeIssues !== null ? issueCountLabel(codeIssues, "code issue", "code issues") : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return {
    id: `activity-event-full-scan-${newerEvent.id}-${webScan.id === newerEvent.id ? codeScan.id : webScan.id}`,
    projectId: newerEvent.projectId,
    eventType: "scan",
    severity: mergeSeverity(webScan.severity, codeScan.severity),
    occurredAtMs: newerEvent.occurredAtMs,
    title: SCAN_LABELS.full,
    summary: summary || "Live site and code checked",
    source: newerEvent.source,
    sourceId: newerEvent.sourceId,
    detail: newerEvent.detail,
    parsedDetail: newerEvent.parsedDetail ?? null,
  };
}

export function normalizeActivityFeedEvents(
  events: SiteEvent[],
  options?: { limit?: number },
): ActivityFeedEvent[] {
  const limit = options?.limit ?? Number.POSITIVE_INFINITY;
  const sorted = [...events]
    .map(withParsedEventDetail)
    .sort((a, b) => b.occurredAtMs - a.occurredAtMs || b.id - a.id);

  const items: ActivityFeedEvent[] = [];
  const consumedIds = new Set<number>();

  for (let index = 0; index < sorted.length && items.length < limit; index += 1) {
    const event = sorted[index];
    if (consumedIds.has(event.id)) continue;

    const isScanPairCandidate = isWebScanEvent(event) || isCodeScanEvent(event);
    if (isScanPairCandidate) {
      const matchingIndex = sorted.findIndex((candidate, candidateIndex) => {
        if (candidateIndex <= index) return false;
        if (consumedIds.has(candidate.id)) return false;
        const withinWindow =
          Math.abs(event.occurredAtMs - candidate.occurredAtMs) <= FULL_SCAN_MERGE_WINDOW_MS;
        if (!withinWindow) return false;
        return (
          (isWebScanEvent(event) && isCodeScanEvent(candidate)) ||
          (isCodeScanEvent(event) && isWebScanEvent(candidate))
        );
      });

      if (matchingIndex >= 0) {
        const matchingEvent = sorted[matchingIndex];
        consumedIds.add(event.id);
        consumedIds.add(matchingEvent.id);
        items.push(
          isWebScanEvent(event)
            ? buildFullScanEvent(event, matchingEvent)
            : buildFullScanEvent(matchingEvent, event),
        );
        continue;
      }
    }

    consumedIds.add(event.id);
    items.push(buildNormalizedEvent(event));
  }

  return items;
}
