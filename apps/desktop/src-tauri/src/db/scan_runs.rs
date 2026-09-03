//! Canonical persistence for immutable scan runs and findings.

use std::collections::HashSet;

use rusqlite::{named_params, params, Connection, OptionalExtension, Transaction};

use crate::checks::CheckStatus;
use crate::core::engine_release::ObservedSurface;
use crate::core::normalized_scan::{NormalizedRunBatch, ScanEvidenceSource, ScanRunKind};

use super::scan_run_projection::reconcile_scan_projection;
use super::{Database, DbError};

/// Mandatory run provenance recorded at the persistence boundary.
struct RunStamp {
    engine_release: String,
    manifest_digest: String,
    canonicalizer: i64,
    crawl_profile: i64,
    execution_profile_json: String,
}

/// Record a run stamp and its referenced inventory in the caller's transaction.
fn record_run_stamp(
    conn: &Connection,
    surface: ObservedSurface,
    scan_profile: Option<&str>,
    browser_ran: bool,
    browser_build: Option<&str>,
    recorded_at: i64,
) -> Result<RunStamp, DbError> {
    let stamp =
        crate::core::engine_release::stamp(surface, scan_profile, browser_ran, browser_build);
    super::engine_release::record_inventory(
        conn,
        &stamp,
        &crate::core::engine_release::CURRENT_INVENTORY,
        recorded_at,
    )?;
    Ok(RunStamp {
        engine_release: stamp.engine_release,
        manifest_digest: stamp.manifest_digest,
        canonicalizer: i64::from(stamp.canonicalizer),
        crawl_profile: i64::from(stamp.crawl_profile),
        execution_profile_json: serde_json::to_string(&stamp.execution)?,
    })
}

/// The scope revision the run was scoped by, when the site has one. Part of
/// the captured basis: two runs taken under different scopes covered different
/// routes, so an absence in one says nothing about the other.
fn scope_revision(conn: &Connection, site_id: Option<i64>) -> Option<i64> {
    let site_id = site_id?;
    conn.query_row(
        "SELECT scope_revision FROM sites WHERE id = ?1",
        params![site_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .ok()
    .flatten()
}

fn validate_batch(batch: &NormalizedRunBatch) -> Result<(), DbError> {
    batch.coverage.validate().map_err(DbError::Other)?;
    if batch.completed_at < batch.started_at {
        return Err(DbError::Other(
            "scan run completion precedes its start".to_string(),
        ));
    }
    match (batch.source, batch.run_kind) {
        (ScanEvidenceSource::CodeScan, ScanRunKind::Code) => {}
        (ScanEvidenceSource::WebScan, ScanRunKind::Single | ScanRunKind::Page) => {}
        (ScanEvidenceSource::WebScan, ScanRunKind::MultiParent) => {}
        _ => {
            return Err(DbError::Other(
                "scan run source and kind are inconsistent".to_string(),
            ));
        }
    }
    let mut occurrences = HashSet::with_capacity(batch.findings.len());
    for finding in &batch.findings {
        if finding.source != batch.source {
            return Err(DbError::Other(
                "scan finding source differs from its run source".to_string(),
            ));
        }
        if finding.canonical_check_id.trim().is_empty()
            || finding.producer_check_id.trim().is_empty()
            || finding.occurrence_id.trim().is_empty()
        {
            return Err(DbError::Other(
                "scan finding identities must be non-empty".to_string(),
            ));
        }
        if !occurrences.insert(finding.occurrence_id.as_str()) {
            return Err(DbError::Other(format!(
                "duplicate occurrence in normalized scan batch: {}",
                finding.occurrence_id
            )));
        }
    }
    Ok(())
}

fn insert_findings(
    tx: &Transaction<'_>,
    run_id: i64,
    batch: &NormalizedRunBatch,
) -> Result<(), DbError> {
    let mut statement = tx.prepare(
        "INSERT INTO scan_findings (
            run_id, ordinal, occurrence_id, source,
            canonical_check_id, producer_check_id, category,
            producer_category, domain, verdict, severity, confidence,
            confidence_reason, title, description, fix_prompt,
            producer_fix_prompt, manual_fix, why_it_matters,
            verification_hint, raw_data, detail_json, location_kind,
            page_url, relative_path, line
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
            ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
            ?24, ?25, ?26
         )",
    )?;
    for (ordinal, finding) in batch.findings.iter().enumerate() {
        statement.execute(params![
            run_id,
            i64::try_from(ordinal)
                .map_err(|_| DbError::Other("scan finding ordinal exceeds SQLite range".into()))?,
            finding.occurrence_id,
            finding.source.as_str(),
            finding.canonical_check_id,
            finding.producer_check_id,
            finding.category,
            finding.producer_category,
            finding.domain,
            finding.verdict.as_str(),
            finding.severity.as_str(),
            finding.confidence.as_str(),
            finding.confidence_reason,
            finding.title,
            finding.description,
            finding.fix_prompt,
            finding.producer_fix_prompt,
            finding.manual_fix,
            finding.why_it_matters,
            finding.verification_hint,
            finding.raw_data,
            finding.detail_json,
            finding.location_kind.as_str(),
            finding.page_url,
            finding.relative_path,
            finding.line.map(i64::from),
        ])?;
    }
    Ok(())
}

impl Database {
    pub fn get_scan_run_execution_id(&self, run_id: i64) -> Result<Option<i64>, DbError> {
        self.execute(move |conn| {
            conn.query_row(
                "SELECT execution_id FROM scan_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(DbError::from)
        })?
    }

    pub fn start_multi_page_scan_run(
        &self,
        execution_id: i64,
        project_id: Option<i64>,
        site_id: i64,
        environment_url: &str,
        focus: crate::core::scanner::ScanType,
        selected_page_urls: &[String],
        axe_enabled: bool,
        started_at: i64,
    ) -> Result<i64, DbError> {
        let environment_url = environment_url.to_string();
        let environment_scope_key = super::helpers::normalize_url(&environment_url).0;
        let selected_page_urls = selected_page_urls.to_vec();
        self.execute(move |conn| {
            // A parent that has not finished has proved nothing. It records
            // the pages it set out to cover so the row reads honestly while
            // it runs; the claim arrives with the outcomes.
            let coverage = crate::core::normalized_scan::ScanCoverageManifest::unproven(
                crate::core::normalized_scan::ScanCoverageKind::PageSet,
                selected_page_urls.clone(),
            );
            let tx = conn.unchecked_transaction()?;
            let stamp = record_run_stamp(
                &tx,
                ObservedSurface::Web,
                Some(focus.as_str()),
                false,
                None,
                started_at,
            )?;
            tx.execute(
                "INSERT INTO scan_runs (
                    execution_id, project_id, site_id, environment_url,
                    environment_scope_key, source, run_kind, status, focus,
                    started_at, timestamp_text, duration_ms, coverage_kind,
                    coverage_json, diagnostics_json, detail_state, total_pages,
                    completed_pages, axe_enabled, engine_release,
                    manifest_digest, canonicalizer, crawl_profile,
                    execution_profile_json, scope_revision
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, 'web_scan', 'multi_parent', 'running',
                    ?6, ?7, ?8, 0, 'page_set', ?9, '{}', 'exact', ?10, 0, ?11,
                    ?12, ?13, ?14, ?15, ?16, ?17
                 )",
                params![
                    execution_id,
                    project_id,
                    site_id,
                    environment_url,
                    environment_scope_key,
                    focus.as_str(),
                    started_at,
                    chrono::DateTime::from_timestamp_millis(started_at)
                        .unwrap_or_default()
                        .to_rfc3339(),
                    serde_json::to_string(&coverage)?,
                    i64::try_from(selected_page_urls.len()).map_err(|_| {
                        DbError::Other("multi-page target count exceeds SQLite range".into())
                    })?,
                    if axe_enabled { 1_i64 } else { 0_i64 },
                    stamp.engine_release,
                    stamp.manifest_digest,
                    stamp.canonicalizer,
                    stamp.crawl_profile,
                    stamp.execution_profile_json,
                    scope_revision(&tx, Some(site_id)),
                ],
            )?;
            let run_id = tx.last_insert_rowid();
            tx.commit()?;
            Ok(run_id)
        })?
    }

    pub fn update_multi_page_scan_run_progress(
        &self,
        run_id: i64,
        completed_pages: usize,
    ) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute(
                "UPDATE scan_runs
                 SET completed_pages = ?1
                 WHERE id = ?2 AND run_kind = 'multi_parent' AND status = 'running'",
                params![
                    i64::try_from(completed_pages).map_err(|_| {
                        DbError::Other("completed page count exceeds SQLite range".into())
                    })?,
                    run_id,
                ],
            )?;
            Ok(())
        })?
    }

    pub fn complete_multi_page_scan_run(
        &self,
        run_id: i64,
        batch: NormalizedRunBatch,
    ) -> Result<(), DbError> {
        validate_batch(&batch)?;
        if batch.run_kind != ScanRunKind::MultiParent || batch.source != ScanEvidenceSource::WebScan
        {
            return Err(DbError::Other(
                "multi-page completion requires a Web parent batch".into(),
            ));
        }
        self.execute_mut(move |conn| {
            let tx = conn.transaction()?;
            let shell_execution_id: i64 = tx.query_row(
                "SELECT execution_id FROM scan_runs
                 WHERE id = ?1 AND run_kind = 'multi_parent'",
                [run_id],
                |row| row.get(0),
            )?;
            if shell_execution_id != batch.execution_id {
                return Err(DbError::Other(
                    "multi-page parent belongs to a different execution".into(),
                ));
            }
            let actionable = batch
                .findings
                .iter()
                .filter(|finding| {
                    matches!(finding.verdict, CheckStatus::Fail | CheckStatus::Warn)
                })
                .collect::<Vec<_>>();
            let count = |severity: crate::checks::Severity| -> i64 {
                actionable
                    .iter()
                    .filter(|finding| finding.severity == severity)
                    .count() as i64
            };
            let passed = batch
                .findings
                .iter()
                .filter(|finding| finding.verdict == CheckStatus::Pass)
                .count() as i64;
            tx.execute(
                "UPDATE scan_runs SET
                    project_id = :project_id,
                    site_id = :site_id,
                    environment_url = :environment_url,
                    environment_scope_key = :environment_scope_key,
                    status = :status,
                    focus = :focus,
                    started_at = :started_at,
                    completed_at = :completed_at,
                    timestamp_text = :timestamp_text,
                    raw_score = :raw_score,
                    duration_ms = :duration_ms,
                    coverage_kind = :coverage_kind,
                    coverage_json = :coverage_json,
                    diagnostics_json = :diagnostics_json,
                    status_detail = :status_detail,
                    mode = :mode,
                    issues_total = :issues_total,
                    issues_critical = :issues_critical,
                    issues_high = :issues_high,
                    issues_medium = :issues_medium,
                    issues_low = :issues_low,
                    issues_passed = :issues_passed,
                    total_pages = :total_pages,
                    completed_pages = :completed_pages,
                    axe_enabled = :axe_enabled
                 WHERE id = :run_id",
                named_params! {
                    ":project_id": batch.project_id,
                    ":site_id": batch.site_id,
                    ":environment_url": batch.environment_url,
                    ":environment_scope_key": batch.environment_scope_key,
                    ":status": batch.status.as_str(),
                    ":focus": batch.diagnostics.focus,
                    ":started_at": batch.started_at,
                    ":completed_at": batch.completed_at,
                    ":timestamp_text": batch.timestamp_text,
                    ":raw_score": batch.raw_score.map(i64::from),
                    ":duration_ms": i64::try_from(batch.duration_ms).map_err(|_| DbError::Other("scan duration exceeds SQLite range".into()))?,
                    ":coverage_kind": batch.coverage.kind.as_str(),
                    ":coverage_json": serde_json::to_string(&batch.coverage)?,
                    ":diagnostics_json": serde_json::to_string(&batch.diagnostics)?,
                    ":status_detail": batch.status_detail,
                    ":mode": batch.diagnostics.mode,
                    ":issues_total": actionable.len() as i64,
                    ":issues_critical": count(crate::checks::Severity::Critical),
                    ":issues_high": count(crate::checks::Severity::High),
                    ":issues_medium": count(crate::checks::Severity::Medium),
                    ":issues_low": count(crate::checks::Severity::Low),
                    ":issues_passed": passed,
                    ":total_pages": batch.diagnostics.total_pages.map(i64::from),
                    ":completed_pages": batch.diagnostics.completed_pages.map(i64::from),
                    ":axe_enabled": batch.diagnostics.axe_enabled.map(i64::from),
                    ":run_id": run_id,
                },
            )?;
            tx.execute("DELETE FROM scan_findings WHERE run_id = ?1", [run_id])?;
            insert_findings(&tx, run_id, &batch)?;
            reconcile_scan_projection(&tx, run_id, &batch)?;
            tx.commit()?;
            Ok(())
        })?
    }

    pub fn fail_scan_run(
        &self,
        run_id: i64,
        completed_at: i64,
        detail: &str,
    ) -> Result<(), DbError> {
        let detail = detail.to_string();
        self.execute(move |conn| {
            conn.execute(
                "UPDATE scan_runs
                 SET status = 'failed', completed_at = ?1,
                     duration_ms = MAX(0, ?1 - started_at), status_detail = ?2
                 WHERE id = ?3 AND status IN ('planned', 'running')",
                params![completed_at, detail, run_id],
            )?;
            Ok(())
        })?
    }

    /// Persist one completed (or explicitly failed/skipped) collector run and
    /// reconcile only the lifecycle scope its coverage proves.
    #[tracing::instrument(skip(self, batch), fields(execution_id = batch.execution_id, source = batch.source.as_str(), run_kind = batch.run_kind.as_str()))]
    pub fn persist_normalized_scan_run(&self, batch: NormalizedRunBatch) -> Result<i64, DbError> {
        validate_batch(&batch)?;
        self.execute_mut(move |conn| persist_run_batch(conn, batch))?
    }

    /// Async twin of [`persist_normalized_scan_run`]. Command paths use this so
    /// the report write waits on the database worker without parking an async
    /// runtime worker thread.
    pub async fn persist_normalized_scan_run_async(
        &self,
        batch: NormalizedRunBatch,
    ) -> Result<i64, DbError> {
        validate_batch(&batch)?;
        self.run_mut(move |conn| persist_run_batch(conn, batch))
            .await?
    }
}

/// Insert one normalized run with its findings and reconcile the lifecycle
/// projection, all inside a single transaction.
fn persist_run_batch(conn: &mut Connection, batch: NormalizedRunBatch) -> Result<i64, DbError> {
    let tx = conn.transaction()?;
    let coverage_json = serde_json::to_string(&batch.coverage)?;
    let diagnostics_json = serde_json::to_string(&batch.diagnostics)?;
    let actionable = batch
        .findings
        .iter()
        .filter(|finding| matches!(finding.verdict, CheckStatus::Fail | CheckStatus::Warn))
        .collect::<Vec<_>>();
    let count = |severity: crate::checks::Severity| -> i64 {
        actionable
            .iter()
            .filter(|finding| finding.severity == severity)
            .count() as i64
    };
    let passed = batch
        .findings
        .iter()
        .filter(|finding| finding.verdict == CheckStatus::Pass)
        .count() as i64;
    let surface = match batch.source {
        ScanEvidenceSource::WebScan => ObservedSurface::Web,
        ScanEvidenceSource::CodeScan => ObservedSurface::Code,
    };
    let stamp = record_run_stamp(
        &tx,
        surface,
        batch
            .diagnostics
            .focus
            .as_deref()
            .or(batch.diagnostics.mode.as_deref()),
        batch.diagnostics.browser_ran.unwrap_or(false),
        batch.diagnostics.browser_build.as_deref(),
        batch.started_at,
    )?;
    let run_scope_revision = scope_revision(&tx, batch.site_id);

    tx.execute(
                "INSERT INTO scan_runs (
                    execution_id, parent_run_id, project_id, site_id,
                    environment_url, environment_scope_key, source, run_kind,
                    status, focus,
                    started_at, completed_at, timestamp_text, raw_score,
                    duration_ms, coverage_kind, coverage_json,
                    diagnostics_json, status_detail, detail_state, mode,
                    security_score, performance_score, seo_score,
                    accessibility_score, compliance_score, config_score,
                    polish_score, issues_total, issues_critical, issues_high,
                    issues_medium, issues_low, issues_passed, detected_stack,
                    page_url, total_pages, completed_pages, axe_enabled,
                    project_path, framework, engine_release, manifest_digest,
                    canonicalizer, crawl_profile, execution_profile_json,
                    scope_revision, code_commit_sha, code_tree_clean
                 ) VALUES (
                    :execution_id, :parent_run_id, :project_id, :site_id,
                    :environment_url, :environment_scope_key, :source,
                    :run_kind, :status, :focus,
                    :started_at, :completed_at, :timestamp_text, :raw_score,
                    :duration_ms, :coverage_kind, :coverage_json,
                    :diagnostics_json, :status_detail, 'exact', :mode,
                    :security_score, :performance_score, :seo_score,
                    :accessibility_score, :compliance_score, :config_score,
                    :polish_score, :issues_total, :issues_critical, :issues_high,
                    :issues_medium, :issues_low, :issues_passed, :detected_stack,
                    :page_url, :total_pages, :completed_pages, :axe_enabled,
                    :project_path, :framework, :engine_release,
                    :manifest_digest, :canonicalizer, :crawl_profile,
                    :execution_profile_json, :scope_revision,
                    :code_commit_sha, :code_tree_clean
                 )",
                named_params! {
                    ":execution_id": batch.execution_id,
                    ":parent_run_id": batch.parent_run_id,
                    ":project_id": batch.project_id,
                    ":site_id": batch.site_id,
                    ":environment_url": batch.environment_url,
                    ":environment_scope_key": batch.environment_scope_key,
                    ":source": batch.source.as_str(),
                    ":run_kind": batch.run_kind.as_str(),
                    ":status": batch.status.as_str(),
                    ":focus": batch.diagnostics.focus,
                    ":started_at": batch.started_at,
                    ":completed_at": batch.completed_at,
                    ":timestamp_text": batch.timestamp_text,
                    ":raw_score": batch.raw_score.map(i64::from),
                    ":duration_ms": i64::try_from(batch.duration_ms).map_err(|_| DbError::Other("scan duration exceeds SQLite range".into()))?,
                    ":coverage_kind": batch.coverage.kind.as_str(),
                    ":coverage_json": coverage_json,
                    ":diagnostics_json": diagnostics_json,
                    ":status_detail": batch.status_detail,
                    ":mode": batch.diagnostics.mode,
                    ":security_score": batch.diagnostics.security_score.map(i64::from),
                    ":performance_score": batch.diagnostics.performance_score.map(i64::from),
                    ":seo_score": batch.diagnostics.seo_score.map(i64::from),
                    ":accessibility_score": batch.diagnostics.accessibility_score.map(i64::from),
                    ":compliance_score": batch.diagnostics.compliance_score.map(i64::from),
                    ":config_score": batch.diagnostics.config_score.map(i64::from),
                    ":polish_score": batch.diagnostics.polish_score.map(i64::from),
                    ":issues_total": actionable.len() as i64,
                    ":issues_critical": count(crate::checks::Severity::Critical),
                    ":issues_high": count(crate::checks::Severity::High),
                    ":issues_medium": count(crate::checks::Severity::Medium),
                    ":issues_low": count(crate::checks::Severity::Low),
                    ":issues_passed": passed,
                    ":detected_stack": batch.diagnostics.detected_stack,
                    ":page_url": batch.diagnostics.page_url,
                    ":project_path": batch.diagnostics.project_path,
                    ":framework": batch.diagnostics.framework,
                    ":code_commit_sha": batch.diagnostics.code_commit_sha,
                    ":code_tree_clean": batch.diagnostics.code_tree_clean.map(i64::from),
                    ":total_pages": batch.diagnostics.total_pages.map(i64::from),
                    ":completed_pages": batch.diagnostics.completed_pages.map(i64::from),
                    ":axe_enabled": batch.diagnostics.axe_enabled.map(i64::from),
                    ":engine_release": stamp.engine_release,
                    ":manifest_digest": stamp.manifest_digest,
                    ":canonicalizer": stamp.canonicalizer,
                    ":crawl_profile": stamp.crawl_profile,
                    ":execution_profile_json": stamp.execution_profile_json,
                    ":scope_revision": run_scope_revision,
                },
            )?;
    let run_id = tx.last_insert_rowid();
    insert_findings(&tx, run_id, &batch)?;
    reconcile_scan_projection(&tx, run_id, &batch)?;
    tx.commit()?;
    Ok(run_id)
}

#[cfg(test)]
#[path = "scan_runs_tests.rs"]
mod tests;
