-- Stamp scans with the producing build and its check inventory.
-- Unstamped runs remain unattested; inventory rows are immutable so release
-- changes are not misattributed to deploys or fixes.

ALTER TABLE scan_runs ADD COLUMN engine_release TEXT;
ALTER TABLE scan_runs ADD COLUMN manifest_digest TEXT;
ALTER TABLE scan_runs ADD COLUMN canonicalizer INTEGER;
ALTER TABLE scan_runs ADD COLUMN crawl_profile INTEGER;
ALTER TABLE scan_runs ADD COLUMN execution_profile_json TEXT;
-- The captured basis: the scope revision the run was scoped by. The connected
-- protocol's currency predicate compares it, and locally it says whether two
-- runs even covered the same routes.
ALTER TABLE scan_runs ADD COLUMN scope_revision INTEGER;

CREATE TABLE IF NOT EXISTS engine_releases (
    engine_release TEXT NOT NULL,
    manifest_digest TEXT NOT NULL,
    manifest_schema INTEGER NOT NULL,
    canonicalizer INTEGER NOT NULL,
    crawl_profile INTEGER NOT NULL,
    recorded_at INTEGER NOT NULL,
    PRIMARY KEY (engine_release, manifest_digest)
);

CREATE TABLE IF NOT EXISTS engine_release_checks (
    engine_release TEXT NOT NULL,
    manifest_digest TEXT NOT NULL,
    check_id TEXT NOT NULL,
    -- Semantic compatibility hash; NULL means the producer has no versioned contract.
    contract TEXT,
    -- Execution-profile dimensions that must also match, as a JSON array.
    compare_on TEXT NOT NULL DEFAULT '[]',
    -- 1 when the id is a family PREFIX rather than a literal check id.
    family INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (engine_release, manifest_digest, check_id),
    FOREIGN KEY (engine_release, manifest_digest)
        REFERENCES engine_releases(engine_release, manifest_digest) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_engine_release_checks_release
    ON engine_release_checks(engine_release, manifest_digest);
