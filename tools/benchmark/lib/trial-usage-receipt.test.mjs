import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { createEvidence } from "../guest/trial-evidence.mjs";
import { fixtureStudy } from "./workflow-fixture.mjs";
import { createPlan } from "./workflow-plan.mjs";
import { totalTokens } from "./workflow-usage.mjs";

test("a completed provider receipt survives a failed benchmark submission", () => {
  const plan = createPlan(fixtureStudy());
  const assignment = plan.assignments[0];
  const item = plan.study.tasks.find((task) => task.id === assignment.task);
  for (const providerCompleted of [true, false]) {
    const directory = mkdtempSync(path.join(tmpdir(), "sitecmd-usage-"));
    writeFileSync(
      path.join(directory, "transcript.jsonl"),
      JSON.stringify({
        type: "result",
        subtype: "success",
        is_error: false,
        modelUsage: {
          "claude-opus-5": {
            inputTokens: 10,
            outputTokens: 20,
            cacheReadInputTokens: 30,
            cacheCreationInputTokens: 40,
          },
        },
      }) + "\n",
    );
    const evidence = createEvidence(directory, plan, assignment, item, {}, directory);
    const record = evidence.finish({
      status: "agent_error",
      failure: "No submission",
      elapsedMs: 1000,
      configuration: { agent: "claude", agentVersion: "fixture", model: "claude-opus-5" },
      quotaAllowed: true,
      evidenceComplete: true,
      providerCompleted,
    });
    assert.equal(record.status, "agent_error");
    assert.equal(totalTokens(record.usage), providerCompleted ? 100 : null);
  }
});
