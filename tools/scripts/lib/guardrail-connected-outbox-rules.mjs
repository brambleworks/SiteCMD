const MIGRATION = "apps/desktop/src-tauri/src/db/migrations/022_connected_mutation_outbox.sql";
const OUTBOX = "apps/desktop/src-tauri/src/db/connected_outbox.rs";
const SITES = "apps/desktop/src-tauri/src/db/connected_sites.rs";
const LIFECYCLE = "apps/desktop/src-tauri/src/db/issue_states.rs";
const COMMANDS = "apps/desktop/src-tauri/src/commands/issues.rs";

// Site bindings must never persist credentials from the OS keychain.
const CREDENTIAL_WORDS = ["token", "secret", "password", "credential"];

// The five transitions a user can decide, and the command that decides each.
const DECISION_COMMANDS = [
  ["snooze_issue", "GroupDecision::Snooze"],
  ["ignore_issue", "GroupDecision::Ignore"],
  ["block_issue", "GroupDecision::Block"],
  ["mark_issue_fixed", "GroupDecision::ClaimFixed"],
  ["reopen_issue", "GroupDecision::Reopen"],
];

// Test modules may legitimately drive the schema directly.
function isProdRustFile(file) {
  if (/[/\\]tests[/\\]/.test(file)) return false;
  return !/(_tests?\.rs|^tests\.rs)$/.test(file.split(/[/\\]/).pop());
}

// Extract a Rust function through its matching closing brace.
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

/**
 * @param {(file: string) => string} read
 * @param {(file: string) => boolean} exists
 * @param {(dir: string, predicate: (file: string) => boolean) => string[]} listFiles
 */
export function connectedOutboxFailures(read, exists, listFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };
  const source = (file) => (exists(file) ? read(file) : "");

  const migration = source(MIGRATION);
  check(
    /CREATE TABLE IF NOT EXISTS connected_sites\b/.test(migration) &&
      /PRIMARY KEY \(project_id, env_url\)/.test(migration),
    `${MIGRATION} must bind a project environment to one connected site. Without the binding nothing can answer whether a decision is owed to anyone, or whether bootstrap has committed.`,
  );
  // Columns only: the header prose says where the credential does live.
  const columns = migration.replace(/^\s*--.*$/gm, "");
  for (const word of CREDENTIAL_WORDS) {
    check(
      !new RegExp(word, "i").test(columns),
      `${MIGRATION} mentions ${word}: the site binding must hold no credential. The installation token belongs in the OS keychain, not in a row that every backup of the scan history copies.`,
    );
  }
  check(
    /CREATE TABLE IF NOT EXISTS connected_group_revisions\b/.test(migration),
    `${MIGRATION} must store the group revisions this installation has pulled: they are the picture a decision is guarded against, and they cannot be recovered after the decision is made.`,
  );
  check(
    /CREATE TABLE IF NOT EXISTS connected_mutation_outbox\b/.test(migration) &&
      /UNIQUE \(project_id, env_url, check_id\)/.test(migration),
    `${MIGRATION} must keep one pending decision per group. Two entries guarding the same revision ask the service to apply one group twice inside a single atomic batch.`,
  );
  check(
    /idempotency_key TEXT NOT NULL UNIQUE/.test(migration),
    `${MIGRATION} must pre-assign a unique idempotency key per recorded decision, so a crash between sending and hearing back resubmits the same request instead of a second one that reads as a new decision.`,
  );
  check(
    (migration.match(/REFERENCES connected_sites\(project_id, env_url\) ON DELETE CASCADE/g) || [])
      .length >= 2,
    `${MIGRATION} must scope pulled revisions and undelivered intent to the binding they were recorded under: kept past a disconnect, they would guard a mutation with a revision from a stream this installation never pulled.`,
  );
  check(
    /\(decision = 'snooze'\) = \(snooze_until IS NOT NULL\)/.test(migration),
    `${MIGRATION} must tie a recorded decision to the payload only that decision carries, the way the lifecycle table ties provenance to status: a snooze with no deadline is a dismissal that never expires.`,
  );

  const outbox = source(OUTBOX);
  check(
    /pub enum GroupDecision/.test(outbox) &&
      /GroupDecision::ClaimFixed => IssueLifecycle::Verified \{\s*by: VerifiedBy::UserClaim/.test(
        outbox,
      ),
    `${OUTBOX} must map the user's terminal decision to VerifiedBy::UserClaim. It is a claim: nothing looked, so a later scan that still finds the issue returns it to the list rather than announcing a regression.`,
  );
  check(
    !/VerifiedBy::LocalScan/.test(outbox) && !/^\s*Regressed\b/m.test(outbox),
    `${OUTBOX} must not give the decision vocabulary a scan-proved or regressed variant. A verification is evidence the service derives from a snapshot, and a regression is observed rather than declared; a variant for either would let this installation assert a state the contract reserves for the side that checked.`,
  );
  check(
    !/(?<!let )based_on_revision\s*=(?!\s*excluded\.based_on_revision)/.test(outbox),
    `${OUTBOX} may only ever write based_on_revision from a value captured at decision time (excluded.based_on_revision). Assigning it from anything else relabels an old decision as based on state the user never saw, which is exactly what the revision guard exists to catch.`,
  );

  const recorder = functionBody(outbox, "record_group_decision");
  check(
    recorder !== null &&
      /unchecked_transaction/.test(recorder) &&
      /write_lifecycle_row\(&tx/.test(recorder),
    `${OUTBOX} record_group_decision must write the lifecycle row and the recorded intent in one transaction. Split, the app can show a state the service is never told about, or owe a mutation the app does not reflect.`,
  );
  check(
    recorder !== null && /group_revision\(&tx/.test(recorder),
    `${OUTBOX} record_group_decision must read the group's revision inside the same transaction that records the decision. Reading it at submission time is the silent rebase.`,
  );
  check(
    recorder !== null && /mint_local_id\("mut_"\)/.test(recorder),
    `${OUTBOX} record_group_decision must mint the idempotency key with the decision. A key assigned at send time makes a retry after a crash a second decision rather than the same request.`,
  );
  check(
    recorder !== null && /accepts_mutations\(\)/.test(recorder),
    `${OUTBOX} record_group_decision must skip the outbox until bootstrap has committed: the bootstrap payload carries every group's current state, so recording a mutation as well submits the same decision twice.`,
  );

  const settle = functionBody(outbox, "settle_group_mutation");
  check(
    settle !== null && /id = \?1 AND idempotency_key = \?2/.test(settle),
    `${OUTBOX} settle_group_mutation must be guarded by the idempotency key. While a mutation was in flight the user may have decided something else for that group, and the replacement reuses the row; settling by id alone drops a decision the service has never heard.`,
  );
  const conflict = functionBody(outbox, "record_mutation_conflict");
  check(
    conflict !== null &&
      /id = \?1 AND idempotency_key = \?2/.test(conflict) &&
      /raise_group_revision\(/.test(conflict),
    `${OUTBOX} record_mutation_conflict must be key-guarded and must record the revision the service reported: hearing the current revision is how the user's next decision comes to be based on what the service actually holds.`,
  );

  const sites = source(SITES);
  const disconnect = functionBody(sites, "disconnect_site");
  check(
    disconnect !== null && /DELETE FROM connected_site_watermarks/.test(disconnect),
    `${SITES} disconnect_site must clear the event watermark with the binding. Left behind, the next scan of a newly connected site declares a basis taken from a stream it never pulled, which is the overstatement that lets stale evidence announce a regression.`,
  );
  check(
    /excluded\.state_revision > connected_group_revisions\.state_revision/.test(sites),
    `${SITES} must refuse to lower a pulled group revision: a reordered or replayed read would otherwise make the next decision guard against state older than what the user was shown.`,
  );

  const lifecycle = source(LIFECYCLE);
  check(
    /pub\(super\) fn write_lifecycle_row/.test(lifecycle) &&
      !/INSERT INTO project_issue_states/.test(outbox),
    `${LIFECYCLE} must own the single lifecycle upsert that both the plain setter and the decision path write through. A second copy lets the outbox describe a state this app never entered.`,
  );
  check(
    !/connected_mutation_outbox|record_group_decision/.test(lifecycle),
    `${LIFECYCLE} must record no intent. The scan paths and the re-observation reconciler write it, and their transitions are either evidence or the execution of a dismissal policy the service runs itself; recording a mutation would apply them a second time.`,
  );

  const commands = source(COMMANDS);
  for (const [command, variant] of DECISION_COMMANDS) {
    const body = functionBody(commands, command);
    check(
      body !== null && body.includes("record_group_decision") && body.includes(variant),
      `${COMMANDS} ${command} must record ${variant} through record_group_decision, so the decision reaches the outbox with the revision it was made under.`,
    );
  }
  const verify = functionBody(commands, "verify_issue");
  check(
    verify !== null && !verify.includes("record_group_decision"),
    `${COMMANDS} verify_issue re-runs the check and observes the result. It travels to the service as a snapshot, never as a decision.`,
  );

  // CI has no user decision and may never write group intent.
  const rustFiles = listFiles("apps/desktop/src-tauri/src", (file) => file.endsWith(".rs"));
  check(
    rustFiles.length > 100,
    `connected-outbox guardrail scanned only ${rustFiles.length} Rust files; the enumeration broke. Update guardrail-connected-outbox-rules.mjs.`,
  );
  for (const file of rustFiles) {
    if (file === OUTBOX || !isProdRustFile(file)) continue;
    check(
      !read(file).includes("INSERT INTO connected_mutation_outbox"),
      `${file} writes the outbox directly. Recorded intent has one writer, because the row and the lifecycle write it accompanies have to be decided together.`,
    );
  }
  const ciFiles = [
    ...listFiles("apps/desktop/src-tauri/src/cli", (file) => file.endsWith(".rs")),
    ...listFiles("apps/desktop/src-tauri/crates/cli/src", (file) => file.endsWith(".rs")),
    ...listFiles("apps/desktop/src-tauri/examples", (file) => file.endsWith(".rs")),
  ];
  check(
    ciFiles.length > 0,
    "connected-outbox guardrail found no CLI sources; update guardrail-connected-outbox-rules.mjs.",
  );
  for (const file of ciFiles) {
    check(
      !read(file).includes("record_group_decision"),
      `${file} records a lifecycle decision: a CI submission cannot touch groups, and a checkout has no user whose decision it would be.`,
    );
  }

  return failures;
}
