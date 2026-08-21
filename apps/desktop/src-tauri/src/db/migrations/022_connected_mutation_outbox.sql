-- Lifecycle decisions retain their observed revision and idempotency key.
-- Bindings store no credentials; revisions are monotonic.
-- The outbox keeps one undelivered decision per group and records conflicts.

CREATE TABLE IF NOT EXISTS connected_sites (
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    site_id TEXT NOT NULL CHECK (length(site_id) > 0),
    connected_at INTEGER NOT NULL,
    bootstrapped_at INTEGER,
    PRIMARY KEY (project_id, env_url)
);

CREATE TABLE IF NOT EXISTS connected_group_revisions (
    project_id INTEGER NOT NULL,
    env_url TEXT NOT NULL,
    check_id TEXT NOT NULL,
    state_revision INTEGER NOT NULL CHECK (state_revision >= 0),
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
    decision TEXT NOT NULL CHECK (
        decision IN ('reopen', 'snooze', 'ignore', 'block', 'claim_fixed')
    ),
    snooze_until INTEGER,
    block_reason TEXT,
    based_on_revision INTEGER NOT NULL CHECK (based_on_revision >= 0),
    idempotency_key TEXT NOT NULL UNIQUE CHECK (length(idempotency_key) > 0),
    decided_at INTEGER NOT NULL,
    conflicted_at INTEGER,
    conflict_state TEXT,
    conflict_revision INTEGER,
    UNIQUE (project_id, env_url, check_id),
    CHECK ((decision = 'snooze') = (snooze_until IS NOT NULL)),
    CHECK (block_reason IS NULL OR decision = 'block'),
    CHECK ((conflicted_at IS NULL) = (conflict_state IS NULL)),
    CHECK ((conflicted_at IS NULL) = (conflict_revision IS NULL)),
    FOREIGN KEY (project_id, env_url)
        REFERENCES connected_sites(project_id, env_url) ON DELETE CASCADE
);
