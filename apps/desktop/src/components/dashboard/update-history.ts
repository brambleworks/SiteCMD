import type { PackageUpdate, SiteEvent, UpdateReport } from "@/lib/types";
import { parseJsonRecord } from "@/lib/json-record";
import type { UpdateQueueBreakdown } from "@/lib/update-summary";

const UPDATE_HISTORY_DEDUPE_WINDOW_MS = 15 * 60 * 1000;

interface AppliedUpdateHistoryRow {
  name: string;
  fromVersion: string;
  toVersion: string;
}

export type { UpdateQueueBreakdown };

function parseEventDetail(detail: string | null): Record<string, unknown> | null {
  if (!detail) return null;
  return parseJsonRecord(detail);
}

function getCurrentPackageNames(report: UpdateReport | null): Set<string> {
  return new Set((report?.packages ?? []).map((pkg) => pkg.name));
}

export function withParsedEventDetail(event: SiteEvent): SiteEvent {
  return {
    ...event,
    parsedDetail: parseEventDetail(event.detail),
  };
}

export function formatHistoryTimestamp(occurredAtMs: number): string {
  return new Date(occurredAtMs).toLocaleString([], {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function buildUpdateHistoryKey(update: Pick<PackageUpdate, "ecosystem" | "name">): string {
  return `${update.ecosystem}:${update.name}`;
}

function parseAppliedUpdateHistoryRow(label: string): AppliedUpdateHistoryRow | null {
  const match = label.match(/^(?<name>.+?)\s+(?<from>\S+)\s+->\s+(?<to>\S+)(?:\s+•.*)?$/);
  if (!match?.groups) return null;
  return {
    name: match.groups.name,
    fromVersion: match.groups.from,
    toVersion: match.groups.to,
  };
}

export function getAppliedUpdateHistoryRows(
  detail: Record<string, unknown> | null,
): AppliedUpdateHistoryRow[] {
  if (!detail) return [];

  if (Array.isArray(detail.applied_updates)) {
    const parsed = detail.applied_updates
      .map((item) => {
        if (!item || typeof item !== "object") return null;
        const name = typeof item.name === "string" ? item.name : null;
        const fromVersion = typeof item.from_version === "string" ? item.from_version : null;
        const toVersion = typeof item.to_version === "string" ? item.to_version : null;
        if (!name || !fromVersion || !toVersion) return null;
        return {
          name,
          fromVersion,
          toVersion,
        } satisfies AppliedUpdateHistoryRow;
      })
      .filter((item): item is AppliedUpdateHistoryRow => item != null);
    if (parsed.length > 0) return parsed;
  }

  if (typeof detail.verified_label === "string") {
    const parsed = parseAppliedUpdateHistoryRow(detail.verified_label);
    if (parsed) return [parsed];
  }

  if (typeof detail.item_label === "string" && readCount(detail.cleared_count) === 1) {
    const parsed = parseAppliedUpdateHistoryRow(detail.item_label);
    if (parsed) return [parsed];
  }

  return [];
}

export function getUpdateHistoryTitle(event: SiteEvent, rows: AppliedUpdateHistoryRow[]): string {
  const detail = event.parsedDetail ?? parseEventDetail(event.detail);
  const verifiedCount = readCount(detail?.verified_count);
  const clearedCount = readCount(detail?.cleared_count);
  const count =
    rows.length ||
    (verifiedCount != null && verifiedCount > 0 ? verifiedCount : 0) ||
    (clearedCount != null && clearedCount > 0 ? clearedCount : 0) ||
    1;
  return `${count} Update${count === 1 ? "" : "s"} Applied`;
}

export function getClearedUpdates(
  previousUpdates: PackageUpdate[],
  nextUpdates: PackageUpdate[],
): PackageUpdate[] {
  const nextKeys = new Set(nextUpdates.map((update) => buildUpdateHistoryKey(update)));
  return previousUpdates.filter((update) => !nextKeys.has(buildUpdateHistoryKey(update)));
}

function isHiddenUpdateHistoryEvent(event: SiteEvent): boolean {
  const detail = event.parsedDetail ?? parseEventDetail(event.detail);
  const statusAfter =
    typeof detail?.status_after === "string" ? detail.status_after.trim().toLowerCase() : "";
  if (statusAfter === "still pending") return true;
  if (event.title.trim().toLowerCase().startsWith("update still pending")) return true;
  return getAppliedUpdateHistoryRows(detail).length === 0;
}

export function belongsToCurrentUpdateProject(
  event: SiteEvent,
  projectPath: string | null,
  report: UpdateReport | null,
): boolean {
  const detail = event.parsedDetail ?? parseEventDetail(event.detail);
  const eventProjectPath = typeof detail?.project_path === "string" ? detail.project_path : null;
  if (projectPath && eventProjectPath) {
    return eventProjectPath === projectPath;
  }

  const rows = getAppliedUpdateHistoryRows(detail);
  if (rows.length === 0) return true;

  const currentPackageNames = getCurrentPackageNames(report);
  if (currentPackageNames.size === 0) return true;

  return rows.some((row) => currentPackageNames.has(row.name));
}

function buildAppliedUpdateHistorySignature(event: SiteEvent): string | null {
  const detail = event.parsedDetail ?? parseEventDetail(event.detail);
  const rows = getAppliedUpdateHistoryRows(detail);
  if (rows.length === 0) return null;
  const remainingUpdates = readCount(detail?.remaining_updates) ?? "unknown";
  const securityUpdates = readCount(detail?.security_updates) ?? "unknown";
  return [
    remainingUpdates,
    securityUpdates,
    ...rows.map((row) => `${row.name}:${row.fromVersion}->${row.toVersion}`).sort(),
  ].join("|");
}

function readCount(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) return null;
  return Math.round(value);
}

function collapseDuplicateUpdateHistoryEvents(events: SiteEvent[]): SiteEvent[] {
  const latestBySignature = new Map<string, number>();

  return events.filter((event) => {
    const signature = buildAppliedUpdateHistorySignature(event);
    if (!signature) return true;
    const timestamp = Number.isFinite(event.occurredAtMs) ? event.occurredAtMs : null;
    if (timestamp == null) return true;
    const latestSeen = latestBySignature.get(signature);
    if (
      typeof latestSeen === "number" &&
      latestSeen - timestamp <= UPDATE_HISTORY_DEDUPE_WINDOW_MS
    ) {
      return false;
    }
    latestBySignature.set(signature, timestamp);
    return true;
  });
}

export function getVisibleUpdateHistoryEvents(events: SiteEvent[]): SiteEvent[] {
  return collapseDuplicateUpdateHistoryEvents(
    events.filter((event) => !isHiddenUpdateHistoryEvent(event)),
  );
}
