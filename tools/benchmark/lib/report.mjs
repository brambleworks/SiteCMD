const ARM_LABEL = {
  blind: "Blind (no scanner output)",
  categories: "Counts + categories",
  brief: "SiteCMD brief (file:line + fix)",
};

function mean(nums) {
  if (nums.length === 0) return 0;
  return nums.reduce((a, b) => a + b, 0) / nums.length;
}

function runsForArm(targets, arm) {
  const out = [];
  for (const t of targets) {
    for (const r of t.runs) {
      if (r.arm !== arm) continue;
      if (r.diff && r.fix && typeof r.fix.totalTokens === "number") out.push(r);
    }
  }
  return out;
}

function isIncomplete(run) {
  return Boolean(run.fix.isError) || run.fix.subtype === "error_max_turns";
}

export function aggregate(targets, arms) {
  const perArm = {};
  for (const arm of arms) {
    const runs = runsForArm(targets, arm);
    const resolved = runs.map((r) => r.diff.resolvedCount);
    const regressions = runs.map((r) => r.diff.regressionCount);
    const rate = runs.map((r) => r.diff.resolutionRate);
    const tokens = runs.map((r) => r.fix.totalTokens);
    const cost = runs.map((r) => r.fix.costUsd);
    const turns = runs.map((r) => r.fix.numTurns);
    const dur = runs.map((r) => r.fix.durationMs);
    const totalResolved = resolved.reduce((a, b) => a + b, 0);
    const totalTokens = tokens.reduce((a, b) => a + b, 0);
    const totalCost = cost.reduce((a, b) => a + b, 0);
    const outcomes = {};
    for (const r of runs) {
      if (r.converged) outcomes[r.converged] = (outcomes[r.converged] || 0) + 1;
    }
    perArm[arm] = {
      n: runs.length,
      incomplete: runs.filter(isIncomplete).length,
      isConverge: runs.some((r) => Boolean(r.converged)),
      meanRounds: mean(runs.map((r) => r.totalRounds || 1)),
      outcomes,
      meanResolved: mean(resolved),
      meanRegressions: mean(regressions),
      meanResolutionRate: mean(rate),
      meanTokens: mean(tokens),
      meanCost: mean(cost),
      meanTurns: mean(turns),
      meanDurationMs: mean(dur),
      tokensPerResolved: totalResolved ? totalTokens / totalResolved : null,
      costPerResolved: totalResolved ? totalCost / totalResolved : null,
    };
  }
  return perArm;
}

function ratio(a, b) {
  if (a == null || b == null || b === 0) return null;
  return a / b;
}

function fmt(n, digits = 2) {
  if (n == null) return "n/a";
  return Number(n).toFixed(digits);
}

function fmtInt(n) {
  if (n == null) return "n/a";
  return Math.round(n).toLocaleString("en-US");
}

export function renderMarkdown({ perArm, arms, targets, config, stamp }) {
  const ref = perArm.brief; // brief is the reference arm
  const lines = [];
  lines.push("# Context-efficiency benchmark");
  lines.push("");
  lines.push(`Generated: ${stamp}`);
  lines.push(
    `Model: \`${config.model}\` | repeats: ${config.repeats} | max turns: ${config.maxTurns}`,
  );
  lines.push("");

  lines.push("## Targets");
  lines.push("");
  lines.push("| Repo | Commit | Baseline issues |");
  lines.push("| --- | --- | --- |");
  for (const t of targets) {
    lines.push(`| ${t.name} | \`${(t.sha || "").slice(0, 10)}\` | ${t.baselineCount} |`);
  }
  lines.push("");

  lines.push("## Per-arm results");
  lines.push("");
  lines.push(
    "| Arm | Runs | Resolved | Resolution rate | Regressions | Turns | Tokens | Cost (USD) | Wall (s) |",
  );
  lines.push("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
  for (const arm of arms) {
    const a = perArm[arm];
    const cap = a.incomplete > 0 ? " *" : "";
    lines.push(
      `| ${ARM_LABEL[arm] || arm} | ${a.n} | ${fmt(a.meanResolved, 1)} | ${fmt(
        a.meanResolutionRate * 100,
        0,
      )}% | ${fmt(a.meanRegressions, 1)} | ${fmt(a.meanTurns, 1)}${cap} | ${fmtInt(
        a.meanTokens,
      )} | ${fmt(a.meanCost, 3)} | ${fmt(a.meanDurationMs / 1000, 1)} |`,
    );
  }
  lines.push("");
  if (arms.some((arm) => perArm[arm].incomplete > 0)) {
    lines.push(
      "> `*` = one or more runs hit the turn cap before finishing. Their numbers are a floor: more turns would mean more spend, not necessarily more fixes.",
    );
    lines.push("");
  }

  lines.push("## Efficiency: compute per issue actually fixed");
  lines.push("");
  lines.push("| Arm | Tokens / resolved issue | Cost / resolved issue | vs brief (cost) |");
  lines.push("| --- | --- | --- | --- |");
  for (const arm of arms) {
    const a = perArm[arm];
    let tpr, cpr, vs;
    if (!a.costPerResolved) {
      tpr = "0 fixed";
      cpr = `$${fmt(a.meanCost, 2)} spent, 0 fixed`;
      vs = "never breaks even";
    } else {
      tpr = fmtInt(a.tokensPerResolved);
      cpr = `$${fmt(a.costPerResolved, 4)}`;
      const r = ratio(a.costPerResolved, ref ? ref.costPerResolved : null);
      vs = arm === "brief" ? "1.00x (ref)" : r == null ? "n/a" : `${fmt(r, 2)}x`;
    }
    lines.push(`| ${ARM_LABEL[arm] || arm} | ${tpr} | ${cpr} | ${vs} |`);
  }
  lines.push("");
  lines.push("> `vs brief (cost)` is cost-per-resolved-issue relative to the brief arm.");
  lines.push("> A value of 2.0x means that arm spent twice as much to fix the same issue.");
  lines.push(
    "> `never breaks even` = the arm spent real money but resolved zero scanner findings.",
  );
  lines.push("");

  if (arms.some((arm) => perArm[arm].isConverge)) {
    lines.push("## Convergence (cost to reach zero issues)");
    lines.push("");
    lines.push("| Arm | Outcome | Mean rounds | Final resolution | Total turns | Total cost |");
    lines.push("| --- | --- | --- | --- | --- | --- |");
    for (const arm of arms) {
      const a = perArm[arm];
      const outcome =
        Object.entries(a.outcomes)
          .sort((x, y) => y[1] - x[1])
          .map(([k, v]) => `${k} x${v}`)
          .join(", ") || "n/a";
      lines.push(
        `| ${ARM_LABEL[arm] || arm} | ${outcome} | ${fmt(a.meanRounds, 1)} | ${fmt(
          a.meanResolutionRate * 100,
          0,
        )}% | ${fmt(a.meanTurns, 0)} | $${fmt(a.meanCost, 2)} |`,
      );
    }
    lines.push("");
    lines.push(
      "> `done` = resolved every baseline issue. `stalled` = stopped making progress. `capped` = hit the round backstop.",
    );
    lines.push(
      "> Total cost here is summed across all rounds: the real spend to get the repo to that state.",
    );
    lines.push("");
  }

  lines.push("## Fairness notes");
  lines.push("");
  lines.push(
    "- All arms run the SAME task on a FRESH copy of the SAME repo with the SAME model and turn cap; only context richness differs.",
  );
  lines.push(
    "- Resolution is verified by re-running the scanner and diffing `checkId` sets, not by trusting the agent's self-report.",
  );
  lines.push(
    "- The brief is produced by a deterministic scanner (no LLM tokens), so the brief arm is not charged model tokens for its context. Scanner wall-time is reported separately in the raw JSON.",
  );
  lines.push("");
  return lines.join("\n");
}
