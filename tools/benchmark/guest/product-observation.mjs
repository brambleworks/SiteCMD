import { DatabaseSync } from "node:sqlite";
import { setTimeout as delay } from "node:timers/promises";

export function readFix(database, id) {
  const db = new DatabaseSync(database, { readOnly: true });
  try {
    return db
      .prepare(
        "SELECT id, project_id, check_id, status, failure_detail, verify_started_at, updated_at FROM fix_attempts WHERE id = ?",
      )
      .get(id);
  } finally {
    db.close();
  }
}

export async function observeVerification(database, id, mcp, log, deadline) {
  let final;
  do {
    final = readFix(database, id);
    if (!final || !["briefed", "verify_requested", "verifying"].includes(final.status)) break;
    await delay(1000);
  } while (Date.now() < deadline);
  const toolResponse = await mcp.call("get_fix_status", { attempt_id: id });
  log("mcp.jsonl", { observation: "final-verification", final, toolResponse });
  return final;
}
