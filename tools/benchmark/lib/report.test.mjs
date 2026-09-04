import { test } from "node:test";
import assert from "node:assert/strict";
import { aggregate, renderMarkdown } from "./report.mjs";
import { decideConvergence } from "./converge.mjs";

const cfg = { baselineCount: 11, stallRounds: 2, maxRounds: 6 };

test("convergence: stops 'done' when all baseline issues resolved", () => {
  assert.deepEqual(decideConvergence([5, 11], cfg), { stop: true, status: "done" });
});

test("convergence: stops 'stalled' after no progress across the window", () => {
  assert.deepEqual(decideConvergence([3, 3, 3], cfg), { stop: true, status: "stalled" });
});

test("convergence: keeps going while still making progress", () => {
  assert.deepEqual(decideConvergence([2, 4], cfg), { stop: false, status: "running" });
});

test("convergence: stops 'capped' at the round backstop", () => {
  assert.deepEqual(decideConvergence([1, 2, 3, 4, 5, 6], cfg), { stop: true, status: "capped" });
});

test("convergence: a flat-zero arm stalls instead of looping forever", () => {
  assert.equal(decideConvergence([0, 0, 0], cfg).status, "stalled");
});

function run(arm, { resolved, regressions, baseline, fix }) {
  return {
    arm,
    diff: {
      resolvedCount: resolved,
      regressionCount: regressions,
      baselineCount: baseline,
      resolutionRate: baseline ? resolved / baseline : 0,
    },
    fix,
  };
}

const measured = (over) => ({
  ok: true,
  isError: false,
  totalTokens: 1000,
  costUsd: 1,
  numTurns: 10,
  durationMs: 1000,
  ...over,
});

test("a turn-capped run is still counted and flagged incomplete", () => {
  const targets = [
    {
      runs: [
        run("categories", {
          resolved: 0,
          regressions: 0,
          baseline: 11,
          fix: measured({ ok: false, isError: true, subtype: "error_max_turns", costUsd: 5 }),
        }),
      ],
    },
  ];
  const perArm = aggregate(targets, ["categories"]);
  assert.equal(perArm.categories.n, 1, "turn-capped run must be included");
  assert.equal(perArm.categories.incomplete, 1, "must be flagged incomplete");
  assert.equal(perArm.categories.meanCost, 5);
});

test("a spawn/parse failure with no metrics is excluded", () => {
  const targets = [
    {
      runs: [
        {
          arm: "blind",
          diff: { resolvedCount: 0, regressionCount: 0, baselineCount: 5, resolutionRate: 0 },
          fix: { ok: false, error: "spawn failed" },
        },
      ],
    },
  ];
  const perArm = aggregate(targets, ["blind"]);
  assert.equal(perArm.blind.n, 0, "metric-less failures must not count");
});

test("zero-resolved arm yields null per-issue cost, not a crash", () => {
  const targets = [
    {
      runs: [
        run("blind", {
          resolved: 0,
          regressions: 0,
          baseline: 11,
          fix: measured({ costUsd: 3.5 }),
        }),
      ],
    },
  ];
  const perArm = aggregate(targets, ["blind"]);
  assert.equal(perArm.blind.costPerResolved, null);
  assert.equal(perArm.blind.meanCost, 3.5);
});

test("per-issue cost pools across runs (total cost / total resolved)", () => {
  const targets = [
    {
      runs: [
        run("brief", {
          resolved: 5,
          regressions: 1,
          baseline: 11,
          fix: measured({ costUsd: 3, totalTokens: 3000 }),
        }),
        run("brief", {
          resolved: 5,
          regressions: 0,
          baseline: 11,
          fix: measured({ costUsd: 3, totalTokens: 3000 }),
        }),
      ],
    },
  ];
  const perArm = aggregate(targets, ["brief"]);
  assert.equal(perArm.brief.costPerResolved, 0.6);
  assert.equal(perArm.brief.tokensPerResolved, 600);
});

test("legacy dry runs never render efficiency or break-even claims", () => {
  const report = renderMarkdown({ targets: [], dryRun: true });
  assert.match(report, /No agent calls or repair outcomes were measured/);
  assert.doesNotMatch(report, /never breaks even|actually fixed|Per-arm results/);
});

test("legacy measured results explicitly limit scanner-clearance interpretations", () => {
  const targets = [
    {
      name: "fixture",
      sha: "a".repeat(40),
      baselineCount: 11,
      runs: [run("brief", { resolved: 0, regressions: 0, baseline: 11, fix: measured({}) })],
    },
  ];
  const report = renderMarkdown({
    perArm: aggregate(targets, ["brief"]),
    arms: ["brief"],
    targets,
    config: { model: "fixture", repeats: 1, maxTurns: 1 },
    stamp: "fixture",
  });
  assert.match(report, /not independently verified repairs/);
  assert.match(report, /model consumes tokens when reading it/);
  assert.doesNotMatch(report, /never breaks even|actually fixed|to fix the same issue/);
});
