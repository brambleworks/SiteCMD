use super::DbError;
use std::collections::BTreeMap;

use rusqlite::params;

use crate::checks::Severity;
use crate::core::types_work_items::{IssueGroup, IssueInstance, IssueStatus};

use super::helpers::{normalize_url, parse_required_enum};
use super::work_items::WorkItemRow;
use super::Database;

fn extract_url_from_signal_id(signal_id: &str) -> Option<String> {
    let tail = signal_id.splitn(3, ':').nth(2)?;
    if tail.is_empty() {
        None
    } else {
        Some(tail.to_string())
    }
}

fn effective_issue_status(
    raw_status: IssueStatus,
    snooze_until: Option<i64>,
    now_ms: i64,
) -> IssueStatus {
    if raw_status == IssueStatus::Snoozed && snooze_until.is_some_and(|until| until <= now_ms) {
        IssueStatus::New
    } else {
        raw_status
    }
}

// The MCP impact-score asset is generated from these weights and the canonical
// severity penalties; the parity test keeps both implementations aligned.
pub(crate) const IMPACT_CATEGORY_WEIGHTS: &[(&str, f64)] = &[
    ("security", 0.25),
    ("performance", 0.25),
    ("seo", 0.15),
    ("accessibility", 0.15),
    ("compliance", 0.10),
    ("polish", 0.10),
    ("code_quality", 0.10),
    ("dependencies", 0.15),
    ("infrastructure", 0.10),
];
pub(crate) const IMPACT_DEFAULT_CATEGORY_WEIGHT: f64 = 0.05;

pub(crate) const IMPACT_BASE_MULTIPLIER: f64 = 100.0;
pub(crate) const IMPACT_EXTRA_SOURCE_BONUS: f64 = 0.01;

pub(crate) fn compute_impact_score(severity: Severity, category: &str, source_count: usize) -> f64 {
    impact_score_from_penalty(
        crate::scoring::calculator::group_severity_penalty(severity),
        category,
        source_count,
    )
}

/// Apply the shared impact formula to an untyped severity penalty.
/// MCP parity uses this for unknown severity values read from SQLite.
pub(crate) fn impact_score_from_penalty(
    sev_penalty: f64,
    category: &str,
    source_count: usize,
) -> f64 {
    let cat_weight = IMPACT_CATEGORY_WEIGHTS
        .iter()
        .find(|(label, _)| *label == category)
        .map_or(IMPACT_DEFAULT_CATEGORY_WEIGHT, |(_, weight)| *weight);
    let base = sev_penalty * cat_weight * IMPACT_BASE_MULTIPLIER;
    base + ((source_count as f64 - 1.0).max(0.0)) * IMPACT_EXTRA_SOURCE_BONUS
}

impl Database {
    /// Return enriched groups for issue lists, dossiers, and page drill-downs.
    /// Scoring uses the cheaper `get_active_issue_groups` path.
    #[tracing::instrument(skip(self, env_url), fields(project_id, now_ms))]
    pub fn get_work_items_grouped(
        &self,
        project_id: i64,
        env_url: Option<&str>,
        now_ms: i64,
    ) -> Result<Vec<IssueGroup>, DbError> {
        let mut result = self.get_active_issue_groups(project_id, env_url, now_ms)?;

        use crate::integrations::IntegrationType;

        let connected_raw = self.get_connected_integration_types(project_id)?;
        let dismissed_raw = self.get_dismissed_integration_hints(project_id)?;
        let connected: std::collections::HashSet<IntegrationType> = connected_raw
            .iter()
            .map(|value| {
                value.parse::<IntegrationType>().map_err(|error| {
                    DbError::Other(format!(
                        "invalid integration_type '{value}' in integration_configs: {error}"
                    ))
                })
            })
            .collect::<Result<_, _>>()?;
        let dismissed: std::collections::HashSet<(String, IntegrationType)> = dismissed_raw
            .into_iter()
            .map(|(check_id, integration_type)| {
                integration_type
                    .parse::<IntegrationType>()
                    .map(|parsed| (check_id, parsed))
                    .map_err(|error| {
                        DbError::Other(format!(
                            "invalid integration_type '{integration_type}' in dismissed_integration_hints: {error}"
                        ))
                    })
            })
            .collect::<Result<_, _>>()?;

        let project_path = self.get_project_path_result(project_id)?;
        let env_url_str = env_url.unwrap_or("");
        crate::core::correlation::resolver::enrich_issue_groups(
            &mut result,
            project_id,
            env_url_str,
            self,
            &connected,
            &dismissed,
            project_path.as_deref(),
        )
        .map_err(|error| DbError::Other(format!("issue enrichment failed: {error}")))?;

        result.sort_by(|left, right| {
            right
                .impact_score
                .partial_cmp(&left.impact_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(result)
    }

    /// Active issue groups for a project and environment without correlation or
    /// integration enrichment. This is the minimal input for current scoring;
    /// `get_work_items_grouped` adds display enrichment.
    pub fn get_active_issue_groups(
        &self,
        project_id: i64,
        env_url: Option<&str>,
        now_ms: i64,
    ) -> Result<Vec<IssueGroup>, DbError> {
        let rows = self.get_active_work_items(project_id, env_url)?;
        let status_by_check = self.get_issue_state_map(project_id, env_url)?;

        let mut groups: BTreeMap<String, Vec<WorkItemRow>> = BTreeMap::new();
        for row in rows {
            groups.entry(row.check_id.clone()).or_default().push(row);
        }

        let mut result: Vec<IssueGroup> = Vec::new();
        for (check_id, group_rows) in groups {
            let severity = group_rows
                .iter()
                .map(|row| row.severity)
                .max_by_key(|s| s.impact_rank())
                .unwrap_or(Severity::Low);

            let mut sources: Vec<String> =
                group_rows.iter().map(|row| row.source.clone()).collect();
            sources.sort();
            sources.dedup();

            let first = group_rows.first().cloned().expect("non-empty group");
            let (raw_status, snooze_until, block_reason, _verified_by) = status_by_check
                .get(&check_id)
                .cloned()
                .unwrap_or((IssueStatus::New, None, None, None));

            let effective_status = effective_issue_status(raw_status, snooze_until, now_ms);

            let instances: Vec<IssueInstance> = group_rows
                .iter()
                .map(|row| IssueInstance {
                    id: row.id,
                    source: row.source.clone(),
                    signal_id: row.signal_id.clone(),
                    producer_check_id: row.metadata.producer_check_id.clone(),
                    url: extract_url_from_signal_id(&row.signal_id),
                    page_url: row.page_url.clone(),
                    severity: row.severity,
                    title: row.title.clone(),
                    description: row.description.clone(),
                    category: Some(row.category.clone()),
                    check_status: row.metadata.check_status,
                    fix_prompt: row.fix_prompt.clone(),
                    manual_fix: row.manual_fix.clone(),
                    why_it_matters: row.why_it_matters.clone(),
                    detail_json: row.detail_json.clone(),
                    first_seen_at: row.first_seen_at,
                    last_seen_at: row.last_seen_at,
                    confidence: row.metadata.confidence,
                    confidence_reason: row.metadata.confidence_reason.clone(),
                    domain: row.metadata.domain,
                    relative_path: row.metadata.relative_path.clone(),
                    line: row.metadata.line,
                    producer_fix_prompt: row.metadata.producer_fix_prompt.clone(),
                    producer_category: row.metadata.producer_category,
                })
                .collect();

            let impact_score = compute_impact_score(severity, &first.category, sources.len());

            result.push(IssueGroup {
                check_id,
                category: first.category.clone(),
                severity,
                title: first.title.clone(),
                description: first.description.clone(),
                instances,
                sources,
                status: effective_status,
                snooze_until,
                block_reason,
                impact_score,
                likely_causes: Vec::new(),
                suggested_integrations: Vec::new(),
                fix_locations: Vec::new(),
                transitive_causes: Vec::new(),
                downstream_effects: Vec::new(),
                recent_events: Vec::new(),
                enrichments: Vec::new(),
                correlation_evidence: Vec::new(),
                affected_pages: Vec::new(),
                cross_env_signal: None,
                cross_project_pattern: None,
                display_confidence: None,
                observation_count: 0,
                anomaly_score: None,
            });
        }

        Ok(result)
    }

    /// Return lifecycle-inactive check IDs for one project environment using
    /// the same effective-status rules as issue grouping and scoring.
    #[tracing::instrument(skip(self, env_url), fields(project_id))]
    pub fn get_inactive_check_ids(
        &self,
        project_id: i64,
        env_url: Option<&str>,
        now_ms: i64,
    ) -> Result<Vec<String>, DbError> {
        let active_check_ids: std::collections::BTreeSet<String> = self
            .get_active_work_items(project_id, env_url)?
            .into_iter()
            .map(|row| row.check_id)
            .collect();
        let status_by_check = self.get_issue_state_map(project_id, env_url)?;

        Ok(active_check_ids
            .into_iter()
            .filter(|check_id| {
                let (raw_status, snooze_until, ..) = status_by_check
                    .get(check_id)
                    .cloned()
                    .unwrap_or((IssueStatus::New, None, None, None));
                effective_issue_status(raw_status, snooze_until, now_ms).is_inactive_for_scoring()
            })
            .collect())
    }

    #[tracing::instrument(skip(self, env_url), fields(project_id))]
    pub fn get_pages_with_issues(
        &self,
        project_id: i64,
        env_url: &str,
        now_ms: i64,
    ) -> Result<Vec<crate::core::types_work_items::PageSummary>, DbError> {
        use crate::core::types_work_items::PageSummary;

        let env_key = normalize_url(env_url).0;
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT
                        work_items.page_url,
                        work_items.check_id,
                        work_items.severity,
                        work_items.source
                     FROM work_items
                     LEFT JOIN project_issue_states
                       ON project_issue_states.project_id = work_items.project_id
                      AND project_issue_states.env_url = work_items.env_url
                      AND project_issue_states.check_id = work_items.check_id
                     WHERE work_items.project_id = ?1
                       AND work_items.env_url = ?2
                       AND work_items.resolved_at IS NULL
                       AND work_items.page_url IS NOT NULL
                       AND (
                         project_issue_states.status IS NULL
                         OR project_issue_states.status IN ('new', 'regressed')
                         OR (
                           project_issue_states.status = 'snoozed'
                           AND project_issue_states.snooze_until IS NOT NULL
                           AND project_issue_states.snooze_until <= ?3
                         )
                       )",
            )?;

            let rows = stmt.query_map(params![project_id, env_key, now_ms], |row| {
                let page_url: String = row.get(0)?;
                let check_id: String = row.get(1)?;
                let severity: Severity =
                    parse_required_enum(2, "work_items.severity", &row.get::<_, String>(2)?)?;
                let source: String = row.get(3)?;
                Ok((page_url, check_id, severity, source))
            })?;

            let mut by_bucket: std::collections::BTreeMap<
                String,
                (
                    std::collections::BTreeSet<String>,
                    Severity,
                    std::collections::BTreeSet<String>,
                ),
            > = std::collections::BTreeMap::new();
            for row in rows {
                let (bucket, check_id, severity, source) = row?;
                let entry = by_bucket
                    .entry(bucket)
                    .or_insert_with(|| (Default::default(), Severity::Low, Default::default()));
                entry.0.insert(check_id);
                if severity.impact_rank() > entry.1.impact_rank() {
                    entry.1 = severity;
                }
                entry.2.insert(source);
            }

            let mut result: Vec<PageSummary> = by_bucket
                .into_iter()
                .map(|(bucket, (check_ids, max_severity, sources))| PageSummary {
                    page_url: bucket.clone(),
                    label: bucket,
                    issue_count: check_ids.len() as i64,
                    max_severity,
                    sources: sources.into_iter().collect(),
                })
                .collect();

            result.sort_by(|left, right| {
                right
                    .max_severity
                    .impact_rank()
                    .cmp(&left.max_severity.impact_rank())
                    .then_with(|| right.issue_count.cmp(&left.issue_count))
                    .then_with(|| left.page_url.cmp(&right.page_url))
            });

            Ok(result)
        })?
    }

    #[tracing::instrument(skip(self, env_url, page_url), fields(project_id, now_ms))]
    pub fn get_work_items_grouped_for_page(
        &self,
        project_id: i64,
        env_url: &str,
        page_url: &str,
        now_ms: i64,
    ) -> Result<Vec<IssueGroup>, DbError> {
        let all = self.get_work_items_grouped(project_id, Some(env_url), now_ms)?;
        let wants_project_wide = page_url == "__project_wide__";

        Ok(all
            .into_iter()
            .filter_map(|mut group| {
                if group.status.is_inactive_for_scoring() {
                    return None;
                }
                let matching_instances: Vec<_> = group
                    .instances
                    .into_iter()
                    .filter(|instance| {
                        if wants_project_wide {
                            instance.page_url.is_none()
                        } else {
                            instance.page_url.as_deref() == Some(page_url)
                        }
                    })
                    .collect();

                if matching_instances.is_empty() {
                    None
                } else {
                    let first = matching_instances.first()?.clone();
                    group.severity = matching_instances
                        .iter()
                        .map(|instance| instance.severity)
                        .max_by_key(|severity| severity.impact_rank())
                        .unwrap_or(Severity::Low);
                    group.title = first.title;
                    group.description = first.description;
                    group.category = first.category.unwrap_or(group.category);
                    group.instances = matching_instances;
                    let mut sources: Vec<String> = group
                        .instances
                        .iter()
                        .map(|instance| instance.source.clone())
                        .collect();
                    sources.sort();
                    sources.dedup();
                    group.sources = sources;
                    group.impact_score =
                        compute_impact_score(group.severity, &group.category, group.sources.len());
                    Some(group)
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests;
