import test from "node:test";
import assert from "node:assert/strict";

import {
  getCausesOf,
  getEffectsOf,
  rankWithCausalReach,
  formatCausalityBlock,
  parseCausalGraph,
} from "../dist/causal_graph.js";

test("parseCausalGraph rejects the whole generated graph when any link is malformed", () => {
  const links = parseCausalGraph({
    links: [
      { cause: "performance.compression", effect: "performance.lcp", confidence: "high" },
      { cause: "security.https", effect: "security.hsts", confidence: "low" },
    ],
  });

  assert.deepEqual(links, [
    { cause: "performance.compression", effect: "performance.lcp", confidence: "high" },
    { cause: "security.https", effect: "security.hsts", confidence: "low" },
  ]);

  for (const malformedLink of [
    { cause: "", effect: "performance.lcp", confidence: "high" },
    { cause: "seo.title", effect: "seo.score", confidence: "unknown" },
    null,
  ]) {
    assert.throws(
      () =>
        parseCausalGraph({
          links: [
            { cause: "performance.compression", effect: "performance.lcp", confidence: "high" },
            malformedLink,
          ],
        }),
      /invalid link at index 1/i,
    );
  }

  assert.throws(() => parseCausalGraph({ links: null }), /missing a links array/i);
});

test("getCausesOf returns only active causes", () => {
  const active = new Set(["performance.compression", "performance.lcp"]);
  const causes = getCausesOf("performance.lcp", active);
  const ids = causes.map((c) => c.check_id);
  assert.ok(ids.includes("performance.compression"), "compression should be listed when active");
});

test("getCausesOf filters out inactive causes", () => {
  const active = new Set(["performance.lcp"]);
  const causes = getCausesOf("performance.lcp", active);
  const ids = causes.map((c) => c.check_id);
  assert.ok(!ids.includes("performance.compression"), "compression is inactive, must not appear");
});

test("getEffectsOf is symmetric", () => {
  const active = new Set(["performance.compression", "performance.lcp", "performance.page_weight"]);
  const effects = getEffectsOf("performance.compression", active);
  const ids = effects.map((e) => e.check_id);
  assert.ok(ids.includes("performance.lcp"));
  assert.ok(ids.includes("performance.page_weight"));
});

test("rankWithCausalReach floats a Medium cause above a standalone High", () => {
  const active = new Set(["performance.compression", "performance.lcp", "seo.title_length"]);
  const issues = [
    { check_id: "seo.title_length", severity: "high" },
    { check_id: "performance.compression", severity: "medium" },
    { check_id: "performance.lcp", severity: "critical" },
  ];
  const ranked = rankWithCausalReach(issues, active);
  // compression reaches critical via lcp, so it sorts above the standalone high
  const compressionIdx = ranked.findIndex((i) => i.check_id === "performance.compression");
  const titleIdx = ranked.findIndex((i) => i.check_id === "seo.title_length");
  assert.ok(
    compressionIdx < titleIdx,
    "medium cause of critical effect must rank above standalone high",
  );
});

test("formatCausalityBlock renders 'Root cause hint' when issue has active effects", () => {
  const active = new Set(["performance.compression", "performance.lcp"]);
  const block = formatCausalityBlock("performance.compression", active);
  assert.match(block, /Root cause/);
  assert.match(block, /performance\.lcp/);
});

test("formatCausalityBlock renders 'Likely caused by' when issue has active causes", () => {
  const active = new Set(["performance.compression", "performance.lcp"]);
  const block = formatCausalityBlock("performance.lcp", active);
  assert.match(block, /Likely caused by/);
  assert.match(block, /performance\.compression/);
});

test("formatCausalityBlock returns empty string when no causes or effects active", () => {
  const active = new Set(["seo.title_length"]);
  const block = formatCausalityBlock("seo.title_length", active);
  assert.equal(block, "");
});
