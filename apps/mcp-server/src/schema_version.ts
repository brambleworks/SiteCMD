/** Fail closed on a database this server's SQL was not written for. */

import { getDb } from "./db_connection.js";
import { SUPPORTED_SCHEMA_VERSIONS } from "./version.js";

let checked = false;

export type SchemaCompatibilityErrorCode =
  "schema_version_missing" | "schema_too_new" | "schema_too_old";

export class SchemaCompatibilityError extends Error {
  constructor(
    public readonly code: SchemaCompatibilityErrorCode,
    public readonly databaseVersion: number | null,
    public readonly supportedMin: number,
    public readonly supportedMax: number,
    message: string,
  ) {
    super(message);
    this.name = "SchemaCompatibilityError";
  }
}

function readSchemaVersion(): number | null {
  const row = getDb().prepare(`SELECT MAX(version) AS version FROM _schema_version`).get() as
    { version: number | null } | undefined;
  return row?.version ?? null;
}

export function assertSupportedSchemaVersion(): void {
  if (checked) return;
  const version = readSchemaVersion();
  const { min, max } = SUPPORTED_SCHEMA_VERSIONS;
  if (version === null) {
    throw new SchemaCompatibilityError(
      "schema_version_missing",
      version,
      min,
      max,
      "SiteCMD database has no schema version; open SiteCMD once so it can migrate.",
    );
  }
  if (version > max) {
    throw new SchemaCompatibilityError(
      "schema_too_new",
      version,
      min,
      max,
      `SiteCMD database schema version ${version} is newer than this MCP server supports (max ${max}); open SiteCMD so it refreshes the bundled MCP server, then reconnect your agent.`,
    );
  }
  if (version < min) {
    throw new SchemaCompatibilityError(
      "schema_too_old",
      version,
      min,
      max,
      `SiteCMD database schema version ${version} is older than this MCP server supports (min ${min}); open SiteCMD so it migrates the database.`,
    );
  }
  checked = true;
}
