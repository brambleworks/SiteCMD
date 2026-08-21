-- Allow unified full scheduled scans by rebuilding SQLite's immutable CHECK.
-- Create the replacement under its final name so sqlite_master matches the schema snapshot.
ALTER TABLE scan_schedules RENAME TO scan_schedules_old;

CREATE TABLE scan_schedules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    environment_id INTEGER NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    frequency TEXT NOT NULL DEFAULT 'off' CHECK(frequency IN ('off', 'daily', 'weekly')),
    time_of_day TEXT NOT NULL DEFAULT '09:00',
    day_of_week INTEGER,
    -- Schedulable focuses (core::scanner::ScheduledScanType): the four web
    -- ScanType values, 'code' (routes to the Code Scan engine), and 'full'
    -- (a web scan plus a Code Scan, mirroring the manual runner).
    scan_type TEXT NOT NULL DEFAULT 'health' CHECK(scan_type IN ('health', 'security', 'accessibility', 'polish', 'code', 'full')),
    last_run_at TEXT,
    next_run_at TEXT,
    -- No now-based column defaults (guardrail: migrations after the baseline
    -- supply timestamps from Rust). Existing rows copy their values below; the
    -- only insert path, save_scan_schedule, sets created_at and updated_at.
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(project_id, environment_id, scan_type)
);

INSERT INTO scan_schedules
    (id, project_id, environment_id, frequency, time_of_day, day_of_week,
     scan_type, last_run_at, next_run_at, created_at, updated_at)
SELECT id, project_id, environment_id, frequency, time_of_day, day_of_week,
       scan_type, last_run_at, next_run_at, created_at, updated_at
FROM scan_schedules_old;

DROP TABLE scan_schedules_old;
