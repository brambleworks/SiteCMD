import assert from "node:assert/strict";
import { test } from "node:test";
import { fixtureRecord, fixtureStudy } from "./workflow-fixture.mjs";
import { createPlan } from "./workflow-plan.mjs";
import { validateTrial } from "./workflow-contract.mjs";
import { trialOutcome } from "./workflow-results.mjs";

test("a CLI model request is not reported as provider-observed identity", () => {
  const plan = createPlan(fixtureStudy());
  const assignment = plan.assignments[0];
  const record = fixtureRecord(plan, assignment);
  record.modelSelection = { requested: record.model, observed: [], source: "explicit-cli-request" };
  record.model = null;
  assert.doesNotThrow(() => validateTrial(record, assignment, plan.study));
  assert.throws(
    () =>
      validateTrial({ ...record, model: record.modelSelection.requested }, assignment, plan.study),
    /provider-observed/,
  );
});

test("unexpected provider models retain a failed assignment without counting its fixes", () => {
  const plan = createPlan(fixtureStudy());
  const assignment = plan.assignments[0];
  const record = fixtureRecord(plan, assignment);
  record.modelSelection = {
    requested: record.model,
    observed: ["unexpected"],
    source: "explicit-cli-request",
  };
  record.model = "unexpected";
  record.status = "infrastructure_error";
  record.failure = "Provider reported another model";
  assert.doesNotThrow(() => validateTrial(record, assignment, plan.study));
  assert.equal(trialOutcome(record, plan.study.limits).first, false);
  assert.throws(
    () => validateTrial({ ...record, status: "completed" }, assignment, plan.study),
    /model mismatch/,
  );
});
