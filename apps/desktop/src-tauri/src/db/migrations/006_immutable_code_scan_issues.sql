-- Store immutable Code Scan issue detail with each run; work_items retain lifecycle state.
ALTER TABLE code_scans ADD COLUMN issue_snapshot_version INTEGER NOT NULL DEFAULT 0
    CHECK (issue_snapshot_version IN (0, 1));

CREATE TABLE code_scan_issues (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL REFERENCES code_scans(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    canonical_check_id TEXT NOT NULL,
    domain TEXT NOT NULL CHECK (domain IN
        ('database', 'ai-safety', 'security', 'architecture', 'operations',
         'supply-chain', 'ai-scaffolding')),
    severity TEXT NOT NULL CHECK (severity IN ('critical', 'high', 'medium', 'low')),
    title TEXT NOT NULL,
    issue_json TEXT NOT NULL,
    UNIQUE (scan_id, ordinal)
);

CREATE INDEX idx_code_scan_issues_scan_id ON code_scan_issues(scan_id);
CREATE INDEX idx_code_scan_issues_check_id ON code_scan_issues(canonical_check_id);
