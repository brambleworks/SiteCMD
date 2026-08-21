import { scanJson, scanReview, diffScans, categoryCounts } from "./scanner.mjs";
import { runClaudeFix } from "./claude.mjs";
import { buildPrompt, continuationPrompt } from "./arms.mjs";

/** Decide whether cumulative resolution history should stop another round. */
export function decideConvergence(history, { baselineCount, stallRounds, maxRounds }) {
  const last = history[history.length - 1] ?? 0;
  if (last >= baselineCount) return { stop: true, status: "done" };
  if (history.length >= maxRounds) return { stop: true, status: "capped" };
  if (history.length > stallRounds) {
    const window = history.slice(-(stallRounds + 1));
    if (window[window.length - 1] <= window[0]) return { stop: true, status: "stalled" };
  }
  return { stop: false, status: "running" };
}

function sumFix(fixes) {
  const acc = {
    ok: true,
    isError: false,
    totalTokens: 0,
    costUsd: 0,
    numTurns: 0,
    durationMs: 0,
    wallMs: 0,
    inputTokens: 0,
    outputTokens: 0,
    cacheCreate: 0,
    cacheRead: 0,
  };
  for (const f of fixes) {
    acc.totalTokens += f.totalTokens || 0;
    acc.costUsd += f.costUsd || 0;
    acc.numTurns += f.numTurns || 0;
    acc.durationMs += f.durationMs || 0;
    acc.wallMs += f.wallMs || 0;
    acc.inputTokens += f.inputTokens || 0;
    acc.outputTokens += f.outputTokens || 0;
    acc.cacheCreate += f.cacheCreate || 0;
    acc.cacheRead += f.cacheRead || 0;
  }
  return acc;
}

export function runConverge({
  arm,
  copy,
  baseline,
  reviewText,
  scanner,
  model,
  perRoundTurns,
  stallRounds,
  maxRounds,
  log,
}) {
  const rounds = [];
  const history = [];
  let status;
  let lastPost = null;

  for (let k = 0; ; k++) {
    let prompt;
    if (k === 0) {
      prompt = buildPrompt(arm, { baseline, reviewText });
    } else {
      const diff = diffScans(baseline, lastPost);
      const remaining = diff.unresolvedCount;
      const ctx = { remaining, counts: categoryCounts(lastPost) };
      if (arm === "brief") ctx.reviewText = scanReview(scanner, copy);
      prompt = continuationPrompt(arm, ctx);
    }

    const fix = runClaudeFix({ prompt, cwd: copy, model, maxTurns: perRoundTurns });
    if (!fix.ok && fix.error) {
      status = "errored";
      rounds.push({ round: k, fix, diff: null });
      break;
    }

    lastPost = scanJson(scanner, copy);
    const diff = diffScans(baseline, lastPost);
    history.push(diff.resolvedCount);
    rounds.push({ round: k, fix, diff });
    log?.(
      `    round ${k}: resolved ${diff.resolvedCount}/${diff.baselineCount}, regressions ${diff.regressionCount} (turns ${fix.numTurns}, $${(fix.costUsd || 0).toFixed(2)})`,
    );

    const decision = decideConvergence(history, {
      baselineCount: baseline.issueCount,
      stallRounds,
      maxRounds,
    });
    if (decision.stop) {
      status = decision.status;
      break;
    }
  }

  const finalDiff = lastPost
    ? diffScans(baseline, lastPost)
    : {
        resolvedCount: 0,
        regressionCount: 0,
        baselineCount: baseline.issueCount,
        resolutionRate: 0,
        resolved: [],
        unresolved: [],
        regressions: [],
      };
  return {
    arm,
    mode: "converge",
    converged: status,
    totalRounds: rounds.length,
    rounds: rounds.map((r) => ({
      round: r.round,
      fix: r.fix,
      resolvedCount: r.diff?.resolvedCount ?? null,
      regressionCount: r.diff?.regressionCount ?? null,
    })),
    fix: sumFix(rounds.map((r) => r.fix)),
    diff: finalDiff,
  };
}
