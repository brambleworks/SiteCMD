import assert from "node:assert/strict";
import { test } from "node:test";
import { fixtureStudy } from "./workflow-fixture.mjs";
import { pilotPolicy, validatePilotStudy } from "./workflow-pilot.mjs";
import { createPlan } from "./workflow-plan.mjs";

function studyForPolicyTest() {
  const study = fixtureStudy();
  study.phase = "calibration";
  study.billing = structuredClone(pilotPolicy.billing);
  study.limits = structuredClone(pilotPolicy.limits);
  study.configurations = pilotPolicy.models.map((model) => ({
    ...study.configurations[0],
    ...model,
    id: `${model.agent}-${model.model.replaceAll(".", "-")}-high`,
  }));
  study.tasks.push(...["c", "d"].map((id) => ({ ...study.tasks[0], id: `repair-${id}` })));
  return study;
}

test("pilot planning rejects model fallback, extra trials, and weakened limits", () => {
  assert.equal(validatePilotStudy(studyForPolicyTest()).tasks.length, 5);
  for (const change of [
    (study) => {
      study.configurations[0].model = "latest";
    },
    (study) => {
      study.tasks.pop();
    },
    (study) => {
      study.tasks[2].kind = "repair";
    },
    (study) => {
      study.repeats = 2;
    },
    (study) => {
      study.limits.trialSeconds = 2400;
    },
    (study) => {
      study.billing.weeklyBudgetPercentagePoints = 50;
    },
    (study) => {
      study.phase = "confirmatory";
    },
  ]) {
    const study = studyForPolicyTest();
    change(study);
    assert.throws(() => validatePilotStudy(study), /pilot/);
  }
});

test("every exact model has separate balanced assignments, including models sharing a client", () => {
  const study = studyForPolicyTest();
  study.configurations.reverse();
  const plan = createPlan(validatePilotStudy(study));
  assert.equal(new Set(plan.assignments.map((item) => item.id)).size, plan.plannedTrials);
  for (const configuration of study.configurations) {
    for (const arm of study.arms) {
      const assigned = plan.assignments.filter(
        (item) => item.configuration === configuration.id && item.arm === arm,
      );
      assert.equal(assigned.length, 5);
      assert.equal(new Set(assigned.map((item) => item.task)).size, 5);
    }
  }
});
