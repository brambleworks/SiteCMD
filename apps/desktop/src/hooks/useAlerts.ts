import { useCallback, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  countUnreadAlerts,
  dismissAlert as rpcDismiss,
  getAlerts,
  markAlertUnread as rpcMarkUnread,
  markAlertViewed as rpcMarkViewed,
  markAlertsViewedBulk,
} from "@/lib/alerts";
import type { AlertFilter, AlertRow } from "@/lib/types";
import { queryKeys } from "@/lib/query/query-keys";
import { emitAppEvent } from "@/lib/app-events";

interface UseAlertsOptions {
  includeRows?: boolean;
  deferMs?: number;
}

export function publishAlertsChanged(projectId: number | null) {
  emitAppEvent("alerts-changed", { projectId });
}

export function useAlerts(
  projectId: number | null,
  filter: AlertFilter = "unread",
  options?: UseAlertsOptions,
) {
  const queryClient = useQueryClient();
  const includeRows = options?.includeRows ?? true;
  const deferMs = Math.max(0, options?.deferMs ?? 0);
  const deferKey = `${projectId ?? "none"}:${deferMs}`;
  const [deferState, setDeferState] = useState(() => ({
    key: deferKey,
    ready: deferMs === 0,
  }));
  if (deferState.key !== deferKey) {
    setDeferState({ key: deferKey, ready: deferMs === 0 });
  }
  const deferReady = deferState.key === deferKey ? deferState.ready : deferMs === 0;

  useEffect(() => {
    if (deferMs === 0) return;
    const timeoutId = window.setTimeout(() => {
      setDeferState((current) =>
        current.key === deferKey ? { ...current, ready: true } : current,
      );
    }, deferMs);
    return () => window.clearTimeout(timeoutId);
  }, [deferKey, deferMs]);

  const enabled = projectId != null && (deferMs === 0 || deferReady);
  const rowsQuery = useQuery<AlertRow[]>({
    queryKey: queryKeys.alerts.rows(projectId ?? 0, filter),
    queryFn: async () => {
      const rows = await getAlerts(projectId as number, filter);
      return Array.isArray(rows) ? rows : [];
    },
    enabled: enabled && includeRows,
  });
  const countsQuery = useQuery({
    queryKey: queryKeys.alerts.counts(projectId ?? 0),
    queryFn: () => countUnreadAlerts(projectId as number),
    enabled,
  });
  const alerts = useMemo(
    () => (includeRows ? (rowsQuery.data ?? []) : []),
    [includeRows, rowsQuery.data],
  );
  const counts = countsQuery.data;

  const refresh = useCallback(async () => {
    if (projectId == null) return;
    await queryClient.invalidateQueries({ queryKey: queryKeys.alerts.all });
  }, [projectId, queryClient]);

  const markViewed = useCallback(
    async (id: number) => {
      await rpcMarkViewed(id);
      await refresh();
    },
    [refresh],
  );

  const dismiss = useCallback(
    async (id: number) => {
      await rpcDismiss(id);
      await refresh();
    },
    [refresh],
  );

  const markUnread = useCallback(
    async (id: number) => {
      await rpcMarkUnread(id);
      await refresh();
    },
    [refresh],
  );

  const markAllRead = useCallback(async () => {
    const ids = alerts
      .filter((a) => a.viewedAt === null && a.dismissedAt === null)
      .map((a) => a.id);
    if (ids.length === 0) return;
    await markAlertsViewedBulk(ids);
    await refresh();
  }, [alerts, refresh]);

  return {
    alerts,
    unreadCount: typeof counts?.total === "number" ? counts.total : 0,
    unreadCriticalCount: typeof counts?.critical === "number" ? counts.critical : 0,
    loading: enabled && (countsQuery.isPending || (includeRows && rowsQuery.isPending)),
    refreshing:
      enabled &&
      !countsQuery.isPending &&
      (!includeRows || !rowsQuery.isPending) &&
      (countsQuery.isFetching || rowsQuery.isFetching),
    error:
      countsQuery.isError || rowsQuery.isError
        ? String(countsQuery.error ?? rowsQuery.error)
        : null,
    refresh,
    dismiss,
    markViewed,
    markUnread,
    markAllRead,
  };
}
