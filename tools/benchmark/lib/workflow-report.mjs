import { validatePlan } from "./workflow-plan.mjs";
import { validateResult, summarizeArm } from "./workflow-results.mjs";
import { pairedComparison } from "./workflow-statistics.mjs";
import { accountedSpend } from "./workflow-usage.mjs";

const COMPARISONS = [
  ["normal", "report"],
  ["report", "mcp"],
  ["normal", "mcp"],
];

export function analyzeStudy(plan, results, { bootstrapSamples = 2000 } = {}) {
  validatePlan(plan);
  const records = new Map();
  for (const record of results) {
    const assignment = plan.assignments.find((item) => item.id === record.trialId);
    if (!assignment) throw new Error(`Unknown trial ${record.trialId}`);
    if (records.has(record.trialId)) throw new Error(`Duplicate trial ${record.trialId}`);
    if (record.studySha256 !== plan.studySha256)
      throw new Error("Trial belongs to a different frozen study");
    validateResult(record, assignment, plan.study);
    records.set(record.trialId, record);
  }
  const groups = [];
  for (const configuration of plan.study.configurations) {
    for (const surface of ["code", "web"]) {
      for (const kind of ["repair", "negative_control"]) {
        const tasks = plan.study.tasks.filter(
          (task) => task.surface === surface && task.kind === kind,
        );
        if (tasks.length === 0) continue;
        const ids = new Set(tasks.map((task) => task.id));
        const assignments = plan.assignments.filter(
          (item) => item.configuration === configuration.id && ids.has(item.task),
        );
        const arms = Object.fromEntries(
          plan.study.arms.map((arm) => [
            arm,
            summarizeArm(
              assignments.filter((item) => item.arm === arm),
              records,
              plan.study.limits,
            ),
          ]),
        );
        const comparisons =
          kind === "repair"
            ? COMPARISONS.map(([baselineArm, treatmentArm]) =>
                pairedComparison({
                  tasks,
                  assignments,
                  records,
                  limits: plan.study.limits,
                  baselineArm,
                  treatmentArm,
                  seed: plan.study.seed,
                  samples: bootstrapSamples,
                }),
              )
            : [];
        groups.push({
          configuration: configuration.id,
          surface,
          kind,
          tasks: tasks.length,
          repositories: new Set(tasks.map((task) => task.repository)).size,
          arms,
          comparisons,
        });
      }
    }
  }
  const blockers = [];
  if (plan.study.phase !== "confirmatory")
    blockers.push(`${plan.study.phase} studies are not confirmatory evidence`);
  if (records.size !== plan.assignments.length)
    blockers.push(`${plan.assignments.length - records.size} trials have not been recorded`);
  if (
    results.some(
      (record) =>
        record.modelSelection &&
        (record.modelSelection.observed.length !== 1 ||
          record.model !== record.modelSelection.requested),
    )
  )
    blockers.push(
      "Provider-observed model identity is missing or differs from the requested model",
    );
  for (const group of groups) {
    const setupModes = new Set(Object.values(group.arms).flatMap((summary) => summary.setupModes));
    if (setupModes.size > 1)
      blockers.push(
        `${group.configuration}/${group.surface}/${group.kind}: workflows used different setup conditions`,
      );
    const costBases = new Set(
      Object.values(group.arms)
        .flatMap((summary) => summary.costBases)
        .filter((basis) => basis !== "unknown"),
    );
    if (costBases.size > 1)
      blockers.push(
        `${group.configuration}/${group.surface}/${group.kind}: estimated and billed costs are mixed`,
      );
    for (const [arm, summary] of Object.entries(group.arms)) {
      const label = `${group.configuration}/${group.surface}/${group.kind}/${arm}`;
      if (summary.pendingReview)
        blockers.push(`${label}: ${summary.pendingReview} trials need blinded review`);
      if (summary.missingUsage || summary.missingCost)
        blockers.push(`${label}: incomplete usage or cost evidence`);
      if (summary.statuses.infrastructure_error)
        blockers.push(`${label}: infrastructure failures require adjudication`);
      if (summary.overBudget) blockers.push(`${label}: the trial budget was exceeded`);
      if (summary.setupModes.length > 1)
        blockers.push(`${label}: cold and warm setup measurements are mixed`);
    }
    if (group.kind === "repair" && group.repositories < 2)
      blockers.push(`${group.configuration}/${group.surface}: fewer than two repository clusters`);
  }
  const knownSpendUsd = results.reduce(
    (sum, result) => sum + (accountedSpend(result.usage) ?? 0),
    0,
  );
  if (knownSpendUsd > plan.study.limits.studyCostUsd)
    blockers.push("The study spending limit was exceeded");
  return {
    schemaVersion: 1,
    studySha256: plan.studySha256,
    phase: plan.study.phase,
    assigned: plan.assignments.length,
    recorded: records.size,
    knownSpendUsd,
    claimReviewReady: blockers.length === 0,
    blockers,
    groups,
  };
}

const percent = (value) => (value === null ? "n/a" : `${(value * 100).toFixed(1)}%`);
const number = (value, digits = 0) => (value === null ? "n/a" : value.toFixed(digits));
const escapeCell = (value) => String(value).replaceAll("|", "\\|").replaceAll(/\r?\n/g, " ");

export function renderWorkflowReport(plan, analysis) {
  const lines = [
    "# Agent workflow benchmark",
    "",
    `Study: ${plan.study.id}`,
    "",
    `Phase: **${analysis.phase}**. Recorded ${analysis.recorded}/${analysis.assigned} assigned trials.`,
    "",
    `Frozen study digest: \`${analysis.studySha256}\``,
    "",
    analysis.claimReviewReady
      ? "Evidence is ready for claim review, not automatic publication approval."
      : "**Not ready for marketing claims.**",
    "",
  ];
  if (analysis.phase === "fixture")
    lines.push(
      "Usage, timing, and review decisions are synthetic test inputs. No AI agent or real MCP workflow was measured.",
      "",
    );
  if (plan.study.billing?.mode === "subscription")
    lines.push(
      "Subscription allowance is not free compute. Dollar efficiency is n/a; API-equivalent estimates are not charges. The spending cap covers additional charges only.",
      "",
    );
  for (const blocker of analysis.blockers) lines.push(`- ${blocker}`);
  for (const group of analysis.groups) {
    lines.push(
      "",
      `## ${group.configuration}: ${group.surface} ${group.kind}`,
      "",
      `${group.tasks} tasks across ${group.repositories} repositories; repeats are not independent tasks.`,
      "",
      "| Workflow | Recorded / assigned | First accepted | Final accepted | First-attempt rate | Tokens / accepted | Cost / accepted | Regressions | Pending review |",
      "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    );
    for (const [arm, summary] of Object.entries(group.arms)) {
      lines.push(
        `| ${arm} | ${summary.recorded}/${summary.assigned} | ${summary.firstAccepted} | ${summary.accepted} | ${percent(summary.firstAttemptRate)} | ${number(summary.tokensPerAccepted)} | ${number(summary.costPerAccepted, 4)} | ${summary.regressions} | ${summary.pendingReview} |`,
      );
    }
    lines.push(
      "",
      "Rates remain unavailable until every assigned trial is recorded and reviews are settled.",
    );
    if (group.kind === "negative_control")
      lines.push("Accepted negative controls are correct triage decisions, not repaired defects.");
    for (const comparison of group.comparisons) {
      const ci = comparison.confidenceIntervals.firstAttemptDifference;
      lines.push(
        "",
        `${comparison.treatmentArm} vs ${comparison.baselineArm}: first-attempt difference ${number(comparison.point.firstAttemptDifference === null ? null : comparison.point.firstAttemptDifference * 100, 1)} percentage points` +
          (ci
            ? ` (95% paired cluster-bootstrap interval ${number(ci[0] * 100, 1)} to ${number(ci[1] * 100, 1)})`
            : " (confidence interval unavailable)") +
          ".",
        `Relative success lift: ${percent(comparison.point.firstAttemptRelativeLift)}. Tokens per accepted fix reduction: ${percent(comparison.point.tokenReduction)}. Cost per accepted fix reduction: ${percent(comparison.point.costReduction)}.`,
      );
      for (const [key, label] of [
        ["firstAttemptRelativeLift", "Relative lift"],
        ["tokenReduction", "Token reduction"],
        ["costReduction", "Cost reduction"],
      ]) {
        const bounds = comparison.confidenceIntervals[key];
        lines.push(
          `${label} 95% interval: ${bounds ? `${percent(bounds[0])} to ${percent(bounds[1])}` : "unavailable"}.`,
        );
      }
    }
    lines.push(
      "",
      "| Workflow | Failure statuses | Missing token receipts | Missing cost | Cost basis | Setup |",
      "| --- | --- | --- | --- | --- | --- |",
    );
    for (const [arm, summary] of Object.entries(group.arms)) {
      const failures =
        Object.entries(summary.statuses)
          .filter(([status]) => status !== "completed")
          .map(([status, count]) => `${status}: ${count}`)
          .join(", ") || "none recorded";
      lines.push(
        `| ${arm} | ${escapeCell(failures)} | ${summary.missingUsage} | ${summary.missingCost} | ${summary.costBases.join(", ") || "n/a"} | ${summary.setupModes.join(", ") || "n/a"} |`,
      );
    }
    lines.push(
      "",
      "| Workflow | Total elapsed seconds | Total human-active seconds |",
      "| --- | --- | --- |",
    );
    for (const [arm, summary] of Object.entries(group.arms)) {
      lines.push(
        `| ${arm} | ${number(summary.elapsedMs === null ? null : summary.elapsedMs / 1000, 1)} | ${number(summary.humanActiveMs === null ? null : summary.humanActiveMs / 1000, 1)} |`,
      );
    }
  }
  lines.push(
    "",
    "## Interpretation",
    "",
    "- All assigned trials, including product errors and timeouts, remain in the denominator.",
    "- Efficiency includes spending on failed attempts. Zero accepted fixes produce n/a, not an infinite savings claim.",
    "- Token categories are disjoint and include cached input and delegated agents. Missing evidence is not zero usage.",
    "- Confidence intervals resample repositories, then tasks, preserving paired workflows and repeated trials.",
    "- Cost estimates are not bills. Model configurations and code/web results are never pooled.",
    "- Human time is reported only when measured; automated run time is not a developer-productivity result.",
    "- Patch receipts and blinded review support acceptance. Scanner clearance alone is not correctness.",
    "- These checks validate recorded evidence, not trial isolation or an operator's honesty. Audit raw traces and execution conditions before publishing.",
    "",
  );
  return lines.join("\n");
}
