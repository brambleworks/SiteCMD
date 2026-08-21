import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { RUNNER, runnerWiringFailures } from "./lib/runner-wiring.mjs";

const SCRIPTS = path.dirname(fileURLToPath(import.meta.url));
const LIB = path.join(SCRIPTS, "lib");

function repoSources() {
  const sources = new Map();
  for (const name of fs.readdirSync(LIB)) {
    if (name.endsWith(".mjs")) sources.set(name, fs.readFileSync(path.join(LIB, name), "utf8"));
  }
  sources.set(RUNNER, fs.readFileSync(path.join(SCRIPTS, RUNNER), "utf8"));
  return sources;
}

describe("runnerWiringFailures", () => {
  it("finds every guardrail rule reachable from the runner", () => {
    expect(runnerWiringFailures(repoSources())).toEqual([]);
  });

  it("covers the whole rule surface, not a sample", () => {
    const sources = repoSources();
    const ruleModules = [...sources.keys()].filter((name) => /^guardrail-.*\.mjs$/.test(name));
    expect(ruleModules.length).toBeGreaterThan(100);

    const unchecked = ruleModules.filter(
      (name) => !/^export (?:async )?function \w+Failures/m.test(sources.get(name)),
    );
    expect(unchecked).toEqual([
      "guardrail-license-sources.mjs",
      "guardrail-script-budgets.mjs",
      "guardrail-source-regex.mjs",
      "guardrail-source-text.mjs",
      "guardrail-tauri-acl-parsing.mjs",
      "guardrail-text-utils.mjs",
    ]);
  });

  it("reports a rule module the runner never imports", () => {
    const sources = repoSources();
    sources.set(
      "guardrail-orphan-rules.mjs",
      "export function orphanFailures(read) {\n  return [read];\n}\n",
    );

    expect(runnerWiringFailures(sources).join("\n")).toContain(
      "guardrail-orphan-rules.mjs exports orphanFailures",
    );
  });

  it("reports a rule the runner imports but never calls", () => {
    const sources = new Map([
      [
        "guardrail-sample-rules.mjs",
        "export function sampleFailures(read) {\n  return [read];\n}\n",
      ],
      [RUNNER, 'import { sampleFailures } from "./lib/guardrail-sample-rules.mjs";\n'],
    ]);

    expect(runnerWiringFailures(sources).join("\n")).toContain(
      "guardrail-sample-rules.mjs exports sampleFailures",
    );
  });

  it("is not satisfied by a comment naming the rule", () => {
    const sources = new Map([
      [
        "guardrail-sample-rules.mjs",
        "export function sampleFailures(read) {\n  return [read];\n}\n",
      ],
      [
        RUNNER,
        'import { sampleFailures } from "./lib/guardrail-sample-rules.mjs";\n// sampleFailures(read) used to run here\n',
      ],
    ]);

    expect(runnerWiringFailures(sources).join("\n")).toContain("exports sampleFailures");
  });

  it("is not satisfied by a rule named inside a string literal", () => {
    const sources = new Map([
      [
        "guardrail-sample-rules.mjs",
        "export function sampleFailures(read) {\n  return [read];\n}\n",
      ],
      [
        RUNNER,
        'import { sampleFailures } from "./lib/guardrail-sample-rules.mjs";\nfailures.push("sampleFailures(read) covers this file");\n',
      ],
    ]);

    expect(runnerWiringFailures(sources).join("\n")).toContain("exports sampleFailures");
  });

  it("accepts rules the runner hands to something else instead of naming a call", () => {
    const sources = new Map([
      [
        "guardrail-sample-rules.mjs",
        "export function aFailures(read) {\n  return [read];\n}\nexport function bFailures(read) {\n  return [read];\n}\n",
      ],
      [
        RUNNER,
        'import { aFailures, bFailures } from "./lib/guardrail-sample-rules.mjs";\nfor (const rule of [aFailures, bFailures]) failures.push(...rule(read));\n',
      ],
    ]);

    expect(runnerWiringFailures(sources)).toEqual([]);
  });

  it("reports a rule declared with export const that nothing wires", () => {
    const sources = new Map([
      ["guardrail-sample-rules.mjs", "export const arrowFailures = (read) => [read];\n"],
      [RUNNER, "const failures = [];\n"],
    ]);

    expect(runnerWiringFailures(sources).join("\n")).toContain(
      "guardrail-sample-rules.mjs exports arrowFailures",
    );
  });

  it("accepts an export-const rule the runner does call", () => {
    const sources = new Map([
      ["guardrail-sample-rules.mjs", "export const arrowFailures = (read) => [read];\n"],
      [
        RUNNER,
        'import { arrowFailures } from "./lib/guardrail-sample-rules.mjs";\nconst failures = arrowFailures(read);\n',
      ],
    ]);

    expect(runnerWiringFailures(sources)).toEqual([]);
  });

  it("reports a re-exported rule nothing wires, at both names", () => {
    const sources = new Map([
      [
        "guardrail-sample-rules.mjs",
        'export { hiddenFailures } from "./guardrail-inner-rules.mjs";\n',
      ],
      [
        "guardrail-inner-rules.mjs",
        "export function hiddenFailures(read) {\n  return [read];\n}\n",
      ],
      [RUNNER, "const failures = [];\n"],
    ]);

    const report = runnerWiringFailures(sources).join("\n");
    expect(report).toContain("guardrail-sample-rules.mjs exports hiddenFailures");
    expect(report).toContain("guardrail-inner-rules.mjs exports hiddenFailures");
  });

  it("accepts a rule the runner reaches through another rule", () => {
    const sources = new Map([
      [
        "guardrail-sample-rules.mjs",
        "export function innerFailures(read) {\n  return [read];\n}\nexport function outerFailures(read) {\n  return innerFailures(read);\n}\n",
      ],
      [
        RUNNER,
        'import { outerFailures } from "./lib/guardrail-sample-rules.mjs";\nconst failures = outerFailures(read);\n',
      ],
    ]);

    expect(runnerWiringFailures(sources)).toEqual([]);
  });

  it("does not reach a rule through a module the runner never reaches", () => {
    const sources = new Map([
      [
        "guardrail-marooned-rules.mjs",
        "export function innerFailures(read) {\n  return [read];\n}\nexport function outerFailures(read) {\n  return innerFailures(read);\n}\n",
      ],
      [RUNNER, "const failures = [];\n"],
    ]);

    expect(runnerWiringFailures(sources)).toHaveLength(2);
  });

  it("reports a missing runner rather than passing vacuously", () => {
    expect(runnerWiringFailures(new Map()).join("\n")).toContain(
      "is missing from the module graph",
    );
  });
});
