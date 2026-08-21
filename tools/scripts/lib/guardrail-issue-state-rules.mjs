const DOSSIERS = [
  "apps/desktop/src/components/issues/WebIssueDossier.tsx",
  "apps/desktop/src/components/scan/CodeIssueDossier.tsx",
  "apps/desktop/src/components/dashboard/SeoIssueDossier.tsx",
  "apps/desktop/src/components/dashboard/UpdateDossier.tsx",
];

const BANNED_LIFECYCLE_SYMBOLS = [
  "setProjectWorkItemStatus",
  "record_project_work_item",
  "set_project_work_item_status",
  "get_project_work_item_status",
  "workItemStatusCache",
];

const ISSUE_ACTION_BAR = "apps/desktop/src/components/issues/IssueActionBar.tsx";
const WORK_ITEM_GROUPS = "apps/desktop/src-tauri/src/db/work_item_groups.rs";
const QUEUE_PROJECTION = "apps/desktop/src-tauri/src/commands/project_work_items.rs";
const BASELINE_SQL = "apps/desktop/src-tauri/src/db/migrations/001_baseline.sql";
const REQUIRED_ACTION_BAR_WRAPPERS = ["blockIssue", "ignoreIssue", "reopenIssue", "getIssueState"];

// Only observed absence may later become a regression.
const ISSUE_STATES_RS = "apps/desktop/src-tauri/src/db/issue_states.rs";
const PROVENANCE_MIGRATION =
  "apps/desktop/src-tauri/src/db/migrations/020_verification_provenance.sql";
const VOCAB_RS = "apps/desktop/src-tauri/crates/engine/src/vocab.rs";
const ISSUE_COMMANDS = "apps/desktop/src-tauri/src/commands/issues.rs";
const FIX_WATCHER = "apps/desktop/src-tauri/src/background/fix_attempt_watcher.rs";
const SCAN_PROJECTION = "apps/desktop/src-tauri/src/db/scan_run_projection.rs";
const WORK_ITEMS_RS = "apps/desktop/src-tauri/src/db/work_items.rs";

function functionBody(source, name) {
  const start = source.search(new RegExp(`fn ${name}\\b`));
  if (start === -1) return null;
  const open = source.indexOf("{", start);
  if (open === -1) return null;
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(open, index + 1);
    }
  }
  return null;
}

export function verificationProvenanceFailures(read, exists) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const vocab = exists(VOCAB_RS) ? read(VOCAB_RS) : "";
  check(
    /pub enum VerifiedBy/.test(vocab) && /pub fn proves_absence/.test(vocab),
    `${VOCAB_RS} must define VerifiedBy with proves_absence: whether a verification observed the issue was gone is the one question the regression path asks, and the desktop, the CLI, and the connected producer must ask it of the same definition.`,
  );

  const migration = exists(PROVENANCE_MIGRATION) ? read(PROVENANCE_MIGRATION) : "";
  check(
    /\(status = 'verified'\) = \(verified_by IS NOT NULL\)/.test(migration),
    `${PROVENANCE_MIGRATION} must keep the CHECK tying provenance to status in BOTH directions. Without it a verified row can exist with nobody attesting to it, and a stale prover can outlive the verification it described.`,
  );

  const store = exists(ISSUE_STATES_RS) ? read(ISSUE_STATES_RS) : "";
  check(
    /pub enum IssueLifecycle/.test(store) && /Verified\s*\{\s*by:\s*VerifiedBy/.test(store),
    `${ISSUE_STATES_RS} must write lifecycle through IssueLifecycle, whose Verified variant carries its prover. A status plus optional columns lets a caller record a verification without saying what verified it.`,
  );
  const setter = functionBody(store, "set_issue_state");
  check(
    setter !== null && !/status:\s*IssueStatus/.test(store),
    `${ISSUE_STATES_RS} set_issue_state must take an IssueLifecycle, not a bare status: the payload columns are not independent of the state.`,
  );

  const commands = exists(ISSUE_COMMANDS) ? read(ISSUE_COMMANDS) : "";
  const markFixed = functionBody(commands, "mark_issue_fixed");
  check(
    markFixed !== null && markFixed.includes("GroupDecision::ClaimFixed"),
    `${ISSUE_COMMANDS} mark_issue_fixed records the user's word and must record GroupDecision::ClaimFixed, the decision that writes VerifiedBy::UserClaim. Recording it as a scan result is the product claiming a proof it never made.`,
  );
  const verifyIssue = functionBody(commands, "verify_issue");
  check(
    verifyIssue !== null && verifyIssue.includes("VerifiedBy::LocalScan"),
    `${ISSUE_COMMANDS} verify_issue re-runs the check and must write VerifiedBy::LocalScan.`,
  );
  const watcher = exists(FIX_WATCHER) ? read(FIX_WATCHER) : "";
  check(
    /VerifiedBy::LocalScan/.test(watcher) && !/VerifiedBy::UserClaim/.test(watcher),
    `${FIX_WATCHER} settles an attempt only when the issue is no longer reported, so it must write VerifiedBy::LocalScan.`,
  );

  const reconciler = functionBody(store, "reconcile_reobserved_lifecycle");
  check(
    reconciler !== null && reconciler.includes("proves_absence()"),
    `${ISSUE_STATES_RS} must decide a re-observed verification's next state from proves_absence(). Branching on the status alone reports a claimed fix that was never real as a regression.`,
  );
  check(
    reconciler !== null && /verified_by = NULL/.test(reconciler),
    `${ISSUE_STATES_RS} re-observation must clear verified_by with the status; the schema CHECK rejects the row otherwise, and a leftover prover would describe a verification that no longer holds.`,
  );
  for (const caller of [SCAN_PROJECTION, WORK_ITEMS_RS]) {
    const source = exists(caller) ? read(caller) : "";
    check(
      source.includes("reconcile_reobserved_lifecycle"),
      `${caller} must call the shared reconcile_reobserved_lifecycle. Both scan paths reconcile the same rows; a second copy of the rule is how they start disagreeing.`,
    );
    check(
      !/IssueStatus::Regressed/.test(source),
      `${caller} must not write the regressed status itself. Regression is decided once, from the verification's provenance, in ${ISSUE_STATES_RS}.`,
    );
  }

  return failures;
}

export function issueStateSafetyFailures(read, exists, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  for (const file of DOSSIERS) {
    check(
      sourceFiles.includes(file),
      `Dossier missing -- update the issue-state guardrail list: ${file}`,
    );
  }

  const desktopFiles = sourceFiles.filter(
    (file) =>
      file.startsWith("apps/desktop/src") &&
      (file.endsWith(".ts") || file.endsWith(".tsx") || file.endsWith(".rs")),
  );
  check(
    desktopFiles.length > 200,
    `issue-state guardrail scanned only ${desktopFiles.length} desktop files - the file enumeration broke; update guardrail-issue-state-rules.mjs.`,
  );
  for (const file of desktopFiles) {
    const source = read(file);
    for (const symbol of BANNED_LIFECYCLE_SYMBOLS) {
      check(
        !source.includes(symbol),
        `${file} references ${symbol}: the project_work_items lifecycle store was deleted (audit F2). Lifecycle writes go through @/lib/issues -> set_issue_group_state -> project_issue_states.`,
      );
    }
  }

  check(
    exists(BASELINE_SQL) && !read(BASELINE_SQL).includes("project_work_items"),
    `${BASELINE_SQL} must not recreate project_work_items - the dashboard queue is a projection, not a store.`,
  );

  const projection = exists(QUEUE_PROJECTION) ? read(QUEUE_PROJECTION) : "";
  check(
    /get_active_issue_groups/.test(projection) && /active_fix_attempt_check_ids/.test(projection),
    `${QUEUE_PROJECTION} must build dashboard entries from get_active_issue_groups (lifecycle) + active_fix_attempt_check_ids (verify-in-flight) so the queue cannot disagree with the score.`,
  );

  if (!sourceFiles.includes(ISSUE_ACTION_BAR)) {
    check(false, `IssueActionBar missing -- update the issue-state guardrail: ${ISSUE_ACTION_BAR}`);
  } else {
    const source = read(ISSUE_ACTION_BAR);
    const importsFromIssues = /from\s+["']@\/lib\/issues["']/.test(source);
    const usesEveryWrapper = REQUIRED_ACTION_BAR_WRAPPERS.every((name) =>
      new RegExp(`\\b${name}\\b`).test(source),
    );
    check(
      importsFromIssues && usesEveryWrapper,
      `IssueActionBar must persist + hydrate issue lifecycle through @/lib/issues (${REQUIRED_ACTION_BAR_WRAPPERS.join(", ")}) so every dossier writes project_issue_states.`,
    );
  }

  check(
    exists(WORK_ITEM_GROUPS) && /get_issue_state_map/.test(read(WORK_ITEM_GROUPS)),
    `${WORK_ITEM_GROUPS} must merge get_issue_state_map so the SiteCMD score reads lifecycle status from project_issue_states.`,
  );

  // Code lifecycle writes use one canonical rule-level identity.
  const ISSUE_STATES = "apps/desktop/src-tauri/src/db/issue_states.rs";
  const issueStates = exists(ISSUE_STATES) ? read(ISSUE_STATES) : "";
  check(
    /pub fn set_issue_group_state/.test(issueStates) &&
      /self\.set_issue_state\(/.test(issueStates) &&
      !/resolve_code_issue_state_ids|for\s+.*location|LIKE\s+'code_scan\.%:%'/.test(issueStates),
    `${ISSUE_STATES} set_issue_group_state must write one canonical group directly; sibling lookup and per-location lifecycle fan-out are forbidden.`,
  );

  const MCP_DB = "apps/mcp-server/src/db.ts";
  const mcpDb = exists(MCP_DB) ? read(MCP_DB) : "";
  check(
    /FROM project_issue_states/.test(mcpDb) && !/FROM project_work_items/.test(mcpDb),
    `${MCP_DB} must read scan-issue lifecycle from project_issue_states, never project_work_items (dead store deleted by audit F2).`,
  );
  check(
    /effectiveIssueStatus\(/.test(mcpDb) &&
      /"snoozed", "ignored", "blocked", "verified"/.test(mcpDb),
    `${MCP_DB} dismissal reads must mirror the desktop's get_inactive_check_ids semantics: snoozed/ignored/blocked/verified with snooze-expiry flipping back to active.`,
  );

  return failures;
}
