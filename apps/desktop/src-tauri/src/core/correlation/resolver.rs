//! Bounded issue-enrichment resolver with per-call evidence caches.

use std::collections::HashSet;

use crate::core::correlation::observations::{dynamic_confidence, ObservationIndex};
use crate::core::correlation::{
    anomaly::AnomalyScore,
    causal_graph::{
        resolve_downstream_effects, resolve_likely_causes, resolve_transitive_causes, Confidence,
    },
    fix_locations::resolve_fix_locations,
    integration_hints::resolve_integration_suggestions,
};
use crate::core::types_work_items::{IssueGroup, RecentEventRef};
use crate::db::types::EventType;
use crate::db::Database;
use crate::integrations::IntegrationType;

const RECENT_EVENT_WINDOW_DAYS: i64 = 30;
const MAX_TRANSITIVE_DEPTH: u8 = 4;
const RECENT_EVENT_LIMIT_PER_GROUP: usize = 3;

/// Emergency kill-switch for v3 enrichments. Default on. Set `CORRELATION_V3=0`
/// (or `false`/`FALSE`) to skip all v3-specific enrichments while keeping v2
/// (likely_causes, suggested_integrations, fix_locations) running normally.
pub fn is_v3_enabled() -> bool {
    !matches!(
        std::env::var("CORRELATION_V3").ok().as_deref(),
        Some("0") | Some("false") | Some("FALSE")
    )
}

pub fn enrich_issue_groups(
    groups: &mut [IssueGroup],
    project_id: i64,
    env_url: &str,
    db: &Database,
    connected_integrations: &HashSet<IntegrationType>,
    dismissed_integration_hints: &HashSet<(String, IntegrationType)>,
    project_path: Option<&str>,
) -> Result<(), String> {
    if groups.is_empty() {
        return Ok(());
    }

    let active_check_ids: HashSet<String> = groups.iter().map(|g| g.check_id.clone()).collect();

    for group in groups.iter_mut() {
        // v2 enrichers - always run regardless of CORRELATION_V3 flag
        group.likely_causes = resolve_likely_causes(&group.check_id, &active_check_ids);
        group.fix_locations = resolve_fix_locations(&group.check_id, project_path);
        group.suggested_integrations = resolve_integration_suggestions(
            &group.check_id,
            connected_integrations,
            dismissed_integration_hints,
        );
    }

    if !is_v3_enabled() {
        return Ok(());
    }

    // Load fix-feedback observations once per call.
    let observations = ObservationIndex::load(db, project_id)
        .map_err(|error| format!("could not load causal-link observations: {error}"))?;

    // Batched recent-events lookup (one DB query for all active check_ids).
    let now_ms = chrono::Utc::now().timestamp_millis();
    let since_ms = now_ms - RECENT_EVENT_WINDOW_DAYS * 24 * 60 * 60 * 1000;
    let active_ids_vec: Vec<String> = active_check_ids.iter().cloned().collect();
    let events_by_check_id = db
        .get_events_for_check_ids(project_id, &active_ids_vec, since_ms)
        .map_err(|error| format!("could not load issue-linked events: {error}"))?;

    // Resolve the current environment with one indexed read.
    let is_current_env_prod = db
        .environment_is_production(project_id, env_url)
        .map_err(|error| format!("could not determine the issue environment: {error}"))?;
    let cross_env_signals = super::cross_env::resolve_for_groups(
        db,
        project_id,
        env_url,
        is_current_env_prod,
        &active_check_ids,
    )
    .map_err(|error| format!("could not load cross-environment issue evidence: {error}"))?;
    let cross_project_patterns =
        super::cross_project::resolve_patterns(db, project_id, &active_check_ids)
            .map_err(|error| format!("could not load cross-project issue evidence: {error}"))?;
    let enrichment_cache = if connected_integrations.is_empty() {
        super::enrichments::EnrichmentCache::default()
    } else {
        super::enrichments::EnrichmentCache::load(db, project_id)
            .map_err(|error| format!("could not load integration issue evidence: {error}"))?
    };

    for group in groups.iter_mut() {
        group.transitive_causes =
            resolve_transitive_causes(&group.check_id, &active_check_ids, MAX_TRANSITIVE_DEPTH);
        group.downstream_effects = resolve_downstream_effects(&group.check_id, &active_check_ids);

        if let Some(events) = events_by_check_id.get(&group.check_id) {
            group.recent_events = events
                .iter()
                .take(RECENT_EVENT_LIMIT_PER_GROUP)
                .map(|e| RecentEventRef {
                    event_id: e.id,
                    event_type: format!("{:?}", e.event_type),
                    occurred_at_ms: e.occurred_at_ms,
                    title: e.title.clone(),
                    correlation_confidence: crate::core::correlation::Confidence::Medium,
                })
                .collect();
        }

        // Use the most recent anomaly event for this check.
        if let Some(events) = events_by_check_id.get(&group.check_id) {
            if let Some(anomaly_event) = events
                .iter()
                .find(|e| matches!(e.event_type, EventType::Anomaly))
            {
                let metadata = anomaly_event.metadata.as_deref().ok_or_else(|| {
                    format!(
                        "anomaly event {} for {} has no metadata",
                        anomaly_event.id, group.check_id
                    )
                })?;
                let score = serde_json::from_str::<AnomalyScore>(metadata).map_err(|error| {
                    format!(
                        "anomaly event {} for {} has invalid metadata: {error}",
                        anomaly_event.id, group.check_id
                    )
                })?;
                group.anomaly_score = Some(score.z);
            }
        }

        let mut max_confidence_after_calibration: Option<Confidence> = None;
        let mut total_resolved_for_group: u32 = 0;

        for cause in &mut group.likely_causes {
            let (resolved, active) = observations.for_link(&cause.check_id, &group.check_id);
            cause.confidence = dynamic_confidence(cause.confidence, resolved, active);
            total_resolved_for_group += resolved;
            let cur = cause.confidence.as_f32();
            if max_confidence_after_calibration
                .map(|c| cur > c.as_f32())
                .unwrap_or(true)
            {
                max_confidence_after_calibration = Some(cause.confidence);
            }
        }

        // Recalibrate depth-1 transitive causes (closest parent hop to this check_id).
        for trans in &mut group.transitive_causes {
            if trans.depth == 1 {
                let (resolved, active) = observations.for_link(&trans.check_id, &group.check_id);
                trans.confidence = dynamic_confidence(trans.confidence, resolved, active);
                total_resolved_for_group = total_resolved_for_group.max(resolved);
            }
        }

        group.observation_count = total_resolved_for_group as i64;
        group.display_confidence = max_confidence_after_calibration;

        group.affected_pages = super::cross_page::resolve_affected_pages(group);

        group.cross_env_signal = cross_env_signals.get(&group.check_id).cloned();
        group.cross_project_pattern = cross_project_patterns.get(&group.check_id).cloned();

        // Add fresh cached evidence from connected integrations.
        let mut group_enrichments = Vec::new();
        for hint in super::integration_hints::INTEGRATION_HINTS {
            if hint.check_id != group.check_id {
                continue;
            }
            if !connected_integrations.contains(&hint.integration) {
                continue;
            }
            let Some(enricher) = hint.enricher else {
                continue;
            };
            if let Some(e) = enricher(&group.check_id, &enrichment_cache).map_err(|error| {
                format!(
                    "could not decode {:?} enrichment for {}: {error}",
                    hint.integration, group.check_id
                )
            })? {
                group_enrichments.push(e);
            }
        }
        group.enrichments = group_enrichments;
    }

    Ok(())
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod resolver_tests;
