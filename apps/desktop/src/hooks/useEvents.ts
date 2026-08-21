import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { parseJsonRecord } from "@/lib/json-record";
import { backfillEvents, getEvents, refreshEvents } from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import type { SiteEvent } from "@/lib/types";

const EVENTS_PAGE_SIZE = 500;

interface EventRange {
  startDate: string;
  endDate: string;
  eventTypes?: string[];
}

interface EventRangeResult {
  events: SiteEvent[];
  hasMore: boolean;
}

function parseEventDetail(detail: string | null): Record<string, unknown> | null {
  if (!detail) return null;
  return parseJsonRecord(detail);
}

function withParsedDetail(event: SiteEvent): SiteEvent {
  return {
    ...event,
    parsedDetail: parseEventDetail(event.detail),
  };
}

function mergeEvents(existing: SiteEvent[], incoming: SiteEvent[]): SiteEvent[] {
  if (incoming.length === 0) return existing;
  const byId = new Map<number, SiteEvent>();
  for (const event of existing) byId.set(event.id, event);
  for (const event of incoming) byId.set(event.id, event);
  return [...byId.values()].sort((a, b) => b.occurredAtMs - a.occurredAtMs || b.id - a.id);
}

function eventTypesKey(eventTypes?: string[]): string {
  return eventTypes?.length ? [...eventTypes].sort().join(",") : "all";
}

function rangeQueryKey(projectId: number, range: EventRange) {
  return queryKeys.events.range(
    projectId,
    range.startDate,
    range.endDate,
    eventTypesKey(range.eventTypes),
  );
}

async function fetchEventRange(projectId: number, range: EventRange): Promise<EventRangeResult> {
  const raw = await getEvents({
    projectId,
    startMs: Date.parse(range.startDate),
    endMs: Date.parse(range.endDate),
    eventTypes: range.eventTypes ?? null,
    sinceMs: null,
    sinceEventId: null,
    limit: EVENTS_PAGE_SIZE + 1,
  });
  const rows = Array.isArray(raw) ? raw : [];
  const hasMore = rows.length > EVENTS_PAGE_SIZE;
  return {
    events: (hasMore ? rows.slice(0, EVENTS_PAGE_SIZE) : rows).map(withParsedDetail),
    hasMore,
  };
}

export function useEvents(projectId: number | null) {
  const queryClient = useQueryClient();
  const [activeRange, setActiveRange] = useState<EventRange | null>(null);
  const activeRangeRef = useRef<EventRange | null>(null);
  const backfilledProjectsRef = useRef<Set<number>>(new Set());
  const backfillInFlightProjectsRef = useRef<Set<number>>(new Set());
  // Guards this hook's own projectId prop across the async backfill. A reusable
  // prop-parameterized hook must not reach into the global selection store.
  const currentProjectIdRef = useRef<number | null>(projectId);

  const query = useQuery({
    queryKey:
      projectId != null && activeRange
        ? rangeQueryKey(projectId, activeRange)
        : queryKeys.events.range(0, "", "", "disabled"),
    queryFn: () => fetchEventRange(projectId as number, activeRange as EventRange),
    enabled: projectId != null && activeRange != null,
  });
  const events = useMemo(() => query.data?.events ?? [], [query.data?.events]);
  const eventsRef = useRef<SiteEvent[]>(events);

  useEffect(() => {
    eventsRef.current = events;
  }, [events]);

  useEffect(() => {
    currentProjectIdRef.current = projectId;
  }, [projectId]);

  const loadEvents = useCallback(
    async (
      startDate: string,
      endDate: string,
      eventTypes?: string[],
      options?: { force?: boolean },
    ) => {
      if (!projectId) return;
      const range = { startDate, endDate, eventTypes };
      activeRangeRef.current = range;
      setActiveRange(range);
      const queryKey = rangeQueryKey(projectId, range);
      try {
        if (options?.force) {
          await queryClient.fetchQuery({
            queryKey,
            queryFn: () => fetchEventRange(projectId, range),
            staleTime: 0,
          });
        } else {
          await queryClient.ensureQueryData({
            queryKey,
            queryFn: () => fetchEventRange(projectId, range),
          });
        }
      } catch {
        // The observed query exposes the page error state.
      }

      if (
        backfilledProjectsRef.current.has(projectId) ||
        backfillInFlightProjectsRef.current.has(projectId)
      ) {
        return;
      }
      backfillInFlightProjectsRef.current.add(projectId);
      void backfillEvents({ projectId })
        .catch(() => {
          // Backfill is best-effort.
        })
        .finally(() => {
          backfillInFlightProjectsRef.current.delete(projectId);
          backfilledProjectsRef.current.add(projectId);
          if (currentProjectIdRef.current !== projectId) return;
          const currentRange = activeRangeRef.current;
          if (!currentRange) return;
          void queryClient.invalidateQueries({
            queryKey: rangeQueryKey(projectId, currentRange),
            exact: true,
          });
        });
    },
    [projectId, queryClient],
  );

  const reloadActiveRange = useCallback(async () => {
    const range = activeRangeRef.current;
    if (!projectId || !range) return;
    const queryKey = rangeQueryKey(projectId, range);
    const current = queryClient.getQueryData<EventRangeResult>(queryKey);
    const newest = current?.events[0] ?? eventsRef.current[0] ?? null;
    try {
      const raw = await getEvents({
        projectId,
        startMs: Date.parse(range.startDate),
        endMs: Date.parse(range.endDate),
        eventTypes: range.eventTypes ?? null,
        sinceMs: newest?.occurredAtMs ?? null,
        sinceEventId: newest?.id ?? null,
        limit: EVENTS_PAGE_SIZE,
      });
      const incoming = (Array.isArray(raw) ? raw : []).map(withParsedDetail);
      queryClient.setQueryData<EventRangeResult>(queryKey, {
        events: mergeEvents(current?.events ?? eventsRef.current, incoming),
        hasMore: current?.hasMore ?? false,
      });
    } catch {
      // Silent polling keeps the last good cached range visible.
    }
  }, [projectId, queryClient]);

  const refreshIntegrations = useCallback(async (): Promise<void> => {
    if (!projectId) return;
    try {
      await refreshEvents({ projectId });
    } catch {
      // Scheduler may not be initialized yet in dev; the cached range remains valid.
    }
  }, [projectId]);

  useEffect(() => {
    if (!projectId) {
      activeRangeRef.current = null;
      return;
    }

    const refreshIfVisible = () => {
      if (document.visibilityState !== "visible") return;
      void reloadActiveRange();
    };

    const intervalId = window.setInterval(refreshIfVisible, 30_000);
    window.addEventListener("focus", refreshIfVisible);
    document.addEventListener("visibilitychange", refreshIfVisible);

    return () => {
      window.clearInterval(intervalId);
      window.removeEventListener("focus", refreshIfVisible);
      document.removeEventListener("visibilitychange", refreshIfVisible);
    };
  }, [projectId, reloadActiveRange]);

  return {
    events,
    hasMore: query.data?.hasMore ?? false,
    // Background refetches must not dim the timeline as an initial load.
    loading: projectId != null && activeRange != null && query.isPending,
    error: query.isError ? "Activity could not load right now." : null,
    loadEvents,
    refreshIntegrations,
  };
}
