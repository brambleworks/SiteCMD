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

export function renderMarkdown({ perArm, arms, targets, config, stamp, dryRun = false }) {
  if (dryRun || targets.some((target) => target.runs.some((run) => run.fix?.dryRun))) {
    return "# Context-efficiency benchmark\n\nDry run only. No agent calls or repair outcomes were measured. Not evidence for product claims.\n";
  }
  const ref = perArm.brief; // brief is the reference arm
  const lines = [];
  lines.push("# Context-efficiency benchmark");
  lines.push("");
  lines.push(
    "Exploratory scanner-clearance results only, not independently verified repairs or evidence for marketing claims. Metric-less failures are excluded from these legacy aggregates; use the paired workflow benchmark for complete accounting.",
  );
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
    "| Arm | Runs | Findings cleared | Clearance rate | New findings | Turns | Tokens | Estimated cost (USD) | Wall (s) |",
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

  lines.push("## Exploratory compute per scanner finding cleared");
  lines.push("");
  lines.push("| Arm | Tokens / finding cleared | Cost / finding cleared | vs brief (cost) |");
  lines.push("| --- | --- | --- | --- |");
  for (const arm of arms) {
    const a = perArm[arm];
    let tpr, cpr, vs;
    if (a.costPerResolved === null) {
      tpr = "n/a";
      cpr = "n/a";
      vs = "n/a";
    } else {
      tpr = fmtInt(a.tokensPerResolved);
      cpr = `$${fmt(a.costPerResolved, 4)}`;
      const r = ratio(a.costPerResolved, ref ? ref.costPerResolved : null);
      vs = arm === "brief" ? "1.00x (ref)" : r == null ? "n/a" : `${fmt(r, 2)}x`;
    }
    lines.push(`| ${ARM_LABEL[arm] || arm} | ${tpr} | ${cpr} | ${vs} |`);
  }
  lines.push("");
  lines.push("> `vs brief (cost)` compares pooled spending per scanner finding cleared.");
  lines.push(
    "> Arms may clear different findings; this is not a paired comparison of the same repair.",
  );
  lines.push("> No clearances produce n/a, not an infinite savings or break-even claim.");
  lines.push("");

  if (arms.some((arm) => perArm[arm].isConverge)) {
    lines.push("## Convergence (cost to clear baseline scanner findings)");
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
      "> `done` = cleared every baseline finding. `stalled` = stopped making progress. `capped` = hit the round backstop.",
    );
    lines.push(
      "> Total cost here is summed across all rounds: the real spend to get the repo to that state.",
    );
    lines.push("");
  }

  lines.push("## Fairness notes");
  lines.push("");
  lines.push(
    "- Arms use fresh copies of the same repository and model, but prompts provide different discovery help. They do not measure the MCP workflow.",
  );
  lines.push(
    "- Clearance means a scanner checkId disappeared. Renames, suppressions, or broken functionality can clear findings without repairing a defect.",
  );
  lines.push(
    "- Creating the brief uses no model tokens, but the model consumes tokens when reading it. Scanner wall-time is recorded separately in the raw JSON.",
  );
  lines.push("");
  return lines.join("\n");
}
