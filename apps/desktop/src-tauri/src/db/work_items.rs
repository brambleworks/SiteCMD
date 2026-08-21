//! Raw issue signals combined with lifecycle overlays at read time.

use super::helpers::{normalize_url, parse_optional_enum_required, parse_required_enum};
pub use super::work_item_types::{IssueCheckMemory, WorkItemInput, WorkItemMetadata, WorkItemRow};
use super::Database;
use super::DbError;
use rusqlite::params;

impl Database {
    /// Upsert a batch of observed signals for one (source, project, env_url) tuple.
    /// Rows with matching (source, signal_id) and resolved_at IS NULL get last_seen_at + detail_json updated.
    /// Active rows not in `observed` get resolved_at = observed_at (diff-based resolution).
    pub fn upsert_work_items_diff(
        &self,
        source: &str,
        project_id: i64,
        env_url: &str,
        observed: Vec<WorkItemInput>,
        observed_at: i64,
    ) -> Result<(), DbError> {
        self.upsert_work_items_diff_with_scan_ref(
            source,
            project_id,
            env_url,
            observed,
            observed_at,
            None,
            true,
            Vec::new(),
            None,
        )
    }

    /// Refresh observed signals without resolving unobserved absences.
    /// Used for partial integration polls where absence is not proof of resolution.
    pub fn upsert_work_items_observe_only(
        &self,
        source: &str,
        project_id: i64,
        env_url: &str,
        observed: Vec<WorkItemInput>,
        observed_at: i64,
    ) -> Result<(), DbError> {
        self.upsert_work_items_diff_with_scan_ref(
            source,
            project_id,
            env_url,
            observed,
            observed_at,
            None,
            false,
            Vec::new(),
            None,
        )
    }

    /// Diff-based upsert for a partial observation.
    ///
    /// Signal families named by `unobserved_signal_prefixes` cannot resolve by
    /// absence; all observed items and other families update normally.
    pub fn upsert_work_items_diff_except_unobserved(
        &self,
        source: &str,
        project_id: i64,
        env_url: &str,
        observed: Vec<WorkItemInput>,
        observed_at: i64,
        unobserved_signal_prefixes: &[String],
    ) -> Result<(), DbError> {
        // Every string has an empty prefix, which would exempt every active row
        // from resolution. Catch that contract violation in debug builds.
        debug_assert!(
            unobserved_signal_prefixes
                .iter()
                .all(|prefix| !prefix.is_empty()),
            "empty unobserved signal prefix would exempt every row from resolution"
        );
        self.upsert_work_items_diff_with_scan_ref(
            source,
            project_id,
            env_url,
            observed,
            observed_at,
            None,
            true,
            unobserved_signal_prefixes.to_vec(),
            None,
        )
    }

    /// Upsert scan-backed work items with exact resolution provenance.
    pub fn upsert_work_items_diff_for_scan(
        &self,
        source: &str,
        project_id: i64,
        env_url: &str,
        observed: Vec<WorkItemInput>,
        observed_at: i64,
        resolution_scan_ref: i64,
    ) -> Result<(), DbError> {
        self.upsert_work_items_diff_with_scan_ref(
            source,
            project_id,
            env_url,
            observed,
            observed_at,
            Some(resolution_scan_ref),
            true,
            Vec::new(),
            None,
        )
    }

    /// Resolve absent findings only for the page this scan observed.
    pub fn upsert_work_items_diff_for_page_scan(
        &self,
        source: &str,
        project_id: i64,
        env_url: &str,
        page_url: &str,
        observed: Vec<WorkItemInput>,
        observed_at: i64,
        resolution_scan_ref: i64,
    ) -> Result<(), DbError> {
        self.upsert_work_items_diff_with_scan_ref(
            source,
            project_id,
            env_url,
            observed,
            observed_at,
            Some(resolution_scan_ref),
            true,
            Vec::new(),
            Some(page_url.to_string()),
        )
    }

    #[tracing::instrument(skip(self, observed, env_url, unobserved_signal_prefixes, resolution_page_url), fields(source = %source, project_id, observed_at, resolution_scan_ref, resolve_absent))]
    fn upsert_work_items_diff_with_scan_ref(
        &self,
        source: &str,
        project_id: i64,
        env_url: &str,
        observed: Vec<WorkItemInput>,
        observed_at: i64,
        resolution_scan_ref: Option<i64>,
        resolve_absent: bool,
        unobserved_signal_prefixes: Vec<String>,
        resolution_page_url: Option<String>,
    ) -> Result<(), DbError> {
        let source = source.to_string();
        let env_url = normalize_url(env_url).0;
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;

            let observed_ids: Vec<String> = observed.iter().map(|o| o.signal_id.clone()).collect();

            // Normalize at the write boundary so diff resolution and scoped reads
            // address the same row.
            for item in &observed {
                let item_env_url = normalize_url(&item.env_url).0;
                tx.execute(
                    "INSERT INTO work_items
                        (project_id, env_url, source, signal_id, check_id, category, severity,
                         title, description, detail_json, scan_ref, page_url, fix_prompt,
                         manual_fix, why_it_matters, first_seen_at, last_seen_at, resolved_at,
                         confidence, domain, relative_path, line, check_status, confidence_reason,
                         producer_check_id, producer_fix_prompt, producer_category,
                         first_seen_scan_ref, resolved_scan_ref)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16, NULL,
                             ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?11, NULL)
                     ON CONFLICT(project_id, env_url, source, signal_id) WHERE resolved_at IS NULL
                     DO UPDATE SET
                        last_seen_at = excluded.last_seen_at,
                        category = excluded.category,
                        severity = excluded.severity,
                        title = excluded.title,
                        description = excluded.description,
                        detail_json = excluded.detail_json,
                        page_url = excluded.page_url,
                        scan_ref = excluded.scan_ref,
                        fix_prompt = excluded.fix_prompt,
                        manual_fix = excluded.manual_fix,
                        why_it_matters = excluded.why_it_matters,
                        confidence = excluded.confidence,
                        domain = excluded.domain,
                        relative_path = excluded.relative_path,
                        line = excluded.line,
                        check_status = excluded.check_status,
                        confidence_reason = excluded.confidence_reason,
                        producer_check_id = excluded.producer_check_id,
                        producer_fix_prompt = excluded.producer_fix_prompt,
                        producer_category = excluded.producer_category",
                    params![
                        item.project_id,
                        item_env_url,
                        item.source,
                        item.signal_id,
                        item.check_id,
                        item.category,
                        item.severity.as_str(),
                        item.title,
                        item.description,
                        item.detail_json,
                        item.scan_ref,
                        item.page_url,
                        item.fix_prompt,
                        item.manual_fix,
                        item.why_it_matters,
                        item.observed_at,
                        item.metadata.confidence.map(|c| c.as_str()),
                        item.metadata.domain.map(|d| d.as_str()),
                        item.metadata.relative_path,
                        item.metadata.line,
                        item.metadata.check_status.map(|status| status.as_str()),
                        item.metadata.confidence_reason,
                        item.metadata.producer_check_id,
                        item.metadata.producer_fix_prompt,
                        item.metadata.producer_category.map(|category| category.as_str()),
                    ],
                )
                ?;
            }

            // Share re-observation lifecycle reconciliation with scan projection.
            super::issue_states::reconcile_reobserved_lifecycle(
                &tx,
                project_id,
                &env_url,
                observed_at,
                observed.iter().map(|o| o.check_id.as_str()),
            )?;

            // Resolve absent findings only after a complete observation.
            if resolve_absent {
                // Unobserved signal families cannot resolve by absence. Use
                // substr prefix matching to avoid wildcard semantics.
                tx.execute_batch(
                    "CREATE TEMP TABLE IF NOT EXISTS temp_unobserved_work_item_signal_prefixes (
                        prefix TEXT PRIMARY KEY
                     ) WITHOUT ROWID;
                     DELETE FROM temp_unobserved_work_item_signal_prefixes;",
                )
                ?;
                for prefix in &unobserved_signal_prefixes {
                    tx.execute(
                        "INSERT OR IGNORE INTO temp_unobserved_work_item_signal_prefixes (prefix)
                         VALUES (?1)",
                        params![prefix],
                    )
                    ?;
                }
                if observed_ids.is_empty() {
                    tx.execute(
                        "UPDATE work_items
                         SET resolved_at = ?1, resolved_scan_ref = ?5
                         WHERE source = ?2 AND project_id = ?3 AND env_url = ?4
                           AND resolved_at IS NULL
                           AND (
                               ?6 IS NULL
                               OR RTRIM(work_items.page_url, '/') = RTRIM(?6, '/')
                           )
                           AND NOT EXISTS (
                               SELECT 1
                               FROM temp_unobserved_work_item_signal_prefixes unobserved
                               WHERE substr(work_items.signal_id, 1, length(unobserved.prefix))
                                     = unobserved.prefix
                           )",
                        params![
                            observed_at,
                            source,
                            project_id,
                            env_url,
                            resolution_scan_ref,
                            resolution_page_url,
                        ],
                    )
                    ?;
                } else {
                    tx.execute_batch(
                        "CREATE TEMP TABLE IF NOT EXISTS temp_observed_work_item_signal_ids (
                            signal_id TEXT PRIMARY KEY
                         ) WITHOUT ROWID;
                         DELETE FROM temp_observed_work_item_signal_ids;",
                    )
                    ?;
                    for id in &observed_ids {
                        tx.execute(
                            "INSERT OR IGNORE INTO temp_observed_work_item_signal_ids (signal_id)
                             VALUES (?1)",
                            params![id],
                        )
                        ?;
                    }
                    tx.execute(
                        "UPDATE work_items
                         SET resolved_at = ?1, resolved_scan_ref = ?5
                         WHERE source = ?2 AND project_id = ?3 AND env_url = ?4
                           AND resolved_at IS NULL
                           AND (
                               ?6 IS NULL
                               OR RTRIM(work_items.page_url, '/') = RTRIM(?6, '/')
                           )
                           AND NOT EXISTS (
                               SELECT 1
                               FROM temp_observed_work_item_signal_ids observed
                               WHERE observed.signal_id = work_items.signal_id
                           )
                           AND NOT EXISTS (
                               SELECT 1
                               FROM temp_unobserved_work_item_signal_prefixes unobserved
                               WHERE substr(work_items.signal_id, 1, length(unobserved.prefix))
                                     = unobserved.prefix
                           )",
                        params![
                            observed_at,
                            source,
                            project_id,
                            env_url,
                            resolution_scan_ref,
                            resolution_page_url,
                        ],
                    )
                    ?;
                    tx.execute("DELETE FROM temp_observed_work_item_signal_ids", [])
                        ?;
                }
                tx.execute("DELETE FROM temp_unobserved_work_item_signal_prefixes", [])
                    ?;
            }

            tx.commit().map_err(DbError::from)
        })?
    }

    /// Earliest `first_seen_at` for a check_id on this project's non-production
    /// environments, excluding the current env_url and bounded by the lookup window.
    pub fn get_check_first_seen_on_nonprod_env(
        &self,
        project_id: i64,
        current_env_url: &str,
        check_id: &str,
        since_ms: i64,
    ) -> Result<Option<i64>, DbError> {
        let current_env_url = current_env_url.to_string();
        let check_id = check_id.to_string();
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT MIN(wi.first_seen_at)
                 FROM work_items wi
                 INNER JOIN environments e
                   ON e.url = wi.env_url AND e.project_id = wi.project_id
                 WHERE wi.project_id = ?1
                   AND wi.env_url != ?2
                   AND e.environment != 'production'
                   AND wi.check_id = ?3
                   AND wi.first_seen_at >= ?4",
            )?;
            stmt.query_row(
                rusqlite::params![project_id, current_env_url, check_id, since_ms],
                |row| row.get::<_, Option<i64>>(0),
            )
        })?
        .map_err(DbError::from)
    }

    /// Indexed source lookup for an active issue group.
    #[tracing::instrument(skip(self, env_url, check_id), fields(project_id))]
    pub fn get_active_issue_sources(
        &self,
        project_id: i64,
        env_url: &str,
        check_id: &str,
    ) -> Result<Vec<String>, DbError> {
        let env_key = normalize_url(env_url).0;
        let check_id = check_id.to_string();
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT source FROM work_items
                     WHERE project_id = ?1 AND env_url = ?2 AND check_id = ?3
                       AND resolved_at IS NULL
                     ORDER BY source",
            )?;
            let sources = stmt
                .query_map(params![project_id, env_key, check_id], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<String>, _>>()?;
            Ok(sources)
        })?
    }

    #[tracing::instrument(skip(self, env_url), fields(project_id))]
    pub fn get_active_work_items(
        &self,
        project_id: i64,
        env_url: Option<&str>,
    ) -> Result<Vec<WorkItemRow>, DbError> {
        let env_key = env_url.map(|u| normalize_url(u).0);
        self.execute(move |conn| {
            let mut rows: Vec<WorkItemRow> = Vec::new();
            let (sql, has_env) = match &env_key {
                Some(_) => (
                    "SELECT id, project_id, env_url, source, signal_id, check_id, category, severity,
                            title, description, detail_json, scan_ref, page_url, fix_prompt, first_seen_at, last_seen_at, resolved_at,
                            confidence, domain, relative_path, line, check_status, confidence_reason,
                            producer_check_id, producer_fix_prompt, producer_category,
                            manual_fix, why_it_matters
                     FROM work_items
                     WHERE project_id = ?1 AND env_url = ?2 AND resolved_at IS NULL",
                    true,
                ),
                None => (
                    "SELECT id, project_id, env_url, source, signal_id, check_id, category, severity,
                            title, description, detail_json, scan_ref, page_url, fix_prompt, first_seen_at, last_seen_at, resolved_at,
                            confidence, domain, relative_path, line, check_status, confidence_reason,
                            producer_check_id, producer_fix_prompt, producer_category,
                            manual_fix, why_it_matters
                     FROM work_items
                     WHERE project_id = ?1 AND resolved_at IS NULL",
                    false,
                ),
            };
            let mut stmt = conn.prepare(sql)?;
            let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<WorkItemRow> {
                Ok(WorkItemRow {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    env_url: row.get(2)?,
                    source: row.get(3)?,
                    signal_id: row.get(4)?,
                    check_id: row.get(5)?,
                    category: row.get(6)?,
                    severity: parse_required_enum(
                        7,
                        "work_items.severity",
                        &row.get::<_, String>(7)?,
                    )?,
                    title: row.get(8)?,
                    description: row.get(9)?,
                    detail_json: row.get(10)?,
                    scan_ref: row.get(11)?,
                    page_url: row.get(12)?,
                    fix_prompt: row.get(13)?,
                    first_seen_at: row.get(14)?,
                    last_seen_at: row.get(15)?,
                    resolved_at: row.get(16)?,
                    metadata: WorkItemMetadata {
                        confidence: parse_optional_enum_required(
                            17,
                            "work_items.confidence",
                            row.get(17)?,
                        )?,
                        domain: parse_optional_enum_required(
                            18,
                            "work_items.domain",
                            row.get(18)?,
                        )?,
                        relative_path: row.get(19)?,
                        line: row.get(20)?,
                        check_status: parse_optional_enum_required(
                            21,
                            "work_items.check_status",
                            row.get(21)?,
                        )?,
                        confidence_reason: row.get(22)?,
                        producer_check_id: row.get(23)?,
                        producer_fix_prompt: row.get(24)?,
                        producer_category: parse_optional_enum_required(
                            25,
                            "work_items.producer_category",
                            row.get(25)?,
                        )?,
                    },
                    manual_fix: row.get(26)?,
                    why_it_matters: row.get(27)?,
                })
            };
            if has_env {
                let key = env_key
                    .as_ref()
                    .expect("env_key is Some when has_env is true");
                let iter = stmt
                    .query_map(params![project_id, key], map_row)
                    ?;
                for r in iter {
                    rows.push(r?);
                }
            } else {
                let iter = stmt
                    .query_map(params![project_id], map_row)
                    ?;
                for r in iter {
                    rows.push(r?);
                }
            }
            // Sort by severity rank because lexical text order is incorrect.
            rows.sort_by(|left, right| {
                left.severity
                    .sort_rank()
                    .cmp(&right.severity.sort_rank())
                    .then_with(|| right.last_seen_at.cmp(&left.last_seen_at))
                    .then_with(|| left.source.cmp(&right.source))
                    .then_with(|| left.signal_id.cmp(&right.signal_id))
                    .then_with(|| left.id.cmp(&right.id))
            });
            Ok(rows)
        })?
    }

    /// Read active `(check_id, source)` pairs without loading issue payloads.
    #[tracing::instrument(skip(self, env_url), fields(project_id))]
    pub fn get_active_work_item_idents(
        &self,
        project_id: i64,
        env_url: Option<&str>,
    ) -> Result<Vec<(String, String)>, DbError> {
        let env_key = env_url.map(|u| normalize_url(u).0);
        self.execute(move |conn| {
            let mut rows: Vec<(String, String)> = Vec::new();
            let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(String, String)> {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            };
            match &env_key {
                Some(key) => {
                    let mut stmt = conn.prepare(
                        "SELECT check_id, source FROM work_items
                             WHERE project_id = ?1 AND env_url = ?2 AND resolved_at IS NULL",
                    )?;
                    let iter = stmt.query_map(params![project_id, key], map_row)?;
                    for r in iter {
                        rows.push(r?);
                    }
                }
                None => {
                    let mut stmt = conn.prepare(
                        "SELECT check_id, source FROM work_items
                             WHERE project_id = ?1 AND resolved_at IS NULL",
                    )?;
                    let iter = stmt.query_map(params![project_id], map_row)?;
                    for r in iter {
                        rows.push(r?);
                    }
                }
            }
            Ok(rows)
        })?
    }

    /// Read issue lifecycle timestamps and active environments for dossier history.
    pub fn get_issue_check_memory(
        &self,
        project_id: i64,
        check_id: &str,
    ) -> Result<IssueCheckMemory, DbError> {
        let check_id = check_id.to_string();
        self.execute(move |conn| {
            let (first_seen, last_failed, last_verified) = conn.query_row(
                "SELECT MIN(first_seen_at), MAX(last_seen_at), MAX(resolved_at)
                     FROM work_items
                     WHERE project_id = ?1 AND check_id = ?2",
                params![project_id, check_id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                },
            )?;

            let mut stmt = conn.prepare(
                "SELECT DISTINCT env_url FROM work_items
                     WHERE project_id = ?1 AND check_id = ?2 AND resolved_at IS NULL
                     ORDER BY env_url",
            )?;
            let affected_env_urls = stmt
                .query_map(params![project_id, check_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(IssueCheckMemory {
                first_seen,
                last_failed,
                last_verified,
                affected_env_urls,
            })
        })?
    }
}

#[cfg(test)]
#[path = "work_items_tests.rs"]
mod tests;
