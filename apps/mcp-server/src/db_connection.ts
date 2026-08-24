/** Shared readonly access plus a fix-attempt-only write connection. */

import { existsSync } from "node:fs";
import { homedir, platform } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

export function resolveDbPath(
  platformName: NodeJS.Platform,
  environment: NodeJS.ProcessEnv,
  home: string,
): string {
  const envPath = environment.SITECMD_DB_PATH;
  if (envPath) return envPath;

  switch (platformName) {
    case "darwin":
      return join(home, "Library", "Application Support", "com.sitecmd.app", "sitecmd.db");
    case "win32":
      return join(
        environment.LOCALAPPDATA || environment.APPDATA || join(home, "AppData", "Local"),
        "com.sitecmd.app",
        "sitecmd.db",
      );
    default: // Linux
      return join(
        environment.XDG_DATA_HOME || join(home, ".local", "share"),
        "com.sitecmd.app",
        "sitecmd.db",
      );
  }
}

function getDbPath(): string {
  return resolveDbPath(platform(), process.env, homedir());
}

let _db: DatabaseSync | null = null;
let _dbReadOnly = false;

export class SiteCmdDatabaseNotFoundError extends Error {
  constructor(path: string) {
    super(
      `SiteCMD database not found at ${path}. ` +
        `Make sure SiteCMD has been run at least once. ` +
        `You can override the path with SITECMD_DB_PATH env var.`,
    );
    this.name = "SiteCmdDatabaseNotFoundError";
  }
}

export function isSiteCmdDatabaseNotFoundError(
  error: unknown,
): error is SiteCmdDatabaseNotFoundError {
  return error instanceof SiteCmdDatabaseNotFoundError;
}

export function getDb(): DatabaseSync {
  if (_db) return _db;

  const dbPath = getDbPath();
  if (!existsSync(dbPath)) {
    throw new SiteCmdDatabaseNotFoundError(dbPath);
  }

  _db = new DatabaseSync(dbPath, { readOnly: true });
  _db.exec("PRAGMA busy_timeout = 5000");
  _dbReadOnly = true;
  return _db;
}

let _dbWrite: DatabaseSync | null = null;

/** Fix-attempt-only writer with a busy timeout for desktop WAL contention. */
export function getDbWrite(): DatabaseSync {
  if (_dbWrite) return _dbWrite;
  const dbPath = getDbPath();
  if (!existsSync(dbPath)) {
    throw new Error(
      `SiteCMD database not found at ${dbPath}. Open the SiteCMD app and click "Fix with your agent" on an issue first.`,
    );
  }
  _dbWrite = new DatabaseSync(dbPath);
  _dbWrite.exec("PRAGMA busy_timeout = 5000");
  return _dbWrite;
}

const SQLITE_BUSY = 5;
const SQLITE_LOCKED = 6;
const BUSY_RETRY_DELAY_MS = 100;

function isBusyError(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  const errcode = (error as { errcode?: unknown }).errcode;
  return errcode === SQLITE_BUSY || errcode === SQLITE_LOCKED;
}

/** busy_timeout covers writer contention; one retry covers WAL recovery, which returns BUSY immediately. */
export function withBusyRetry<T>(run: () => T): T {
  try {
    return run();
  } catch (error) {
    if (!isBusyError(error)) throw error;
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, BUSY_RETRY_DELAY_MS);
    return run();
  }
}

/** Test seam: the busy timeout the shared read connection declared. */
export function __test_readBusyTimeout(): number {
  const row = getDb().prepare("PRAGMA busy_timeout").get() as { timeout?: number } | undefined;
  return Number(row?.timeout ?? 0);
}

/** Test seam confirming the shared read connection remains readonly. */
export function __test_isReadDbReadonly(): boolean {
  getDb();
  return _dbReadOnly;
}
