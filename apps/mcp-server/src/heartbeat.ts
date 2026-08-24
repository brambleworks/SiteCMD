/** Reads the liveness file the desktop watcher rewrites every five seconds. */

import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { resolveDbPath } from "./db_connection.js";
import { homedir, platform } from "node:os";

/** Mirrors crate::constants::DESKTOP_HEARTBEAT_STALE_MS; pinned by test/heartbeat_parity.test.mjs. */
export const DESKTOP_HEARTBEAT_STALE_MS = 30_000;

/** One short retry covers a heartbeat caught mid-write; torn twice in a row counts as unreadable, not stale. */
const HEARTBEAT_READ_RETRY_DELAY_MS = 50;

function heartbeatPath(): string {
  return join(dirname(resolveDbPath(platform(), process.env, homedir())), "desktop-heartbeat.json");
}

interface ParsedHeartbeat {
  updatedAtMs: number;
}

function readHeartbeatFileOnce(path: string): ParsedHeartbeat | null {
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as { updated_at_ms?: unknown };
    return typeof parsed.updated_at_ms === "number" ? { updatedAtMs: parsed.updated_at_ms } : null;
  } catch {
    return null;
  }
}

function sleepSync(ms: number): void {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

export interface DesktopHeartbeatStatus {
  alive: boolean;
  ageMs: number | null;
  /** The file exists but could not be read or parsed, even after one retry: unknown, never "not running". */
  unknown: boolean;
}

export function readDesktopHeartbeat(nowMs: number): DesktopHeartbeatStatus {
  const path = heartbeatPath();
  if (!existsSync(path)) return { alive: false, ageMs: null, unknown: false };
  let parsed = readHeartbeatFileOnce(path);
  if (!parsed) {
    sleepSync(HEARTBEAT_READ_RETRY_DELAY_MS);
    parsed = readHeartbeatFileOnce(path);
  }
  if (!parsed) return { alive: false, ageMs: null, unknown: true };
  const ageMs = nowMs - parsed.updatedAtMs;
  return { alive: ageMs >= 0 && ageMs < DESKTOP_HEARTBEAT_STALE_MS, ageMs, unknown: false };
}

export function desktopStatusLine(nowMs: number): string {
  const { alive, ageMs, unknown } = readDesktopHeartbeat(nowMs);
  if (alive) return `Desktop app: running (heartbeat ${Math.round((ageMs ?? 0) / 1000)}s ago).`;
  if (unknown) {
    return "Desktop app: could not read the heartbeat file; the request was recorded and will run once SiteCMD is confirmed open.";
  }
  return "Desktop app: SiteCMD is not running; pending work starts when it opens (requests expire after 24 hours).";
}
