/** Fail closed on a database this server's SQL was not written for. */

import { getDb } from "./db_connection.js";
import { SUPPORTED_SCHEMA_VERSIONS } from "./version.js";

let checked = false;

function readSchemaVersion(): number | null {
  const row = getDb().prepare(`SELECT MAX(version) AS version FROM _schema_version`).get() as
    { version: number | null } | undefined;
  return row?.version ?? null;
}

export function assertSupportedSchemaVersion(): void {
  if (checked) return;
  const version = readSchemaVersion();
  const { min, max } = SUPPORTED_SCHEMA_VERSIONS;
  if (version === null)
    throw new Error("SiteCMD database has no schema version; open SiteCMD once so it can migrate.");
  if (version > max) {
    throw new Error(
      `SiteCMD database schema version ${version} is newer than this MCP server supports (max ${max}); open SiteCMD so it refreshes the bundled MCP server, then reconnect your agent.`,
    );
  }
  if (version < min) {
    throw new Error(
      `SiteCMD database schema version ${version} is older than this MCP server supports (min ${min}); open SiteCMD so it migrates the database.`,
    );
  }
  checked = true;
}
