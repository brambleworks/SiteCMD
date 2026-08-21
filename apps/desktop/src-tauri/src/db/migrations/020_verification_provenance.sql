-- Distinguish scan-proven verification from a user's fixed claim.
-- The CHECK keeps `verified` status and prover inseparable; existing verified
-- rows came from local scans. Rebuild the table to apply the constraint and
-- drop unused TEXT timestamps in favor of last_status_changed_at.

CREATE TABLE project_issue_states_rebuild (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    check_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('new', 'snoozed', 'ignored', 'blocked', 'verified', 'regressed')),
    snooze_until INTEGER,
    block_reason TEXT,
    last_status_changed_at INTEGER NOT NULL,
    verified_by TEXT CHECK ((status = 'verified') = (verified_by IS NOT NULL)
        AND (verified_by IS NULL OR verified_by IN ('user_claim', 'local_scan')))
);

INSERT INTO project_issue_states_rebuild (
    id, project_id, env_url, check_id, status, snooze_until, block_reason,
    last_status_changed_at, verified_by
)
SELECT id, project_id, env_url, check_id, status, snooze_until, block_reason,
       last_status_changed_at,
       CASE WHEN status = 'verified' THEN 'local_scan' END
FROM project_issue_states;

DROP TABLE project_issue_states;
ALTER TABLE project_issue_states_rebuild RENAME TO project_issue_states;

CREATE UNIQUE INDEX IF NOT EXISTS uq_project_issue_states_identity
    ON project_issue_states(project_id, env_url, check_id);
CREATE INDEX IF NOT EXISTS idx_project_issue_states_project_env
    ON project_issue_states(project_id, env_url);
