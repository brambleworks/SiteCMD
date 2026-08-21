-- Canonical immutable scan persistence. One execution owns one or more runs;
-- every Web, Code, page, and cross-page finding uses the same row shape.
CREATE TABLE scan_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id INTEGER NOT NULL REFERENCES scan_executions(id) ON DELETE CASCADE,
    parent_run_id INTEGER REFERENCES scan_runs(id) ON DELETE CASCADE,
    project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
    site_id INTEGER REFERENCES sites(id) ON DELETE SET NULL,
    environment_url TEXT,
    environment_scope_key TEXT NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('web_scan', 'code_scan')),
    run_kind TEXT NOT NULL CHECK(run_kind IN ('single', 'multi_parent', 'page', 'code')),
    status TEXT NOT NULL CHECK(status IN
        ('planned', 'running', 'complete', 'failed', 'cancelled', 'skipped')),
    focus TEXT,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    timestamp_text TEXT NOT NULL,
    raw_score INTEGER,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    coverage_kind TEXT NOT NULL CHECK(coverage_kind IN
        ('site', 'page_set', 'page', 'project', 'check_set', 'rule_set')),
    coverage_json TEXT NOT NULL DEFAULT '{}',
    diagnostics_json TEXT,
    status_detail TEXT,
    detail_state TEXT NOT NULL DEFAULT 'exact'
        CHECK(detail_state IN ('exact', 'limited_legacy')),
    -- Web diagnostics.
    mode TEXT,
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
    detected_stack TEXT,
    page_url TEXT,
    total_pages INTEGER,
    completed_pages INTEGER,
    axe_enabled INTEGER,
    -- Code diagnostics.
    project_path TEXT,
    framework TEXT,
    -- Deterministic one-way-backfill provenance. New runs leave these NULL.
    legacy_source TEXT,
    legacy_id INTEGER,
    UNIQUE(legacy_source, legacy_id)
);

CREATE INDEX idx_scan_runs_execution
    ON scan_runs(execution_id, id);
CREATE INDEX idx_scan_runs_parent
    ON scan_runs(parent_run_id, id);
CREATE INDEX idx_scan_runs_history
    ON scan_runs(project_id, environment_url, started_at DESC);

CREATE TABLE scan_findings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_runs(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    occurrence_id TEXT NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('web_scan', 'code_scan')),
    canonical_check_id TEXT NOT NULL,
    producer_check_id TEXT NOT NULL,
    category TEXT NOT NULL,
    producer_category TEXT NOT NULL,
    domain TEXT,
    verdict TEXT NOT NULL CHECK(verdict IN ('pass', 'fail', 'warn', 'skipped')),
    severity TEXT NOT NULL CHECK(severity IN ('critical', 'high', 'medium', 'low')),
    confidence TEXT NOT NULL CHECK(confidence IN ('confirmed', 'high', 'needs_review')),
    confidence_reason TEXT,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    fix_prompt TEXT,
    producer_fix_prompt TEXT,
    manual_fix TEXT,
    why_it_matters TEXT,
    verification_hint TEXT,
    raw_data TEXT,
    detail_json TEXT,
    location_kind TEXT NOT NULL CHECK(location_kind IN
        ('page', 'file', 'project', 'site', 'none')),
    page_url TEXT,
    relative_path TEXT,
    line INTEGER,
    UNIQUE(run_id, ordinal),
    UNIQUE(run_id, occurrence_id)
);

CREATE INDEX idx_scan_findings_run
    ON scan_findings(run_id, ordinal);
CREATE INDEX idx_scan_findings_check
    ON scan_findings(canonical_check_id, run_id);
CREATE INDEX idx_scan_findings_occurrence
    ON scan_findings(occurrence_id, run_id);

-- Unlinked historical rows become explicit migration executions. Never infer
-- an old Full action by timestamp proximity.
INSERT INTO scan_executions (
    project_id, environment_url, environment_scope_key, requested_mode,
    web_focus, trigger, admission_class, status, idempotency_key,
    request_fingerprint, started_at, completed_at, quota_date, quota_state,
    counts_toward_quota, legacy_provenance, web_status
)
SELECT site.project_id, site.url, site.url, 'web',
       COALESCE((SELECT scan_type FROM scans page WHERE page.session_id = session.id LIMIT 1), 'health'),
       'migration', 'system_exempt',
       CASE session.status WHEN 'complete' THEN 'complete'
            WHEN 'error' THEN 'failed' ELSE 'running' END,
       'migration:web-session:' || session.id,
       'migration:web-session:' || session.id,
       COALESCE(CAST(strftime('%s', session.started_at) AS INTEGER) * 1000, 0),
       CASE WHEN session.completed_at IS NULL THEN NULL
            ELSE COALESCE(CAST(strftime('%s', session.completed_at) AS INTEGER) * 1000, 0) END,
       COALESCE(date(session.started_at), '1970-01-01'), 'exempt', 0,
       'scan_sessions:' || session.id,
       CASE session.status WHEN 'complete' THEN 'complete'
            WHEN 'error' THEN 'failed' ELSE 'running' END
FROM scan_sessions session
JOIN sites site ON site.id = session.site_id
WHERE session.execution_id IS NULL;

UPDATE scan_sessions
SET execution_id = (
    SELECT execution.id FROM scan_executions execution
    WHERE execution.idempotency_key = 'migration:web-session:' || scan_sessions.id
)
WHERE execution_id IS NULL;

UPDATE scans
SET execution_id = (
    SELECT session.execution_id FROM scan_sessions session
    WHERE session.id = scans.session_id
)
WHERE execution_id IS NULL AND session_id IS NOT NULL;

INSERT INTO scan_executions (
    project_id, environment_url, environment_scope_key, requested_mode,
    web_focus, trigger, admission_class, status, idempotency_key,
    request_fingerprint, started_at, completed_at, quota_date, quota_state,
    counts_toward_quota, legacy_provenance, web_status
)
SELECT site.project_id, COALESCE(scan.page_url, site.url), site.url, 'web',
       scan.scan_type, 'migration', 'system_exempt', 'complete',
       'migration:web-scan:' || scan.id,
       'migration:web-scan:' || scan.id,
       COALESCE(CAST(strftime('%s', scan.timestamp) AS INTEGER) * 1000, 0),
       COALESCE(CAST(strftime('%s', scan.timestamp) AS INTEGER) * 1000, 0) + scan.duration_ms,
       COALESCE(date(scan.timestamp), '1970-01-01'), 'exempt', 0,
       'scans:' || scan.id, 'complete'
FROM scans scan
JOIN sites site ON site.id = scan.site_id
WHERE scan.execution_id IS NULL AND scan.session_id IS NULL;

UPDATE scans
SET execution_id = (
    SELECT execution.id FROM scan_executions execution
    WHERE execution.idempotency_key = 'migration:web-scan:' || scans.id
)
WHERE execution_id IS NULL AND session_id IS NULL;

INSERT INTO scan_executions (
    project_id, environment_url, environment_scope_key, requested_mode,
    trigger, admission_class, status, idempotency_key, request_fingerprint,
    started_at, completed_at, quota_date, quota_state, counts_toward_quota,
    legacy_provenance, code_status
)
SELECT code.project_id, code.environment_url,
       COALESCE(NULLIF(code.environment_url, ''), 'project:' || code.project_id),
       'code', 'migration', 'system_exempt', 'complete',
       'migration:code-scan:' || code.id,
       'migration:code-scan:' || code.id,
       COALESCE(CAST(strftime('%s', code.checked_at) AS INTEGER) * 1000, 0),
       COALESCE(CAST(strftime('%s', code.checked_at) AS INTEGER) * 1000, 0) + code.duration_ms,
       COALESCE(date(code.checked_at), '1970-01-01'), 'exempt', 0,
       'code_scans:' || code.id, 'complete'
FROM code_scans code
WHERE code.execution_id IS NULL;

UPDATE code_scans
SET execution_id = (
    SELECT execution.id FROM scan_executions execution
    WHERE execution.idempotency_key = 'migration:code-scan:' || code_scans.id
)
WHERE execution_id IS NULL;

-- Parent runs first so page runs can reference them.
INSERT INTO scan_runs (
    execution_id, project_id, site_id, environment_url,
    environment_scope_key, source, run_kind,
    status, focus, started_at, completed_at, timestamp_text, raw_score,
    duration_ms, coverage_kind, coverage_json, detail_state, total_pages,
    completed_pages, axe_enabled, issues_total, legacy_source, legacy_id
)
SELECT session.execution_id, site.project_id, site.id, site.url, site.url,
       'web_scan',
       'multi_parent',
       CASE session.status WHEN 'complete' THEN 'complete'
            WHEN 'error' THEN 'failed' ELSE 'running' END,
       COALESCE((SELECT scan_type FROM scans page WHERE page.session_id = session.id LIMIT 1), 'health'),
       COALESCE(CAST(strftime('%s', session.started_at) AS INTEGER) * 1000, 0),
       CASE WHEN session.completed_at IS NULL THEN NULL
            ELSE COALESCE(CAST(strftime('%s', session.completed_at) AS INTEGER) * 1000, 0) END,
       session.started_at, session.overall_score, COALESCE(session.duration_ms, 0),
       'page_set', json_object(
           'kind', 'page_set',
           'successful', CASE session.status WHEN 'complete' THEN json('true') ELSE json('false') END,
           'pageUrls', COALESCE((
               SELECT json_group_array(COALESCE(page.page_url, site.url))
               FROM scans page WHERE page.session_id = session.id
           ), json('[]')),
           'producerIds', json('[]')
       ),
       CASE session.issue_snapshot_version WHEN 1 THEN 'exact' ELSE 'limited_legacy' END,
       session.total_pages, session.completed_pages, session.axe_enabled,
       (SELECT COUNT(*) FROM session_issues issue WHERE issue.session_id = session.id),
       'web_session', session.id
FROM scan_sessions session
JOIN sites site ON site.id = session.site_id;

INSERT INTO scan_runs (
    execution_id, parent_run_id, project_id, site_id, environment_url,
    environment_scope_key,
    source, run_kind, status, focus, started_at, completed_at,
    timestamp_text, raw_score, duration_ms, coverage_kind, coverage_json,
    detail_state, mode, security_score, performance_score, seo_score,
    accessibility_score, compliance_score, config_score, polish_score,
    issues_total, issues_critical, issues_high, issues_medium, issues_low,
    issues_passed, detected_stack, page_url, legacy_source, legacy_id
)
SELECT scan.execution_id,
       (SELECT parent.id FROM scan_runs parent
        WHERE parent.legacy_source = 'web_session' AND parent.legacy_id = scan.session_id),
       site.project_id, site.id, COALESCE(scan.page_url, site.url), site.url,
       'web_scan',
       CASE WHEN scan.session_id IS NULL THEN 'single' ELSE 'page' END,
       'complete', scan.scan_type,
       COALESCE(CAST(strftime('%s', scan.timestamp) AS INTEGER) * 1000, 0),
       COALESCE(CAST(strftime('%s', scan.timestamp) AS INTEGER) * 1000, 0) + scan.duration_ms,
       scan.timestamp, scan.overall_score, scan.duration_ms,
       'page',
       json_object(
           'kind', 'page', 'successful', json('true'),
           'pageUrls', json_array(COALESCE(scan.page_url, site.url)),
           'producerIds', json('[]')
       ),
       CASE scan.issue_snapshot_version WHEN 1 THEN 'exact' ELSE 'limited_legacy' END,
       scan.mode, scan.security_score, scan.performance_score, scan.seo_score,
       scan.accessibility_score, scan.compliance_score, scan.config_score,
       scan.polish_score, scan.issues_total, scan.issues_critical,
       scan.issues_high, scan.issues_medium, scan.issues_low,
       scan.issues_passed, scan.detected_stack, scan.page_url,
       'web_scan', scan.id
FROM scans scan
JOIN sites site ON site.id = scan.site_id;

INSERT INTO scan_runs (
    execution_id, project_id, environment_url, environment_scope_key,
    source, run_kind, status,
    started_at, completed_at, timestamp_text, raw_score, duration_ms,
    coverage_kind, coverage_json, detail_state, issues_total,
    issues_critical, issues_high, issues_medium, issues_low, project_path,
    framework, legacy_source, legacy_id
)
SELECT code.execution_id, code.project_id, code.environment_url,
       COALESCE(NULLIF(code.environment_url, ''), 'project:' || code.project_id),
       'code_scan',
       'code', 'complete',
       COALESCE(CAST(strftime('%s', code.checked_at) AS INTEGER) * 1000, 0),
       COALESCE(CAST(strftime('%s', code.checked_at) AS INTEGER) * 1000, 0) + code.duration_ms,
       code.checked_at, code.overall_score, code.duration_ms, 'project',
       json_object(
           'kind', 'project', 'successful', json('true'),
           'pageUrls', json('[]'), 'producerIds', json('[]')
       ),
       CASE code.issue_snapshot_version WHEN 1 THEN 'exact' ELSE 'limited_legacy' END,
       code.issue_count, code.critical_count, code.high_count,
       code.medium_count, code.low_count, code.project_path, code.framework,
       'code_scan', code.id
FROM code_scans code;

-- Immutable Web findings. Canonicalize the small alias set before persistence;
-- producer_check_id preserves the exact engine id.
INSERT INTO scan_findings (
    run_id, ordinal, occurrence_id, source, canonical_check_id,
    producer_check_id, category, verdict, severity, confidence,
    producer_category,
    confidence_reason, title, description, fix_prompt, producer_fix_prompt, manual_fix,
    why_it_matters, raw_data, location_kind, page_url
)
SELECT run.id, issue.ordinal,
       'web_scan:' || issue.check_id || ':' ||
           COALESCE(scan.page_url, (SELECT url FROM sites WHERE id = scan.site_id)) ||
           ':' || issue.ordinal,
       'web_scan',
       CASE issue.check_id
           WHEN 'security.headers.csp' THEN 'security.csp'
           WHEN 'security.headers.hsts' THEN 'security.hsts'
           WHEN 'polish.missing-og-tags' THEN 'seo.open_graph'
           WHEN 'polish.default-favicon' THEN 'config.favicon'
           WHEN 'polish.source-maps-production' THEN 'security.source_maps'
           WHEN 'polish.console-log-production' THEN 'config.console_logs'
           ELSE issue.check_id END,
       issue.check_id, issue.category, issue.check_status, issue.severity,
       issue.confidence, issue.category,
       issue.confidence_reason, issue.title,
       issue.description, issue.fix_prompt, issue.fix_prompt, issue.manual_fix,
       issue.why_it_matters, issue.raw_data, 'page',
       COALESCE(scan.page_url, (SELECT url FROM sites WHERE id = scan.site_id))
FROM scan_issues issue
JOIN scans scan ON scan.id = issue.scan_id
JOIN scan_runs run
  ON run.legacy_source = 'web_scan' AND run.legacy_id = issue.scan_id;

INSERT INTO scan_findings (
    run_id, ordinal, occurrence_id, source, canonical_check_id,
    producer_check_id, category, verdict, severity, confidence,
    producer_category,
    confidence_reason, title, description, fix_prompt, producer_fix_prompt, manual_fix,
    why_it_matters, raw_data, location_kind
)
SELECT run.id, issue.ordinal,
       'site_scan:' || issue.check_id || ':' || issue.session_id || ':' || issue.ordinal,
       'web_scan',
       CASE issue.check_id
           WHEN 'security.headers.csp' THEN 'security.csp'
           WHEN 'security.headers.hsts' THEN 'security.hsts'
           WHEN 'polish.missing-og-tags' THEN 'seo.open_graph'
           WHEN 'polish.default-favicon' THEN 'config.favicon'
           WHEN 'polish.source-maps-production' THEN 'security.source_maps'
           WHEN 'polish.console-log-production' THEN 'config.console_logs'
           ELSE issue.check_id END,
       issue.check_id, issue.category, issue.check_status, issue.severity,
       issue.confidence, issue.category,
       issue.confidence_reason, issue.title,
       issue.description, issue.fix_prompt, issue.fix_prompt, issue.manual_fix,
       issue.why_it_matters, issue.raw_data, 'site'
FROM session_issues issue
JOIN scan_runs run
  ON run.legacy_source = 'web_session' AND run.legacy_id = issue.session_id;

INSERT INTO scan_findings (
    run_id, ordinal, occurrence_id, source, canonical_check_id,
    producer_check_id, category, domain, verdict, severity, confidence,
    producer_category,
    confidence_reason, title, description, fix_prompt, producer_fix_prompt, why_it_matters,
    verification_hint, raw_data, detail_json, location_kind,
    relative_path, line
)
SELECT run.id, issue.ordinal,
       'code_scan:' ||
           CASE
               WHEN instr(COALESCE(json_extract(issue.issue_json, '$.id'), ''), ':') > 0
               THEN substr(
                   json_extract(issue.issue_json, '$.id'),
                   1,
                   instr(json_extract(issue.issue_json, '$.id'), ':') - 1
               )
               ELSE replace(issue.canonical_check_id, 'code_scan.', '')
           END || ':' ||
           COALESCE(json_extract(issue.issue_json, '$.relativePath'), '') || ':' ||
           COALESCE(json_extract(issue.issue_json, '$.line'), '') || ':' || issue.ordinal,
       'code_scan', issue.canonical_check_id,
       CASE
           WHEN instr(COALESCE(json_extract(issue.issue_json, '$.id'), ''), ':') > 0
           THEN substr(
               json_extract(issue.issue_json, '$.id'),
               1,
               instr(json_extract(issue.issue_json, '$.id'), ':') - 1
           )
           ELSE replace(issue.canonical_check_id, 'code_scan.', '')
       END,
       COALESCE(json_extract(issue.issue_json, '$.category'), 'code_quality'),
       issue.domain, 'fail', issue.severity,
       COALESCE(json_extract(issue.issue_json, '$.confidence'), 'high'),
       COALESCE(json_extract(issue.issue_json, '$.category'), 'code_quality'),
       json_extract(issue.issue_json, '$.confidenceReason'),
       issue.title,
       COALESCE(json_extract(issue.issue_json, '$.description'), ''),
       json_extract(issue.issue_json, '$.likelyFix'),
       json_extract(issue.issue_json, '$.likelyFix'),
       json_extract(issue.issue_json, '$.whyNow'),
       json_extract(issue.issue_json, '$.verifyHint'),
       json_extract(issue.issue_json, '$.evidence'), issue.issue_json, 'file',
       json_extract(issue.issue_json, '$.relativePath'),
       json_extract(issue.issue_json, '$.line')
FROM code_scan_issues issue
JOIN scan_runs run
  ON run.legacy_source = 'code_scan' AND run.legacy_id = issue.scan_id;

-- New projection provenance points at canonical runs. Source disambiguates the
-- old id spaces; first/resolved references follow the same rule.
UPDATE work_items
SET scan_ref = CASE source
        WHEN 'code_scan' THEN COALESCE(
            (SELECT id FROM scan_runs WHERE legacy_source = 'code_scan' AND legacy_id = work_items.scan_ref),
            scan_ref)
        ELSE COALESCE(
            (SELECT id FROM scan_runs WHERE legacy_source = 'web_scan' AND legacy_id = work_items.scan_ref),
            scan_ref)
    END,
    first_seen_scan_ref = CASE source
        WHEN 'code_scan' THEN COALESCE(
            (SELECT id FROM scan_runs WHERE legacy_source = 'code_scan' AND legacy_id = work_items.first_seen_scan_ref),
            first_seen_scan_ref)
        ELSE COALESCE(
            (SELECT id FROM scan_runs WHERE legacy_source = 'web_scan' AND legacy_id = work_items.first_seen_scan_ref),
            first_seen_scan_ref)
    END,
    resolved_scan_ref = CASE source
        WHEN 'code_scan' THEN COALESCE(
            (SELECT id FROM scan_runs WHERE legacy_source = 'code_scan' AND legacy_id = work_items.resolved_scan_ref),
            resolved_scan_ref)
        ELSE COALESCE(
            (SELECT id FROM scan_runs WHERE legacy_source = 'web_scan' AND legacy_id = work_items.resolved_scan_ref),
            resolved_scan_ref)
    END
WHERE source IN ('web_scan', 'site_scan', 'code_scan');

CREATE INDEX idx_work_items_canonical_scan_ref
    ON work_items(source, scan_ref);
