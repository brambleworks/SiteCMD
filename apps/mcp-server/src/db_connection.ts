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

/** Test seam confirming the shared read connection remains readonly. */
export function __test_isReadDbReadonly(): boolean {
  getDb();
  return _dbReadOnly;
}
