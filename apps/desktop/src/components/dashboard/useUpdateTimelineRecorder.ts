import { useCallback } from "react";

import type { AppTarget } from "@/lib/app-targets";
import { recordUpdateEvent } from "@/lib/event-writes";
import type { PackageUpdate } from "@/lib/types";
import { getPackageUpdateTargetVersion } from "@/lib/update-priority";

import type { UpdateQueueBreakdown } from "./update-history";

interface UseUpdateTimelineRecorderOptions {
  loadUpdateHistory: () => Promise<void>;
  normalizedUrl: string;
  projectId: number;
  projectPath: string | null;
}

interface RecordUpdateTimelineEventOptions {
  sourceId: string;
  title: string;
  summary: string;
  target?: AppTarget | null;
  itemLabel?: string | null;
  verifiedLabel?: string | null;
  nextItemLabel?: string | null;
  appliedUpdates?: PackageUpdate[] | null;
  statusBefore?: string | null;
  statusAfter?: string | null;
  verifiedCount?: number;
  remainingUpdates: number;
  securityUpdates: number;
  remainingBreakdown?: UpdateQueueBreakdown | null;
  workflowLabel?: string | null;
  severity?: "info" | "warning";
}

export function useUpdateTimelineRecorder({
  loadUpdateHistory,
  normalizedUrl,
  projectId,
  projectPath,
}: UseUpdateTimelineRecorderOptions) {
  return useCallback(
    (options: RecordUpdateTimelineEventOptions) => {
      void recordUpdateEvent({
        projectId,
        title: options.title,
        summary: options.summary,
        detail: JSON.stringify({
          page: "updates",
          project_path: projectPath,
          url: normalizedUrl,
          item_id: options.target?.page === "updates" ? (options.target.itemId ?? null) : null,
          item_label: options.itemLabel ?? null,
          verified_label: options.verifiedLabel ?? null,
          next_item_label: options.nextItemLabel ?? null,
          applied_updates:
            options.appliedUpdates?.map((update) => ({
              name: update.name,
              from_version: update.currentVersion,
              to_version: getPackageUpdateTargetVersion(update) ?? "resolved",
            })) ?? null,
          status_before: options.statusBefore ?? null,
          status_after: options.statusAfter ?? null,
          reason: "dependency-verification",
          verified_count: options.verifiedCount ?? 1,
          remaining_updates: options.remainingUpdates,
          security_updates: options.securityUpdates,
          critical_updates: options.remainingBreakdown?.critical ?? null,
          major_updates: options.remainingBreakdown?.major ?? null,
          minor_updates: options.remainingBreakdown?.minor ?? null,
          patch_updates: options.remainingBreakdown?.patch ?? null,
          workflow_label: options.workflowLabel ?? null,
        }),
        sourceId: options.sourceId,
        severity: options.severity ?? (options.securityUpdates > 0 ? "warning" : "info"),
      })
        .then(() => loadUpdateHistory())
        .catch(() => {});
    },
    [loadUpdateHistory, normalizedUrl, projectId, projectPath],
  );
}
