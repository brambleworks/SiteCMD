// Use the desktop schema snapshot so MCP tests cannot drift from production.
// Open the fixture before the first db.js query, which caches its connection.

import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import Database from "better-sqlite3";

const __dirname = dirname(fileURLToPath(import.meta.url));

const SCHEMA_SNAPSHOT_PATH = join(
  __dirname,
  "..",
  "..",
  "..",
  "desktop",
  "src-tauri",
  "src",
  "db",
  "schema_snapshot.sql",
);

export function openSchemaFixtureDb(prefix) {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  const dbPath = join(dir, "sitecmd.db");
  process.env.SITECMD_DB_PATH = dbPath;

  const db = new Database(dbPath);
  // The desktop opens its DB with foreign keys enforced; keep the fixture
  // honest so seeds that would violate real FK constraints fail here too.
  db.exec("PRAGMA foreign_keys = ON");
  db.exec(readFileSync(SCHEMA_SNAPSHOT_PATH, "utf8"));

  process.on("exit", () => {
    db.close();
    rmSync(dir, { recursive: true, force: true });
  });

  return db;
}

/** Insert the projects row FK-constrained seeds hang off (idempotent). */
export function ensureProject(db, projectId, overrides = {}) {
  db.prepare(
    `INSERT OR IGNORE INTO projects (id, name, path, framework, secret_namespace)
     VALUES (?, ?, ?, ?, ?)`,
  ).run(
    projectId,
    overrides.name ?? `project-${projectId}`,
    overrides.path ?? `/tmp/sitecmd-project-${projectId}`,
    overrides.framework ?? null,
    overrides.secretNamespace ?? `ns-${projectId}`,
  );
}

export function makeSeeders(db) {
  const insertWorkItem = db.prepare(`
    INSERT INTO work_items (
      project_id, env_url, source, signal_id, check_id, category, severity,
      title, description, scan_ref, first_seen_at, last_seen_at,
      resolved_at, page_url, fix_prompt
    ) VALUES (
      @projectId, @envUrl, @source, @signalId, @checkId, @category, @severity,
      @title, @description, @scanRef, @firstSeenAt, @lastSeenAt,
      @resolvedAt, @pageUrl, @fixPrompt
    )
  `);

  function addWorkItem(overrides) {
    ensureProject(db, overrides.projectId);
    const signalId = overrides.signalId ?? overrides.checkId ?? "security.hsts";
    insertWorkItem.run({
      projectId: overrides.projectId,
      envUrl: overrides.envUrl ?? "https://example.com",
      source: overrides.source ?? "web_scan",
      signalId,
      checkId: overrides.checkId ?? signalId,
      category: overrides.category ?? "security",
      severity: overrides.severity ?? "high",
      title: overrides.title ?? signalId,
      description: overrides.description ?? `${signalId} description`,
      scanRef: overrides.scanRef ?? null,
      firstSeenAt: overrides.firstSeenAt ?? Date.parse("2026-05-06T12:00:00.000Z"),
      lastSeenAt: overrides.lastSeenAt ?? Date.parse("2026-05-06T12:00:00.000Z"),
      resolvedAt: overrides.resolvedAt ?? null,
      pageUrl: overrides.pageUrl ?? null,
      fixPrompt: overrides.fixPrompt ?? null,
    });
  }

  function addEvent(overrides) {
    ensureProject(db, overrides.projectId);
    const result = db
      .prepare(
        `INSERT INTO events (project_id, event_type, title, metadata, occurred_at_ms)
         VALUES (?, ?, ?, ?, ?)`,
      )
      .run(
        overrides.projectId,
        overrides.eventType ?? "deploy",
        overrides.title ?? "Test event",
        overrides.metadata ?? null,
        overrides.occurredAtMs ?? Date.parse("2026-05-06T12:00:00.000Z"),
      );
    return result.lastInsertRowid;
  }

  function linkEventToCheckId(eventId, checkId) {
    db.prepare(`INSERT INTO site_event_check_ids (event_id, check_id) VALUES (?, ?)`).run(
      eventId,
      checkId,
    );
  }

  function addObservation(projectId, causeCheckId, effectCheckId, coResolved, coActive) {
    ensureProject(db, projectId);
    db.prepare(
      `INSERT INTO causal_link_observations
       (project_id, cause_check_id, effect_check_id, observed_at, co_active, co_resolved)
       VALUES (?, ?, ?, ?, ?, ?)`,
    ).run(projectId, causeCheckId, effectCheckId, Date.now(), coActive, coResolved);
  }

  function setIssueState(overrides) {
    ensureProject(db, overrides.projectId);
    const verifiedBy =
      overrides.status === "verified" ? (overrides.verifiedBy ?? "local_scan") : null;
    db.prepare(
      `INSERT INTO project_issue_states
       (project_id, env_url, check_id, status, snooze_until, verified_by,
        last_status_changed_at)
       VALUES (?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(project_id, env_url, check_id)
       DO UPDATE SET status = excluded.status,
                     snooze_until = excluded.snooze_until,
                     verified_by = excluded.verified_by,
                     last_status_changed_at = excluded.last_status_changed_at`,
    ).run(
      overrides.projectId,
      overrides.envUrl ?? "https://example.com",
      overrides.checkId,
      overrides.status,
      overrides.snoozeUntil ?? null,
      verifiedBy,
      overrides.lastStatusChangedAt ?? Date.parse("2026-05-06T12:30:00.000Z"),
    );
  }

  function addFixAttempt(overrides = {}) {
    const projectId = overrides.projectId ?? 1;
    ensureProject(db, projectId);
    const now = Date.now();
    const result = db
      .prepare(
        `INSERT INTO fix_attempts (
          project_id, env_url, check_id, agent_tool, status, brief_md, agent_summary,
          created_at, updated_at
        ) VALUES (
          @projectId, @envUrl, @checkId, @agentTool, @status, @briefMd, @agentSummary,
          @createdAt, @updatedAt
        )`,
      )
      .run({
        projectId,
        envUrl: overrides.envUrl ?? "https://example.com",
        checkId: overrides.checkId ?? "security.hsts",
        agentTool: overrides.agentTool ?? "claude-code",
        status: overrides.status ?? "briefed",
        briefMd: overrides.briefMd ?? "# Fix brief\n\nAdd the HSTS header.",
        agentSummary: overrides.agentSummary ?? null,
        createdAt: overrides.createdAt ?? now,
        updatedAt: overrides.updatedAt ?? now,
      });
    return Number(result.lastInsertRowid);
  }

  return {
    addWorkItem,
    addEvent,
    linkEventToCheckId,
    addObservation,
    setIssueState,
    addFixAttempt,
  };
}
