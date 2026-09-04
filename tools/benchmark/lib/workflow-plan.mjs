import { createHash } from "node:crypto";
import { validateStudy } from "./workflow-contract.mjs";

export function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

export function digest(value) {
  return createHash("sha256")
    .update(typeof value === "string" || Buffer.isBuffer(value) ? value : canonicalJson(value))
    .digest("hex");
}

export function seededRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state += 0x6d2b79f5;
    let value = Math.imul(state ^ (state >>> 15), 1 | state);
    value ^= value + Math.imul(value ^ (value >>> 7), 61 | value);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

function shuffled(items, random) {
  const result = [...items];
  for (let index = result.length - 1; index > 0; index--) {
    const other = Math.floor(random() * (index + 1));
    [result[index], result[other]] = [result[other], result[index]];
  }
  return result;
}

export function createPlan(study) {
  validateStudy(study);
  const random = seededRandom(study.seed);
  const studySha256 = digest(study);
  const blocks = [];
  for (const task of study.tasks) {
    for (const configuration of study.configurations) {
      for (let repeat = 0; repeat < study.repeats; repeat++) {
        blocks.push({ task: task.id, configuration: configuration.id, repeat });
      }
    }
  }
  const assignments = shuffled(blocks, random).flatMap((block) =>
    shuffled(study.arms, random).map((arm) => ({
      id: digest({ studySha256, ...block, arm }).slice(0, 24),
      ...block,
      arm,
    })),
  );
  return {
    schemaVersion: 1,
    study,
    studySha256,
    assignments,
    plannedTrials: assignments.length,
    maximumConfiguredSpendUsd: assignments.length * study.limits.trialCostUsd,
  };
}

export function validatePlan(plan) {
  const expected = createPlan(plan.study);
  if (
    plan.schemaVersion !== 1 ||
    plan.studySha256 !== expected.studySha256 ||
    canonicalJson(plan.assignments) !== canonicalJson(expected.assignments) ||
    plan.plannedTrials !== expected.plannedTrials ||
    plan.maximumConfiguredSpendUsd !== expected.maximumConfiguredSpendUsd
  ) {
    throw new Error("Frozen study or assignments changed; create a new study instead");
  }
  return plan;
}
