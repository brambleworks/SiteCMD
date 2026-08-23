import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  getWorkspaceScan,
  parsePackageDependencyNames,
  parseWorkspaceIssue,
  parseWorkspaceScanResult,
} from "../dist/workspace.js";
import {
  getCodeScanHistoryForProject,
  getDismissedIssues,
  getEffectiveTier,
  getFixPromptsForProject,
  getIssueComparisonForProject,
  getIssuesForProject,
  getLatestScan,
  getProjectByUrl,
  getScanHistory,
  SUPPORTED_ISSUE_STATUSES,
} from "../dist/db.js";
import { ensureProject, makeSeeders, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

// db.js resolves SITECMD_DB_PATH lazily at first query, so seeding the
// fixture (which sets the env var) after the static imports is safe.
const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-db-");
const { addWorkItem, setIssueState } = makeSeeders(fixtureDb);
function makeIssue() {
  return {
    id: 1,
    category: "security",
    check_id: "security.hsts",
    severity: "high",
    status: "fail",
    title: "Missing HSTS header",
    description: "Strict-Transport-Security is not set.",
    fix_prompt: "Add the header in your reverse proxy config.",
    manual_fix: "Set Strict-Transport-Security for HTTPS responses.",
  };
}

test("parseWorkspaceIssue rejects malformed cached issue rows", () => {
  assert.equal(parseWorkspaceIssue(null), null);
  assert.equal(parseWorkspaceIssue({ ...makeIssue(), check_id: "" }), null);
  assert.equal(parseWorkspaceIssue({ ...makeIssue(), status: 7 }), null);
  assert.equal(parseWorkspaceIssue({ ...makeIssue(), status: "resolved" }), null);
  assert.equal(parseWorkspaceIssue({ ...makeIssue(), severity: "urgent" }), null);
  assert.equal(parseWorkspaceIssue({ ...makeIssue(), fix_prompt: 7 }), null);
});

test("parseWorkspaceScanResult rejects malformed scan cache envelopes", () => {
  assert.equal(parseWorkspaceScanResult(null), null);
  assert.equal(parseWorkspaceScanResult({ url: "https://example.com", issues: [] }), null);
  assert.equal(
    parseWorkspaceScanResult({
      url: "https://example.com",
      overall_score: 90,
      timestamp: "not-a-date",
      issues: [],
    }),
    null,
  );
});

test("parseWorkspaceScanResult rejects the whole cache when any issue row is malformed", () => {
  const scan = parseWorkspaceScanResult({
    url: "https://example.com",
    overall_score: 90,
    timestamp: "2026-05-06T12:00:00.000Z",
    categories: [{ category: "security", score: 88 }],
    issues: [makeIssue(), { title: "bad row" }],
  });

  assert.equal(scan, null);
});

test("parseWorkspaceScanResult rejects the whole cache when a category row is malformed", () => {
  const scan = parseWorkspaceScanResult({
    url: "https://example.com",
    overall_score: 90,
    timestamp: "2026-05-06T12:00:00.000Z",
    categories: [{ category: "security", score: 88 }, { category: 7 }],
    issues: [makeIssue()],
  });

  assert.equal(scan, null);
});

test("getWorkspaceScan reports an invalid existing cache instead of returning no scan", () => {
  const root = mkdtempSync(join(tmpdir(), "sitecmd-workspace-cache-"));
  const sitecmdDir = join(root, ".sitecmd");
  mkdirSync(sitecmdDir);
  writeFileSync(
    join(sitecmdDir, "config.json"),
    JSON.stringify({ version: 1, url: "https://example.com", name: "Fixture" }),
  );
  writeFileSync(
    join(sitecmdDir, "last-scan.json"),
    JSON.stringify({
      url: "https://example.com",
      overall_score: 90,
      timestamp: "2026-05-06T12:00:00.000Z",
      issues: [makeIssue(), { title: "bad row" }],
    }),
  );

  const previousCwd = process.cwd();
  try {
    process.chdir(root);
    assert.throws(
      () => getWorkspaceScan("https://example.com"),
      /invalid envelope, issue row, or category row/,
    );
  } finally {
    process.chdir(previousCwd);
    rmSync(root, { recursive: true, force: true });
  }
});

test("parsePackageDependencyNames rejects malformed package manifests", () => {
  assert.equal(parsePackageDependencyNames(null), null);
  assert.equal(parsePackageDependencyNames([]), null);
  assert.equal(parsePackageDependencyNames("not a manifest"), null);
});

test("parsePackageDependencyNames reads dependency keys from object manifests", () => {
  const deps = parsePackageDependencyNames({
    dependencies: { astro: "^5.0.0", react: "^19.0.0" },
    devDependencies: { typescript: "^6.0.0" },
  });

  assert.equal(deps?.has("astro"), true);
  assert.equal(deps?.has("react"), true);
  assert.equal(deps?.has("typescript"), true);
});

/** Ignore an issue the way the desktop does: a project_issue_states row. */
function ignoreCheck(projectId, checkId) {
  setIssueState({ projectId, checkId, status: "ignored" });
}

function addProjectWithScanHistory(projectId, url = "https://variants.example.com") {
  ensureProject(fixtureDb, projectId, { name: "Variant URL project", framework: "astro" });
  const environmentId = Number(
    fixtureDb
      .prepare("INSERT INTO environments (project_id, environment, label, url) VALUES (?, ?, ?, ?)")
      .run(projectId, "production", "Production", url).lastInsertRowid,
  );
  fixtureDb
    .prepare("INSERT INTO sites (id, project_id, url) VALUES (?, ?, ?)")
    .run(projectId, projectId, url);
  fixtureDb
    .prepare(
      `INSERT INTO scan_executions (
        id, project_id, environment_id, environment_url, environment_scope_key,
        requested_mode, web_focus, trigger, admission_class, status,
        idempotency_key, request_fingerprint, started_at, completed_at,
        web_status
      ) VALUES (?, ?, ?, ?, ?, 'web', 'health', 'manual', 'general_scan', 'complete',
                ?, ?, ?, ?, 'complete')`,
    )
    .run(
      projectId,
      projectId,
      environmentId,
      url,
      url,
      `workspace-web-${projectId}`,
      `v1:workspace-web-${projectId}`,
      Date.parse("2026-05-06T12:00:00.000Z"),
      Date.parse("2026-05-06T12:00:01.200Z"),
    );
  fixtureDb
    .prepare(
      `INSERT INTO scan_runs (
        id, execution_id, project_id, site_id, environment_url, environment_scope_key,
        source, run_kind, status, focus, started_at, completed_at, timestamp_text,
        raw_score, duration_ms, coverage_kind, coverage_json, mode,
        security_score, performance_score, seo_score, accessibility_score,
        issues_total, issues_critical, issues_high
      ) VALUES (?, ?, ?, ?, ?, ?, 'web_scan', 'single', 'complete', 'health',
                ?, ?, ?, 88, 1200, 'site', '{"successful":true}', 'live',
                90, 80, 86, 92, 2, 1, 1)`,
    )
    .run(
      projectId,
      projectId,
      projectId,
      projectId,
      url,
      url,
      Date.parse("2026-05-06T12:00:00.000Z"),
      Date.parse("2026-05-06T12:00:01.200Z"),
      "2026-05-06T12:00:00.000Z",
    );
}

test("getIssuesForProject returns active web and code scan items from one source of truth", () => {
  const projectId = 501;
  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "security.hsts",
    checkId: "security.hsts",
    title: "Missing HSTS header",
  });
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: "code.security.raw-sql",
    checkId: "code.security.raw-sql",
    title: "Unsafe SQL construction",
  });
  addWorkItem({
    projectId,
    source: "updates",
    signalId: "updates.react",
    checkId: "updates.react",
    title: "React update available",
  });

  const issues = getIssuesForProject(projectId, "https://example.com", { status: "fail" });

  assert.deepEqual(issues.map((issue) => issue.check_id).sort(), [
    "code.security.raw-sql",
    "security.hsts",
  ]);
});

test("getIssuesForProject excludes ignored work items by default", () => {
  const projectId = 509;
  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "security.active",
    checkId: "security.active",
    title: "Active security issue",
  });
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: "code.security.ignored",
    checkId: "code.security.ignored",
    title: "Ignored code issue",
  });
  ignoreCheck(projectId, "code.security.ignored");

  const issues = getIssuesForProject(projectId, "https://example.com", { status: "fail" });

  assert.deepEqual(
    issues.map((issue) => issue.check_id),
    ["security.active"],
  );
});

test("getIssuesForProject does not pretend persisted work items contain pass or warn states", () => {
  const projectId = 502;
  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "seo.title",
    checkId: "seo.title",
    category: "seo",
    title: "Missing page title",
  });

  assert.equal(getIssuesForProject(projectId, "https://example.com", { status: "pass" }).length, 0);
  assert.equal(getIssuesForProject(projectId, "https://example.com", { status: "warn" }).length, 0);
});

test("MCP issue status contract only advertises statuses backed by persisted work items", () => {
  assert.deepEqual([...SUPPORTED_ISSUE_STATUSES], ["fail"]);
});

test("MCP DB URL lookups tolerate trailing slash variants", () => {
  const projectId = 507;
  addProjectWithScanHistory(projectId, "https://variants.example.com");

  const project = getProjectByUrl("https://variants.example.com/");
  const latestScan = getLatestScan("https://variants.example.com/");
  const scanHistory = getScanHistory("https://variants.example.com/");

  assert.equal(project?.id, projectId);
  assert.equal(latestScan?.scan_id, projectId);
  assert.deepEqual(
    scanHistory.map((scan) => scan.scan_id),
    [projectId],
  );
});

test("Web scan artifact counts exclude Code findings from a Full execution", () => {
  const projectId = 512;
  const url = "https://full-counts.example.com";
  addProjectWithScanHistory(projectId, url);
  fixtureDb
    .prepare(
      `UPDATE scan_executions
       SET requested_mode = 'full', code_status = 'complete'
       WHERE id = ?`,
    )
    .run(projectId);
  const codeRunId = 9512;
  fixtureDb
    .prepare(
      `INSERT INTO scan_runs (
        id, execution_id, project_id, environment_url, environment_scope_key,
        source, run_kind, status, started_at, completed_at, timestamp_text,
        raw_score, duration_ms, coverage_kind, coverage_json
      ) VALUES (?, ?, ?, ?, ?, 'code_scan', 'code', 'complete', ?, ?, ?,
                91, 900, 'project', '{"successful":true}')`,
    )
    .run(
      codeRunId,
      projectId,
      projectId,
      url,
      url,
      Date.parse("2026-05-06T12:00:00.000Z"),
      Date.parse("2026-05-06T12:00:00.900Z"),
      "2026-05-06T12:00:00.000Z",
    );
  const insertFinding = fixtureDb.prepare(
    `INSERT INTO scan_findings (
      run_id, ordinal, occurrence_id, source, canonical_check_id,
      producer_check_id, category, producer_category, verdict, severity,
      confidence, title, description, location_kind
    ) VALUES (?, ?, ?, ?, ?, ?, 'security', 'security', 'fail', ?,
              'high', ?, ?, ?)`,
  );
  insertFinding.run(
    projectId,
    0,
    "web:critical",
    "web_scan",
    "security.web-critical",
    "security.web-critical",
    "critical",
    "Web critical",
    "Web critical description",
    "page",
  );
  for (let ordinal = 0; ordinal < 3; ordinal += 1) {
    insertFinding.run(
      codeRunId,
      ordinal,
      `code:${ordinal}`,
      "code_scan",
      `code.security.${ordinal}`,
      `security.${ordinal}`,
      ordinal === 0 ? "critical" : "high",
      `Code finding ${ordinal}`,
      `Code finding ${ordinal} description`,
      "file",
    );
  }

  const latest = getLatestScan(url);
  const history = getScanHistory(url, 1);

  assert.equal(latest?.issues_total, 1);
  assert.equal(latest?.issues_critical, 1);
  assert.equal(latest?.issues_high, 0);
  assert.equal(history[0]?.issues_total, 1);
  assert.equal(history[0]?.issues_critical, 1);
  assert.equal(history[0]?.issues_high, 0);
});

test("getEffectiveTier rejects future validation timestamps", () => {
  fixtureDb
    .prepare(
      `
        INSERT OR REPLACE INTO license_state (
          id, license_key, instance_id, variant_id, tier, status,
          last_validated_at, activated_at, expires_at
        )
        VALUES (1, 'test-key', 'inst-1', 1, 'pro', 'active',
          '2099-01-01T00:00:00.000Z', '2026-05-01T00:00:00.000Z', NULL)
      `,
    )
    .run();

  assert.equal(getEffectiveTier(), "free");
});

test("getIssuesForProject treats severity as a minimum threshold", () => {
  const projectId = 506;
  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "security.critical-list",
    checkId: "security.critical-list",
    severity: "critical",
    title: "Critical list issue",
  });
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: "code.security.high-list",
    checkId: "code.security.high-list",
    severity: "high",
    title: "High list issue",
  });

  const issues = getIssuesForProject(projectId, "https://example.com", { min_severity: "high" });

  assert.deepEqual(
    issues.map((issue) => issue.check_id),
    ["security.critical-list", "code.security.high-list"],
  );
});

test("getFixPromptsForProject includes code scan prompts as well as web scan prompts", () => {
  const projectId = 503;
  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "security.csp",
    checkId: "security.csp",
    title: "Missing CSP header",
    fixPrompt: "Add a CSP header.",
  });
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: "code.security.xss",
    checkId: "code.security.xss",
    title: "Unsafe HTML rendering",
    fixPrompt: "Replace raw HTML rendering with sanitized output.",
  });

  const prompts = getFixPromptsForProject(projectId, "https://example.com");

  assert.deepEqual(prompts.map((prompt) => prompt.check_id).sort(), [
    "code.security.xss",
    "security.csp",
  ]);
});

test("getFixPromptsForProject excludes ignored issue prompts by default", () => {
  const projectId = 510;
  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "security.active-prompt",
    checkId: "security.active-prompt",
    title: "Active prompt issue",
    fixPrompt: "Fix the active issue.",
  });
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: "code.security.ignored-prompt",
    checkId: "code.security.ignored-prompt",
    title: "Ignored prompt issue",
    fixPrompt: "Do not suggest this ignored issue.",
  });
  ignoreCheck(projectId, "code.security.ignored-prompt");

  const prompts = getFixPromptsForProject(projectId, "https://example.com");

  assert.deepEqual(
    prompts.map((prompt) => prompt.check_id),
    ["security.active-prompt"],
  );
});

test("getFixPromptsForProject treats severity as a minimum threshold", () => {
  const projectId = 505;
  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "security.critical",
    checkId: "security.critical",
    severity: "critical",
    title: "Critical security issue",
    fixPrompt: "Fix the critical issue.",
  });
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: "code.security.high",
    checkId: "code.security.high",
    severity: "high",
    title: "High code issue",
    fixPrompt: "Fix the high issue.",
  });
  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "seo.medium",
    checkId: "seo.medium",
    category: "seo",
    severity: "medium",
    title: "Medium SEO issue",
    fixPrompt: "Fix the medium issue.",
  });

  const prompts = getFixPromptsForProject(projectId, "https://example.com", {
    min_severity: "high",
  });

  assert.deepEqual(prompts.map((prompt) => prompt.check_id).sort(), [
    "code.security.high",
    "security.critical",
  ]);
});

test("getIssueComparisonForProject classifies fixed, new, and remaining issues without a fake previous snapshot", () => {
  const projectId = 504;
  const previous = "2026-05-05T12:00:00.000Z";
  const latest = "2026-05-06T12:00:00.000Z";
  const previousMs = Date.parse(previous);
  const latestMs = Date.parse(latest);

  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "security.fixed",
    checkId: "security.fixed",
    title: "Fixed web issue",
    firstSeenAt: previousMs - 60_000,
    lastSeenAt: previousMs,
    resolvedAt: latestMs,
  });
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: "code.security.new",
    checkId: "code.security.new",
    title: "New code issue",
    firstSeenAt: previousMs + 60_000,
    lastSeenAt: latestMs,
  });
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: "code.security.remaining",
    checkId: "code.security.remaining",
    title: "Still failing code issue",
    firstSeenAt: previousMs - 60_000,
    lastSeenAt: latestMs,
  });
  addWorkItem({
    projectId,
    source: "web_scan",
    signalId: "security.remaining",
    checkId: "security.remaining",
    title: "Still failing web issue",
    firstSeenAt: previousMs - 60_000,
    lastSeenAt: latestMs,
  });
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: "code.security.future",
    checkId: "code.security.future",
    title: "Future code issue",
    firstSeenAt: latestMs + 60_000,
    lastSeenAt: latestMs + 60_000,
  });

  const webComparison = getIssueComparisonForProject(
    projectId,
    "https://example.com",
    previous,
    latest,
    "web_scan",
  );
  const codeComparison = getIssueComparisonForProject(
    projectId,
    "https://example.com",
    previous,
    latest,
    "code_scan",
  );

  assert.deepEqual(
    webComparison.fixed.map((issue) => issue.check_id),
    ["security.fixed"],
  );
  assert.deepEqual(
    webComparison.newIssues.map((issue) => issue.check_id),
    [],
  );
  assert.deepEqual(
    webComparison.remaining.map((issue) => issue.check_id),
    ["security.remaining"],
  );
  assert.deepEqual(
    codeComparison.fixed.map((issue) => issue.check_id),
    [],
  );
  assert.deepEqual(
    codeComparison.newIssues.map((issue) => issue.check_id),
    ["code.security.new"],
  );
  assert.deepEqual(
    codeComparison.remaining.map((issue) => issue.check_id),
    ["code.security.remaining"],
  );
});

test("getCodeScanHistoryForProject uses Code Scan timestamps instead of Web Scan windows", () => {
  const projectId = 508;
  ensureProject(fixtureDb, projectId, { name: "Code history project", framework: "astro" });

  const insertExecution = fixtureDb.prepare(
    `INSERT INTO scan_executions (
      id, project_id, environment_url, environment_scope_key, requested_mode,
      trigger, admission_class, status, idempotency_key, request_fingerprint,
      started_at, completed_at, code_status
    ) VALUES (?, ?, ?, ?, 'code', 'manual', 'general_scan', 'complete',
              ?, ?, ?, ?, 'complete')`,
  );
  const insertCodeScan = fixtureDb.prepare(
    `INSERT INTO scan_runs (
      id, execution_id, project_id, environment_url, environment_scope_key,
      source, run_kind, status, started_at, completed_at, timestamp_text,
      raw_score, duration_ms, coverage_kind, coverage_json, project_path,
      framework, issues_total, issues_critical, issues_high
    ) VALUES (?, ?, ?, ?, ?, 'code_scan', 'code', 'complete', ?, ?, ?, ?, ?,
              'project', '{"successful":true}', ?, ?, ?, ?, ?)`,
  );
  insertExecution.run(
    8001,
    projectId,
    "https://example.com",
    "https://example.com",
    "workspace-code-9001",
    "v1:workspace-code-9001",
    Date.parse("2026-05-06T12:00:00.000Z"),
    Date.parse("2026-05-06T12:00:01.200Z"),
  );
  insertCodeScan.run(
    9001,
    8001,
    projectId,
    "https://example.com",
    "https://example.com",
    Date.parse("2026-05-06T12:00:00.000Z"),
    Date.parse("2026-05-06T12:00:01.200Z"),
    "2026-05-06T12:00:00.000Z",
    82,
    1200,
    `/tmp/sitecmd-project-${projectId}`,
    "astro",
    3,
    1,
    2,
  );
  insertExecution.run(
    8002,
    projectId,
    "https://example.com/",
    "https://example.com/",
    "workspace-code-9002",
    "v1:workspace-code-9002",
    Date.parse("2026-05-07T12:00:00.000Z"),
    Date.parse("2026-05-07T12:00:00.900Z"),
  );
  insertCodeScan.run(
    9002,
    8002,
    projectId,
    "https://example.com/",
    "https://example.com/",
    Date.parse("2026-05-07T12:00:00.000Z"),
    Date.parse("2026-05-07T12:00:00.900Z"),
    "2026-05-07T12:00:00.000Z",
    91,
    900,
    `/tmp/sitecmd-project-${projectId}`,
    "astro",
    1,
    0,
    1,
  );

  const insertFinding = fixtureDb.prepare(
    `INSERT INTO scan_findings (
      run_id, ordinal, occurrence_id, source, canonical_check_id,
      producer_check_id, category, producer_category, domain, verdict,
      severity, confidence, title, description, location_kind
    ) VALUES (?, ?, ?, 'code_scan', ?, ?, 'security', 'security', 'security',
              'fail', ?, 'confirmed', ?, ?, 'file')`,
  );
  for (let ordinal = 0; ordinal < 3; ordinal += 1) {
    const rule = `history-old-${ordinal}`;
    insertFinding.run(
      9001,
      ordinal,
      `9001:${ordinal}`,
      `code_scan.${rule}`,
      rule,
      ordinal === 0 ? "critical" : "high",
      `Old finding ${ordinal}`,
      `Old finding ${ordinal} description`,
    );
  }
  insertFinding.run(
    9002,
    0,
    "9002:0",
    "code_scan.history-new",
    "history-new",
    "high",
    "New finding",
    "New finding description",
  );

  const history = getCodeScanHistoryForProject(projectId, "https://example.com", 2);

  assert.deepEqual(
    history.map((scan) => scan.scan_id),
    [9002, 9001],
  );
  assert.equal(history[0].issue_count, 1);
});

test("ignoring an issue the desktop way surfaces it in getDismissedIssues and hides it from getIssuesForProject", () => {
  const projectId = 511;
  addWorkItem({
    projectId,
    checkId: "security.dismissed-roundtrip",
    title: "Dismissed roundtrip issue",
  });
  addWorkItem({ projectId, checkId: "security.still-active" });
  ignoreCheck(projectId, "security.dismissed-roundtrip");

  const issues = getIssuesForProject(projectId, "https://example.com", { status: "fail" });
  assert.deepEqual(
    issues.map((issue) => issue.check_id),
    ["security.still-active"],
  );

  const dismissed = getDismissedIssues(projectId, "https://example.com");
  assert.equal(dismissed.length, 1);
  assert.equal(dismissed[0].check_id, "security.dismissed-roundtrip");
  assert.equal(dismissed[0].status, "ignored");
  assert.equal(dismissed[0].title, "Dismissed roundtrip issue");
  assert.ok(
    !Number.isNaN(Date.parse(dismissed[0].last_status_changed_at)),
    "last_status_changed_at must be an ISO timestamp",
  );
});

test("blocked and verified issues are dismissed; an expired snooze flips back to active", () => {
  const projectId = 512;
  const past = Date.now() - 60_000;
  const future = Date.now() + 60 * 60 * 1000;
  for (const checkId of [
    "security.blocked",
    "security.verified",
    "security.snoozed",
    "security.snooze-expired",
  ]) {
    addWorkItem({ projectId, checkId });
  }
  setIssueState({ projectId, checkId: "security.blocked", status: "blocked" });
  setIssueState({ projectId, checkId: "security.verified", status: "verified" });
  setIssueState({ projectId, checkId: "security.snoozed", status: "snoozed", snoozeUntil: future });
  setIssueState({
    projectId,
    checkId: "security.snooze-expired",
    status: "snoozed",
    snoozeUntil: past,
  });

  const issues = getIssuesForProject(projectId, "https://example.com", { status: "fail" });
  assert.deepEqual(
    issues.map((issue) => issue.check_id),
    ["security.snooze-expired"],
  );

  const dismissedIds = getDismissedIssues(projectId, "https://example.com")
    .map((d) => d.check_id)
    .sort();
  assert.deepEqual(dismissedIds, ["security.blocked", "security.snoozed", "security.verified"]);
});

test("issue-state rows are environment-scoped, matching the desktop store", () => {
  const projectId = 513;
  addWorkItem({ projectId, checkId: "security.env-scoped" });
  setIssueState({
    projectId,
    envUrl: "https://other.example.com",
    checkId: "security.env-scoped",
    status: "ignored",
  });

  const issues = getIssuesForProject(projectId, "https://example.com", { status: "fail" });
  assert.deepEqual(
    issues.map((issue) => issue.check_id),
    ["security.env-scoped"],
  );
  assert.equal(getDismissedIssues(projectId, "https://example.com").length, 0);
  assert.equal(getDismissedIssues(projectId, "https://other.example.com").length, 1);
});
