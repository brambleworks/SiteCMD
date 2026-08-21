import { command } from "./invoke";
import type { SiteEvent } from "@/lib/types";

export function getEvents(args: {
  projectId: number;
  startMs: number;
  endMs: number;
  // The Rust command takes Vec<String>, so callers may pass any event-type
  // strings; the wire does not narrow this to the EventType union.
  eventTypes?: string[] | null;
  sinceMs?: number | null;
  sinceEventId?: number | null;
  limit?: number | null;
}): Promise<SiteEvent[]> {
  return command<SiteEvent[]>("get_events", args);
}

interface RecordEventArgs {
  projectId: number;
  title: string;
  summary: string;
  detail?: string | null;
  sourceId?: string | null;
  severity?: string | null;
}

export function recordUpdateEvent(args: RecordEventArgs): Promise<number> {
  return command<number>("record_update_event", args);
}

export function recordSearchEvent(args: RecordEventArgs): Promise<number> {
  return command<number>("record_search_event", args);
}

export function refreshEvents(args: { projectId: number }): Promise<void> {
  return command<void>("refresh_events", args);
}

export function backfillEvents(args: { projectId: number }): Promise<number> {
  return command<number>("backfill_events", args);
}
