-- Cross-page analysis produces findings that belong to the scan session, not
-- to any one page scan. Persist them independently from mutable site_scan work
-- items so old session reports remain exact after later scans.
ALTER TABLE scan_sessions ADD COLUMN issue_snapshot_version INTEGER NOT NULL DEFAULT 0
    CHECK (issue_snapshot_version IN (0, 1));

CREATE TABLE session_issues (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES scan_sessions(id) ON DELETE CASCADE,
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
    UNIQUE (session_id, ordinal)
);

CREATE INDEX idx_session_issues_session_id ON session_issues(session_id);
