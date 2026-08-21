-- Write-on-change SiteCMD score history per project environment.
-- The daily retention sweep prunes aged rows.
CREATE TABLE IF NOT EXISTS score_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- Normalized environment URL ('' when scored without an environment),
    -- matching the env scoping of the work_items the score reads.
    environment_url TEXT NOT NULL DEFAULT '',
    overall REAL NOT NULL,
    critical_count INTEGER NOT NULL DEFAULT 0,
    high_count INTEGER NOT NULL DEFAULT 0,
    medium_count INTEGER NOT NULL DEFAULT 0,
    low_count INTEGER NOT NULL DEFAULT 0,
    exploitable_capped INTEGER NOT NULL DEFAULT 0,
    computed_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_score_snapshots_project_env
    ON score_snapshots(project_id, environment_url, id);
