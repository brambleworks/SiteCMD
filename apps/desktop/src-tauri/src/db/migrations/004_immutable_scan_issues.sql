-- A scan is historical evidence. `work_items` is intentionally mutable: the
-- same active signal is updated and reassigned to the newest scan, so it
-- cannot also be the source of truth for an older scan's issue list.
ALTER TABLE scans ADD COLUMN issue_snapshot_version INTEGER NOT NULL DEFAULT 0
    CHECK (issue_snapshot_version IN (0, 1));

CREATE TABLE scan_issues (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    check_id TEXT NOT NULL,
    category TEXT NOT NULL CHECK (category IN
        ('security', 'performance', 'seo', 'accessibility', 'compliance', 'config', 'polish')),
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    check_status TEXT NOT NULL CHECK (check_status IN ('pass', 'fail', 'warn', 'skipped')),
    severity TEXT NOT NULL CHECK (severity IN ('critical', 'high', 'medium', 'low')),
    fix_prompt TEXT,
    manual_fix TEXT,
    raw_data TEXT,
    confidence TEXT NOT NULL CHECK (confidence IN ('confirmed', 'high', 'needs_review')),
    confidence_reason TEXT,
    why_it_matters TEXT,
    UNIQUE (scan_id, ordinal)
);

CREATE INDEX idx_scan_issues_scan_id ON scan_issues(scan_id);
