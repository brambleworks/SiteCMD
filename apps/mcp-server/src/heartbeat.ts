/** Reads the liveness file the desktop watcher rewrites every five seconds. */

import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { resolveDbPath } from "./db_connection.js";
import { homedir, platform } from "node:os";

/** Mirrors crate::constants::DESKTOP_HEARTBEAT_STALE_MS. */
export const DESKTOP_HEARTBEAT_STALE_MS = 30_000;

export function heartbeatPath(): string {
  return join(dirname(resolveDbPath(platform(), process.env, homedir())), "desktop-heartbeat.json");
}

export function readDesktopHeartbeat(nowMs: number): { alive: boolean; ageMs: number | null } {
  const path = heartbeatPath();
  if (!existsSync(path)) return { alive: false, ageMs: null };
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as { updated_at_ms?: unknown };
    if (typeof parsed.updated_at_ms !== "number") return { alive: false, ageMs: null };
    const ageMs = nowMs - parsed.updated_at_ms;
    return { alive: ageMs >= 0 && ageMs < DESKTOP_HEARTBEAT_STALE_MS, ageMs };
  } catch {
    return { alive: false, ageMs: null };
  }
}

export function desktopStatusLine(nowMs: number): string {
  const { alive, ageMs } = readDesktopHeartbeat(nowMs);
  return alive
    ? `Desktop app: running (heartbeat ${Math.round((ageMs ?? 0) / 1000)}s ago).`
    : "Desktop app: SiteCMD is not running; pending work starts when it opens (requests expire after 24 hours).";
}
