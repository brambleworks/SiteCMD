import { useCallback } from "react";
import { useQuery } from "@tanstack/react-query";

import { getEvents } from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import type { SiteEvent, UpdateReport } from "@/lib/types";
import {
  belongsToCurrentUpdateProject,
  getVisibleUpdateHistoryEvents,
  withParsedEventDetail,
} from "./update-history";
import { UPDATE_HISTORY_LIMIT } from "./updates-page-model";

export function useUpdatesHistory({
  projectId,
  projectPath,
  report,
}: {
  projectId: number;
  projectPath: string | null;
  /** Reactive report state used by the render-time history filter. */
  report: UpdateReport | null;
}) {
  const historyQuery = useQuery<SiteEvent[]>({
    queryKey: queryKeys.events.updates(projectId),
    queryFn: async () => {
      const now = new Date();
      const start = new Date(now);
      start.setMonth(start.getMonth() - 6);
      const result = await getEvents({
        projectId,
        startMs: start.getTime(),
        endMs: now.getTime(),
        eventTypes: ["update"],
        sinceMs: null,
        sinceEventId: null,
        limit: UPDATE_HISTORY_LIMIT,
      });
      return Array.isArray(result) ? result.map(withParsedEventDetail) : [];
    },
  });

  const updateHistory = getVisibleUpdateHistoryEvents(historyQuery.data ?? []).filter((event) =>
    belongsToCurrentUpdateProject(event, projectPath, report),
  );
  const refetchHistory = historyQuery.refetch;

  const loadUpdateHistory = useCallback(async () => {
    await refetchHistory();
  }, [refetchHistory]);

  return {
    loadUpdateHistory,
    updateHistory,
    updateHistoryLoading: historyQuery.isPending,
    updateHistoryRefreshing: historyQuery.isFetching && !historyQuery.isPending,
  };
}
