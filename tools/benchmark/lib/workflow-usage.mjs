import { requireCondition, requireNumber } from "./workflow-contract.mjs";

const TOKEN_FIELDS = ["inputTokens", "outputTokens", "cacheReadTokens", "cacheWriteTokens"];

export function validateUsage(usage) {
  requireCondition(
    usage !== null && typeof usage === "object",
    "usage is required, even when unknown",
  );
  for (const field of TOKEN_FIELDS) {
    if (usage[field] !== null) requireNumber(usage[field], field, { integer: true });
  }
  if (usage.costUsd !== null) requireNumber(usage.costUsd, "costUsd");
  requireCondition(
    ["estimated", "billed", "unknown", "subscription"].includes(usage.costBasis),
    "invalid cost basis",
  );
  if (usage.costBasis === "subscription") {
    requireCondition(usage.costUsd === null, "subscription allowance has no per-trial dollar cost");
    for (const field of ["incrementalCostUsd", "apiEquivalentCostUsd"]) {
      if (usage[field] !== null) requireNumber(usage[field], field);
    }
  } else {
    requireCondition(
      usage.costUsd === null ? usage.costBasis === "unknown" : usage.costBasis !== "unknown",
      "cost amount and basis disagree",
    );
  }
  requireCondition(
    typeof usage.includesAllAgents === "boolean",
    "whole-agent-tree accounting status is required",
  );
  requireCondition(
    typeof usage.receipt === "string" && usage.receipt.length > 0,
    "usage receipt artifact is required",
  );
  return usage;
}

export function totalTokens(usage) {
  if (!usage.includesAllAgents || TOKEN_FIELDS.some((field) => usage[field] === null)) return null;
  return TOKEN_FIELDS.reduce((sum, field) => sum + usage[field], 0);
}

export function accountedSpend(usage) {
  return usage.costBasis === "subscription" ? usage.incrementalCostUsd : usage.costUsd;
}

function tokenCount(value) {
  return Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function amount(value) {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}

function costFields(billingMode, estimate, incrementalCostUsd) {
  requireCondition(["api", "subscription"].includes(billingMode), "invalid billing mode");
  if (billingMode === "subscription") {
    return {
      costUsd: null,
      costBasis: "subscription",
      incrementalCostUsd: amount(incrementalCostUsd),
      apiEquivalentCostUsd: estimate,
    };
  }
  return { costUsd: estimate, costBasis: estimate === null ? "unknown" : "estimated" };
}

/** Normalize one completed Codex turn; the caller accounts for other turns and agents. */
export function codexUsage(
  event,
  {
    noSubagents = false,
    receipt = "usage.json",
    billingMode = "api",
    incrementalCostUsd = null,
  } = {},
) {
  requireCondition(event?.type === "turn.completed", "expected one Codex turn.completed event");
  const usage = event.usage ?? {};
  const input = tokenCount(usage.input_tokens);
  const cached = tokenCount(usage.cached_input_tokens);
  return {
    inputTokens: input !== null && cached !== null && cached <= input ? input - cached : null,
    outputTokens: tokenCount(usage.output_tokens),
    cacheReadTokens: cached,
    cacheWriteTokens: 0,
    includesAllAgents: noSubagents,
    ...costFields(billingMode, null, incrementalCostUsd),
    receipt,
  };
}

/** Normalize one final Claude result; never sum cumulative result messages. */
export function claudeUsage(
  result,
  {
    noSubagents = false,
    receipt = "usage.json",
    billingMode = "api",
    incrementalCostUsd = null,
  } = {},
) {
  const models =
    result.modelUsage && !Array.isArray(result.modelUsage) && typeof result.modelUsage === "object"
      ? Object.values(result.modelUsage)
      : [];
  const sumModelField = (field) => {
    const values = models.map((model) => tokenCount(model?.[field]));
    return values.some((value) => value === null)
      ? null
      : values.reduce((sum, value) => sum + value, 0);
  };
  const usage = result.usage ?? {};
  const costUsd = amount(result.total_cost_usd);
  return {
    inputTokens: models.length ? sumModelField("inputTokens") : tokenCount(usage.input_tokens),
    outputTokens: models.length ? sumModelField("outputTokens") : tokenCount(usage.output_tokens),
    cacheReadTokens: models.length
      ? sumModelField("cacheReadInputTokens")
      : tokenCount(usage.cache_read_input_tokens),
    cacheWriteTokens: models.length
      ? sumModelField("cacheCreationInputTokens")
      : tokenCount(usage.cache_creation_input_tokens),
    includesAllAgents: models.length > 0 || noSubagents,
    ...costFields(billingMode, costUsd, incrementalCostUsd),
    receipt,
  };
}
