#!/usr/bin/env node

/** Process entry: the health probe the desktop runs, or the stdio transport. */

import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { getDb } from "./db_connection.js";
import { createSiteCmdServer } from "./server.js";

async function main() {
  if (process.argv.includes("--sitecmd-health-check")) {
    const row = getDb()
      .prepare(
        `SELECT COUNT(*) AS table_count
         FROM sqlite_schema
         WHERE type = 'table'
           AND name IN ('_schema_version', 'projects', 'work_items', 'fix_attempts')`,
      )
      .get() as { table_count?: number } | undefined;
    if (row?.table_count !== 4) throw new Error("SiteCMD database schema health query failed");
    process.stdout.write(`${JSON.stringify({ marker: "SITECMD_MCP_HEALTH_V1", ok: true })}\n`);
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
