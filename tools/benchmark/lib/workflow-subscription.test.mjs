import assert from "node:assert/strict";
import { test } from "node:test";
import { fixtureStudy, fixtureRecord } from "./workflow-fixture.mjs";
import { createPlan } from "./workflow-plan.mjs";
import { analyzeStudy, renderWorkflowReport } from "./workflow-report.mjs";

function subscriptionStudy() {
  const study = fixtureStudy();
  study.billing = {
    mode: "subscription",
    paidFallback: false,
    automaticResets: false,
    weeklyBudgetPercentagePoints: 20,
    minimumRemainingPercent: 30,
    quotaMaxAgeSeconds: 300,
  };
  Object.assign(study.limits, { trialCostUsd: 0, studyCostUsd: 0, trialTokens: null });
  return study;
}

test("subscription studies freeze a zero extra-spend budget without an invented token cap", () => {
  const plan = createPlan(subscriptionStudy());
  assert.equal(plan.maximumConfiguredSpendUsd, 0);
  assert.equal(plan.study.limits.trialTokens, null);
  const study = subscriptionStudy();
  study.limits.studyCostUsd = 1;
  assert.throws(() => createPlan(study), /zero/);
  study.limits.studyCostUsd = 0;
  study.billing.paidFallback = true;
  assert.throws(() => createPlan(study), /fallback/);
});

function measuredFixture() {
  const plan = createPlan(subscriptionStudy());
  const records = plan.assignments.map((assignment) => {
    const record = fixtureRecord(plan, assignment);
    Object.assign(record.usage, {
      costBasis: "subscription",
      costUsd: null,
      incrementalCostUsd: 0,
      apiEquivalentCostUsd: 12,
    });
    return record;
  });
  return { plan, records };
}

test("subscription allowances are not API bills or dollar-savings claims", () => {
  const { plan, records } = measuredFixture();
  const analysis = analyzeStudy(plan, records, { bootstrapSamples: 10 });
  assert.equal(analysis.knownSpendUsd, 0);
  const group = analysis.groups.find((item) => item.kind === "repair");
  for (const arm of Object.values(group.arms)) {
    assert.equal(arm.overBudget, 0);
    assert.equal(arm.missingCost, 0);
    assert.equal(arm.costPerAccepted, null);
    assert.equal(arm.tokensPerAccepted, 150);
  }
  assert.equal(group.comparisons[0].point.costReduction, null);
  assert.match(renderWorkflowReport(plan, analysis), /Subscription allowance is not free compute/);
});

test("unknown extra spending remains unknown and actual overages fail the zero-spend cap", () => {
  const { plan, records } = measuredFixture();
  records[0].usage.incrementalCostUsd = null;
  let analysis = analyzeStudy(plan, records, { bootstrapSamples: 10 });
  assert.ok(analysis.blockers.some((item) => item.includes("incomplete usage or cost")));
  records[0].usage.incrementalCostUsd = 0.01;
  analysis = analyzeStudy(plan, records, { bootstrapSamples: 10 });
  assert.equal(analysis.knownSpendUsd, 0.01);
  assert.ok(analysis.blockers.some((item) => item.includes("spending limit")));
  assert.ok(analysis.blockers.some((item) => item.includes("trial budget")));
  records[0].usage.costBasis = "estimated";
  records[0].usage.costUsd = 0.01;
  assert.throws(() => analyzeStudy(plan, records), /subscription accounting/);
});
