-- Requests the MCP server queues for the desktop to fulfil with its own
-- brief generation and scan admission paths.
CREATE TABLE agent_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL CHECK (kind IN ('start_fix', 'run_scan')),
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    check_id TEXT,
    scope TEXT CHECK (scope IS NULL OR scope IN ('web', 'code', 'full')),
    agent_tool TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'requested'
        CHECK (status IN ('requested', 'running', 'fulfilled', 'failed', 'expired')),
    result_json TEXT,
    failure_detail TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_agent_requests_status ON agent_requests (status, id);
