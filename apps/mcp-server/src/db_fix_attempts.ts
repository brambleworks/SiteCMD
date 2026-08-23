import { getDb, getDbWrite } from "./db_connection.js";

// This is the only MCP module allowed to update existing fix_attempts rows.

const ACTIVE_FIX_ATTEMPT_STATUSES = ["briefed", "verify_requested", "verifying"] as const;
const SETTLED_FIX_ATTEMPT_STATUSES = ["verified", "verify_failed", "canceled", "expired"] as const;

export interface FixAttemptSummary {
  id: number;
  projectId: number;
  checkId: string;
  status: string;
  agentTool: string;
  createdAt: number;
}

export interface FixAttemptRow {
  id: number;
  project_id: number;
  env_url: string;
  check_id: string;
  producer_rule: string | null;
  status: string;
  agent_summary: string | null;
  failure_detail: string | null;
  verify_started_at: number | null;
  brief_fetched_at: number | null;
  created_at: number;
  updated_at: number;
}

function rethrowFixAttemptError(e: unknown): never {
  if (e instanceof Error && e.message.includes("no such table: fix_attempts")) {
    throw new Error("This SiteCMD app version does not support fix attempts yet. Update SiteCMD.");
  }
  throw e;
}

export function getFixBrief(attemptId: number): { briefMd: string; status: string } {
  const db = getDb();
  let row: { brief_md: string; status: string } | undefined;
  try {
    row = db.prepare("SELECT brief_md, status FROM fix_attempts WHERE id = ?").get(attemptId) as
      { brief_md: string; status: string } | undefined;
  } catch (e) {
    rethrowFixAttemptError(e);
  }
  if (!row) {
    throw new Error(
      `No fix attempt with id ${attemptId}. Ask the user to click "Fix with your agent" on the issue in SiteCMD.`,
    );
  }
  stampBriefFetched(attemptId);
  return { briefMd: row.brief_md, status: row.status };
}

/** Record only the first brief fetch without changing attempt status. */
function stampBriefFetched(attemptId: number): void {
  const db = getDbWrite();
  const now = Date.now();
  try {
    db.prepare(
      `UPDATE fix_attempts
       SET brief_fetched_at = ?, updated_at = ?
       WHERE id = ? AND brief_fetched_at IS NULL`,
    ).run(now, now, attemptId);
  } catch (e) {
    rethrowFixAttemptError(e);
  }
}

export function requestVerification(attemptId: number, summary: string): void {
  const db = getDbWrite();
  // The guarded update is the claim authority and avoids a cancellation race.
  let changes = 0;
  try {
    const info = db
      .prepare(
        `UPDATE fix_attempts
         SET status = 'verify_requested', agent_summary = ?, updated_at = ?
         WHERE id = ? AND status IN ('briefed', 'verify_requested')`,
      )
      .run(summary, Date.now(), attemptId);
    changes = Number(info.changes);
  } catch (e) {
    rethrowFixAttemptError(e);
  }
  if (changes === 0) {
    const row = db.prepare("SELECT status FROM fix_attempts WHERE id = ?").get(attemptId) as
      { status: string } | undefined;
    if (!row) {
      throw new Error(`No fix attempt with id ${attemptId}.`);
    }
    throw new Error(
      `Fix attempt ${attemptId} is already '${row.status}'; verification can only be requested while it is 'briefed' or 'verify_requested'. Ask the user to start a new fix attempt from the issue in SiteCMD.`,
    );
  }
}

export function getLatestFixAttemptForIssue(
  projectId: number,
  envUrl: string,
  checkId: string,
): { id: number; status: string; failure_detail: string | null } | null {
  const trimmed = envUrl.replace(/\/+$/, "");
  try {
    const row = getDb()
      .prepare(
        `SELECT id, status, failure_detail FROM fix_attempts
         WHERE project_id = ? AND env_url IN (?, ?) AND check_id = ?
         ORDER BY id DESC LIMIT 1`,
      )
      .get(projectId, trimmed, `${trimmed}/`, checkId) as
      { id: number; status: string; failure_detail: string | null } | undefined;
    return row ?? null;
  } catch (e) {
    rethrowFixAttemptError(e);
  }
}

type FixAttemptSummaryRow = {
  id: number;
  project_id: number;
  check_id: string;
  status: string;
  agent_tool: string;
  created_at: number;
};

function toSummary(row: FixAttemptSummaryRow): FixAttemptSummary {
  return {
    id: row.id,
    projectId: row.project_id,
    checkId: row.check_id,
    status: row.status,
    agentTool: row.agent_tool,
    createdAt: row.created_at,
  };
}

/** Open attempts (unbounded); settled ones are added and capped when include_settled is asked for. */
export function listFixAttempts(includeSettled = false): FixAttemptSummary[] {
  const db = getDb();
  try {
    const activePlaceholders = ACTIVE_FIX_ATTEMPT_STATUSES.map(() => "?").join(", ");
    const activeRows = db
      .prepare(
        `SELECT id, project_id, check_id, status, agent_tool, created_at
         FROM fix_attempts
         WHERE status IN (${activePlaceholders})
         ORDER BY id DESC`,
      )
      .all(...ACTIVE_FIX_ATTEMPT_STATUSES) as FixAttemptSummaryRow[];
    if (!includeSettled) return activeRows.map(toSummary);

    const settledPlaceholders = SETTLED_FIX_ATTEMPT_STATUSES.map(() => "?").join(", ");
    const settledRows = db
      .prepare(
        `SELECT id, project_id, check_id, status, agent_tool, created_at
         FROM fix_attempts
         WHERE status IN (${settledPlaceholders})
         ORDER BY id DESC
         LIMIT 20`,
      )
      .all(...SETTLED_FIX_ATTEMPT_STATUSES) as FixAttemptSummaryRow[];
    return [...activeRows, ...settledRows].map(toSummary);
  } catch (e) {
    rethrowFixAttemptError(e);
  }
}

export function getFixAttempt(attemptId: number): FixAttemptRow | null {
  try {
    const row = getDb()
      .prepare(
        `SELECT id, project_id, env_url, check_id, producer_rule, status, agent_summary, failure_detail,
                verify_started_at, brief_fetched_at, created_at, updated_at
         FROM fix_attempts WHERE id = ?`,
      )
      .get(attemptId) as FixAttemptRow | undefined;
    return row ?? null;
  } catch (e) {
    rethrowFixAttemptError(e);
  }
}
