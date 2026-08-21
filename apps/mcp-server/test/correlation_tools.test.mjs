import test from "node:test";
import assert from "node:assert/strict";

import {
  computeImpactScore,
  getActiveCheckIds,
  getRecentEvents,
  getActiveIssueGroupsEnriched,
  getCausalMapPayload,
  previewDeployRisk,
  whatifResolve,
} from "../dist/db.js";
import { makeSeeders, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

// db.js resolves SITECMD_DB_PATH lazily at first query, so seeding the
// fixture (which sets the env var) after the static imports is safe.
const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-corr-");
const { addWorkItem, addEvent, linkEventToCheckId, addObservation } = makeSeeders(fixtureDb);

// Minimal causal links fixture for tests
const TEST_LINKS = [
  { cause: "performance.compression", effect: "performance.lcp", confidence: "high" },
  { cause: "performance.compression", effect: "performance.page_weight", confidence: "high" },
  { cause: "security.https", effect: "security.hsts", confidence: "high" },
  { cause: "infrastructure.uptime", effect: "analytics.traffic-drop", confidence: "high" },
];

test("getActiveCheckIds returns distinct unresolved check_ids for a project", () => {
  const projectId = 1001;
  // Same check_id on two pages: distinct signal_ids, because the real
  // uq_work_items_active index forbids two active rows for one signal_id.
  addWorkItem({
    projectId,
    checkId: "security.hsts",
    signalId: "web_scan:security.hsts:https://example.com/a",
  });
  addWorkItem({
    projectId,
    checkId: "security.hsts",
    signalId: "web_scan:security.hsts:https://example.com/b",
  }); // duplicate check_id - should dedupe
  addWorkItem({ projectId, checkId: "performance.lcp" });
  addWorkItem({ projectId, checkId: "resolved.item", resolvedAt: Date.now() });

  const ids = getActiveCheckIds(projectId);

  assert.ok(ids instanceof Set);
  assert.ok(ids.has("security.hsts"), "should contain security.hsts");
  assert.ok(ids.has("performance.lcp"), "should contain performance.lcp");
  assert.ok(!ids.has("resolved.item"), "should not contain resolved check_ids");
});

test("getActiveCheckIds returns empty set when project has no active items", () => {
  const projectId = 1002;
  addWorkItem({ projectId, checkId: "resolved.only", resolvedAt: Date.now() });

  const ids = getActiveCheckIds(projectId);
  assert.equal(ids.size, 0);
});

test("getRecentEvents returns events within the requested day window", () => {
  const projectId = 1003;
  const recentMs = Date.now() - 5 * 24 * 60 * 60 * 1000;
  const oldMs = Date.now() - 40 * 24 * 60 * 60 * 1000;

  const recentEventId = addEvent({
    projectId,
    title: "Recent deploy",
    occurredAtMs: recentMs,
    timestamp: new Date(recentMs).toISOString(),
  });
  addEvent({
    projectId,
    title: "Old deploy",
    occurredAtMs: oldMs,
    timestamp: new Date(oldMs).toISOString(),
  });
  linkEventToCheckId(recentEventId, "security.hsts");

  const events = getRecentEvents(projectId, 30);

  assert.ok(events.length >= 1, "should return at least one recent event");
  assert.ok(
    events.some((e) => e.title === "Recent deploy"),
    "should include the recent event",
  );
  assert.ok(
    !events.some((e) => e.title === "Old deploy"),
    "should not include events outside the window",
  );
});

test("getRecentEvents returns correct shape for each event", () => {
  const projectId = 1004;
  const nowMs = Date.now() - 1000;
  const eventId = addEvent({
    projectId,
    eventType: "deploy",
    title: "Shape test deploy",
    occurredAtMs: nowMs,
    timestamp: new Date(nowMs).toISOString(),
  });
  linkEventToCheckId(eventId, "performance.lcp");

  const events = getRecentEvents(projectId, 7);

  assert.ok(events.length >= 1);
  const ev = events.find((e) => e.title === "Shape test deploy");
  assert.ok(ev, "event should be present");
  assert.equal(typeof ev.eventId, "number");
  assert.equal(typeof ev.eventType, "string");
  // events has no TEXT timestamp column anymore; the ISO string must be
  // derived from occurred_at_ms.
  assert.equal(ev.timestamp, new Date(nowMs).toISOString());
  assert.equal(typeof ev.title, "string");
  assert.ok(
    ev.correlationConfidence === "high" ||
      ev.correlationConfidence === "medium" ||
      ev.correlationConfidence === "low",
    "correlationConfidence should be a valid V3Confidence",
  );
});

test("getActiveIssueGroupsEnriched returns groups for a seeded project", () => {
  const projectId = 1005;
  addWorkItem({ projectId, checkId: "security.https", category: "security", severity: "critical" });
  addWorkItem({ projectId, checkId: "security.hsts", category: "security", severity: "high" });
  addWorkItem({ projectId, checkId: "performance.lcp", category: "performance", severity: "high" });

  const groups = getActiveIssueGroupsEnriched(projectId, TEST_LINKS);

  assert.ok(Array.isArray(groups), "should return an array");
  assert.equal(groups.length, 3, "should have one group per distinct check_id");

  const checkIds = groups.map((g) => g.checkId).sort();
  // "security.hsts" sorts before "security.https" alphabetically ('t' < 'x')
  assert.deepEqual(checkIds, ["performance.lcp", "security.hsts", "security.https"]);
});

test("getActiveIssueGroupsEnriched group has correct shape", () => {
  const projectId = 1006;
  addWorkItem({
    projectId,
    checkId: "security.https",
    category: "security",
    severity: "critical",
    title: "No HTTPS",
    pageUrl: "https://example.com/page1",
  });

  const groups = getActiveIssueGroupsEnriched(projectId, TEST_LINKS);
  const group = groups.find((g) => g.checkId === "security.https");

  assert.ok(group, "should find the group");
  assert.equal(group.checkId, "security.https");
  assert.equal(group.category, "security");
  assert.equal(group.severity, "critical");
  assert.ok(typeof group.title === "string");
  assert.ok(typeof group.description === "string");
  assert.ok(Array.isArray(group.sources));
  assert.ok(Array.isArray(group.transitiveCauses));
  assert.ok(Array.isArray(group.downstreamEffects));
  assert.ok(Array.isArray(group.recentEvents));
  assert.ok(Array.isArray(group.enrichments));
  assert.ok(Array.isArray(group.affectedPages));
  assert.ok(typeof group.observationCount === "number");
  assert.ok(group.crossEnvSignal === null || typeof group.crossEnvSignal === "object");
  assert.ok(group.crossProjectPattern === null || typeof group.crossProjectPattern === "object");
  assert.ok(group.anomalyScore === null || typeof group.anomalyScore === "number");
  assert.equal(group.affectedPages[0], "https://example.com/page1");
  // work_items has no impact_score column; the score is computed from the
  // generated impact_score.json weights and must be non-trivial here.
  assert.equal(group.impactScore, computeImpactScore("critical", "security", 1));
  assert.ok(group.impactScore > 0, "critical security group should carry a positive impact score");
});

test("getActiveIssueGroupsEnriched computes downstream effects via causal graph", () => {
  const projectId = 1007;
  addWorkItem({ projectId, checkId: "security.https", category: "security", severity: "critical" });
  addWorkItem({ projectId, checkId: "security.hsts", category: "security", severity: "high" });

  const groups = getActiveIssueGroupsEnriched(projectId, TEST_LINKS);
  const httpsGroup = groups.find((g) => g.checkId === "security.https");

  assert.ok(httpsGroup, "security.https group should exist");
  assert.ok(
    httpsGroup.downstreamEffects.includes("security.hsts"),
    "security.https should list security.hsts as a downstream effect",
  );
});

test("getActiveIssueGroupsEnriched computes transitive causes via causal graph", () => {
  const projectId = 1008;
  addWorkItem({
    projectId,
    checkId: "performance.compression",
    category: "performance",
    severity: "medium",
  });
  addWorkItem({ projectId, checkId: "performance.lcp", category: "performance", severity: "high" });

  const groups = getActiveIssueGroupsEnriched(projectId, TEST_LINKS);
  const lcpGroup = groups.find((g) => g.checkId === "performance.lcp");

  assert.ok(lcpGroup, "performance.lcp group should exist");
  assert.ok(
    lcpGroup.transitiveCauses.some((c) => c.checkId === "performance.compression"),
    "performance.lcp should list performance.compression as a transitive cause",
  );
});

test("getActiveIssueGroupsEnriched excludes resolved work items", () => {
  const projectId = 1009;
  addWorkItem({ projectId, checkId: "active.check" });
  addWorkItem({ projectId, checkId: "resolved.check", resolvedAt: Date.now() });

  const groups = getActiveIssueGroupsEnriched(projectId, TEST_LINKS);
  const checkIds = groups.map((g) => g.checkId);

  assert.ok(checkIds.includes("active.check"), "active check should appear");
  assert.ok(!checkIds.includes("resolved.check"), "resolved check should not appear");
});

test("getActiveIssueGroupsEnriched returns empty array for project with no active items", () => {
  const projectId = 1010;
  const groups = getActiveIssueGroupsEnriched(projectId, TEST_LINKS);
  assert.deepEqual(groups, []);
});

test("getCausalMapPayload returns nodes and edges with active check_ids only", () => {
  const projectId = 1011;
  addWorkItem({ projectId, checkId: "performance.compression", severity: "medium" });
  addWorkItem({ projectId, checkId: "performance.lcp", severity: "high" });
  addWorkItem({ projectId, checkId: "seo.title", category: "seo", severity: "low" });

  const payload = getCausalMapPayload(projectId, TEST_LINKS);

  assert.ok(Array.isArray(payload.nodes), "nodes should be an array");
  assert.ok(Array.isArray(payload.edges), "edges should be an array");

  const nodeIds = payload.nodes.map((n) => n.checkId);
  assert.ok(nodeIds.includes("performance.compression"));
  assert.ok(nodeIds.includes("performance.lcp"));
  assert.ok(nodeIds.includes("seo.title"));

  const relevantEdge = payload.edges.find(
    (e) => e.from === "performance.compression" && e.to === "performance.lcp",
  );
  assert.ok(relevantEdge, "should have an edge from compression to lcp");
  assert.equal(relevantEdge.confidence, "high");
});

test("getCausalMapPayload excludes edges where either endpoint is inactive", () => {
  const projectId = 1012;
  addWorkItem({ projectId, checkId: "performance.compression" });
  // performance.lcp NOT seeded for this project

  const payload = getCausalMapPayload(projectId, TEST_LINKS);

  const edgeToLcp = payload.edges.find(
    (e) => e.from === "performance.compression" && e.to === "performance.lcp",
  );
  assert.equal(edgeToLcp, undefined, "edge should not appear when effect is not active");
});

test("getCausalMapPayload node has correct shape", () => {
  const projectId = 1013;
  addWorkItem({
    projectId,
    checkId: "security.https",
    severity: "critical",
    title: "No HTTPS configured",
  });

  const payload = getCausalMapPayload(projectId, TEST_LINKS);
  const node = payload.nodes.find((n) => n.checkId === "security.https");

  assert.ok(node, "node should exist");
  assert.equal(typeof node.checkId, "string");
  assert.equal(typeof node.severity, "string");
  assert.equal(typeof node.title, "string");
  assert.equal(typeof node.anomaly, "boolean");
});

test("previewDeployRisk returns the correct schema shape", () => {
  const projectId = 1014;
  const preview = previewDeployRisk(projectId, ["src/config/nginx.conf"], TEST_LINKS);

  assert.ok(Array.isArray(preview.directRisks), "directRisks should be an array");
  assert.ok(Array.isArray(preview.downstreamRisks), "downstreamRisks should be an array");
  assert.ok(
    Array.isArray(preview.historicalRegressions),
    "historicalRegressions should be an array",
  );
});

test("previewDeployRisk returns directRisks when changed_files match fix_locations", () => {
  const projectId = 1019;
  addWorkItem({
    projectId,
    checkId: "security.csp",
    category: "security",
    severity: "high",
    title: "Missing CSP",
  });

  // "next.config.ts" is a known fix_location path for security.csp
  const preview = previewDeployRisk(projectId, ["next.config.ts"], TEST_LINKS);

  assert.ok(preview.directRisks.length >= 1, "should have at least one direct risk");
  const risk = preview.directRisks.find((r) => r.checkId === "security.csp");
  assert.ok(risk, "security.csp should appear in directRisks");
  assert.ok(
    risk.matchedFiles.includes("next.config.ts"),
    "matchedFiles should include next.config.ts",
  );
  assert.equal(risk.confidence, "high");
});

test("previewDeployRisk returns empty directRisks when no files match", () => {
  const projectId = 1020;
  addWorkItem({ projectId, checkId: "security.csp", category: "security", severity: "high" });

  const preview = previewDeployRisk(projectId, ["unrelated-file.ts"], TEST_LINKS);

  assert.equal(preview.directRisks.length, 0, "no direct risks when no paths match");
});

test("previewDeployRisk surfaces downstream risks when causal chain matches", () => {
  const projectId = 1021;
  // TEST_LINKS maps security.https to security.hsts.
  addWorkItem({ projectId, checkId: "security.https", category: "security", severity: "critical" });
  addWorkItem({ projectId, checkId: "security.hsts", category: "security", severity: "high" });

  // vercel.json maps directly to security.hsts.
  const preview = previewDeployRisk(projectId, ["vercel.json"], TEST_LINKS);

  const hstsDirect = preview.directRisks.find((r) => r.checkId === "security.hsts");
  assert.ok(hstsDirect, "security.hsts should be a direct risk when vercel.json changes");
});

test("whatifResolve returns a downstream effect when causal link and active effect exist", () => {
  const projectId = 1015;
  addWorkItem({ projectId, checkId: "security.https", severity: "critical" });
  addWorkItem({ projectId, checkId: "security.hsts", severity: "high" });

  const result = whatifResolve(projectId, ["security.https"], TEST_LINKS);

  assert.ok(Array.isArray(result.alsoResolves), "alsoResolves should be an array");
  const effect = result.alsoResolves.find((e) => e.checkId === "security.hsts");
  assert.ok(effect, "security.hsts should appear as a likely also-resolve");
  assert.ok(
    effect.confidence === "high" || effect.confidence === "medium" || effect.confidence === "low",
    "confidence should be a valid V3Confidence",
  );
  assert.ok(effect.via.includes("security.https"), "via should include the hypothetical resolve");
});

test("whatifResolve does not include inactive effects", () => {
  const projectId = 1016;
  addWorkItem({ projectId, checkId: "security.https", severity: "critical" });
  addWorkItem({ projectId, checkId: "security.hsts", severity: "high", resolvedAt: Date.now() });

  const result = whatifResolve(projectId, ["security.https"], TEST_LINKS);

  const hsts = result.alsoResolves.find((e) => e.checkId === "security.hsts");
  assert.equal(hsts, undefined, "resolved effects should not appear in alsoResolves");
});

test("whatifResolve uses observation calibration when enough observations present", () => {
  const projectId = 1017;
  addWorkItem({ projectId, checkId: "performance.compression" });
  addWorkItem({ projectId, checkId: "performance.lcp" });

  // Low co-resolve ratio: 1 resolved out of 20 active -> should downgrade confidence
  addObservation(projectId, "performance.compression", "performance.lcp", 1, 20);

  const result = whatifResolve(projectId, ["performance.compression"], TEST_LINKS);
  const effect = result.alsoResolves.find((e) => e.checkId === "performance.lcp");

  assert.ok(effect, "performance.lcp should appear");
  // ratio 0.05 (< 0.2) with base "high" (1.0) => adjusted 0.6 => "medium"
  assert.equal(
    effect.confidence,
    "medium",
    "low co-resolve ratio should downgrade confidence from high to medium",
  );
});

test("whatifResolve returns empty alsoResolves when no causal links apply", () => {
  const projectId = 1018;
  addWorkItem({ projectId, checkId: "seo.title", category: "seo" });

  const result = whatifResolve(projectId, ["seo.title"], TEST_LINKS);
  assert.deepEqual(result.alsoResolves, []);
});

test("getActiveIssueGroupsEnriched sets displayConfidence null when check_id has no active causes", () => {
  const projectId = 1022;
  // performance.compression has no incoming links in TEST_LINKS (it is only a cause)
  addWorkItem({
    projectId,
    checkId: "performance.compression",
    category: "performance",
    severity: "medium",
  });

  const groups = getActiveIssueGroupsEnriched(projectId, TEST_LINKS);
  const group = groups.find((g) => g.checkId === "performance.compression");

  assert.ok(group, "group should exist");
  assert.equal(group.displayConfidence, null, "no active causes means displayConfidence is null");
});

test("getActiveIssueGroupsEnriched populates displayConfidence when active cause exists", () => {
  const projectId = 1023;
  // security.https -> security.hsts (high confidence in TEST_LINKS)
  addWorkItem({ projectId, checkId: "security.https", category: "security", severity: "critical" });
  addWorkItem({ projectId, checkId: "security.hsts", category: "security", severity: "high" });

  const groups = getActiveIssueGroupsEnriched(projectId, TEST_LINKS);
  const hstsGroup = groups.find((g) => g.checkId === "security.hsts");

  assert.ok(hstsGroup, "security.hsts group should exist");
  assert.ok(
    hstsGroup.displayConfidence !== null,
    "displayConfidence should be non-null when an active cause exists",
  );
  assert.ok(
    hstsGroup.displayConfidence === "high" ||
      hstsGroup.displayConfidence === "medium" ||
      hstsGroup.displayConfidence === "low",
    "displayConfidence should be a valid V3Confidence string",
  );
  // With no observation history, base confidence (high) is returned unchanged
  assert.equal(hstsGroup.displayConfidence, "high", "base confidence is high for https->hsts link");
});

test("getActiveIssueGroupsEnriched populates displayConfidence based on observations", () => {
  const projectId = 1024;
  // infrastructure.uptime -> analytics.traffic-drop (high confidence in TEST_LINKS)
  addWorkItem({
    projectId,
    checkId: "infrastructure.uptime",
    category: "infrastructure",
    severity: "critical",
  });
  addWorkItem({
    projectId,
    checkId: "analytics.traffic-drop",
    category: "analytics",
    severity: "high",
  });

  // Low co-resolve ratio: 1/10 -> ratio 0.1 < 0.2, base high (1.0) - 0.4 = 0.6 = medium
  addObservation(projectId, "infrastructure.uptime", "analytics.traffic-drop", 1, 10);

  const groups = getActiveIssueGroupsEnriched(projectId, TEST_LINKS);
  const trafficGroup = groups.find((g) => g.checkId === "analytics.traffic-drop");

  assert.ok(trafficGroup, "analytics.traffic-drop group should exist");
  assert.equal(
    trafficGroup.displayConfidence,
    "medium",
    "low co-resolve ratio should downgrade displayConfidence from high to medium",
  );
});
