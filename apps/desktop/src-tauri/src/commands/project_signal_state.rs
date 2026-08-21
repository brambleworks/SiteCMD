use std::collections::HashMap;
use tauri::AppHandle;

use crate::core::code_scan::CodeIssueView;
use crate::core::scanner::ScanResult;
use crate::db::{
    CodeScanResult, CodeScanSummary, Database, ProjectAttentionTargets, ProjectMonitoringSignals,
    ProjectSignalSnapshotRecord, ProjectWorkItem, ProjectWorkQueue, ProjectWorkSummary,
};

use super::project_maintenance_items::build_project_maintenance_items;
use super::project_signal_monitoring::{
    build_project_monitoring_signals, enabled_integration_names, infer_security_target,
};
use super::project_work_items::{
    build_canonical_issue_work_entries, build_issue_work_entries, build_update_work_entries,
};
use crate::core::project_snapshot::{build_project_work_queue, build_project_work_summary};

#[cfg(test)]
pub(crate) use super::project_signal_monitoring::take_monitored_integrations;

const PROJECT_SIGNAL_SNAPSHOT_TTL_MINUTES: i64 = 5;

/// Display group key for a Code issue. Identity is the canonical check id;
/// titles, severities, and locations are mutable evidence, not row keys.
fn code_group_key(view: &CodeIssueView) -> String {
    view.check_id.clone()
}

/// Display-deduplicated `(total, critical, high)` counts over active views.
pub(crate) fn grouped_active_code_counts(views: &[CodeIssueView]) -> (u32, u32, u32) {
    grouped_code_counts(
        views
            .iter()
            .map(|view| (view.severity, code_group_key(view))),
    )
}

/// Same counts from column-derived keys (no detail_json parse); the hot
/// badge/summary paths use these instead of full issue views.
pub(crate) fn grouped_code_counts_from_keys(
    keys: &[crate::core::code_scan::CodeIssueCountKey],
) -> (u32, u32, u32) {
    grouped_code_counts(keys.iter().map(|key| (key.severity, key.check_id.clone())))
}

fn grouped_code_counts(
    items: impl Iterator<Item = (crate::checks::Severity, String)>,
) -> (u32, u32, u32) {
    let mut grouped: HashMap<String, crate::checks::Severity> = HashMap::new();
    for (severity, key) in items {
        match grouped.get_mut(&key) {
            Some(current)
                if crate::core::code_scan::severity_rank(&severity)
                    < crate::core::code_scan::severity_rank(current) =>
            {
                *current = severity;
            }
            Some(_) => {}
            None => {
                grouped.insert(key, severity);
            }
        }
    }
    let critical = grouped
        .values()
        .filter(|severity| **severity == crate::checks::Severity::Critical)
        .count() as u32;
    let high = grouped
        .values()
        .filter(|severity| **severity == crate::checks::Severity::High)
        .count() as u32;
    (grouped.len() as u32, critical, high)
}

pub(crate) fn normalize_snapshot_url(url: Option<&str>) -> Option<String> {
    url.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
}

pub(crate) fn snapshot_is_fresh(timestamp: Option<&str>) -> bool {
    let Some(timestamp) = timestamp else {
        return false;
    };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
        return false;
    };
    let age = chrono::Utc::now().signed_duration_since(parsed.with_timezone(&chrono::Utc));
    age < chrono::Duration::minutes(PROJECT_SIGNAL_SNAPSHOT_TTL_MINUTES)
}

pub(crate) fn load_cached_json<T: serde::de::DeserializeOwned>(
    value: Option<&String>,
) -> Option<T> {
    value.and_then(|raw| serde_json::from_str(raw).ok())
}

fn parse_snapshot_timestamp(timestamp: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(timestamp?)
        .ok()
        .map(|parsed| parsed.with_timezone(&chrono::Utc))
}

fn fallback_snapshot_is_newer(
    fallback_timestamp: Option<&str>,
    exact_timestamp: Option<&str>,
) -> bool {
    match (
        parse_snapshot_timestamp(fallback_timestamp),
        parse_snapshot_timestamp(exact_timestamp),
    ) {
        (Some(fallback), Some(exact)) => fallback > exact,
        (Some(_), None) => true,
        _ => false,
    }
}

pub(crate) fn load_cached_project_updates_snapshot(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
    cached: Option<&ProjectSignalSnapshotRecord>,
) -> (Option<crate::updates::types::UpdateReport>, Option<String>) {
    let exact_cached_updates: Option<crate::updates::types::UpdateReport> =
        load_cached_json(cached.and_then(|entry| entry.updates_json.as_ref()));
    let exact_cached_refreshed_at = cached.and_then(|entry| entry.updates_refreshed_at.clone());

    let fallback_cached = if environment_url.is_some() {
        db.get_project_signal_snapshot_record(project_id, None)
            .ok()
            .flatten()
    } else {
        None
    };
    let fallback_cached_updates: Option<crate::updates::types::UpdateReport> = load_cached_json(
        fallback_cached
            .as_ref()
            .and_then(|entry| entry.updates_json.as_ref()),
    );
    let fallback_refreshed_at = fallback_cached
        .as_ref()
        .and_then(|entry| entry.updates_refreshed_at.clone());

    if fallback_cached_updates.is_some()
        && (exact_cached_updates.is_none()
            || fallback_snapshot_is_newer(
                fallback_refreshed_at.as_deref(),
                exact_cached_refreshed_at.as_deref(),
            ))
    {
        return (fallback_cached_updates, fallback_refreshed_at);
    }

    (exact_cached_updates, exact_cached_refreshed_at)
}

/// Latest + previous code scan summaries plus the latest detail if requested.
pub(crate) type RelevantCodeScan = (
    Option<CodeScanSummary>,
    Option<CodeScanSummary>,
    Option<CodeScanResult>,
);

pub(crate) fn load_relevant_code_scan(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
    include_detail: bool,
) -> Result<RelevantCodeScan, String> {
    // Hydrate domain data only for the two selected summaries.
    let history = db.get_code_scan_summaries_lean(project_id, 50)?;
    let mut history = select_relevant_code_scan_history(history, environment_url);
    history.truncate(2);
    let history = db.hydrate_code_scan_domain_data(history)?;

    let Some(summary) = history.first().cloned() else {
        return Ok((None, None, None));
    };
    let previous_summary = history.get(1).cloned();
    let detail = if include_detail {
        db.get_code_scan_detail(summary.id)?
    } else {
        None
    };
    Ok((Some(summary), previous_summary, detail))
}

pub(crate) fn load_relevant_code_scan_summary(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
) -> Result<(Option<CodeScanSummary>, Option<CodeScanSummary>), String> {
    let (mut summary, previous_summary, _) =
        load_relevant_code_scan(db, project_id, environment_url, false)?;

    if let Some(entry) = summary.as_mut() {
        if entry.issue_count > 0 && entry.grouped_issue_count == 0 {
            let keys = db.get_code_scan_issue_count_keys(entry.id)?;
            if !keys.is_empty() {
                // Derive grouped totals and severities from the same keys so the
                // severity breakdown cannot exceed the displayed issue count.
                let (grouped, critical, high) = grouped_code_counts_from_keys(&keys);
                entry.grouped_issue_count = grouped;
                entry.critical_count = critical;
                entry.high_count = high;
            }
        }
    }

    Ok((summary, previous_summary))
}

pub(crate) fn select_relevant_code_scan_history(
    history: Vec<CodeScanSummary>,
    environment_url: Option<&str>,
) -> Vec<CodeScanSummary> {
    let normalized_target = normalize_snapshot_url(environment_url);
    let Some(target) = normalized_target else {
        return history;
    };

    let mut exact = Vec::new();
    let mut project_wide = Vec::new();
    let mut other = Vec::new();

    for entry in history {
        match entry
            .environment_url
            .as_deref()
            .map(|value| value.trim_end_matches('/'))
        {
            Some(url) if url == target.as_str() => exact.push(entry),
            None => project_wide.push(entry),
            _ => other.push(entry),
        }
    }

    if !exact.is_empty() {
        exact
    } else if !project_wide.is_empty() {
        project_wide
    } else {
        other
    }
}

pub(crate) async fn load_project_monitoring_snapshot(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
    cached: Option<&ProjectSignalSnapshotRecord>,
    force_refresh: bool,
    allow_refresh: bool,
) -> (ProjectMonitoringSignals, Option<String>) {
    let cached_monitoring: Option<ProjectMonitoringSignals> =
        load_cached_json(cached.and_then(|entry| entry.monitoring_json.as_ref()));
    let cached_refreshed_at = cached.and_then(|entry| entry.monitoring_refreshed_at.clone());

    if !force_refresh && (!allow_refresh || snapshot_is_fresh(cached_refreshed_at.as_deref())) {
        if let Some(cached_monitoring) = cached_monitoring {
            return (cached_monitoring, cached_refreshed_at);
        }
    }

    if !allow_refresh {
        return (
            monitoring_without_refresh(db, project_id, cached_monitoring),
            cached_refreshed_at,
        );
    }

    match build_project_monitoring_signals(app, db, project_id, environment_url).await {
        Ok(monitoring) => {
            let refreshed_at = chrono::Utc::now().to_rfc3339();
            if let Err(error) = db.save_project_monitoring_snapshot(
                project_id,
                environment_url,
                &monitoring,
                &refreshed_at,
            ) {
                tracing::warn!("Failed to persist project monitoring snapshot: {}", error);
            }
            (monitoring, Some(refreshed_at))
        }
        Err(error) => {
            tracing::warn!("Failed to refresh project monitoring snapshot: {}", error);
            (
                monitoring_without_refresh(db, project_id, cached_monitoring),
                cached_refreshed_at,
            )
        }
    }
}

/// Use cached monitoring signals or derive configured integrations when a live
/// refresh is skipped or unavailable.
fn monitoring_without_refresh(
    db: &Database,
    project_id: i64,
    cached_monitoring: Option<ProjectMonitoringSignals>,
) -> ProjectMonitoringSignals {
    cached_monitoring.unwrap_or_else(|| ProjectMonitoringSignals {
        enabled_integrations: enabled_integration_names(db, project_id),
        ..ProjectMonitoringSignals::default()
    })
}

pub(crate) async fn load_project_updates_snapshot(
    db: &Database,
    project_id: i64,
    project_path: Option<&str>,
    environment_url: Option<&str>,
    cached: Option<&ProjectSignalSnapshotRecord>,
    force_refresh: bool,
    allow_refresh: bool,
) -> (Option<crate::updates::types::UpdateReport>, Option<String>) {
    let Some(project_path) = project_path.filter(|value| !value.is_empty()) else {
        return (None, None);
    };
    let (cached_updates, cached_refreshed_at) =
        load_cached_project_updates_snapshot(db, project_id, environment_url, cached);

    if !force_refresh && (!allow_refresh || snapshot_is_fresh(cached_refreshed_at.as_deref())) {
        if let Some(cached_updates) = cached_updates {
            return (Some(cached_updates), cached_refreshed_at);
        }
    }

    if !allow_refresh {
        return (cached_updates, cached_refreshed_at);
    }

    let project_path = std::path::Path::new(project_path);
    match super::updates::detect_updates_for_path(project_path).await {
        Ok(report) => {
            let refreshed_at = chrono::Utc::now().to_rfc3339();
            if let Err(error) = db.save_project_updates_snapshot(
                project_id,
                environment_url,
                &report,
                &refreshed_at,
            ) {
                tracing::warn!("Failed to persist project updates snapshot: {}", error);
            }
            (Some(report), Some(refreshed_at))
        }
        Err(error) => {
            tracing::warn!("Failed to refresh project updates snapshot: {}", error);
            (cached_updates, cached_refreshed_at)
        }
    }
}

pub(crate) fn load_latest_site_scan_detail(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
) -> Result<Option<ScanResult>, String> {
    let Some(url) = environment_url else {
        return Ok(None);
    };
    let history = db.get_scan_history_for_project(project_id, url, 1)?;
    let Some(scan_id) = history.first().map(|scan| scan.id) else {
        return Ok(None);
    };
    db.get_scan_detail(scan_id)?
        .map(Some)
        .ok_or_else(|| format!("scan history references missing scan {scan_id}"))
}

pub(crate) fn build_canonical_issue_work_summary(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
) -> Result<ProjectWorkSummary, String> {
    let issue_items = build_canonical_issue_work_entries(db, project_id, environment_url)?;
    Ok(build_project_work_summary(
        &issue_items,
        &ProjectWorkQueue::default(),
    ))
}

fn replace_issue_counts(
    summary: &mut ProjectWorkSummary,
    canonical_issue_summary: &ProjectWorkSummary,
) {
    summary.issue_count = canonical_issue_summary.issue_count;
    summary.issue_web_count = canonical_issue_summary.issue_web_count;
    summary.issue_code_count = canonical_issue_summary.issue_code_count;
    summary.issue_critical_count = canonical_issue_summary.issue_critical_count;
    summary.issue_high_count = canonical_issue_summary.issue_high_count;
    summary.issue_medium_count = canonical_issue_summary.issue_medium_count;
    summary.issue_low_count = canonical_issue_summary.issue_low_count;
}

pub(crate) fn refresh_project_work_state(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
    latest_site_scan: Option<&ScanResult>,
    code_scan_summary: Option<&CodeScanSummary>,
    updates: Option<&crate::updates::types::UpdateReport>,
    updates_refreshed_at: Option<&str>,
    monitoring: &ProjectMonitoringSignals,
) -> Result<(Vec<ProjectWorkItem>, ProjectWorkQueue, ProjectWorkSummary), String> {
    // Derived per load: lifecycle from project_issue_states (via
    // get_active_issue_groups), verify-in-flight from fix_attempts, security
    // updates from the snapshot refreshed just above. Nothing is persisted.
    let mut items = build_issue_work_entries(db, project_id, environment_url)?;
    items.extend(build_update_work_entries(
        db,
        project_id,
        environment_url,
        updates,
    )?);
    let maintenance = build_project_maintenance_items(
        db,
        project_id,
        environment_url,
        latest_site_scan,
        code_scan_summary,
        updates_refreshed_at,
        monitoring,
    )?;
    let queue = build_project_work_queue(&items, maintenance);
    let mut summary = build_project_work_summary(&items, &queue);
    let canonical_issue_summary =
        build_canonical_issue_work_summary(db, project_id, environment_url)?;
    replace_issue_counts(&mut summary, &canonical_issue_summary);
    Ok((items, queue, summary))
}

pub(crate) fn build_project_attention_targets(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
) -> ProjectAttentionTargets {
    let Some(environment_url) = environment_url else {
        return ProjectAttentionTargets::default();
    };

    let (security_focus, security_issue_id) =
        infer_security_target(db, project_id, environment_url).unwrap_or((None, None));

    ProjectAttentionTargets {
        security_issue_id,
        security_focus,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::temp_db;
    use crate::integrations::{IntegrationConfig, IntegrationType};

    #[test]
    fn monitoring_without_refresh_reports_configured_integrations_when_cache_is_cold() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Uptime Project", "/tmp/uptime-project", None)
            .expect("project");
        db.save_integration(
            project_id,
            &IntegrationConfig {
                integration_type: IntegrationType::UptimeRobot,
                api_key: Some("ur-key".to_string()),
                site_id: None,
                extra: None,
                enabled: true,
            },
        )
        .expect("save integration");

        let monitoring = monitoring_without_refresh(&db, project_id, None);

        assert!(
            monitoring
                .enabled_integrations
                .iter()
                .any(|integration| integration == "uptimerobot"),
            "cold-cache fallback must still list configured integrations, got {:?}",
            monitoring.enabled_integrations,
        );
    }

    #[test]
    fn monitoring_without_refresh_excludes_disabled_integrations() {
        let db = temp_db();
        let project_id = db
            .upsert_project("Disabled Project", "/tmp/disabled-project", None)
            .expect("project");
        db.save_integration(
            project_id,
            &IntegrationConfig {
                integration_type: IntegrationType::UptimeRobot,
                api_key: Some("ur-key".to_string()),
                site_id: None,
                extra: None,
                enabled: false,
            },
        )
        .expect("save integration");

        let monitoring = monitoring_without_refresh(&db, project_id, None);

        assert!(
            monitoring.enabled_integrations.is_empty(),
            "disabled integrations must not count as connected, got {:?}",
            monitoring.enabled_integrations,
        );
    }

    #[test]
    fn monitoring_without_refresh_prefers_cached_snapshot() {
        // When a cached snapshot exists we return it verbatim - the live refresh
        // is intentionally skipped, so we never clobber previously-fetched data.
        let db = temp_db();
        let project_id = db
            .upsert_project("Cached Project", "/tmp/cached-project", None)
            .expect("project");
        let cached = ProjectMonitoringSignals {
            enabled_integrations: vec!["plausible".to_string()],
            integration_failure_count: 3,
            ..ProjectMonitoringSignals::default()
        };

        let monitoring = monitoring_without_refresh(&db, project_id, Some(cached.clone()));

        assert_eq!(monitoring.enabled_integrations, cached.enabled_integrations);
        assert_eq!(monitoring.integration_failure_count, 3);
    }
}
