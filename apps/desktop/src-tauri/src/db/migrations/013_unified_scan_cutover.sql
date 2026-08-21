-- Atomic one-way cutover from the three legacy scan-history families to the
-- canonical execution/run/finding model created by migration 012.

CREATE TEMP TABLE _unified_scan_cutover_guard (
    valid INTEGER NOT NULL CHECK (valid = 1)
);

-- Every legacy header and immutable finding must already have an exact
-- canonical counterpart. A zero inserted into the guard aborts this migration
-- and rolls the surrounding transaction back.
INSERT INTO _unified_scan_cutover_guard(valid)
SELECT 0 WHERE
    (SELECT COUNT(*) FROM scans) !=
        (SELECT COUNT(*) FROM scan_runs WHERE legacy_source = 'web_scan')
 OR (SELECT COUNT(*) FROM scan_sessions) !=
        (SELECT COUNT(*) FROM scan_runs WHERE legacy_source = 'web_session')
 OR (SELECT COUNT(*) FROM code_scans) !=
        (SELECT COUNT(*) FROM scan_runs WHERE legacy_source = 'code_scan')
 OR (SELECT COUNT(*) FROM scan_issues) != (
        SELECT COUNT(*)
        FROM scan_findings finding
        JOIN scan_runs run ON run.id = finding.run_id
        WHERE run.legacy_source = 'web_scan'
    )
 OR (SELECT COUNT(*) FROM session_issues) != (
        SELECT COUNT(*)
        FROM scan_findings finding
        JOIN scan_runs run ON run.id = finding.run_id
        WHERE run.legacy_source = 'web_session'
    )
 OR (SELECT COUNT(*) FROM code_scan_issues) != (
        SELECT COUNT(*)
        FROM scan_findings finding
        JOIN scan_runs run ON run.id = finding.run_id
        WHERE run.legacy_source = 'code_scan'
    );

INSERT INTO _unified_scan_cutover_guard(valid)
SELECT 0 WHERE EXISTS (
    SELECT 1
    FROM scan_issues legacy
    JOIN scan_runs run
      ON run.legacy_source = 'web_scan' AND run.legacy_id = legacy.scan_id
    LEFT JOIN scan_findings finding
      ON finding.run_id = run.id AND finding.ordinal = legacy.ordinal
    WHERE finding.id IS NULL
       OR finding.producer_check_id != legacy.check_id
       OR finding.producer_category != legacy.category
       OR finding.title != legacy.title
       OR finding.description != legacy.description
       OR finding.verdict != legacy.check_status
       OR finding.severity != legacy.severity
       OR COALESCE(finding.raw_data, '') != COALESCE(legacy.raw_data, '')
);

INSERT INTO _unified_scan_cutover_guard(valid)
SELECT 0 WHERE EXISTS (
    SELECT 1
    FROM session_issues legacy
    JOIN scan_runs run
      ON run.legacy_source = 'web_session' AND run.legacy_id = legacy.session_id
    LEFT JOIN scan_findings finding
      ON finding.run_id = run.id AND finding.ordinal = legacy.ordinal
    WHERE finding.id IS NULL
       OR finding.producer_check_id != legacy.check_id
       OR finding.producer_category != legacy.category
       OR finding.title != legacy.title
       OR finding.description != legacy.description
       OR finding.verdict != legacy.check_status
       OR finding.severity != legacy.severity
       OR COALESCE(finding.raw_data, '') != COALESCE(legacy.raw_data, '')
);

INSERT INTO _unified_scan_cutover_guard(valid)
SELECT 0 WHERE EXISTS (
    SELECT 1
    FROM code_scan_issues legacy
    JOIN scan_runs run
      ON run.legacy_source = 'code_scan' AND run.legacy_id = legacy.scan_id
    LEFT JOIN scan_findings finding
      ON finding.run_id = run.id AND finding.ordinal = legacy.ordinal
    WHERE finding.id IS NULL
       OR finding.canonical_check_id != legacy.canonical_check_id
       OR finding.domain != legacy.domain
       OR finding.title != legacy.title
       OR finding.severity != legacy.severity
       OR COALESCE(finding.detail_json, '') != COALESCE(legacy.issue_json, '')
);

-- Issue-tracker links point at the exact canonical Web run that supplied the
-- evidence. The legacy table had a hard FK to scans, so every row is
-- unambiguously in the old Web id space.
CREATE TEMP TABLE _issue_link_run_map (
    link_id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL
);

INSERT INTO _issue_link_run_map(link_id, run_id)
SELECT link.id, run.id
FROM issue_links link
JOIN scan_runs run
  ON run.legacy_source = 'web_scan' AND run.legacy_id = link.scan_id;

INSERT INTO _unified_scan_cutover_guard(valid)
SELECT 0 WHERE (SELECT COUNT(*) FROM issue_links) !=
               (SELECT COUNT(*) FROM _issue_link_run_map);

ALTER TABLE issue_links RENAME TO issue_links_legacy;

CREATE TABLE issue_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    check_id TEXT NOT NULL,
    run_id INTEGER NOT NULL REFERENCES scan_runs(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    external_id TEXT NOT NULL,
    external_url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    created_at TEXT NOT NULL,
    resolved_at TEXT
);

INSERT INTO issue_links (
    id, project_id, check_id, run_id, provider, external_id,
    external_url, status, created_at, resolved_at
)
SELECT legacy.id, legacy.project_id, legacy.check_id, mapping.run_id,
       legacy.provider, legacy.external_id, legacy.external_url,
       legacy.status, legacy.created_at, legacy.resolved_at
FROM issue_links_legacy legacy
JOIN _issue_link_run_map mapping ON mapping.link_id = legacy.id;

DROP TABLE issue_links_legacy;

CREATE INDEX idx_issue_links_project_check
    ON issue_links(project_id, check_id, provider);
CREATE INDEX idx_issue_links_status
    ON issue_links(project_id, status);
CREATE INDEX idx_issue_links_run
    ON issue_links(run_id);

-- A regression created after migration 012 already contains canonical run
-- ids; its current run has no legacy provenance. Otherwise both ids are from
-- the old engine-specific namespace and are mapped through provenance.
CREATE TEMP TABLE _regression_run_map (
    regression_id INTEGER PRIMARY KEY,
    prev_run_id INTEGER NOT NULL,
    run_id INTEGER NOT NULL
);

INSERT INTO _regression_run_map(regression_id, prev_run_id, run_id)
SELECT regression.id,
       CASE
           WHEN direct_current.id IS NOT NULL THEN direct_previous.id
           ELSE mapped_previous.id
       END,
       CASE
           WHEN direct_current.id IS NOT NULL THEN direct_current.id
           ELSE mapped_current.id
       END
FROM regressions regression
LEFT JOIN scan_runs direct_current
  ON direct_current.id = regression.scan_id
 AND direct_current.project_id = regression.project_id
 AND direct_current.source = CASE regression.scan_type
        WHEN 'web' THEN 'web_scan' ELSE 'code_scan' END
 AND direct_current.legacy_source IS NULL
LEFT JOIN scan_runs direct_previous
  ON direct_previous.id = regression.prev_scan_id
 AND direct_previous.project_id = regression.project_id
 AND direct_previous.source = CASE regression.scan_type
        WHEN 'web' THEN 'web_scan' ELSE 'code_scan' END
LEFT JOIN scan_runs mapped_current
  ON mapped_current.legacy_source = CASE regression.scan_type
        WHEN 'web' THEN 'web_scan' ELSE 'code_scan' END
 AND mapped_current.legacy_id = regression.scan_id
LEFT JOIN scan_runs mapped_previous
  ON mapped_previous.legacy_source = CASE regression.scan_type
        WHEN 'web' THEN 'web_scan' ELSE 'code_scan' END
 AND mapped_previous.legacy_id = regression.prev_scan_id
WHERE (direct_current.id IS NOT NULL AND direct_previous.id IS NOT NULL)
   OR (direct_current.id IS NULL
       AND mapped_current.id IS NOT NULL
       AND mapped_previous.id IS NOT NULL);

INSERT INTO _unified_scan_cutover_guard(valid)
SELECT 0 WHERE (SELECT COUNT(*) FROM regressions) !=
               (SELECT COUNT(*) FROM _regression_run_map);

-- Rewrite self-contained deploy-regression alert evidence before replacing
-- the regression table. The dossier can then navigate by canonical run id.
UPDATE alerts
SET alert_id = 'deploy-regression:' || (
        SELECT regression.scan_type FROM regressions regression
        WHERE regression.id = CAST(json_extract(alerts.detail_json, '$.regression_id') AS INTEGER)
    ) || ':' || (
        SELECT mapping.run_id FROM _regression_run_map mapping
        WHERE mapping.regression_id = CAST(json_extract(alerts.detail_json, '$.regression_id') AS INTEGER)
    ),
    detail_json = json_remove(
        json_set(
            detail_json,
            '$.run_id',
            (SELECT mapping.run_id FROM _regression_run_map mapping
             WHERE mapping.regression_id = CAST(json_extract(alerts.detail_json, '$.regression_id') AS INTEGER))
        ),
        '$.scan_id'
    )
WHERE json_valid(detail_json)
  AND json_extract(detail_json, '$.alert_type') = 'deploy_regression'
  AND EXISTS (
      SELECT 1 FROM _regression_run_map mapping
      WHERE mapping.regression_id = CAST(json_extract(alerts.detail_json, '$.regression_id') AS INTEGER)
  );

ALTER TABLE regression_check_ids RENAME TO regression_check_ids_legacy;
ALTER TABLE regressions RENAME TO regressions_legacy;

CREATE TABLE regressions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    env_url TEXT NOT NULL,
    scan_type TEXT NOT NULL CHECK (scan_type IN ('web', 'code')),
    prev_run_id INTEGER NOT NULL,
    run_id INTEGER NOT NULL,
    prev_score INTEGER NOT NULL,
    score INTEGER NOT NULL,
    commit_from TEXT NOT NULL,
    commit_to TEXT NOT NULL,
    commit_count INTEGER NOT NULL DEFAULT 0,
    commits_json TEXT NOT NULL DEFAULT '[]',
    fixed_check_ids_json TEXT NOT NULL DEFAULT '[]',
    created_at INTEGER NOT NULL,
    UNIQUE (scan_type, run_id)
);

INSERT INTO regressions (
    id, project_id, env_url, scan_type, prev_run_id, run_id,
    prev_score, score, commit_from, commit_to, commit_count,
    commits_json, fixed_check_ids_json, created_at
)
SELECT legacy.id, legacy.project_id, legacy.env_url, legacy.scan_type,
       mapping.prev_run_id, mapping.run_id, legacy.prev_score, legacy.score,
       legacy.commit_from, legacy.commit_to, legacy.commit_count,
       legacy.commits_json, legacy.fixed_check_ids_json, legacy.created_at
FROM regressions_legacy legacy
JOIN _regression_run_map mapping ON mapping.regression_id = legacy.id;

DROP TABLE regressions_legacy;

CREATE INDEX idx_regressions_project ON regressions(project_id);
CREATE INDEX idx_regressions_run ON regressions(scan_type, run_id);

CREATE TABLE regression_check_ids (
    regression_id INTEGER NOT NULL REFERENCES regressions(id) ON DELETE CASCADE,
    check_id TEXT NOT NULL,
    PRIMARY KEY (regression_id, check_id)
);

INSERT INTO regression_check_ids(regression_id, check_id)
SELECT regression_id, check_id FROM regression_check_ids_legacy;

DROP TABLE regression_check_ids_legacy;

CREATE INDEX idx_regression_check_ids_by_check
    ON regression_check_ids(check_id, regression_id);

-- Remove component-level legacy timeline rows and replace them with one event
-- per execution. Pruned legacy result rows intentionally produce no orphan
-- timeline entry. Existing canonical events are retained by the unique key.
DELETE FROM site_event_check_ids
WHERE event_id IN (
    SELECT id FROM events
    WHERE source = 'internal'
      AND event_type IN ('scan', 'security', 'accessibility')
      AND (
          source_id GLOB 'scan_[0-9]*'
          OR source_id GLOB 'session_[0-9]*'
          OR source_id GLOB 'code_scan_[0-9]*'
      )
);

DELETE FROM events
WHERE source = 'internal'
  AND event_type IN ('scan', 'security', 'accessibility')
  AND (
      source_id GLOB 'scan_[0-9]*'
      OR source_id GLOB 'session_[0-9]*'
      OR source_id GLOB 'code_scan_[0-9]*'
  );

INSERT OR IGNORE INTO events (
    project_id, event_type, severity, occurred_at_ms, title, summary,
    detail, source, source_id
)
SELECT execution.project_id,
       CASE execution.trigger WHEN 'verification' THEN 'verification' ELSE 'scan' END,
       CASE
           WHEN score.overall < 50 THEN 'critical'
           WHEN score.overall < 80 THEN 'warning'
           WHEN score.overall IS NOT NULL THEN 'info'
           WHEN EXISTS (
               SELECT 1 FROM scan_findings finding
               JOIN scan_runs run ON run.id = finding.run_id
               WHERE run.execution_id = execution.id
                 AND finding.verdict IN ('fail', 'warn')
                 AND finding.severity = 'critical'
           ) THEN 'critical'
           WHEN EXISTS (
               SELECT 1 FROM scan_findings finding
               JOIN scan_runs run ON run.id = finding.run_id
               WHERE run.execution_id = execution.id
                 AND finding.verdict IN ('fail', 'warn')
                 AND finding.severity = 'high'
           ) THEN 'warning'
           ELSE 'info'
       END,
       COALESCE(execution.completed_at, execution.started_at),
       CASE execution.requested_mode
           WHEN 'full' THEN 'Full scan'
           WHEN 'web' THEN 'Web Scan'
           ELSE 'Code Scan'
       END || ': ' || replace(execution.status, '_', ' ') ||
       CASE WHEN score.overall IS NULL THEN ''
            ELSE ' · SiteCMD Score ' || CAST(round(score.overall) AS INTEGER) END,
       (SELECT COUNT(*) FROM scan_findings finding
        JOIN scan_runs run ON run.id = finding.run_id
        WHERE run.execution_id = execution.id) ||
       ' collector findings across one ' || execution.requested_mode || ' execution.',
       json_object(
           'execution_id', execution.id,
           'requested_mode', execution.requested_mode,
           'web_focus', execution.web_focus,
           'status', execution.status,
           'web_status', execution.web_status,
           'code_status', execution.code_status,
           'sitecmd_score', score.overall,
           'url', execution.environment_url
       ),
       'internal',
       'scan_execution_' || execution.id
FROM scan_executions execution
LEFT JOIN score_snapshots score ON score.id = execution.score_snapshot_id
WHERE execution.project_id IS NOT NULL
  AND execution.status IN ('complete', 'partial', 'failed', 'cancelled');

INSERT OR IGNORE INTO site_event_check_ids(event_id, check_id)
SELECT event.id, finding.canonical_check_id
FROM events event
JOIN scan_executions execution
  ON event.source = 'internal'
 AND event.source_id = 'scan_execution_' || execution.id
JOIN scan_runs run ON run.execution_id = execution.id
JOIN scan_findings finding ON finding.run_id = run.id
WHERE finding.verdict IN ('fail', 'warn');

-- All production consumers now use the canonical tables. Drop immutable
-- duplicates in dependency order so no runtime fallback can survive.
DROP TABLE scan_issues;
DROP TABLE session_issues;
DROP TABLE code_scan_issues;
DROP TABLE scans;
DROP TABLE scan_sessions;
DROP TABLE code_scans;

INSERT INTO _unified_scan_cutover_guard(valid)
SELECT 0 FROM pragma_foreign_key_check LIMIT 1;

DROP TABLE _unified_scan_cutover_guard;
DROP TABLE _issue_link_run_map;
DROP TABLE _regression_run_map;
