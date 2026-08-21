-- Squashed baseline for the pre-launch schema redesign.
-- Pre-squash databases are backed up and recreated by the incompatible-version guard.
-- Project-scoped foreign keys cascade, event times use epoch milliseconds,
-- lifecycle values are constrained, and removed tables are omitted.

CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT NOT NULL DEFAULT '',
    framework TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    secret_namespace TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_secret_namespace
    ON projects(secret_namespace);

CREATE TABLE IF NOT EXISTS environments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    label TEXT NOT NULL,
    environment TEXT NOT NULL DEFAULT 'production',
    source TEXT,
    last_scanned_at TEXT,
    UNIQUE(project_id, url)
);
CREATE INDEX IF NOT EXISTS idx_environments_project_id ON environments(project_id);

CREATE TABLE IF NOT EXISTS sites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    sitemap_url TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_scanned_at TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_sites_project_url
    ON sites(project_id, url) WHERE project_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_sites_adhoc_url
    ON sites(url) WHERE project_id IS NULL;

CREATE TABLE IF NOT EXISTS scan_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    total_pages INTEGER NOT NULL,
    completed_pages INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'running',
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    overall_score INTEGER,
    duration_ms INTEGER,
    axe_enabled INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_scan_sessions_site_id ON scan_sessions(site_id);

CREATE TABLE IF NOT EXISTS scans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    timestamp TEXT NOT NULL,
    mode TEXT NOT NULL,
    -- Web-scan focus vocabulary (core::scanner::ScanType). Distinct from the
    -- regressions.scan_type web/code subsystem discriminator below.
    scan_type TEXT NOT NULL DEFAULT 'health' CHECK(scan_type IN ('health', 'security', 'accessibility', 'polish')),
    overall_score INTEGER NOT NULL,
    security_score INTEGER,
    performance_score INTEGER,
    seo_score INTEGER,
    accessibility_score INTEGER,
    compliance_score INTEGER,
    config_score INTEGER,
    polish_score INTEGER,
    issues_total INTEGER NOT NULL DEFAULT 0,
    issues_critical INTEGER NOT NULL DEFAULT 0,
    issues_high INTEGER NOT NULL DEFAULT 0,
    issues_medium INTEGER NOT NULL DEFAULT 0,
    issues_low INTEGER NOT NULL DEFAULT 0,
    issues_passed INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL,
    detected_stack TEXT,
    session_id INTEGER REFERENCES scan_sessions(id) ON DELETE SET NULL,
    page_url TEXT
);
CREATE INDEX IF NOT EXISTS idx_scans_site_id ON scans(site_id);
CREATE INDEX IF NOT EXISTS idx_scans_timestamp ON scans(timestamp);
CREATE INDEX IF NOT EXISTS idx_scans_site_id_timestamp
    ON scans(site_id, timestamp DESC);

CREATE TABLE IF NOT EXISTS pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    path TEXT NOT NULL,
    title TEXT,
    last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
    source TEXT NOT NULL DEFAULT 'auto',
    UNIQUE(site_id, url)
);
CREATE INDEX IF NOT EXISTS idx_pages_site_id ON pages(site_id);

CREATE TABLE IF NOT EXISTS integration_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    integration_type TEXT NOT NULL,
    api_key TEXT,
    site_id TEXT,
    extra TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    UNIQUE(project_id, integration_type)
);

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'info',
    occurred_at_ms INTEGER NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    detail TEXT,
    source TEXT NOT NULL DEFAULT 'internal',
    source_id TEXT,
    metadata TEXT,
    UNIQUE(project_id, source, source_id)
);
CREATE INDEX IF NOT EXISTS idx_events_project_id ON events(project_id);
CREATE INDEX IF NOT EXISTS idx_events_occurred_at ON events(occurred_at_ms);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_project_occurred
    ON events(project_id, occurred_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_events_project_type_occurred
    ON events(project_id, event_type, occurred_at_ms DESC);

CREATE TABLE IF NOT EXISTS site_event_check_ids (
    event_id INTEGER NOT NULL,
    check_id TEXT NOT NULL,
    PRIMARY KEY (event_id, check_id),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_site_event_check_ids_by_check
    ON site_event_check_ids(check_id, event_id);

CREATE TABLE IF NOT EXISTS webhook_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    events TEXT NOT NULL DEFAULT '[]',
    secret TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(project_id, url)
);

CREATE TABLE IF NOT EXISTS report_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    site_url TEXT NOT NULL,
    period_days INTEGER NOT NULL,
    report_title TEXT NOT NULL DEFAULT 'Website Health Report',
    generated_at TEXT NOT NULL DEFAULT (datetime('now')),
    branding_json TEXT,
    sections_json TEXT,
    report_summary_json TEXT,
    output_format TEXT NOT NULL DEFAULT 'preview'
);
CREATE INDEX IF NOT EXISTS idx_report_history_project ON report_history(project_id);

CREATE TABLE IF NOT EXISTS scan_schedules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    environment_id INTEGER NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    frequency TEXT NOT NULL DEFAULT 'off' CHECK(frequency IN ('off', 'daily', 'weekly')),
    time_of_day TEXT NOT NULL DEFAULT '09:00',
    day_of_week INTEGER,
    -- Schedulable focuses (core::scanner::ScheduledScanType): the four web
    -- ScanType values plus 'code', which routes to the Code Scan engine.
    scan_type TEXT NOT NULL DEFAULT 'health' CHECK(scan_type IN ('health', 'security', 'accessibility', 'polish', 'code')),
    last_run_at TEXT,
    next_run_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(project_id, environment_id, scan_type)
);

CREATE TABLE IF NOT EXISTS code_scans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    environment_url TEXT,
    project_path TEXT NOT NULL,
    checked_at TEXT NOT NULL,
    overall_score INTEGER NOT NULL,
    framework TEXT,
    issue_count INTEGER NOT NULL DEFAULT 0,
    critical_count INTEGER NOT NULL DEFAULT 0,
    high_count INTEGER NOT NULL DEFAULT 0,
    medium_count INTEGER NOT NULL DEFAULT 0,
    low_count INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_code_scans_project_id ON code_scans(project_id);
CREATE INDEX IF NOT EXISTS idx_code_scans_checked_at ON code_scans(checked_at);

CREATE TABLE IF NOT EXISTS project_signal_snapshots (
    project_id INTEGER NOT NULL,
    environment_url TEXT NOT NULL DEFAULT '',
    monitoring_json TEXT,
    monitoring_refreshed_at TEXT,
    updates_json TEXT,
    updates_refreshed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (project_id, environment_url),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_project_signal_snapshots_project
    ON project_signal_snapshots(project_id);

CREATE TABLE IF NOT EXISTS project_ui_state (
    project_id INTEGER PRIMARY KEY,
    first_scan_banner_dismissed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_project_ui_state_project
    ON project_ui_state(project_id);

CREATE TABLE IF NOT EXISTS issue_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    check_id TEXT NOT NULL,
    scan_id INTEGER NOT NULL,
    provider TEXT NOT NULL,
    external_id TEXT NOT NULL,
    external_url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (scan_id) REFERENCES scans(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_issue_links_project_check
    ON issue_links(project_id, check_id, provider);
CREATE INDEX IF NOT EXISTS idx_issue_links_status
    ON issue_links(project_id, status);

CREATE TABLE IF NOT EXISTS work_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    source TEXT NOT NULL,
    signal_id TEXT NOT NULL,
    check_id TEXT NOT NULL,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    detail_json TEXT,
    scan_ref INTEGER,
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    resolved_at INTEGER,
    page_url TEXT,
    fix_prompt TEXT,
    manual_fix TEXT,
    why_it_matters TEXT,
    -- Promoted metadata used by scoring and fix targeting; absent for unsupported sources.
    confidence TEXT,
    domain TEXT,
    relative_path TEXT,
    line INTEGER
);
CREATE INDEX IF NOT EXISTS idx_work_items_active
    ON work_items(project_id, env_url, resolved_at);
CREATE INDEX IF NOT EXISTS idx_work_items_check_id
    ON work_items(check_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_work_items_active
    ON work_items(project_id, env_url, source, signal_id)
    WHERE resolved_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_work_items_page_url
    ON work_items(project_id, env_url, page_url)
    WHERE resolved_at IS NULL AND page_url IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_work_items_source_scan_ref
    ON work_items(source, scan_ref);
CREATE INDEX IF NOT EXISTS idx_work_items_project_first_seen
    ON work_items(project_id, first_seen_at);
CREATE INDEX IF NOT EXISTS idx_work_items_first_seen
    ON work_items(first_seen_at);
CREATE INDEX IF NOT EXISTS idx_work_items_source_env_resolved
    ON work_items (source, env_url, resolved_at DESC)
    WHERE resolved_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL,
    alert_id TEXT NOT NULL,
    severity TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    detail_json TEXT,
    occurred_at INTEGER NOT NULL,
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    viewed_at INTEGER,
    dismissed_at INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_alerts_identity
    ON alerts(project_id, env_url, source, alert_id);
CREATE INDEX IF NOT EXISTS idx_alerts_unread
    ON alerts(project_id, viewed_at, dismissed_at, occurred_at DESC);

CREATE TABLE IF NOT EXISTS dismissed_integration_hints (
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    check_id TEXT NOT NULL,
    integration_type TEXT NOT NULL,
    dismissed_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, check_id, integration_type)
);

CREATE TABLE IF NOT EXISTS project_issue_states (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    check_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('new', 'snoozed', 'ignored', 'blocked', 'verified', 'regressed')),
    snooze_until INTEGER,
    block_reason TEXT,
    last_status_changed_at INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_project_issue_states_identity
    ON project_issue_states(project_id, env_url, check_id);
CREATE INDEX IF NOT EXISTS idx_project_issue_states_project_env
    ON project_issue_states(project_id, env_url);

CREATE TABLE IF NOT EXISTS trial_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    email TEXT NOT NULL,
    started_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    last_validated_at TEXT NOT NULL,
    validation_token TEXT,
    max_observed_at TEXT
);

CREATE TABLE IF NOT EXISTS launch_snapshots (
    id TEXT PRIMARY KEY,
    project_id INTEGER NOT NULL,
    environment_url TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    checked_at TEXT,
    readiness TEXT NOT NULL,
    decision TEXT NOT NULL,
    decision_label TEXT NOT NULL,
    profile_kind_label TEXT NOT NULL,
    profile_confidence_label TEXT NOT NULL,
    stack_label TEXT NOT NULL,
    passing_checks INTEGER NOT NULL,
    total_checks INTEGER NOT NULL,
    score_percent INTEGER NOT NULL,
    open_blockers INTEGER NOT NULL,
    critical_open INTEGER NOT NULL,
    important_open INTEGER NOT NULL,
    owner_proof_open INTEGER NOT NULL,
    skipped_checks INTEGER NOT NULL,
    not_applicable_checks INTEGER NOT NULL,
    launch_code_risk_count INTEGER NOT NULL,
    source_mapped_checks INTEGER NOT NULL,
    top_blocker_json TEXT,
    used_surfaces_json TEXT NOT NULL,
    confirmed_not_used_surfaces_json TEXT NOT NULL,
    unknown_surfaces_json TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    persisted_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_launch_snapshots_project_env_created
    ON launch_snapshots(project_id, environment_url, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_launch_snapshots_project_decision
    ON launch_snapshots(project_id, decision, created_at DESC);

CREATE TABLE IF NOT EXISTS launch_site_profiles (
    project_id INTEGER NOT NULL,
    environment_url TEXT NOT NULL DEFAULT '',
    overrides_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (project_id, environment_url),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_launch_site_profiles_project
    ON launch_site_profiles(project_id);

CREATE TABLE IF NOT EXISTS launch_proof_notes (
    project_id INTEGER NOT NULL,
    environment_url TEXT NOT NULL DEFAULT '',
    item_id TEXT NOT NULL,
    proof_text TEXT NOT NULL DEFAULT '',
    artifact_text TEXT NOT NULL DEFAULT '',
    verified_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (project_id, environment_url, item_id),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_launch_proof_notes_project_env
    ON launch_proof_notes(project_id, environment_url);

CREATE TABLE IF NOT EXISTS causal_link_observations (
    project_id          INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    cause_check_id      TEXT NOT NULL,
    effect_check_id     TEXT NOT NULL,
    observed_at         INTEGER NOT NULL,
    co_active           INTEGER NOT NULL,
    co_resolved         INTEGER NOT NULL,
    resolution_event_id INTEGER,
    FOREIGN KEY (resolution_event_id) REFERENCES events(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_observations_link
    ON causal_link_observations(project_id, cause_check_id, effect_check_id);

CREATE TABLE IF NOT EXISTS signal_baselines (
    project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    signal_key   TEXT NOT NULL,
    window_days  INTEGER NOT NULL,
    mean         REAL NOT NULL,
    stddev       REAL NOT NULL,
    sample_count INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    PRIMARY KEY (project_id, signal_key, window_days)
);

CREATE TABLE IF NOT EXISTS signal_history (
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    signal_key TEXT NOT NULL,
    ts_ms      INTEGER NOT NULL,
    value      REAL NOT NULL,
    PRIMARY KEY (project_id, signal_key, ts_ms)
);
CREATE INDEX IF NOT EXISTS idx_signal_history_window
    ON signal_history(project_id, signal_key, ts_ms);

CREATE TABLE IF NOT EXISTS historical_enrichments (
    work_item_id INTEGER NOT NULL,
    integration  TEXT NOT NULL,
    payload      TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    PRIMARY KEY (work_item_id, integration),
    FOREIGN KEY (work_item_id) REFERENCES work_items(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS cross_project_pattern_index (
    check_id        TEXT PRIMARY KEY,
    project_count   INTEGER NOT NULL,
    latest_seen_ms  INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS integration_enrichment_cache (
    project_id    INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    integration   TEXT NOT NULL,
    signal_key    TEXT NOT NULL,
    payload_json  TEXT NOT NULL,
    refreshed_at  INTEGER NOT NULL,
    PRIMARY KEY (project_id, integration, signal_key)
);
CREATE INDEX IF NOT EXISTS idx_iec_freshness
    ON integration_enrichment_cache(project_id, refreshed_at);

CREATE TABLE IF NOT EXISTS fix_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    check_id TEXT NOT NULL,
    agent_tool TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'briefed',
    brief_md TEXT NOT NULL DEFAULT '',
    agent_summary TEXT,
    failure_detail TEXT,
    verify_started_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    brief_fetched_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_fix_attempts_status ON fix_attempts(status);
CREATE UNIQUE INDEX IF NOT EXISTS uq_fix_attempts_active
    ON fix_attempts(project_id, env_url, check_id)
    WHERE status IN ('briefed', 'verify_requested', 'verifying');

CREATE TABLE IF NOT EXISTS regressions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    scan_type TEXT NOT NULL CHECK (scan_type IN ('web', 'code')),
    prev_scan_id INTEGER NOT NULL,
    scan_id INTEGER NOT NULL,
    prev_score INTEGER NOT NULL,
    score INTEGER NOT NULL,
    commit_from TEXT NOT NULL,
    commit_to TEXT NOT NULL,
    commit_count INTEGER NOT NULL DEFAULT 0,
    commits_json TEXT NOT NULL DEFAULT '[]',
    fixed_check_ids_json TEXT NOT NULL DEFAULT '[]',
    created_at INTEGER NOT NULL,
    UNIQUE (scan_type, scan_id)
);
CREATE INDEX IF NOT EXISTS idx_regressions_project
    ON regressions(project_id);

CREATE TABLE IF NOT EXISTS regression_check_ids (
    regression_id INTEGER NOT NULL REFERENCES regressions(id) ON DELETE CASCADE,
    check_id TEXT NOT NULL,
    PRIMARY KEY (regression_id, check_id)
);
CREATE INDEX IF NOT EXISTS idx_regression_check_ids_by_check
    ON regression_check_ids(check_id, regression_id);
