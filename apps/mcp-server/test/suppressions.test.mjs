import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  applyRepoSuppressions,
  codeFindingFingerprint,
  pathMatchesSuppression,
} from "../dist/suppressions.js";
import { getIssuesForProject, getRepoSuppressedIssues } from "../dist/db.js";
import { ensureProject, makeSeeders, openSchemaFixtureDb } from "./helpers/schema-fixture.mjs";

const fixtureDb = openSchemaFixtureDb("sitecmd-mcp-suppressions-");
const { addWorkItem } = makeSeeders(fixtureDb);
const RUST_VECTOR = "sha256:4522c6c0147aa43bbd24ea42bb759ad735e2dcddea6d2b83ed75de0fa5bfb1a6";
const TODAY = new Date("2026-08-22T00:00:00.000Z");

function projectWithConfig(projectId, suppressions) {
  const root = mkdtempSync(join(tmpdir(), "sitecmd-mcp-suppressed-project-"));
  mkdirSync(join(root, ".sitecmd"));
  writeFileSync(
    join(root, ".sitecmd", "config.json"),
    JSON.stringify({
      version: 1,
      url: "https://example.com",
      name: "suppression fixture",
      code_scan: { suppressions },
    }),
  );
  ensureProject(fixtureDb, projectId, { path: root });
  return root;
}

// Real code_scan rows carry detail_json serialized from Rust's CodeIssue,
// which is #[serde(rename_all = "camelCase")]. This is the shape production
// data actually has, so it is the default every test below exercises.
function codeFinding(projectId, checkId, relativePath, excerpt) {
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: `code_scan:${checkId}:${relativePath}`,
    checkId,
    severity: "high",
    title: `Finding in ${relativePath}`,
    relativePath,
    line: 10,
    detailJson: JSON.stringify({
      id: `${checkId.replace("code_scan.", "")}:${relativePath}`,
      checkId,
      relativePath,
      sourceExcerpt: excerpt,
      evidence: null,
    }),
  });
}

// Only the fallback test below seeds this legacy snake_case shape; it proves
// identityOf's snake_case branch still resolves an identity, in case some
// older row was ever persisted with those keys.
function legacyCodeFinding(projectId, checkId, relativePath, excerpt) {
  addWorkItem({
    projectId,
    source: "code_scan",
    signalId: `code_scan:${checkId}:${relativePath}`,
    checkId,
    severity: "high",
    title: `Finding in ${relativePath}`,
    relativePath,
    line: 10,
    detailJson: JSON.stringify({
      id: `${checkId.replace("code_scan.", "")}:${relativePath}`,
      check_id: checkId,
      relative_path: relativePath,
      source_excerpt: excerpt,
      evidence: null,
    }),
  });
}

test("fingerprints match the CLI vector and survive line movement", () => {
  const identity = {
    check_id: "code_scan.hardcoded-secret",
    relative_path: "src/config.ts",
    occurrence: "const secret = 'fixture';",
  };
  assert.equal(codeFindingFingerprint(identity), RUST_VECTOR);
  assert.equal(
    codeFindingFingerprint({ ...identity, occurrence: "  const   secret = 'fixture'; " }),
    RUST_VECTOR,
  );
  assert.notEqual(
    codeFindingFingerprint({ ...identity, occurrence: "const secret = 'different';" }),
    RUST_VECTOR,
  );
});

test("path patterns follow the gitignore semantics the CLI uses", () => {
  assert.ok(pathMatchesSuppression("examples/**", "examples/insecure.ts"));
  assert.ok(pathMatchesSuppression("examples", "examples/deep/insecure.ts"));
  assert.ok(pathMatchesSuppression("src/keys.js", "src/keys.js"));
  assert.ok(pathMatchesSuppression("*.fixture.ts", "src/deep/thing.fixture.ts"));
  assert.ok(!pathMatchesSuppression("examples/**", "src/config.ts"));
  assert.ok(!pathMatchesSuppression("src/keys.js", "src/keys.jsx"));
});

// codeFinding seeds the real camelCase detail_json shape, so this is the
// product-path assertion: a suppressed camelCase row is hidden from
// getIssuesForProject (backs get_issues) and listed by getRepoSuppressedIssues
// (backs get_dismissed_issues), with the configured reason attached.
test("rule plus path suppression hides the finding and reports it as suppressed", () => {
  const projectId = 701;
  projectWithConfig(projectId, [
    {
      match: { path: "examples/**", rule: "code_scan.hardcoded-secret" },
      reason: "The examples contain inert security fixtures.",
    },
  ]);
  codeFinding(
    projectId,
    "code_scan.hardcoded-secret",
    "examples/insecure.ts",
    "const secret = 'fixture';",
  );
  codeFinding(projectId, "code_scan.hardcoded-secret", "src/config.ts", "const secret = 'real';");

  const issues = getIssuesForProject(projectId, "https://example.com");
  assert.deepEqual(
    issues.map((issue) => issue.relative_path),
    ["src/config.ts"],
  );

  const suppressed = getRepoSuppressedIssues(projectId, "https://example.com");
  assert.equal(suppressed.length, 1);
  assert.equal(suppressed[0].issue.relative_path, "examples/insecure.ts");
  assert.equal(suppressed[0].reason, "The examples contain inert security fixtures.");
});

test("skipped scan evidence remains visible through get_dismissed_issues", () => {
  const projectId = 705;
  projectWithConfig(projectId, [
    {
      match: { path: "examples/**", rule: "code_scan.hardcoded-secret" },
      reason: "The examples contain inert security fixtures.",
    },
  ]);
  const executionId = 1705;
  const runId = 2705;
  fixtureDb
    .prepare(
      `INSERT INTO scan_executions (
        id, project_id, environment_url, environment_scope_key, requested_mode,
        trigger, admission_class, status, idempotency_key, request_fingerprint,
        started_at, completed_at, code_status
      ) VALUES (?, ?, 'https://example.com', 'https://example.com', 'code',
                'manual', 'general_scan', 'complete', ?, ?, 1000, 2000, 'complete')`,
    )
    .run(executionId, projectId, `suppression-${projectId}`, `suppression-${projectId}`);
  fixtureDb
    .prepare(
      `INSERT INTO scan_runs (
        id, execution_id, project_id, environment_url, environment_scope_key,
        source, run_kind, status, started_at, completed_at, timestamp_text,
        raw_score, duration_ms, coverage_kind, coverage_json, issues_total,
        project_path
      ) VALUES (?, ?, ?, 'https://example.com', 'https://example.com',
                'code_scan', 'code', 'complete', 1000, 2000,
                '2026-08-22T00:00:00Z', 100, 1000, 'project',
                '{"successful":true}', 0, ?)`,
    )
    .run(runId, executionId, projectId, `/tmp/sitecmd-project-${projectId}`);
  const detailJson = JSON.stringify({
    id: "hardcoded-secret:examples/insecure.ts",
    checkId: "code_scan.hardcoded-secret",
    relativePath: "examples/insecure.ts",
    sourceExcerpt: "const secret = 'fixture';",
    evidence: null,
  });
  fixtureDb
    .prepare(
      `INSERT INTO scan_findings (
        run_id, ordinal, occurrence_id, source, canonical_check_id,
        producer_check_id, category, producer_category, domain, verdict,
        severity, confidence, title, description, detail_json, location_kind,
        relative_path, line
      ) VALUES (?, 0, 'code_scan:hardcoded-secret:examples/insecure.ts:10',
                'code_scan', 'code_scan.hardcoded-secret', 'hardcoded-secret',
                'security', 'security', 'security', 'skipped', 'high', 'high',
                'Finding in examples/insecure.ts', 'Fixture finding', ?, 'file',
                'examples/insecure.ts', 10)`,
    )
    .run(runId, detailJson);

  const suppressed = getRepoSuppressedIssues(projectId, "https://example.com");
  assert.equal(suppressed.length, 1);
  assert.equal(suppressed[0].issue.relative_path, "examples/insecure.ts");
  assert.equal(suppressed[0].reason, "The examples contain inert security fixtures.");
});

test("fingerprint suppression hides only the exact occurrence", () => {
  const projectId = 702;
  projectWithConfig(projectId, [
    {
      match: { fingerprint: RUST_VECTOR },
      reason: "This exact occurrence is an inert test fixture.",
    },
  ]);
  codeFinding(
    projectId,
    "code_scan.hardcoded-secret",
    "src/config.ts",
    "const secret = 'fixture';",
  );
  codeFinding(
    projectId,
    "code_scan.hardcoded-secret",
    "src/other.ts",
    "const secret = 'different';",
  );

  const issues = getIssuesForProject(projectId, "https://example.com");
  assert.deepEqual(
    issues.map((issue) => issue.relative_path),
    ["src/other.ts"],
  );
});

test("expired suppressions keep the finding visible", () => {
  const rows = [
    {
      check_id: "code_scan.hardcoded-secret",
      source: "code_scan",
      relative_path: "src/config.ts",
      detail_json: JSON.stringify({
        id: "x",
        checkId: "code_scan.hardcoded-secret",
        relativePath: "src/config.ts",
        sourceExcerpt: "const secret = 'fixture';",
      }),
    },
  ];
  const root = mkdtempSync(join(tmpdir(), "sitecmd-mcp-expired-"));
  mkdirSync(join(root, ".sitecmd"));
  writeFileSync(
    join(root, ".sitecmd", "config.json"),
    JSON.stringify({
      version: 1,
      url: "https://example.com",
      name: "expired",
      code_scan: {
        suppressions: [
          {
            match: { rule: "code_scan.hardcoded-secret" },
            reason: "Temporary.",
            expires: "2026-08-18",
          },
        ],
      },
    }),
  );
  const view = applyRepoSuppressions(root, rows, TODAY);
  assert.equal(view.kept.length, 1);
  assert.equal(view.ignored.length, 0);
});

test("an invalid suppression fails the read with the CLI message", () => {
  const projectId = 703;
  projectWithConfig(projectId, [{ match: { rule: "code_scan.hardcoded-secret" }, reason: "  " }]);
  codeFinding(
    projectId,
    "code_scan.hardcoded-secret",
    "src/config.ts",
    "const secret = 'fixture';",
  );
  assert.throws(
    () => getIssuesForProject(projectId, "https://example.com"),
    /Code Scan suppression 1 requires a non-empty reason/,
  );
});

test("a legacy snake_case detail_json still resolves an identity for suppression", () => {
  const projectId = 704;
  projectWithConfig(projectId, [
    {
      match: { rule: "code_scan.hardcoded-secret" },
      reason: "Legacy snake_case rows must still suppress.",
    },
  ]);
  legacyCodeFinding(
    projectId,
    "code_scan.hardcoded-secret",
    "src/legacy.ts",
    "const secret = 'fixture';",
  );

  const issues = getIssuesForProject(projectId, "https://example.com");
  assert.deepEqual(
    issues.map((issue) => issue.relative_path),
    [],
  );

  const suppressed = getRepoSuppressedIssues(projectId, "https://example.com");
  assert.equal(suppressed.length, 1);
  assert.equal(suppressed[0].issue.relative_path, "src/legacy.ts");
  assert.equal(suppressed[0].reason, "Legacy snake_case rows must still suppress.");
});
