-- Collapse location-bearing Code Scan IDs to rule identity while preserving
-- path and line as occurrence evidence. Resolve collisions before restoring indexes.

CREATE TEMP TABLE _m011_code_id_map (
    old_id TEXT PRIMARY KEY,
    producer_rule TEXT,
    canonical_id TEXT
);

INSERT OR IGNORE INTO _m011_code_id_map(old_id)
SELECT check_id FROM work_items WHERE check_id LIKE 'code_scan.%'
UNION SELECT canonical_check_id FROM code_scan_issues WHERE canonical_check_id LIKE 'code_scan.%'
UNION SELECT check_id FROM project_issue_states WHERE check_id LIKE 'code_scan.%'
UNION SELECT check_id FROM issue_links WHERE check_id LIKE 'code_scan.%'
UNION SELECT check_id FROM fix_attempts WHERE check_id LIKE 'code_scan.%'
UNION SELECT check_id FROM dismissed_integration_hints WHERE check_id LIKE 'code_scan.%'
UNION SELECT check_id FROM site_event_check_ids WHERE check_id LIKE 'code_scan.%'
UNION SELECT cause_check_id FROM causal_link_observations WHERE cause_check_id LIKE 'code_scan.%'
UNION SELECT effect_check_id FROM causal_link_observations WHERE effect_check_id LIKE 'code_scan.%'
UNION SELECT check_id FROM cross_project_pattern_index WHERE check_id LIKE 'code_scan.%'
UNION SELECT check_id FROM regression_check_ids WHERE check_id LIKE 'code_scan.%'
UNION SELECT value FROM regressions, json_each(regressions.fixed_check_ids_json)
    WHERE json_valid(regressions.fixed_check_ids_json)
      AND value LIKE 'code_scan.%';

UPDATE _m011_code_id_map
SET producer_rule = substr(
    CASE WHEN instr(old_id, ':') > 0
         THEN substr(old_id, 1, instr(old_id, ':') - 1)
         ELSE old_id
    END,
    length('code_scan.') + 1
);

-- Keep this mapping in lockstep with correlation/signal_mapping.rs. These
-- legacy producer signals dedupe with Web checks; every current registry slug
-- otherwise falls through to `code_scan.<rule>`.
UPDATE _m011_code_id_map
SET canonical_id = CASE producer_rule
    WHEN 'security_headers' THEN 'security.csp'
    WHEN 'env_exposure' THEN 'security.exposed-env'
    WHEN 'mixed_content' THEN 'security.mixed_content'
    WHEN 'cors_wildcard' THEN 'security.cors'
    WHEN 'cookie_flags' THEN 'security.cookie-flags'
    WHEN 'robots_config' THEN 'seo.robots'
    WHEN 'canonical_missing' THEN 'seo.canonical.missing'
    WHEN 'sitemap_missing' THEN 'seo.sitemap.missing'
    WHEN 'https_redirect' THEN 'security.https'
    WHEN 'hsts_missing' THEN 'security.hsts'
    ELSE 'code_scan.' || producer_rule
END;

-- Materialize fix targets before canonical ids lose their location. Prefer the
-- promoted work-item columns over parsing because some producer ids contain
-- qualifiers in addition to the path.
DROP INDEX uq_fix_attempts_active;
ALTER TABLE fix_attempts ADD COLUMN target_kind TEXT NOT NULL DEFAULT 'group'
    CHECK(target_kind IN ('group', 'occurrence'));
ALTER TABLE fix_attempts ADD COLUMN target_relative_path TEXT;
ALTER TABLE fix_attempts ADD COLUMN target_line INTEGER;
ALTER TABLE fix_attempts ADD COLUMN producer_rule TEXT;

UPDATE fix_attempts
SET target_kind = 'occurrence',
    target_relative_path = COALESCE(
        (SELECT wi.relative_path
         FROM work_items wi
         WHERE wi.project_id = fix_attempts.project_id
           AND wi.env_url = fix_attempts.env_url
           AND wi.check_id = fix_attempts.check_id
           AND wi.relative_path IS NOT NULL
         ORDER BY wi.last_seen_at DESC, wi.id DESC LIMIT 1),
        (SELECT json_extract(csi.issue_json, '$.relativePath')
         FROM code_scan_issues csi
         WHERE csi.canonical_check_id = fix_attempts.check_id
           AND json_valid(csi.issue_json)
         ORDER BY csi.scan_id DESC, csi.ordinal LIMIT 1),
        substr(check_id, instr(check_id, ':') + 1)
    ),
    target_line = COALESCE(
        (SELECT wi.line
         FROM work_items wi
         WHERE wi.project_id = fix_attempts.project_id
           AND wi.env_url = fix_attempts.env_url
           AND wi.check_id = fix_attempts.check_id
           AND wi.relative_path IS NOT NULL
         ORDER BY wi.last_seen_at DESC, wi.id DESC LIMIT 1),
        (SELECT json_extract(csi.issue_json, '$.line')
         FROM code_scan_issues csi
         WHERE csi.canonical_check_id = fix_attempts.check_id
           AND json_valid(csi.issue_json)
         ORDER BY csi.scan_id DESC, csi.ordinal LIMIT 1)
    )
WHERE check_id LIKE 'code_scan.%:%';

UPDATE fix_attempts
SET check_id = (SELECT canonical_id FROM _m011_code_id_map WHERE old_id = fix_attempts.check_id)
WHERE check_id IN (SELECT old_id FROM _m011_code_id_map);

UPDATE fix_attempts
SET producer_rule = (
    SELECT producer_rule FROM _m011_code_id_map
    WHERE canonical_id = fix_attempts.check_id
    ORDER BY old_id LIMIT 1
)
WHERE producer_rule IS NULL
  AND check_id IN (SELECT canonical_id FROM _m011_code_id_map);

-- Only exact group+target duplicates collide. Preserve completed history and
-- keep the newest active row while canceling older active duplicates.
UPDATE fix_attempts
SET status = 'canceled'
WHERE id IN (
    SELECT id FROM (
        SELECT id,
               row_number() OVER (
                   PARTITION BY project_id, env_url, check_id, target_kind,
                                COALESCE(target_relative_path, ''),
                                COALESCE(target_line, -1)
                   ORDER BY updated_at DESC, id DESC
               ) AS duplicate_rank
        FROM fix_attempts
        WHERE status IN ('briefed', 'verify_requested', 'verifying')
    )
    WHERE duplicate_rank > 1
);

CREATE UNIQUE INDEX uq_fix_attempts_active
    ON fix_attempts(
        project_id, env_url, check_id, target_kind,
        COALESCE(target_relative_path, ''), COALESCE(target_line, -1)
    )
    WHERE status IN ('briefed', 'verify_requested', 'verifying');

-- Preserve every occurrence row. Only the group identity and serialized
-- canonical field change; signal_id, relative_path, line, and raw issue id stay.
UPDATE work_items
SET producer_check_id = (
        SELECT producer_rule FROM _m011_code_id_map WHERE old_id = work_items.check_id
    ),
    detail_json = CASE
        WHEN detail_json IS NULL THEN NULL
        WHEN json_valid(detail_json) THEN json_set(
            detail_json,
            '$.checkId',
            (SELECT canonical_id FROM _m011_code_id_map WHERE old_id = work_items.check_id)
        )
        ELSE detail_json
    END,
    check_id = (
        SELECT canonical_id FROM _m011_code_id_map WHERE old_id = work_items.check_id
    )
WHERE source = 'code_scan'
  AND check_id IN (SELECT old_id FROM _m011_code_id_map);

UPDATE code_scan_issues
SET issue_json = CASE
        WHEN json_valid(issue_json) THEN json_set(
            issue_json,
            '$.checkId',
            (SELECT canonical_id FROM _m011_code_id_map
             WHERE old_id = code_scan_issues.canonical_check_id)
        )
        ELSE issue_json
    END,
    canonical_check_id = (
        SELECT canonical_id FROM _m011_code_id_map
        WHERE old_id = code_scan_issues.canonical_check_id
    )
WHERE canonical_check_id IN (SELECT old_id FROM _m011_code_id_map);

-- Lifecycle rows can collapse many old locations (and a pre-existing mapped
-- Web row) into one identity. The most recently changed row wins.
DROP INDEX uq_project_issue_states_identity;
CREATE TEMP TABLE _m011_state_normalized AS
SELECT id,
       COALESCE(
           (SELECT canonical_id FROM _m011_code_id_map WHERE old_id = pis.check_id),
           pis.check_id
       ) AS canonical_id,
       row_number() OVER (
           PARTITION BY project_id, env_url,
               COALESCE(
                   (SELECT canonical_id FROM _m011_code_id_map WHERE old_id = pis.check_id),
                   pis.check_id
               )
           ORDER BY last_status_changed_at DESC, id DESC
       ) AS winner_rank
FROM project_issue_states pis;

DELETE FROM project_issue_states
WHERE id IN (SELECT id FROM _m011_state_normalized WHERE winner_rank > 1);

UPDATE project_issue_states
SET check_id = (
    SELECT canonical_id FROM _m011_state_normalized
    WHERE id = project_issue_states.id
)
WHERE id IN (SELECT id FROM _m011_state_normalized);

DROP TABLE _m011_state_normalized;
CREATE UNIQUE INDEX uq_project_issue_states_identity
    ON project_issue_states(project_id, env_url, check_id);

UPDATE issue_links
SET check_id = (SELECT canonical_id FROM _m011_code_id_map WHERE old_id = issue_links.check_id)
WHERE check_id IN (SELECT old_id FROM _m011_code_id_map);

-- Rebuild primary-key junctions so many-to-one updates cannot violate their
-- old composite keys mid-statement.
CREATE TABLE _m011_site_event_check_ids (
    event_id INTEGER NOT NULL,
    check_id TEXT NOT NULL,
    PRIMARY KEY (event_id, check_id),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);
INSERT INTO _m011_site_event_check_ids(event_id, check_id)
SELECT DISTINCT event_id,
       COALESCE((SELECT canonical_id FROM _m011_code_id_map WHERE old_id = src.check_id), src.check_id)
FROM site_event_check_ids src;
DROP TABLE site_event_check_ids;
ALTER TABLE _m011_site_event_check_ids RENAME TO site_event_check_ids;
CREATE INDEX idx_site_event_check_ids_by_check
    ON site_event_check_ids(check_id, event_id);

CREATE TABLE _m011_dismissed_integration_hints (
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    check_id TEXT NOT NULL,
    integration_type TEXT NOT NULL,
    dismissed_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, check_id, integration_type)
);
INSERT INTO _m011_dismissed_integration_hints(
    project_id, check_id, integration_type, dismissed_at
)
SELECT project_id,
       COALESCE((SELECT canonical_id FROM _m011_code_id_map WHERE old_id = src.check_id), src.check_id),
       integration_type,
       MAX(dismissed_at)
FROM dismissed_integration_hints src
GROUP BY project_id,
         COALESCE((SELECT canonical_id FROM _m011_code_id_map WHERE old_id = src.check_id), src.check_id),
         integration_type;
DROP TABLE dismissed_integration_hints;
ALTER TABLE _m011_dismissed_integration_hints RENAME TO dismissed_integration_hints;

CREATE TABLE _m011_regression_check_ids (
    regression_id INTEGER NOT NULL REFERENCES regressions(id) ON DELETE CASCADE,
    check_id TEXT NOT NULL,
    PRIMARY KEY (regression_id, check_id)
);
INSERT INTO _m011_regression_check_ids(regression_id, check_id)
SELECT DISTINCT regression_id,
       COALESCE((SELECT canonical_id FROM _m011_code_id_map WHERE old_id = src.check_id), src.check_id)
FROM regression_check_ids src;
DROP TABLE regression_check_ids;
ALTER TABLE _m011_regression_check_ids RENAME TO regression_check_ids;
CREATE INDEX idx_regression_check_ids_by_check
    ON regression_check_ids(check_id, regression_id);

UPDATE regressions
SET fixed_check_ids_json = (
    SELECT json_group_array(mapped_id)
    FROM (
        SELECT DISTINCT COALESCE(
            (SELECT canonical_id FROM _m011_code_id_map WHERE old_id = value),
            value
        ) AS mapped_id
        FROM json_each(regressions.fixed_check_ids_json)
    )
)
WHERE json_valid(fixed_check_ids_json);

UPDATE causal_link_observations
SET cause_check_id = COALESCE(
        (SELECT canonical_id FROM _m011_code_id_map
         WHERE old_id = causal_link_observations.cause_check_id),
        cause_check_id
    ),
    effect_check_id = COALESCE(
        (SELECT canonical_id FROM _m011_code_id_map
         WHERE old_id = causal_link_observations.effect_check_id),
        effect_check_id
    );
DELETE FROM causal_link_observations
WHERE rowid NOT IN (
    SELECT MIN(rowid)
    FROM causal_link_observations
    GROUP BY project_id, cause_check_id, effect_check_id, observed_at,
             co_active, co_resolved, COALESCE(resolution_event_id, -1)
);

-- This index is derived data; rebuilding from migrated occurrences is safer
-- than trying to merge primary-key collisions in place.
DELETE FROM cross_project_pattern_index;
INSERT INTO cross_project_pattern_index(check_id, project_count, latest_seen_ms, updated_at)
SELECT check_id, COUNT(DISTINCT project_id), MAX(last_seen_at), MAX(last_seen_at)
FROM work_items
GROUP BY check_id;

-- Abort the transaction if any canonical Code reference or serialized
-- canonical field remains location-bearing, or if a structured occurrence
-- target could not retain a path.
CREATE TEMP TABLE _m011_validation_guard (
    ok INTEGER NOT NULL CHECK(ok = 1)
);

INSERT INTO _m011_validation_guard(ok)
SELECT CASE WHEN NOT EXISTS (
    SELECT 1 FROM work_items WHERE check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM code_scan_issues WHERE canonical_check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM project_issue_states WHERE check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM issue_links WHERE check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM fix_attempts WHERE check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM dismissed_integration_hints WHERE check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM site_event_check_ids WHERE check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM causal_link_observations WHERE cause_check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM causal_link_observations WHERE effect_check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM cross_project_pattern_index WHERE check_id LIKE 'code_scan.%:%'
    UNION ALL SELECT 1 FROM regression_check_ids WHERE check_id LIKE 'code_scan.%:%'
) THEN 1 ELSE 0 END;

INSERT INTO _m011_validation_guard(ok)
SELECT CASE WHEN NOT EXISTS (
    SELECT 1 FROM work_items
    WHERE source = 'code_scan'
      AND (producer_check_id IS NULL
           OR (detail_json IS NOT NULL AND NOT json_valid(detail_json))
           OR (detail_json IS NOT NULL
               AND json_extract(detail_json, '$.checkId') <> check_id))
    UNION ALL
    SELECT 1 FROM code_scan_issues
    WHERE NOT json_valid(issue_json)
       OR json_extract(issue_json, '$.checkId') <> canonical_check_id
    UNION ALL
    SELECT 1 FROM fix_attempts
    WHERE target_kind = 'occurrence'
      AND (producer_rule IS NULL
           OR target_relative_path IS NULL
           OR target_relative_path = '')
    UNION ALL
    SELECT 1
    FROM regressions, json_each(regressions.fixed_check_ids_json)
    WHERE json_valid(regressions.fixed_check_ids_json)
      AND value LIKE 'code_scan.%:%'
) THEN 1 ELSE 0 END;

DROP TABLE _m011_validation_guard;
DROP TABLE _m011_code_id_map;
