import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const UNFENCED = new Set([
  "how_to_rescan",
  "request_scan",
  "get_scan_score",
  "get_scan_history",
  "request_verification",
  "run_scan",
]);

function registrations(file) {
  const source = readFileSync(join(import.meta.dirname, "..", "src", file), "utf8");
  const names = [...source.matchAll(/registerTool\(\s*\n\s*"([^"]+)"/g)].map((match) => match[1]);
  return names.map((name, index) => {
    const start = source.indexOf(`"${name}"`);
    const end = index + 1 < names.length ? source.indexOf(`"${names[index + 1]}"`) : source.length;
    return { name, body: source.slice(start, end) };
  });
}

/** Shared by the real source scan and its negative control below. */
function assertRegistrationIsFenced(name, body) {
  if (UNFENCED.has(name)) {
    assert.ok(
      !body.includes("untrustedScanData("),
      `${name} is listed as unfenced but fences output; update UNFENCED`,
    );
    return;
  }
  assert.ok(
    body.includes("untrustedScanData("),
    `${name} prints scan-derived text and must wrap it with untrustedScanData`,
  );
  assert.ok(
    body.includes("UNTRUSTED_DATA_INSTRUCTION"),
    `${name} must carry the boundary instruction`,
  );
}

test("every tool that returns scan-derived text wraps it in the untrusted block", () => {
  for (const file of ["server.ts", "correlation_tools.ts"]) {
    for (const { name, body } of registrations(file)) {
      assertRegistrationIsFenced(name, body);
    }
  }
});

test("the scan fails a registration that prints scan-derived text unfenced", () => {
  // A tool not in UNFENCED that returns scan data without routing it through
  // untrustedScanData must fail the scan; this is the negative control proving
  // the positive test above can actually catch a real regression.
  const leaky = `
  server.registerTool(
    "get_totally_unfenced_issue",
    { title: "Leaky", description: "leaks scan data" },
    async ({ url }) =>
      runTool(() => {
        const issue = getIssueFromScan(url);
        return text(issue.title);
      }),
  );
  `;
  assert.throws(
    () => assertRegistrationIsFenced("get_totally_unfenced_issue", leaky),
    /must wrap it with untrustedScanData/,
  );
});
