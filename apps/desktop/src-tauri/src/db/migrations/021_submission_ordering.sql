-- Persist installation-scoped submission order and the last pulled site-event watermark.
-- Identity is independent of credentials so rotation cannot restart the sequence.
-- Watermarks are monotonic and stamped when execution starts, giving all runs
-- in one execution the same evidence basis.

CREATE TABLE IF NOT EXISTS connected_producer (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    installation_id TEXT NOT NULL CHECK (length(installation_id) > 0),
    submission_sequence INTEGER NOT NULL CHECK (submission_sequence >= 0),
    minted_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS connected_site_watermarks (
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    event_sequence INTEGER NOT NULL CHECK (event_sequence >= 0),
    pulled_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, env_url)
);

ALTER TABLE scan_executions ADD COLUMN based_on_event_sequence INTEGER NOT NULL DEFAULT 0;
