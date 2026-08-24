import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";

import { createSiteCmdServer } from "../dist/server.js";

const SNAPSHOT_PATH = join(import.meta.dirname, "fixtures", "tools-list.snapshot.json");

export async function connectInMemory() {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const server = createSiteCmdServer();
  const client = new Client({ name: "sitecmd-mcp-test", version: "0.0.0" });
  await Promise.all([server.connect(serverTransport), client.connect(clientTransport)]);
  return {
    client,
    async close() {
      await client.close();
      await server.close();
    },
  };
}

async function listTools() {
  const session = await connectInMemory();
  try {
    const { tools } = await session.client.listTools();
    return tools
      .map(({ name, title, description, inputSchema, annotations }) => ({
        name,
        title,
        description,
        inputSchema,
        annotations,
      }))
      .sort((a, b) => a.name.localeCompare(b.name));
  } finally {
    await session.close();
  }
}

test("tools/list matches the committed snapshot", async () => {
  const actual = `${JSON.stringify(await listTools(), null, 2)}\n`;
  if (process.env.UPDATE_MCP_SNAPSHOT === "1") {
    mkdirSync(dirname(SNAPSHOT_PATH), { recursive: true });
    writeFileSync(SNAPSHOT_PATH, actual);
  }
  assert.ok(existsSync(SNAPSHOT_PATH), "snapshot missing; run UPDATE_MCP_SNAPSHOT=1 pnpm test:mcp");
  assert.equal(
    actual,
    readFileSync(SNAPSHOT_PATH, "utf8"),
    "tool names, descriptions, and schemas are downstream public API; run UPDATE_MCP_SNAPSHOT=1 pnpm test:mcp to accept a deliberate change",
  );
});
