import assert from "node:assert/strict";
import { test } from "node:test";
import { calibrationCases, caseFiles, caseIdentity } from "./calibration-cases.mjs";

test("calibration has four owned repairs and an unchanged negative control", () => {
  assert.equal(calibrationCases.length, 5);
  assert.equal(calibrationCases.filter((item) => item.kind === "repair").length, 4);
  for (const item of calibrationCases) {
    assert.ok(caseFiles(item)[item.entry]);
    assert.match(caseIdentity(item), /^[a-f0-9]{64}$/);
    assert.equal(caseIdentity(item) === caseIdentity(item, true), item.kind === "negative_control");
    assert.ok(caseFiles(item)["README.md"].includes(item.requirements));
  }
});

test("case manifest hashing is order independent and includes file names", () => {
  const base = calibrationCases[0];
  const reversed = { ...base, files: Object.fromEntries(Object.entries(base.files).reverse()) };
  assert.equal(caseIdentity(base), caseIdentity(reversed));
  assert.throws(() => caseFiles({ ...base, files: { "../escape": "bad" } }), /path/);
});
