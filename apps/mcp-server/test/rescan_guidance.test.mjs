import test from "node:test";
import assert from "node:assert/strict";

import { connectInMemory } from "./tools_list_snapshot.test.mjs";

test("how_to_rescan tells the agent about sitecmd init, --url, and the desktop path", async () => {
  const session = await connectInMemory();
  try {
    const { content } = await session.client.callTool({
      name: "how_to_rescan",
      arguments: { url: "https://guide.test" },
    });
    const output = content[0].text;
    assert.match(output, /sitecmd init https:\/\/guide\.test/);
    assert.match(output, /sitecmd scan --url https:\/\/guide\.test/);
    assert.match(output, /does not queue a scan/);
    assert.match(output, /compare_scans/);
    const { tools } = await session.client.listTools();
    const alias = tools.find((tool) => tool.name === "request_scan");
    assert.ok(alias, "request_scan stays registered until the next major release");
    assert.match(alias.description, /^Deprecated: call how_to_rescan/);
  } finally {
    await session.close();
  }
});
