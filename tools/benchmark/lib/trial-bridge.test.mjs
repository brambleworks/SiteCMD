import assert from "node:assert/strict";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { bridgeRequest } from "../guest/bridge-client.mjs";
import { createTrialBridge } from "../guest/trial-bridge.mjs";

test("baseline has an explicit submission boundary but no SiteCMD access", async () => {
  const channel = path.join(mkdtempSync(path.join(tmpdir(), "scb-")), "channel");
  const bridge = await createTrialBridge({
    channel,
    arm: "normal",
    submit: async (summary) => ({ summary }),
  });
  try {
    assert.deepEqual(await bridgeRequest(channel, "/submit", { summary: "No changes needed" }), {
      summary: "No changes needed",
    });
    await assert.rejects(
      bridgeRequest(channel, "/mcp", { method: "tools/list" }),
      /does not expose/,
    );
  } finally {
    await bridge.close();
  }
});

test("verification snapshots precede real MCP forwarding and replies remain unchanged", async () => {
  const order = [];
  const channel = path.join(mkdtempSync(path.join(tmpdir(), "scb-")), "channel");
  const expected = { content: [{ type: "text", text: "Verification requested" }] };
  const bridge = await createTrialBridge({
    channel,
    arm: "mcp",
    submit: async () => order.push("snapshot"),
    mcp: {
      request: async () => {
        order.push("server");
        return expected;
      },
    },
  });
  try {
    const result = await bridgeRequest(channel, "/mcp", {
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: {
        name: "request_verification",
        arguments: { attempt_id: 1, summary: "Fixed origin validation" },
      },
    });
    assert.deepEqual(order, ["snapshot", "server"]);
    assert.deepEqual(result, expected);
  } finally {
    await bridge.close();
  }
});
