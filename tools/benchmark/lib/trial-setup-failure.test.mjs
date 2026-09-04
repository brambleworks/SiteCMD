import assert from "node:assert/strict";
import { test } from "node:test";
import { fixtureRecord, fixtureStudy } from "./workflow-fixture.mjs";
import { createPlan } from "./workflow-plan.mjs";
import { validateTrial } from "./workflow-contract.mjs";

test("a setup failure records no observed model and no fabricated submissions", () => {
  const plan = createPlan(fixtureStudy());
  const assignment = plan.assignments[0];
  const record = {
    ...fixtureRecord(plan, assignment),
    agentInvoked: false,
    model: null,
    status: "product_error",
    failure: "No repair handoff was available",
    submissions: [],
  };
  assert.doesNotThrow(() => validateTrial(record, assignment, plan.study));
  assert.throws(
    () => validateTrial({ ...record, model: "no-model" }, assignment, plan.study),
    /uninvoked/,
  );
  assert.throws(
    () => validateTrial({ ...record, status: "completed" }, assignment, plan.study),
    /setup failures/,
  );
});
