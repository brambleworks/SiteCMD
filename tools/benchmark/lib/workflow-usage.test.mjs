import assert from "node:assert/strict";
import { test } from "node:test";
import { claudeUsage, codexUsage, totalTokens, validateUsage } from "./workflow-usage.mjs";

test("Claude final modelUsage covers the agent tree and all cache categories", () => {
  const model = {
    inputTokens: 10,
    outputTokens: 20,
    cacheReadInputTokens: 30,
    cacheCreationInputTokens: 40,
  };
  const usage = claudeUsage({
    modelUsage: { parent: model, child: model },
    usage: { input_tokens: 999 },
    total_cost_usd: 0.2,
  });
  assert.equal(totalTokens(validateUsage(usage)), 200);
  assert.equal(usage.inputTokens, 20);
  assert.equal(usage.costBasis, "estimated");
  assert.equal(usage.costUsd, 0.2);
});

test("root-only usage cannot silently stand in for whole-tree usage", () => {
  const result = {
    usage: {
      input_tokens: 1,
      output_tokens: 2,
      cache_read_input_tokens: 3,
      cache_creation_input_tokens: 0,
    },
  };
  assert.equal(totalTokens(claudeUsage(result)), null);
  assert.equal(totalTokens(claudeUsage(result, { noSubagents: true })), 6);
  assert.equal(claudeUsage(result).costUsd, null);
});

test("missing, malformed, and negative counts remain unknown rather than zero", () => {
  for (const modelUsage of [{ child: null }, { child: { inputTokens: -1 } }, []]) {
    const usage = claudeUsage({ modelUsage });
    assert.equal(totalTokens(validateUsage(usage)), null);
  }
  const usage = claudeUsage({});
  assert.equal(usage.costBasis, "unknown");
  assert.throws(() => validateUsage({ ...usage, costUsd: 1 }), /disagree/);
  assert.throws(() => validateUsage({ ...usage, inputTokens: NaN }), /non-negative/);
});

test("Codex cached input is a subset of input and reasoning is a subset of output", () => {
  const event = {
    type: "turn.completed",
    usage: {
      input_tokens: 100,
      cached_input_tokens: 80,
      output_tokens: 30,
      reasoning_output_tokens: 20,
    },
  };
  const usage = codexUsage(event, { noSubagents: true, billingMode: "subscription" });
  assert.equal(usage.inputTokens, 20);
  assert.equal(usage.cacheReadTokens, 80);
  assert.equal(totalTokens(validateUsage(usage)), 130);
  assert.equal(usage.costBasis, "subscription");
  assert.equal(usage.incrementalCostUsd, null);
  assert.equal(totalTokens(codexUsage(event)), null);
  event.usage.cached_input_tokens = 101;
  assert.equal(totalTokens(codexUsage(event, { noSubagents: true })), null);
  assert.throws(() => codexUsage({ type: "item.completed" }), /turn.completed/);
});

test("subscription normalization preserves estimates without assuming zero extra charges", () => {
  const usage = claudeUsage({ total_cost_usd: 4 }, { billingMode: "subscription" });
  validateUsage(usage);
  assert.equal(usage.costUsd, null);
  assert.equal(usage.apiEquivalentCostUsd, 4);
  assert.equal(usage.incrementalCostUsd, null);
  assert.equal(
    claudeUsage({}, { billingMode: "subscription", incrementalCostUsd: 0 }).incrementalCostUsd,
    0,
  );
  assert.throws(() => claudeUsage({}, { billingMode: "fallback" }), /billing mode/);
});
