import { emitAppEvent } from "@/lib/app-events";
import {
  recordSearchEvent as recordSearchEventCmd,
  recordUpdateEvent as recordUpdateEventCmd,
} from "@/lib/commands";

interface RecordEventArgs {
  projectId: number;
  title: string;
  summary: string;
  detail?: string | null;
  sourceId?: string | null;
  severity?: string | null;
}

/** Best-effort announcement that timeline rows landed. */
export function publishEventsRecorded(projectId: number): void {
  emitAppEvent("events-recorded", { projectId });
}

export async function recordSearchEvent(args: RecordEventArgs): Promise<number> {
  const eventId = await recordSearchEventCmd(args);
  publishEventsRecorded(args.projectId);
  return eventId;
}

export async function recordUpdateEvent(args: RecordEventArgs): Promise<number> {
  const eventId = await recordUpdateEventCmd(args);
  publishEventsRecorded(args.projectId);
  return eventId;
}
