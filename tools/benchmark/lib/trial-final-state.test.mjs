import assert from "node:assert/strict";
import { test } from "node:test";
import { fixtureRecord, fixtureStudy } from "./workflow-fixture.mjs";
import { createPlan } from "./workflow-plan.mjs";
import { trialOutcome } from "./workflow-results.mjs";

test("a later interrupted trial cannot claim final acceptance from an earlier good patch", () => {
  const plan = createPlan(fixtureStudy());
  const record = fixtureRecord(plan, plan.assignments[0]);
  record.status = "agent_error";
  record.failure = "Unsubmitted changes remained at shutdown";
  const outcome = trialOutcome(record, plan.study.limits);
  assert.equal(outcome.first, true);
  assert.equal(outcome.eventual, false);
});
