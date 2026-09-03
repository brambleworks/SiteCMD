//! Transactional reconciliation of canonical scan runs and work-item projections.

use std::collections::BTreeSet;

use rusqlite::types::ToSql;
use rusqlite::{params, Transaction};

use super::helpers::{lifecycle_env_url, normalize_occurrence_url};
use super::issue_states::reconcile_reobserved_lifecycle;
use super::DbError;
use crate::checks::CheckStatus;
use crate::core::normalized_scan::{
    NormalizedFinding, NormalizedRunBatch, ScanCoverageManifest, ScanEvidenceSource, ScanRunKind,
};

fn projection_source(batch: &NormalizedRunBatch) -> &'static str {
    match (batch.source, batch.run_kind) {
        (ScanEvidenceSource::WebScan, ScanRunKind::MultiParent) => "site_scan",
        (ScanEvidenceSource::WebScan, _) => "web_scan",
        (ScanEvidenceSource::CodeScan, _) => "code_scan",
    }
}

fn actionable(finding: &NormalizedFinding) -> bool {
    matches!(finding.verdict, CheckStatus::Fail | CheckStatus::Warn)
}

pub(crate) fn reconcile_scan_projection(
    tx: &Transaction<'_>,
    run_id: i64,
    batch: &NormalizedRunBatch,
) -> Result<(), DbError> {
    let Some(project_id) = batch.project_id else {
        // Ad-hoc Web evidence has immutable history but no project-scoped
        // lifecycle projection until it is attached to a project.
        return Ok(());
    };
    let environment_url = lifecycle_env_url(&batch.environment_scope_key);
    let source = projection_source(batch);
    let observed_at = batch.completed_at;
    let observed: Vec<&NormalizedFinding> =
        batch.findings.iter().filter(|f| actionable(f)).collect();

    for finding in &observed {
        let detail_json = finding.detail_json.as_ref().or(finding.raw_data.as_ref());
        let producer_category = (batch.source == ScanEvidenceSource::WebScan)
            .then_some(finding.producer_category.as_str());
        tx.execute(
            "INSERT INTO work_items
                (project_id, env_url, source, signal_id, check_id, category,
                 severity, title, description, detail_json, scan_ref, page_url,
                 fix_prompt, manual_fix, why_it_matters, first_seen_at,
                 last_seen_at, resolved_at, confidence, domain, relative_path,
                 line, check_status, confidence_reason, producer_check_id,
                 producer_fix_prompt, producer_category, first_seen_scan_ref,
                 resolved_scan_ref)
             VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16, ?16, NULL, ?17, ?18, ?19, ?20, ?21, ?22,
                 ?23, ?24, ?25, ?11, NULL
             )
             ON CONFLICT(project_id, env_url, source, signal_id)
                 WHERE resolved_at IS NULL
             DO UPDATE SET
                 last_seen_at = excluded.last_seen_at,
                 category = excluded.category,
                 severity = excluded.severity,
                 title = excluded.title,
                 description = excluded.description,
                 detail_json = excluded.detail_json,
                 scan_ref = excluded.scan_ref,
                 page_url = excluded.page_url,
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
                project_id,
                environment_url,
                source,
                finding.occurrence_id,
                finding.canonical_check_id,
                finding.category,
                finding.severity.as_str(),
                finding.title,
                finding.description,
                detail_json,
                run_id,
                finding.page_url,
                finding.fix_prompt,
                finding.manual_fix,
                finding.why_it_matters,
                observed_at,
                finding.confidence.as_str(),
                finding.domain,
                finding.relative_path,
                finding.line.map(i64::from),
                finding.verdict.as_str(),
                finding.confidence_reason,
                finding.producer_check_id,
                finding.producer_fix_prompt,
                producer_category,
            ],
        )?;
    }

    reconcile_reobserved_lifecycle(
        tx,
        project_id,
        &environment_url,
        observed_at,
        observed
            .iter()
            .map(|finding| finding.canonical_check_id.as_str()),
    )?;

    resolve_covered_absences(
        tx,
        ResolveScope {
            run_id,
            observed_at,
            project_id,
            source,
            environment_url: &environment_url,
        },
        batch,
        &observed,
    )
}

/// The lifecycle rows one run may speak for.
struct ResolveScope<'a> {
    run_id: i64,
    observed_at: i64,
    project_id: i64,
    source: &'a str,
    environment_url: &'a str,
}

/// Resolve open rows absent from a run only when engine coverage proves their
/// `(route, check)` pairs were observed.
fn resolve_covered_absences(
    tx: &Transaction<'_>,
    scope: ResolveScope<'_>,
    batch: &NormalizedRunBatch,
    observed: &[&NormalizedFinding],
) -> Result<(), DbError> {
    if !batch.coverage.successful {
        return Ok(());
    }
    let coverage = as_stored_keys(&batch.coverage);
    let seen: BTreeSet<&str> = observed
        .iter()
        .map(|finding| finding.occurrence_id.as_str())
        .collect();

    let resolved: Vec<i64> = load_open_candidates(tx, &scope, &coverage)?
        .into_iter()
        .filter(|candidate| {
            let route = candidate.page_url.as_deref().map(normalize_occurrence_url);
            !seen.contains(candidate.signal_id.as_str())
                && coverage.covers(route.as_deref(), &candidate.check_id)
        })
        .map(|candidate| candidate.id)
        .collect();

    for id in resolved {
        tx.execute(
            "UPDATE work_items
             SET resolved_at = ?1, resolved_scan_ref = ?2
             WHERE id = ?3",
            params![scope.observed_at, scope.run_id, id],
        )?;
    }
    Ok(())
}

/// One open lifecycle row a run may speak for.
struct OpenCandidate {
    id: i64,
    signal_id: String,
    check_id: String,
    page_url: Option<String>,
}

/// Load the open rows whose `(route, check)` pairs the run could speak for.
///
/// A route-scoped claim cannot cover a route it never observed, so the claimed
/// routes are bound into the query and one finished page never decodes the
/// whole site's open findings. The bound is deliberately wider than the
/// answer: it keeps every routeless row, and it compares routes without ASCII
/// case because `normalize_occurrence_url` lowercases only the origin. Every
/// row that survives it is still put to `covers`, which alone decides the
/// pair.
fn load_open_candidates(
    tx: &Transaction<'_>,
    scope: &ResolveScope<'_>,
    coverage: &ScanCoverageManifest,
) -> Result<Vec<OpenCandidate>, DbError> {
    let mut sql = String::from(
        "SELECT id, signal_id, check_id, page_url FROM work_items
         WHERE source = ?1 AND project_id = ?2 AND env_url = ?3
           AND resolved_at IS NULL",
    );
    let mut bound: Vec<Box<dyn ToSql>> = vec![
        Box::new(scope.source.to_string()),
        Box::new(scope.project_id),
        Box::new(scope.environment_url.to_string()),
    ];
    if let Some(routes) = coverage.route_bound() {
        let first = bound.len() + 1;
        let placeholders = (0..routes.len())
            .map(|index| format!("?{}", first + index))
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(
            " AND (page_url IS NULL OR lower(page_url) IN ({placeholders}))"
        ));
        bound.extend(
            routes
                .iter()
                .map(|route| Box::new(route.to_ascii_lowercase()) as Box<dyn ToSql>),
        );
    }

    let mut statement = tx.prepare(&sql)?;
    let values: Vec<&dyn ToSql> = bound.iter().map(|value| value.as_ref()).collect();
    let candidates = statement
        .query_map(values.as_slice(), |row| {
            Ok(OpenCandidate {
                id: row.get(0)?,
                signal_id: row.get(1)?,
                check_id: row.get(2)?,
                page_url: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(candidates)
}

/// The claim in DB key form. Both sides of the route comparison go through
/// one normalizer, so a mixed-case host cannot make a covered page look
/// uncovered and leave a fixed issue open forever.
fn as_stored_keys(coverage: &ScanCoverageManifest) -> ScanCoverageManifest {
    let mut stored = coverage.clone();
    stored.page_urls = coverage
        .page_urls
        .iter()
        .map(|url| normalize_occurrence_url(url))
        .collect();
    for exception in &mut stored.exceptions {
        exception.route = exception.route.as_deref().map(normalize_occurrence_url);
    }
    stored
}

#[cfg(test)]
#[path = "scan_run_projection_tests.rs"]
mod tests;
