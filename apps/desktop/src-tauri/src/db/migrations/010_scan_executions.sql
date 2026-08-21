CREATE TABLE scan_executions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
    environment_id INTEGER REFERENCES environments(id) ON DELETE SET NULL,
    environment_url TEXT,
    environment_scope_key TEXT NOT NULL,
    requested_mode TEXT NOT NULL
        CHECK (requested_mode IN ('full', 'web', 'code')),
    web_focus TEXT
        CHECK (web_focus IS NULL OR web_focus IN
            ('health', 'security', 'accessibility', 'polish')),
    trigger TEXT NOT NULL
        CHECK (trigger IN
            ('manual', 'tray', 'scheduled', 'verification', 'background', 'migration')),
    admission_class TEXT NOT NULL
        CHECK (admission_class IN
            ('general_scan', 'bounded_verification', 'system_exempt')),
    status TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN
            ('planned', 'running', 'complete', 'partial', 'failed', 'cancelled')),
    idempotency_key TEXT NOT NULL UNIQUE,
    request_fingerprint TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    quota_date TEXT NOT NULL,
    quota_state TEXT NOT NULL
        CHECK (quota_state IN ('exempt', 'reserved', 'consumed', 'released')),
    counts_toward_quota INTEGER NOT NULL
        CHECK (counts_toward_quota IN (0, 1)),
    score_snapshot_id INTEGER REFERENCES score_snapshots(id) ON DELETE SET NULL,
    failure_summary TEXT,
    legacy_provenance TEXT,
    web_status TEXT
        CHECK (web_status IS NULL OR web_status IN
            ('planned', 'running', 'complete', 'failed', 'cancelled', 'skipped')),
    web_detail TEXT,
    code_status TEXT
        CHECK (code_status IS NULL OR code_status IN
            ('planned', 'running', 'complete', 'failed', 'cancelled', 'skipped')),
    code_detail TEXT
);

CREATE INDEX idx_scan_executions_history
    ON scan_executions(project_id, environment_scope_key, started_at DESC);

CREATE INDEX idx_scan_executions_quota
    ON scan_executions(quota_date, quota_state)
    WHERE counts_toward_quota = 1;

ALTER TABLE scans ADD COLUMN execution_id INTEGER
    REFERENCES scan_executions(id) ON DELETE SET NULL;

ALTER TABLE scan_sessions ADD COLUMN execution_id INTEGER
    REFERENCES scan_executions(id) ON DELETE SET NULL;

ALTER TABLE code_scans ADD COLUMN execution_id INTEGER
    REFERENCES scan_executions(id) ON DELETE SET NULL;

CREATE INDEX idx_scans_execution_id ON scans(execution_id);
CREATE INDEX idx_scan_sessions_execution_id ON scan_sessions(execution_id);
CREATE INDEX idx_code_scans_execution_id ON code_scans(execution_id);
