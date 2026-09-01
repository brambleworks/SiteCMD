#!/usr/bin/env node

/** Process entry: the health probe the desktop runs, or the stdio transport. */

import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { getDb, isSiteCmdDatabaseNotFoundError } from "./db_connection.js";
import { assertSupportedSchemaVersion, SchemaCompatibilityError } from "./schema_version.js";
import { createSiteCmdServer } from "./server.js";

const HEALTH_MARKER = "SITECMD_MCP_HEALTH_V1";

class InvalidSiteCmdDatabaseError extends Error {}

function writeHealthResult(result: Record<string, unknown>) {
  process.stdout.write(`${JSON.stringify({ marker: HEALTH_MARKER, ...result })}\n`);
}

function writeHealthFailure(error: unknown) {
  if (error instanceof SchemaCompatibilityError) {
    writeHealthResult({
      ok: false,
      errorCode: error.code,
      databaseVersion: error.databaseVersion,
      supportedMin: error.supportedMin,
      supportedMax: error.supportedMax,
    });
  } else if (isSiteCmdDatabaseNotFoundError(error)) {
    writeHealthResult({ ok: false, errorCode: "database_not_found" });
  } else if (error instanceof InvalidSiteCmdDatabaseError) {
    writeHealthResult({ ok: false, errorCode: "invalid_database" });
  } else {
    writeHealthResult({ ok: false, errorCode: "database_unavailable" });
  }
}

function runHealthCheck() {
  try {
    const row = getDb()
      .prepare(
        `SELECT COUNT(*) AS table_count
         FROM sqlite_schema
         WHERE type = 'table'
           AND name IN ('_schema_version', 'projects', 'work_items', 'fix_attempts')`,
      )
      .get() as { table_count?: number } | undefined;
    if (row?.table_count !== 4) {
      throw new InvalidSiteCmdDatabaseError("SiteCMD database schema health query failed");
    }
    assertSupportedSchemaVersion();
    writeHealthResult({ ok: true });
  } catch (error) {
    writeHealthFailure(error);
    throw error;
  }
}

async function main() {
  if (process.argv.includes("--sitecmd-health-check")) {
    runHealthCheck();
    return;
  }
  const transport = new StdioServerTransport();
  await createSiteCmdServer().connect(transport);
  process.stderr.write("SiteCMD MCP server running on stdio\n");
}

main().catch((e) => {
  process.stderr.write(`Fatal: ${e}\n`);
  process.exit(1);
});
