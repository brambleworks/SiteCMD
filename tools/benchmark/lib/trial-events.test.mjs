import assert from "node:assert/strict";
import { test } from "node:test";
import { watchProviderEvents } from "../guest/trial-events.mjs";

test("provider event observation preserves split UTF-8 and rejects model fallback", () => {
  const failures = [];
  const observer = watchProviderEvents("claude-opus-5", (reason) => failures.push(reason));
  const raw = Buffer.from(
    `${JSON.stringify({ type: "system", subtype: "init", model: "claude-opus-5", name: "café" })}\n`,
  );
  const split = raw.indexOf(Buffer.from("é")) + 1;
  observer.write(raw.subarray(0, split));
  observer.write(raw.subarray(split));
  assert.deepEqual(observer.models(), ["claude-opus-5"]);
  assert.deepEqual(failures, []);
  observer.write(Buffer.from('{"type":"assistant","message":{"model":"another-model"}}\n'));
  assert.match(failures[0], /model differs/);
});

test("rate limits stop execution and an absent model remains unknown", () => {
  const failures = [];
  const observer = watchProviderEvents("gpt-5.6-sol", (reason) => failures.push(reason));
  observer.write(Buffer.from('{"type":"turn.completed","usage":{"input_tokens":1}}\n'));
  assert.deepEqual(observer.models(), []);
  observer.write(
    Buffer.from('{"type":"rate_limit_event","rate_limit_info":{"status":"rejected"}}\n'),
  );
  assert.match(failures[0], /rate limit/);
});
