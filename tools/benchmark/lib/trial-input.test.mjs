import assert from "node:assert/strict";
import { test } from "node:test";
import { trialInput } from "../guest/trial-input.mjs";

test("Claude receives no task until initialization files have been captured", () => {
  const writes = [];
  const order = [];
  const input = trialInput({
    agent: "claude",
    prompt: "Repair the case",
    stdin: {
      write: (value) => writes.push(JSON.parse(value)),
      end: (value) => {
        order.push("prompt");
        writes.push(JSON.parse(value));
      },
    },
    initialized: () => order.push("snapshot"),
    fail: assert.fail,
  });
  assert.equal(writes.length, 1);
  assert.equal(writes[0].request.subtype, "initialize");
  const event =
    JSON.stringify({
      type: "control_response",
      response: { subtype: "success", request_id: writes[0].request_id },
    }) + "\n";
  input.write(Buffer.from(event.slice(0, 20)));
  assert.equal(writes.length, 1);
  input.write(Buffer.from(event.slice(20)));
  assert.deepEqual(order, ["snapshot", "prompt"]);
  assert.equal(writes[1].message.content, "Repair the case");
  input.write(Buffer.from(event));
  assert.equal(writes.length, 2);
});

test("failed initialization never sends a task", () => {
  for (const rejected of [false, true]) {
    const failures = [];
    const input = trialInput({
      agent: "claude",
      prompt: "Never send",
      stdin: { write() {}, end: assert.fail },
      initialized: () => {
        throw new Error("Unsafe initialization");
      },
      fail: (reason) => failures.push(reason),
    });
    input.write(
      Buffer.from(
        JSON.stringify({
          type: "control_response",
          response: {
            request_id: "benchmark-initialize",
            subtype: rejected ? "error" : "success",
            error: "Client refused initialization",
          },
        }) + "\n",
      ),
    );
    assert.match(failures[0], /initialization/);
  }
});
