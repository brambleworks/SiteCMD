import { getDb, getDbWrite } from "./db_connection.js";

// This is the only MCP module allowed to update existing fix_attempts rows.

const ACTIVE_FIX_ATTEMPT_STATUSES = ["briefed", "verify_requested", "verifying"] as const;

export interface FixAttemptSummary {
  id: number;
  projectId: number;
  checkId: string;
  status: string;
  agentTool: string;
  createdAt: number;
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

export function listFixAttempts(): FixAttemptSummary[] {
  const db = getDb();
  try {
    const placeholders = ACTIVE_FIX_ATTEMPT_STATUSES.map(() => "?").join(", ");
    const rows = db
      .prepare(
        `SELECT id, project_id, check_id, status, agent_tool, created_at
         FROM fix_attempts
         WHERE status IN (${placeholders})
         ORDER BY id`,
      )
      .all(...ACTIVE_FIX_ATTEMPT_STATUSES) as Array<{
      id: number;
      project_id: number;
      check_id: string;
      status: string;
      agent_tool: string;
      created_at: number;
    }>;
    return rows.map((r) => ({
      id: r.id,
      projectId: r.project_id,
      checkId: r.check_id,
      status: r.status,
      agentTool: r.agent_tool,
      createdAt: r.created_at,
    }));
  } catch (e) {
    rethrowFixAttemptError(e);
  }
}
