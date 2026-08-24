/* eslint-disable react-refresh/only-export-components -- test helpers are exported here. */

import { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { writeExportFile } from "@/lib/commands";
import { HeaderActions } from "@/app/ShellHeader";
import { save } from "@tauri-apps/plugin-dialog";
import { useEvents } from "@/hooks/useEvents";
import { useProject } from "@/hooks/useProject";
import { useToast } from "@/hooks/useToast";
import { MonthView } from "./MonthView";
import { WeekView } from "./WeekView";
import { DayView } from "./DayView";
import { EventsFeedView } from "./EventsFeedView";
import { SurfaceState } from "@/components/ui/surface-state";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import {
  finishPerformanceTimerAfterPaint,
  startPerformanceTimer,
  type PerformanceTimer,
} from "@/lib/performance-metrics";
import {
  buildEventsCsvContent,
  dateRangeForView,
  EVENT_FILTER_GROUPS,
  EVENT_VIEW_OPTIONS,
  formatDateRange,
  navigate,
  type CalendarView,
  type FilterGroupKey,
} from "./events-page-model";
import { ChevronLeft, ChevronRight, RefreshCw, FileJson, FileSpreadsheet } from "lucide-react";
import type { AppTarget } from "@/lib/app-targets";
import { getProjectSignalSnapshot, type ProjectWorkSummary } from "@/lib/project-summary-signals";
import { Button } from "@/components/ui/button";
import { useQuery } from "@tanstack/react-query";
import { queryKeys } from "@/lib/query/query-keys";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import { userFacingError } from "@/lib/user-facing-error";

export { buildEventScanTarget, humanizeEventDetail } from "./event-presentation";
export {
  buildEventsCsvContent,
  buildFeedGroups,
  dateRangeForView,
  endOfWeek,
  escapeCsvCell,
  formatDateRange,
  getRelativeTime,
  navigate,
  startOfWeek,
  type CalendarView,
} from "./events-page-model";

interface EventsPageProps {
  projectId: number;
  onOpenTarget?: (target: AppTarget) => void;
}

export function EventsPage({ projectId, onOpenTarget }: EventsPageProps) {
  const { activeEnv } = useProject();
  const [view, setView] = useState<CalendarView>("feed");
  const [cursor, setCursor] = useState(() => new Date());
  const {
    events,
    hasMore = false,
    loading,
    error,
    loadEvents,
    refreshIntegrations,
  } = useEvents(projectId);
  const [refreshing, setRefreshing] = useState(false);
  const [activeFilters, setActiveFilters] = useState<Set<FilterGroupKey>>(new Set());
  const normalizedEnvUrl = normalizeAppUrlForKey(activeEnv?.url ?? null);
  const workSummaryQuery = useQuery({
    queryKey: queryKeys.projectSummary.signals(projectId, normalizedEnvUrl),
    queryFn: () =>
      getProjectSignalSnapshot(projectId, activeEnv?.url ?? null, {
        includeCodeScanDetail: false,
      }),
  });
  const workSummary: ProjectWorkSummary | null = workSummaryQuery.data?.workSummary ?? null;
  const toast = useToast();
  const activeRange = useMemo(() => dateRangeForView(view, cursor), [view, cursor]);
  const eventsReadyTimerRef = useRef<PerformanceTimer | null>(null);

  const toggleFilter = (key: FilterGroupKey) => {
    setActiveFilters((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const allowedTypes = useMemo(() => {
    if (activeFilters.size === 0) return null;
    const types = new Set<string>();
    for (const group of EVENT_FILTER_GROUPS) {
      if (activeFilters.has(group.key)) {
        for (const t of group.types) types.add(t);
      }
    }
    return types;
  }, [activeFilters]);

  const filteredEvents = useMemo(
    () => (allowedTypes ? events.filter((e) => allowedTypes.has(e.eventType)) : events),
    [events, allowedTypes],
  );

  useEffect(() => {
    if (!eventsReadyTimerRef.current || loading) return;
    finishPerformanceTimerAfterPaint(eventsReadyTimerRef.current, {
      status: error ? "error" : "ready",
      eventCount: events.length,
      view,
    });
    eventsReadyTimerRef.current = null;
  }, [error, events.length, loading, view]);

  useEffect(() => {
    eventsReadyTimerRef.current = startPerformanceTimer("events.initial_ready_ms", {
      projectId,
      view,
    });
    void loadEvents(activeRange.start, activeRange.end);
  }, [activeRange.end, activeRange.start, projectId, view, loadEvents]);

  const handleRefresh = async () => {
    setRefreshing(true);
    await refreshIntegrations();
    // Polls are queued asynchronously in the scheduler; reload events after a
    // brief pause to pick up any results that landed quickly.
    toast.info("Refresh started", "Integration polls are running - new events will appear shortly");
    await new Promise((r) => setTimeout(r, 2000));
    void loadEvents(activeRange.start, activeRange.end, undefined, { force: true });
    setRefreshing(false);
  };

  const handleDayClick = (date: Date) => {
    setCursor(date);
    setView("day");
  };

  const handleExportJSON = async () => {
    const data = JSON.stringify(
      {
        date_range: dateRangeForView(view, cursor),
        filters: activeFilters.size > 0 ? [...activeFilters] : "all",
        event_count: filteredEvents.length,
        events: filteredEvents,
      },
      null,
      2,
    );
    try {
      const filePath = await save({
        title: "Export Events as JSON",
        defaultPath: `sitecmd-events-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!filePath) return;
      await writeExportFile({ path: filePath, content: data });
      toast.success(
        "Exported",
        `${filteredEvents.length} events saved to ${filePath.split("/").pop()}`,
      );
    } catch (e) {
      toast.error("Export failed", userFacingError(e, "Nothing was written. Try again."));
    }
  };

  const handleExportCSV = async () => {
    const content = buildEventsCsvContent(filteredEvents);
    try {
      const filePath = await save({
        title: "Export Events as CSV",
        defaultPath: `sitecmd-events-${new Date().toISOString().slice(0, 10)}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!filePath) return;
      await writeExportFile({ path: filePath, content });
      toast.success(
        "Exported",
        `${filteredEvents.length} events saved to ${filePath.split("/").pop()}`,
      );
    } catch (e) {
      toast.error("Export failed", userFacingError(e, "Nothing was written. Try again."));
    }
  };

  const handleRetryLoad = useCallback(() => {
    void loadEvents(activeRange.start, activeRange.end, undefined, { force: true });
  }, [activeRange.end, activeRange.start, loadEvents]);

  const dateLabel = formatDateRange(view, cursor);

  return (
    <div className="page-content stack-section">
      <HeaderActions>
        <div className="date-view-toggle">
          {EVENT_VIEW_OPTIONS.map((v) => (
            <Button
              unstyled
              key={v}
              onClick={() => setView(v)}
              className={`events-view-btn ${
                view === v ? "events-view-btn--active" : "events-view-btn--inactive"
              }`}>
              {v}
            </Button>
          ))}
        </div>
      </HeaderActions>

      <div className="row-between">
        <div className="row-tight">
          {EVENT_FILTER_GROUPS.map(({ key, label }) => {
            const active = activeFilters.size === 0 || activeFilters.has(key);
            return (
              <Button
                unstyled
                key={key}
                onClick={() => toggleFilter(key)}
                className={`inline-filter ${active ? "inline-filter-active" : "inline-filter-inactive"}`}>
                {label}
              </Button>
            );
          })}
          {activeFilters.size > 0 && (
            <Button
              unstyled
              onClick={() => setActiveFilters(new Set())}
              className="events-reset-btn">
              Reset
            </Button>
          )}
        </div>
        <div className="row-tight">
          {view !== "feed" && (
            <>
              <Button
                unstyled
                onClick={() => setCursor(navigate(view, cursor, -1))}
                aria-label="Previous"
                className="icon-btn-sm">
                <ChevronLeft className="icon-md text-muted-foreground" />
              </Button>
              <Button unstyled onClick={() => setCursor(new Date())} className="events-today-btn">
                Today
              </Button>
              <Button
                unstyled
                onClick={() => setCursor(navigate(view, cursor, 1))}
                aria-label="Next"
                className="icon-btn-sm">
                <ChevronRight className="icon-md text-muted-foreground" />
              </Button>
              <span className="subtitle-xs events-toolbar-trail">{dateLabel}</span>
              <div className="date-view-separator" />
            </>
          )}
          <Button
            unstyled
            onClick={handleExportJSON}
            title="Export JSON"
            aria-label="Export JSON"
            className="icon-btn-sm">
            <FileJson className="icon-muted-sm" aria-hidden="true" />
          </Button>
          <Button
            unstyled
            onClick={handleExportCSV}
            title="Export CSV"
            aria-label="Export CSV"
            className="icon-btn-sm">
            <FileSpreadsheet className="icon-muted-sm" aria-hidden="true" />
          </Button>
          <Button
            unstyled
            onClick={handleRefresh}
            disabled={refreshing}
            title="Refresh"
            aria-label="Refresh"
            className="icon-btn-sm">
            <RefreshCw
              className={`icon-muted-sm ${refreshing ? "animate-spin" : ""}`}
              aria-hidden="true"
            />
          </Button>
          <span className="meta-num events-toolbar-trail">
            {filteredEvents.length} event{filteredEvents.length !== 1 ? "s" : ""}
          </span>
        </div>
      </div>
      {hasMore && !loading && !error && filteredEvents.length > 0 && (
        <p className="text-meta">
          Showing the latest 500 events for this range to keep Activity responsive.
        </p>
      )}

      {loading && events.length === 0 ? (
        <LoadingRegion label="Loading activity" className="stack-base">
          {[1, 2, 3, 4, 5].map((index) => (
            <div key={index} className="card card--muted">
              <Skeleton className="events-skeleton-title" />
              <Skeleton className="events-skeleton-line" />
              <Skeleton className="events-skeleton-line-short" />
            </div>
          ))}
        </LoadingRegion>
      ) : error && events.length === 0 ? (
        <SurfaceState
          kind="error"
          title="Activity could not load"
          description="We could not pull this timeline right now. Retry in a moment to bring the latest history back in."
          primaryAction={{ label: "Retry", onClick: handleRetryLoad }}
        />
      ) : filteredEvents.length === 0 ? (
        <SurfaceState
          kind="empty"
          title="No activity in this period"
          description="Scans, deploys, verifications, and integration events will show up here as this project changes."
        />
      ) : (
        <div className={`events-content ${loading ? "events-content--loading" : ""}`}>
          {view === "feed" && (
            <EventsFeedView events={filteredEvents} onOpenTarget={onOpenTarget} />
          )}
          {view === "month" && (
            <MonthView cursor={cursor} events={filteredEvents} onDayClick={handleDayClick} />
          )}
          {view === "week" && (
            <WeekView cursor={cursor} events={filteredEvents} onDayClick={handleDayClick} />
          )}
          {view === "day" && (
            <DayView
              date={cursor}
              events={filteredEvents}
              workSummary={workSummary}
              onOpenTarget={onOpenTarget}
            />
          )}
        </div>
      )}
    </div>
  );
}
