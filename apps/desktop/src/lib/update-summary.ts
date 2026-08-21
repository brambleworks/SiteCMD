import type { PackageUpdate } from "@/lib/types";

export interface UpdateQueueBreakdown {
  critical: number;
  major: number;
  minor: number;
  patch: number;
}

export interface UpdateQueueSummary {
  total: number;
  security: number;
  regular: number;
  major: number;
  minor: number;
  patch: number;
  securityUpdates: PackageUpdate[];
  regularUpdates: PackageUpdate[];
  majorUpdates: PackageUpdate[];
  minorUpdates: PackageUpdate[];
  patchUpdates: PackageUpdate[];
  breakdown: UpdateQueueBreakdown;
}

export function buildUpdateQueueSummary(updates: readonly PackageUpdate[]): UpdateQueueSummary {
  const securityUpdates: PackageUpdate[] = [];
  const regularUpdates: PackageUpdate[] = [];
  const majorUpdates: PackageUpdate[] = [];
  const minorUpdates: PackageUpdate[] = [];
  const patchUpdates: PackageUpdate[] = [];

  for (const update of updates) {
    if (update.isSecurity) {
      securityUpdates.push(update);
      continue;
    }

    regularUpdates.push(update);
    if (update.updateType === "major") {
      majorUpdates.push(update);
    } else if (update.updateType === "minor") {
      minorUpdates.push(update);
    } else {
      patchUpdates.push(update);
    }
  }

  return {
    total: updates.length,
    security: securityUpdates.length,
    regular: regularUpdates.length,
    major: majorUpdates.length,
    minor: minorUpdates.length,
    patch: patchUpdates.length,
    securityUpdates,
    regularUpdates,
    majorUpdates,
    minorUpdates,
    patchUpdates,
    breakdown: {
      critical: securityUpdates.length,
      major: majorUpdates.length,
      minor: minorUpdates.length,
      patch: patchUpdates.length,
    },
  };
}

export function countSecurityUpdates(updates: readonly PackageUpdate[]): number {
  return buildUpdateQueueSummary(updates).security;
}

export function buildUpdateQueueBreakdown(updates: readonly PackageUpdate[]): UpdateQueueBreakdown {
  return buildUpdateQueueSummary(updates).breakdown;
}
