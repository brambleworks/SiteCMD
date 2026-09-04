import { seededRandom } from "./workflow-plan.mjs";
import { summarizeArm } from "./workflow-results.mjs";
import { requireNumber } from "./workflow-contract.mjs";

function effect(baseline, treatment) {
  const reduction = (before, after) =>
    before !== null && after !== null && before > 0 ? 1 - after / before : null;
  return {
    firstAttemptDifference:
      baseline.firstAttemptRate !== null && treatment.firstAttemptRate !== null
        ? treatment.firstAttemptRate - baseline.firstAttemptRate
        : null,
    firstAttemptRelativeLift:
      baseline.firstAttemptRate > 0 && treatment.firstAttemptRate !== null
        ? treatment.firstAttemptRate / baseline.firstAttemptRate - 1
        : null,
    tokenReduction: reduction(baseline.tokensPerAccepted, treatment.tokensPerAccepted),
    costReduction: reduction(baseline.costPerAccepted, treatment.costPerAccepted),
  };
}

function interval(values) {
  if (values.some((value) => value === null)) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const quantile = (fraction) => {
    const index = (sorted.length - 1) * fraction;
    const lower = Math.floor(index);
    return sorted[lower] + (sorted[Math.ceil(index)] - sorted[lower]) * (index - lower);
  };
  return [quantile(0.025), quantile(0.975)];
}

/** Resample repositories and tasks, keeping paired arms and all repeats together. */
export function pairedComparison({
  tasks,
  assignments,
  records,
  limits,
  baselineArm,
  treatmentArm,
  seed,
  samples = 2000,
}) {
  requireNumber(samples, "bootstrap samples", { positive: true, integer: true });
  const summarize = (selected, arm) =>
    summarizeArm(
      selected.filter((item) => item.arm === arm),
      records,
      limits,
    );
  const point = effect(summarize(assignments, baselineArm), summarize(assignments, treatmentArm));
  const repositories = new Map();
  for (const task of tasks) {
    if (!repositories.has(task.repository)) repositories.set(task.repository, []);
    repositories
      .get(task.repository)
      .push(assignments.filter((assignment) => assignment.task === task.id));
  }
  const clusters = [...repositories.values()];
  const confidenceIntervals = Object.fromEntries(Object.keys(point).map((key) => [key, null]));
  if (clusters.length < 2 || Object.values(point).every((value) => value === null)) {
    return {
      baselineArm,
      treatmentArm,
      point,
      confidenceIntervals,
      repositories: clusters.length,
      tasks: tasks.length,
    };
  }
  const random = seededRandom(seed);
  const choose = (items) => items[Math.floor(random() * items.length)];
  const estimates = Object.fromEntries(Object.keys(point).map((key) => [key, []]));
  for (let sample = 0; sample < samples; sample++) {
    const selected = [];
    for (let repository = 0; repository < clusters.length; repository++) {
      const cluster = choose(clusters);
      for (let task = 0; task < cluster.length; task++) selected.push(...choose(cluster));
    }
    const value = effect(summarize(selected, baselineArm), summarize(selected, treatmentArm));
    for (const key of Object.keys(estimates)) estimates[key].push(value[key]);
  }
  for (const key of Object.keys(estimates)) confidenceIntervals[key] = interval(estimates[key]);
  return {
    baselineArm,
    treatmentArm,
    point,
    confidenceIntervals,
    repositories: clusters.length,
    tasks: tasks.length,
  };
}
