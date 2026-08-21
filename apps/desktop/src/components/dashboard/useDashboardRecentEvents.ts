import { useCallback } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getEvents } from "@/lib/commands";
import { MS_PER_DAY } from "@/lib/format";
import type { SiteEvent } from "@/lib/types";
import { queryKeys } from "@/lib/query/query-keys";

const RECENT_EVENTS_LOOKBACK_DAYS = 45;
const RECENT_EVENTS_LIMIT = 10;
const UPDATE_TREND_EVENTS_LOOKBACK_DAYS = 183;
const UPDATE_TREND_EVENTS_LIMIT = 10;

function fetchRecentEvents(projectId: number) {
  const endMs = Date.now();
  return getEvents({
    projectId,
    startMs: endMs - RECENT_EVENTS_LOOKBACK_DAYS * MS_PER_DAY,
    endMs,
    eventTypes: null,
    sinceMs: null,
    sinceEventId: null,
    limit: RECENT_EVENTS_LIMIT,
  });
}

function fetchUpdateEvents(projectId: number) {
  const endMs = Date.now();
  return getEvents({
    projectId,
    startMs: endMs - UPDATE_TREND_EVENTS_LOOKBACK_DAYS * MS_PER_DAY,
    endMs,
    eventTypes: ["update"],
    sinceMs: null,
    sinceEventId: null,
    limit: UPDATE_TREND_EVENTS_LIMIT,
  });
}

export function useDashboardRecentEvents({
  includeReferenceSignals,
  projectId,
}: {
  includeReferenceSignals: boolean;
  projectId: number;
}) {
  const queryClient = useQueryClient();
  const recentQuery = useQuery<SiteEvent[]>({
    queryKey: queryKeys.events.dashboardRecent(projectId),
    queryFn: () => fetchRecentEvents(projectId),
    enabled: includeReferenceSignals,
  });
  const updateQuery = useQuery<SiteEvent[]>({
    queryKey: queryKeys.events.dashboardUpdates(projectId),
    queryFn: () => fetchUpdateEvents(projectId),
    enabled: includeReferenceSignals,
  });
  const refetchRecent = recentQuery.refetch;
  const refetchUpdates = updateQuery.refetch;

  const loadRecentEvents = useCallback(
    async (options?: { force?: boolean }) => {
      if (!includeReferenceSignals) return;
      if (options?.force) {
        await refetchRecent();
        return;
      }
      await queryClient.ensureQueryData({
        queryKey: queryKeys.events.dashboardRecent(projectId),
        queryFn: () => fetchRecentEvents(projectId),
      });
    },
    [includeReferenceSignals, projectId, queryClient, refetchRecent],
  );

  const loadUpdateEvents = useCallback(
    async (options?: { force?: boolean }) => {
      if (!includeReferenceSignals) return;
      if (options?.force) {
        await refetchUpdates();
        return;
      }
      await queryClient.ensureQueryData({
        queryKey: queryKeys.events.dashboardUpdates(projectId),
        queryFn: () => fetchUpdateEvents(projectId),
      });
    },
    [includeReferenceSignals, projectId, queryClient, refetchUpdates],
  );

  return {
    recentEvents: includeReferenceSignals ? (recentQuery.data ?? []) : [],
    recentEventsLoading:
      includeReferenceSignals && (recentQuery.isPending || updateQuery.isPending),
    loadRecentEvents,
    loadUpdateEvents,
    updateEvents: includeReferenceSignals ? (updateQuery.data ?? []) : [],
  };
}
