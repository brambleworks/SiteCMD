import type { PackageUpdate, UpdateReport } from "@/lib/types";
import type { PendingVerificationEntry } from "@/lib/pending-verification";
import { formatRelativeTime } from "@/lib/format";
import { buildUpdateQueueSummary } from "@/lib/update-summary";
import { buildCommand } from "./update-commands";
import type { UpdateFilter } from "./update-sections";

export const UPDATE_REPORT_CACHE_TTL_MS = 5 * 60 * 1000;
export const UPDATE_HISTORY_LIMIT = 8;

interface UpdateSectionViewModel {
  label: string;
  color: string;
  updates: PackageUpdate[];
}

interface UpdateDisplayModel {
  securityUpdates: PackageUpdate[];
  regularUpdates: PackageUpdate[];
  /** Every update that has a real command, security first. Drives "Copy All Commands". */
  copyableUpdates: PackageUpdate[];
  majors: PackageUpdate[];
  minors: PackageUpdate[];
  patches: PackageUpdate[];
  totalCount: number;
  packageCount: number;
  sections: UpdateSectionViewModel[];
}

const MEMBER_DISPLAY_LIMIT = 3;

/** Formats a workspace update location, mapping the backend `.` key to `root`. */
export function formatWorkspaceMembers(members: string[]): string | null {
  if (members.length === 0) return null;
  const labels = members.map((member) => (member === "." ? "root" : member));
  if (labels.length <= MEMBER_DISPLAY_LIMIT) return labels.join(", ");
  const shown = labels.slice(0, MEMBER_DISPLAY_LIMIT).join(", ");
  return `${shown} +${labels.length - MEMBER_DISPLAY_LIMIT} more`;
}

export function formatLastChecked(ts: number, nowMs: number): string {
  return formatRelativeTime(ts, nowMs);
}

export function findPackageUpdateByItemId(
  updates: PackageUpdate[],
  itemId: string | null | undefined,
): PackageUpdate | null {
  if (!itemId) return null;
  const [ecosystem, name] = itemId.split(":");
  if (!ecosystem || !name) return null;
  return (
    updates.find((candidate) => candidate.ecosystem === ecosystem && candidate.name === name) ??
    null
  );
}

export function getPendingUpdateEntries(
  entries: PendingVerificationEntry[],
  projectId: number,
  normalizedUrl: string,
): PendingVerificationEntry[] {
  return entries
    .filter(
      (entry) =>
        entry.projectId === projectId && entry.url === normalizedUrl && entry.page === "updates",
    )
    .sort((a, b) => b.updatedAt - a.updatedAt);
}

export function buildUpdateDisplayModel(
  report: UpdateReport | null,
  filter: UpdateFilter,
): UpdateDisplayModel {
  const summary = buildUpdateQueueSummary(report?.updates ?? []);
  const securityUpdates = summary.securityUpdates;
  const regularUpdates = summary.regularUpdates;
  const majors = summary.majorUpdates;
  const minors = summary.minorUpdates;
  const patches = summary.patchUpdates;

  const filtered =
    filter === "all"
      ? regularUpdates
      : filter === "major"
        ? majors
        : filter === "minor"
          ? minors
          : patches;

  return {
    securityUpdates,
    regularUpdates,
    // Include security updates in "All" only when OSV verified a target release.
    copyableUpdates: [...securityUpdates, ...regularUpdates].filter(
      (update) => buildCommand(update) !== null,
    ),
    majors,
    minors,
    patches,
    totalCount: summary.total,
    packageCount: report?.packages?.length ?? 0,
    sections: buildUpdateSections({ filter, filtered, majors, minors, patches }),
  };
}

function buildUpdateSections({
  filter,
  filtered,
  majors,
  minors,
  patches,
}: {
  filter: UpdateFilter;
  filtered: PackageUpdate[];
  majors: PackageUpdate[];
  minors: PackageUpdate[];
  patches: PackageUpdate[];
}): UpdateSectionViewModel[] {
  const sections: UpdateSectionViewModel[] = [];

  if (filter === "all" || filter === "major") {
    const items = filter === "all" ? majors : filtered;
    if (items.length > 0) {
      sections.push({
        label: `MAJOR UPDATES (${items.length})`,
        color: "text-amber-400",
        updates: items,
      });
    }
  }

  if (filter === "all" || filter === "minor") {
    const items = filter === "all" ? minors : filtered;
    if (items.length > 0) {
      sections.push({
        label: `MINOR UPDATES (${items.length})`,
        color: "text-primary",
        updates: items,
      });
    }
  }

  if (filter === "all" || filter === "patch") {
    const items = filter === "all" ? patches : filtered;
    if (items.length > 0) {
      sections.push({
        label: `PATCH UPDATES (${items.length})`,
        color: "text-emerald-400",
        updates: items,
      });
    }
  }

  return sections;
}
