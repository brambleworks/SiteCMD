import { readFileSync } from "node:fs";
import { requireCondition, validateStudy } from "./workflow-contract.mjs";
import { canonicalJson } from "./workflow-plan.mjs";

const policy = JSON.parse(readFileSync(new URL("../pilot-policy.json", import.meta.url), "utf8"));
export const pilotPolicy = policy;

export function validatePilotStudy(study) {
  const same = (actual, expected, label) =>
    requireCondition(
      canonicalJson(actual) === canonicalJson(expected),
      `pilot ${label} differs from the approved policy`,
    );
  for (const key of ["phase", "repeats", "arms", "limits", "billing"])
    same(study[key], policy[key], key);
  same(study.tasks?.length, policy.caseCount, "case count");
  same(
    study.tasks.filter((task) => task.kind === "negative_control").length,
    policy.negativeControls,
    "negative controls",
  );
  requireCondition(
    study.tasks.every((task) => task.surface === "code"),
    "pilot cases must use Code Scan",
  );
  const models = (items) =>
    items
      .map(({ agent, model }) => ({ agent, model }))
      .sort((left, right) => left.agent.localeCompare(right.agent));
  same(models(study.configurations ?? []), models(policy.models), "model configurations");
  return validateStudy(study);
}
