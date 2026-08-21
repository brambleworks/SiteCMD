import { useCallback, useMemo } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { fetchAnalytics, invalidateAnalyticsCache } from "@/lib/commands";
import { readAnalyticsSnapshot, writeAnalyticsSnapshot } from "@/lib/analytics-snapshot-cache";
import { queryKeys } from "@/lib/query/query-keys";
import { errorMessage } from "@/lib/error-message";
import { useVisibilityRefresh } from "@/lib/useVisibilityRefresh";
import type { AnalyticsResponse } from "@/lib/analytics-types";

// Reuse fresh snapshots across remounts; older data revalidates on mount or visibility change.
const ANALYTICS_STALE_MS = 5 * 60 * 1000;

interface AnalyticsSnapshot {
  data: AnalyticsResponse;
  fetchedAtMs: number;
}

// Treat "no integrations" as an empty analytics state, not a retryable failure.
function isEmptyAnalyticsStateError(error: unknown): boolean {
  const message = errorMessage(error);
  return message.toLowerCase().includes("no analytics integrations configured");
}

interface UseAnalyticsQueryArgs {
  projectId: number;
  period: string;
  /** Traffic scopes to a site url; the Search page is project-wide (`null`). */
  siteUrl?: string | null;
  /** Page-specific persistence key so Traffic and Search cache separately. */
  snapshotKey: string;
}

interface UseAnalyticsQueryResult {
  data: AnalyticsResponse | null;
  fetchedAt: Date | null;
  /** A fetch (initial or background) is in flight. */
  isFetching: boolean;
  /** The query failed with no data to fall back to. */
  isError: boolean;
  /** Bust the backend cache, then refetch every period for this project. */
  refresh: () => Promise<void>;
}

export function useAnalyticsQuery({
  projectId,
  period,
  siteUrl = null,
  snapshotKey,
}: UseAnalyticsQueryArgs): UseAnalyticsQueryResult {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: queryKeys.analytics.forQuery(projectId, period, siteUrl),
    queryFn: async () => {
      let response: AnalyticsResponse;
      try {
        response = await fetchAnalytics<AnalyticsResponse>({
          projectId,
          period,
          siteUrl: siteUrl ?? undefined,
        });
      } catch (error) {
        if (!isEmptyAnalyticsStateError(error)) throw error;
        response = {};
      }
      const fetchedAtMs = Date.now();
      writeAnalyticsSnapshot<AnalyticsSnapshot>(
        snapshotKey,
        { data: response, fetchedAtMs },
        fetchedAtMs,
      );
      return { data: response, fetchedAtMs } satisfies AnalyticsSnapshot;
    },
    staleTime: ANALYTICS_STALE_MS,
    initialData: () => readAnalyticsSnapshot<AnalyticsSnapshot>(snapshotKey) ?? undefined,
    initialDataUpdatedAt: () => readAnalyticsSnapshot<AnalyticsSnapshot>(snapshotKey)?.fetchedAtMs,
  });

  const refresh = useCallback(async () => {
    try {
      await invalidateAnalyticsCache({ projectId });
    } catch {
      // Best-effort backend cache bust; the refetch below still runs.
    }
    await queryClient.invalidateQueries({ queryKey: queryKeys.analytics.forProject(projectId) });
  }, [projectId, queryClient]);

  // A long-idle return can leave stale (or WKWebView-dropped) numbers on screen.
  useVisibilityRefresh({ staleAfterMs: ANALYTICS_STALE_MS, onRefresh: refresh });

  // Refresh when an OAuth reconnect finishes while its card is unmounted.
  useTauriEvent("google-integration-updated", (payload) => {
    if (payload?.projectId === projectId) void refresh();
  });

  // Structural sharing keeps fetchedAt stable until the payload changes.
  const fetchedAt = useMemo(
    () => (query.data ? new Date(query.data.fetchedAtMs) : null),
    [query.data],
  );

  return {
    data: query.data?.data ?? null,
    fetchedAt,
    isFetching: query.isFetching,
    isError: query.isError,
    refresh,
  };
}
