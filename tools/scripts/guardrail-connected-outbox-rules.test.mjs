import { describe, expect, it } from "vitest";
import { connectedOutboxFailures } from "./lib/guardrail-connected-outbox-rules.mjs";

const MIGRATION = "apps/desktop/src-tauri/src/db/migrations/022_connected_mutation_outbox.sql";
const OUTBOX = "apps/desktop/src-tauri/src/db/connected_outbox.rs";
const SITES = "apps/desktop/src-tauri/src/db/connected_sites.rs";
const LIFECYCLE = "apps/desktop/src-tauri/src/db/issue_states.rs";
const COMMANDS = "apps/desktop/src-tauri/src/commands/issues.rs";
const OTHER = "apps/desktop/src-tauri/src/background/sync.rs";
const OTHER_TESTS = "apps/desktop/src-tauri/src/db/migrations_tests.rs";
const CLI = "apps/desktop/src-tauri/src/cli/scan.rs";
const CLI_BINARY = "apps/desktop/src-tauri/crates/cli/src/main.rs";

function sources() {
  const files = {
    [MIGRATION]: `-- The installation token and every other credential live in the keychain.
CREATE TABLE IF NOT EXISTS connected_sites (
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    site_id TEXT NOT NULL,
    connected_at INTEGER NOT NULL,
    bootstrapped_at INTEGER,
    PRIMARY KEY (project_id, env_url)
);
CREATE TABLE IF NOT EXISTS connected_group_revisions (
    project_id INTEGER NOT NULL,
    env_url TEXT NOT NULL,
    check_id TEXT NOT NULL,
    state_revision INTEGER NOT NULL,
    pulled_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, env_url, check_id),
    FOREIGN KEY (project_id, env_url)
        REFERENCES connected_sites(project_id, env_url) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS connected_mutation_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    env_url TEXT NOT NULL,
    check_id TEXT NOT NULL,
    decision TEXT NOT NULL,
    snooze_until INTEGER,
    block_reason TEXT,
    based_on_revision INTEGER NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    decided_at INTEGER NOT NULL,
    UNIQUE (project_id, env_url, check_id),
    CHECK ((decision = 'snooze') = (snooze_until IS NOT NULL)),
    FOREIGN KEY (project_id, env_url)
        REFERENCES connected_sites(project_id, env_url) ON DELETE CASCADE
);`,
    [OUTBOX]: `pub enum GroupDecision { Reopen, Snooze { until: i64 }, Ignore, Block { reason: Option<String> }, ClaimFixed }
impl GroupDecision {
    pub fn lifecycle(&self) -> IssueLifecycle {
        match self {
            GroupDecision::ClaimFixed => IssueLifecycle::Verified {
                by: VerifiedBy::UserClaim,
            },
        }
    }
}
impl Database {
    pub fn record_group_decision(&self) -> Result<DecisionRecord, DbError> {
        self.execute(move |conn| {
            let tx = conn.unchecked_transaction()?;
            write_lifecycle_row(&tx, project_id, &env_url, &check_id, &lifecycle, now_ms)?;
            if !site.accepts_mutations() { return Ok(DecisionRecord::CarriedByBootstrap); }
            let based_on_revision = group_revision(&tx, project_id, &env_url, &check_id)?;
            let idempotency_key = mint_local_id("mut_")?;
            tx.query_row("INSERT INTO connected_mutation_outbox
                 (project_id, env_url, check_id, decision, based_on_revision, idempotency_key)
                 VALUES (?1, ?2, ?3, ?4, ?7, ?8)
                 ON CONFLICT(project_id, env_url, check_id) DO UPDATE SET
                     decision = excluded.decision,
                     based_on_revision = excluded.based_on_revision,
                     idempotency_key = excluded.idempotency_key
                 RETURNING id", params![], read)?;
            tx.commit()?;
            Ok(record)
        })?
    }
    pub fn settle_group_mutation(&self, id: i64, idempotency_key: &str) -> Result<bool, DbError> {
        conn.execute("DELETE FROM connected_mutation_outbox WHERE id = ?1 AND idempotency_key = ?2", params![])
    }
    pub fn record_mutation_conflict(&self) -> Result<bool, DbError> {
        tx.execute("UPDATE connected_mutation_outbox SET conflicted_at = ?3
              WHERE id = ?1 AND idempotency_key = ?2", params![])?;
        raise_group_revision(&tx, project_id, &env_url, &check_id, server_revision, now_ms)?;
        Ok(true)
    }
}`,
    [SITES]: `pub(super) fn raise_group_revision(conn: &Connection) -> Result<i64, DbError> {
    conn.execute("INSERT INTO connected_group_revisions VALUES (?1)
         ON CONFLICT(project_id, env_url, check_id) DO UPDATE SET
             state_revision = excluded.state_revision
         WHERE excluded.state_revision > connected_group_revisions.state_revision", params![])?;
    group_revision(conn, project_id, env_url, check_id)
}
impl Database {
    pub fn disconnect_site(&self) -> Result<(), DbError> {
        tx.execute("DELETE FROM connected_sites WHERE project_id = ?1 AND env_url = ?2", params![])?;
        tx.execute("DELETE FROM connected_site_watermarks WHERE project_id = ?1 AND env_url = ?2", params![])?;
        tx.commit()?;
        Ok(())
    }
}`,
    [LIFECYCLE]: `pub(super) fn write_lifecycle_row(conn: &Connection) -> Result<(), DbError> {
    conn.execute("INSERT INTO project_issue_states (status) VALUES (?1)", params![])?;
    Ok(())
}`,
    [COMMANDS]: `pub async fn snooze_issue() -> Result<(), String> {
    db.record_group_decision(p, &env, &check, GroupDecision::Snooze { until }, now_ms()).map(|_| ())
}
pub async fn ignore_issue() -> Result<(), String> {
    db.record_group_decision(p, &env, &check, GroupDecision::Ignore, now_ms()).map(|_| ())
}
pub async fn block_issue() -> Result<(), String> {
    db.record_group_decision(p, &env, &check, GroupDecision::Block { reason }, now_ms()).map(|_| ())
}
pub async fn mark_issue_fixed() -> Result<(), String> {
    db.record_group_decision(p, &env, &check, GroupDecision::ClaimFixed, now_ms()).map(|_| ())
}
pub async fn reopen_issue() -> Result<(), String> {
    db.record_group_decision(p, &env, &check, GroupDecision::Reopen, now_ms()).map(|_| ())
}
pub async fn verify_issue() -> Result<(), String> {
    db.set_issue_group_state(p, &env, &check, IssueLifecycle::Verified { by: VerifiedBy::LocalScan }, now_ms())
}`,
    [OTHER]: `fn drain() {}`,
    [OTHER_TESTS]: `conn.execute("INSERT INTO connected_mutation_outbox (decision) VALUES ('ignore')", [])`,
    [CLI]: `pub fn run_scan() {}`,
    [CLI_BINARY]: `fn main() {}`,
  };
  for (let index = 0; index < 120; index += 1) {
    files[`apps/desktop/src-tauri/src/filler_${index}.rs`] = "fn noop() {}";
  }
  return files;
}

function failures(mutate = () => {}) {
  const files = sources();
  mutate(files);
  const read = (file) => {
    if (!(file in files)) throw new Error(`no fixture for ${file}`);
    return files[file];
  };
  const exists = (file) => file in files;
  const listFiles = (dir, predicate) =>
    Object.keys(files).filter((file) => file.startsWith(`${dir}/`) && predicate(file));
  return connectedOutboxFailures(read, exists, listFiles).join("\n");
}

describe("the schema", () => {
  it("passes when every rule holds", () => {
    expect(failures()).toBe("");
  });

  it("fails when the site binding is gone", () => {
    expect(failures((files) => delete files[MIGRATION])).toContain("bind a project environment");
  });

  it("fails when the binding carries a credential", () => {
    expect(
      failures((files) => {
        files[MIGRATION] = files[MIGRATION].replace(
          "    site_id TEXT NOT NULL,",
          "    site_id TEXT NOT NULL,\n    installation_token TEXT NOT NULL,",
        );
      }),
    ).toContain("OS keychain");
  });

  it("fails when nothing records what was pulled about each group", () => {
    expect(
      failures((files) => {
        files[MIGRATION] = files[MIGRATION].replace(
          "CREATE TABLE IF NOT EXISTS connected_group_revisions",
          "CREATE TABLE IF NOT EXISTS something_else",
        );
      }),
    ).toContain("picture a decision is guarded against");
  });

  it("fails when a group can hold two pending decisions at once", () => {
    expect(
      failures((files) => {
        files[MIGRATION] = files[MIGRATION].replace(
          "    UNIQUE (project_id, env_url, check_id),\n",
          "",
        );
      }),
    ).toContain("one pending decision per group");
  });

  it("fails when the idempotency key is not unique", () => {
    expect(
      failures((files) => {
        files[MIGRATION] = files[MIGRATION].replace(
          "idempotency_key TEXT NOT NULL UNIQUE",
          "idempotency_key TEXT NOT NULL",
        );
      }),
    ).toContain("pre-assign a unique idempotency key");
  });

  it("fails when intent outlives the binding it was recorded under", () => {
    expect(
      failures((files) => {
        files[MIGRATION] = files[MIGRATION].replace(
          `    FOREIGN KEY (project_id, env_url)
        REFERENCES connected_sites(project_id, env_url) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS connected_mutation_outbox`,
          `    FOREIGN KEY (project_id, env_url) REFERENCES projects(id)
);
CREATE TABLE IF NOT EXISTS connected_mutation_outbox`,
        );
      }),
    ).toContain("scope pulled revisions and undelivered intent");
  });

  it("fails when a recorded snooze can have no deadline", () => {
    expect(
      failures((files) => {
        files[MIGRATION] = files[MIGRATION].replace(
          "    CHECK ((decision = 'snooze') = (snooze_until IS NOT NULL)),\n",
          "",
        );
      }),
    ).toContain("payload only that decision carries");
  });
});

describe("what a user can decide", () => {
  it("fails when the terminal decision is recorded as proof", () => {
    expect(
      failures((files) => {
        files[OUTBOX] = files[OUTBOX].replace("VerifiedBy::UserClaim", "VerifiedBy::LocalScan");
      }),
    ).toContain("terminal decision");
  });

  it("fails when the decision vocabulary gains a scan-proved variant", () => {
    expect(
      failures((files) => {
        files[OUTBOX] += `\nconst PROVEN: VerifiedBy = VerifiedBy::LocalScan;`;
      }),
    ).toContain("scan-proved or regressed variant");
  });

  it("fails when a regression becomes something a client can declare", () => {
    expect(
      failures((files) => {
        files[OUTBOX] = files[OUTBOX].replace(
          "pub enum GroupDecision { Reopen,",
          "pub enum GroupDecision {\n    Regressed,\n    Reopen,",
        );
      }),
    ).toContain("scan-proved or regressed variant");
  });
});

describe("the basis a decision was made under", () => {
  it("fails when a recorded decision can be rebased onto newer state", () => {
    expect(
      failures((files) => {
        files[OUTBOX] = files[OUTBOX].replace(
          "based_on_revision = excluded.based_on_revision,",
          "based_on_revision = ?9,",
        );
      }),
    ).toContain("captured at decision time");
  });

  it("fails when the revision is read outside the recording transaction", () => {
    expect(
      failures((files) => {
        files[OUTBOX] = files[OUTBOX].replace(
          "group_revision(&tx, project_id, &env_url, &check_id)?",
          "self.pulled_revision(project_id, &env_url, &check_id)?",
        );
      }),
    ).toContain("same transaction that records the decision");
  });

  it("fails when the local row and the recorded intent are written separately", () => {
    expect(
      failures((files) => {
        files[OUTBOX] = files[OUTBOX].replace(
          "write_lifecycle_row(&tx, project_id, &env_url, &check_id, &lifecycle, now_ms)?;",
          "self.set_issue_state(project_id, &env_url, &check_id, lifecycle, now_ms)?;",
        );
      }),
    ).toContain("one transaction");
  });

  it("fails when the idempotency key is not minted with the decision", () => {
    expect(
      failures((files) => {
        files[OUTBOX] = files[OUTBOX].replace('mint_local_id("mut_")?', "String::new()");
      }),
    ).toContain("mint the idempotency key with the decision");
  });

  it("fails when a decision is recorded before bootstrap has committed", () => {
    expect(
      failures((files) => {
        files[OUTBOX] = files[OUTBOX].replace("!site.accepts_mutations()", "false");
      }),
    ).toContain("bootstrap");
  });

  it("fails when a pulled revision can be lowered", () => {
    expect(
      failures((files) => {
        files[SITES] = files[SITES].replace(
          "\n         WHERE excluded.state_revision > connected_group_revisions.state_revision",
          "",
        );
      }),
    ).toContain("refuse to lower");
  });
});

describe("settling and conflicting", () => {
  it("fails when an acknowledgement can delete the decision that replaced it", () => {
    expect(
      failures((files) => {
        files[OUTBOX] = files[OUTBOX].replace(
          "DELETE FROM connected_mutation_outbox WHERE id = ?1 AND idempotency_key = ?2",
          "DELETE FROM connected_mutation_outbox WHERE id = ?1",
        );
      }),
    ).toContain("settle_group_mutation must be guarded");
  });

  it("fails when a conflict does not record the revision the service reported", () => {
    expect(
      failures((files) => {
        files[OUTBOX] = files[OUTBOX].replace(
          "raise_group_revision(&tx, project_id, &env_url, &check_id, server_revision, now_ms)?;",
          "",
        );
      }),
    ).toContain("record_mutation_conflict");
  });

  it("fails when disconnecting leaves the event watermark behind", () => {
    expect(
      failures((files) => {
        files[SITES] = files[SITES].replace(
          `        tx.execute("DELETE FROM connected_site_watermarks WHERE project_id = ?1 AND env_url = ?2", params![])?;\n`,
          "",
        );
      }),
    ).toContain("clear the event watermark");
  });
});

describe("one writer", () => {
  it("fails when the lifecycle upsert is copied instead of shared", () => {
    expect(
      failures((files) => {
        files[OUTBOX] +=
          `\nconn.execute("INSERT INTO project_issue_states (status) VALUES (?1)", params![])?;`;
      }),
    ).toContain("single lifecycle upsert");
  });

  it("fails when the raw lifecycle store records intent of its own", () => {
    expect(
      failures((files) => {
        files[LIFECYCLE] += `\nsuper::connected_outbox::record_group_decision(&tx)?;`;
      }),
    ).toContain("must record no intent");
  });

  it("fails when another module writes the outbox directly", () => {
    expect(
      failures((files) => {
        files[OTHER] =
          `conn.execute("INSERT INTO connected_mutation_outbox (decision) VALUES (?1)", p)?;`;
      }),
    ).toContain("Recorded intent has one writer");
  });

  it("allows tests to drive the schema directly", () => {
    expect(failures()).not.toContain("Recorded intent has one writer");
  });

  it("fails when the sweep stops seeing the tree", () => {
    expect(
      failures((files) => {
        for (const file of Object.keys(files)) {
          if (file.includes("/filler_")) delete files[file];
        }
      }),
    ).toContain("the enumeration broke");
  });
});

describe("a user decides, CI does not", () => {
  for (const [command, variant] of [
    ["snooze_issue", "GroupDecision::Snooze { until }"],
    ["ignore_issue", "GroupDecision::Ignore"],
    ["block_issue", "GroupDecision::Block { reason }"],
    ["mark_issue_fixed", "GroupDecision::ClaimFixed"],
    ["reopen_issue", "GroupDecision::Reopen"],
  ]) {
    it(`fails when ${command} writes the lifecycle store behind the outbox's back`, () => {
      expect(
        failures((files) => {
          files[COMMANDS] = files[COMMANDS].replace(
            `db.record_group_decision(p, &env, &check, ${variant}, now_ms()).map(|_| ())`,
            `db.set_issue_group_state(p, &env, &check, lifecycle, now_ms())`,
          );
        }),
      ).toContain(command);
    });
  }

  it("fails when the re-scan command records a decision", () => {
    expect(
      failures((files) => {
        files[COMMANDS] = files[COMMANDS].replace(
          "db.set_issue_group_state(p, &env, &check, IssueLifecycle::Verified { by: VerifiedBy::LocalScan }, now_ms())",
          "db.record_group_decision(p, &env, &check, GroupDecision::ClaimFixed, now_ms())",
        );
      }),
    ).toContain("never as a decision");
  });

  it("fails when the CI path decides a group's lifecycle", () => {
    expect(
      failures((files) => {
        files[CLI] = `db.record_group_decision(p, &env, &check, GroupDecision::Ignore, now)?;`;
      }),
    ).toContain("no user whose decision it would be");
  });

  it("fails when the CLI enumeration finds nothing", () => {
    expect(
      failures((files) => {
        delete files[CLI];
        delete files[CLI_BINARY];
      }),
    ).toContain("found no CLI sources");
  });
});
