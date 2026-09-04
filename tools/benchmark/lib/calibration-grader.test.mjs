import assert from "node:assert/strict";
import { test } from "node:test";
import { gradeCase } from "../guest/calibration-grader.mjs";
import { calibrationCases } from "./calibration-cases.mjs";

test("every case rejects invalid output without dropping failed checks", () => {
  for (const item of calibrationCases) {
    const result = gradeCase(item, "/not-executed", () => ({}));
    assert.equal(result.acceptancePass, false);
    assert.equal(result.regressionsPass, false);
    assert.ok(result.acceptance.length >= 3);
    assert.ok(result.regressions.length >= 2);
  }
});

test("the negative control checks query behavior and record preservation", () => {
  const item = calibrationCases.find((item) => item.kind === "negative_control");
  const safe = gradeCase(item, "/not-executed", (_item, _path, input) =>
    input.operation === "public-tests"
      ? { exitCode: 0 }
      : {
          result: input.users.filter((row) => row[1] === input.args.name),
          remaining: input.users,
        },
  );
  assert.equal(safe.acceptancePass, true);
  assert.equal(safe.regressionsPass, true);
  const broken = gradeCase(item, "/not-executed", (_item, _path, input) =>
    input.operation === "public-tests"
      ? { exitCode: 0 }
      : {
          result: input.users.filter((row) => row[1] === input.args.name),
          remaining: [],
        },
  );
  assert.equal(broken.regressionsPass, false);
});
