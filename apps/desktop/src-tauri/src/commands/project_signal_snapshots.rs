use tauri::AppHandle;

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory};
use crate::core::{git, scanner::ScanResult};
use crate::db::{
    Database, ProjectAttentionTargets, ProjectMonitoringSignals, ProjectSignalSnapshot,
};

use super::project_dashboard::{CodeScanTrendPoint, DashboardAggregatedCheckCounts};
use super::project_signal_monitoring::enabled_integration_names;
use super::project_signal_state::{
    build_canonical_issue_work_summary, build_project_attention_targets,
    grouped_active_code_counts, grouped_code_counts_from_keys, load_cached_json,
    load_cached_project_updates_snapshot, load_latest_site_scan_detail,
    load_project_monitoring_snapshot, load_project_updates_snapshot, load_relevant_code_scan,
    load_relevant_code_scan_summary, normalize_snapshot_url, refresh_project_work_state,
    select_relevant_code_scan_history,
};

pub(crate) struct DashboardScanState {
    pub trend: Vec<crate::db::ScoreTrendPoint>,
    pub latest_scan_id: Option<i64>,
    pub latest_detail: Option<ScanResult>,
    pub previous_detail: Option<ScanResult>,
    pub aggregated_check_counts: DashboardAggregatedCheckCounts,
    pub aggregated_failed_issues: Vec<CheckResult>,
    pub latest_scan_timestamp: Option<String>,
}

#[tracing::instrument(
    skip(app, db, url),
    fields(project_id, force_refresh, allow_refresh, requested_code_scan_detail)
)]
pub(crate) async fn get_project_signal_snapshot_internal(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    url: Option<&str>,
    force_refresh: bool,
    allow_refresh: bool,
    requested_code_scan_detail: bool,
) -> Result<ProjectSignalSnapshot, String> {
    let normalized_url = normalize_snapshot_url(url);
    let cached = db.get_project_signal_snapshot_record(project_id, normalized_url.as_deref())?;
    let project_path = db.get_project_path(project_id);
    let include_code_scan_detail = requested_code_scan_detail;
    let first_scan_banner_dismissed = db.is_first_scan_banner_dismissed(project_id)?;
    let (mut code_scan_summary, previous_code_scan_summary, internal_code_scan_detail) =
        load_relevant_code_scan(
            db,
            project_id,
            normalized_url.as_deref(),
            include_code_scan_detail,
        )?;
    let lightweight_code_scan_issue_views = if internal_code_scan_detail.is_none() {
        if let Some(summary) = code_scan_summary.as_ref() {
            Some(db.get_code_scan_issue_views(summary.id)?)
        } else {
            None
        }
    } else {
        None
    };
    let code_scan_issue_views = internal_code_scan_detail
        .as_ref()
        .map(|detail| detail.issues.as_slice())
        .or(lightweight_code_scan_issue_views.as_deref());
    if let (Some(summary), Some(views)) = (code_scan_summary.as_mut(), code_scan_issue_views) {
        // Recompute total and severity counts from the same grouped view so the
        // breakdown cannot exceed its total.
        let (grouped, critical, high) = grouped_active_code_counts(views);
        summary.grouped_issue_count = grouped;
        summary.critical_count = critical;
        summary.high_count = high;
    }
    let (monitoring, monitoring_refreshed_at) = load_project_monitoring_snapshot(
        app,
        db,
        project_id,
        normalized_url.as_deref(),
        cached.as_ref(),
        force_refresh,
        allow_refresh,
    )
    .await;
    let (updates, updates_refreshed_at) = load_project_updates_snapshot(
        db,
        project_id,
        project_path.as_deref(),
        normalized_url.as_deref(),
        cached.as_ref(),
        force_refresh,
        allow_refresh,
    )
    .await;
    let targets = build_project_attention_targets(db, project_id, normalized_url.as_deref());
    let latest_site_scan = load_latest_site_scan_detail(db, project_id, normalized_url.as_deref())?;
    let (_, _, work_summary) = refresh_project_work_state(
        db,
        project_id,
        normalized_url.as_deref(),
        latest_site_scan.as_ref(),
        code_scan_summary.as_ref(),
        updates.as_ref(),
        updates_refreshed_at.as_deref(),
        &monitoring,
    )?;

    Ok(ProjectSignalSnapshot {
        project_id,
        environment_url: normalized_url,
        first_scan_banner_dismissed,
        code_scan_summary,
        previous_code_scan_summary,
        code_scan_detail: if include_code_scan_detail {
            internal_code_scan_detail
        } else {
            None
        },
        monitoring,
        monitoring_refreshed_at,
        updates,
        updates_refreshed_at,
        targets,
        work_summary,
    })
}

fn load_active_web_scan_issues(
    db: &Database,
    project_id: i64,
    url: &str,
) -> Result<Vec<CheckResult>, String> {
    let groups =
        db.get_active_issue_groups(project_id, Some(url), chrono::Utc::now().timestamp_millis())?;
    let mut by_check_id: std::collections::HashMap<String, CheckResult> =
        std::collections::HashMap::new();

    for group in groups {
        if group.status.is_inactive_for_scoring() {
            continue;
        }
        for instance in group
            .instances
            .into_iter()
            .filter(|instance| matches!(instance.source.as_str(), "web_scan" | "site_scan"))
        {
            let category = match instance.producer_category {
                Some(category) => category,
                None => group.category.parse::<ScanCategory>().map_err(|error| {
                    format!(
                        "active Web issue {} has an invalid category: {error}",
                        group.check_id
                    )
                })?,
            };
            let mut raw_data = instance
                .detail_json
                .as_deref()
                .map(serde_json::from_str::<serde_json::Value>)
                .transpose()
                .map_err(|error| {
                    format!(
                        "active Web issue {} has invalid detail JSON: {error}",
                        group.check_id
                    )
                })?;
            if let Some(page_url) = &instance.page_url {
                match &mut raw_data {
                    Some(serde_json::Value::Object(record)) => {
                        record
                            .entry("pageUrl".to_string())
                            .or_insert_with(|| serde_json::Value::String(page_url.clone()));
                    }
                    None => {
                        raw_data = Some(serde_json::json!({ "pageUrl": page_url }));
                    }
                    Some(_) => {}
                }
            }

            merge_aggregated_web_issue(
                &mut by_check_id,
                CheckResult {
                    check_id: group.check_id.clone(),
                    category,
                    severity: instance.severity,
                    status: instance.check_status.unwrap_or(CheckStatus::Fail),
                    title: instance.title,
                    description: instance.description,
                    fix_prompt: instance.producer_fix_prompt.or(instance.fix_prompt),
                    manual_fix: instance.manual_fix,
                    raw_data,
                    confidence: instance.confidence.unwrap_or(IssueConfidence::High),
                    confidence_reason: instance.confidence_reason,
                    why_it_matters: instance.why_it_matters,
                },
            );
        }
    }

    let mut issues: Vec<CheckResult> = by_check_id.into_values().collect();
    issues.sort_by(|left, right| {
        left.check_id
            .cmp(&right.check_id)
            .then_with(|| left.title.cmp(&right.title))
    });
    Ok(issues)
}

/// Load active web issues for the sidebar badge.
pub(crate) fn load_nav_badge_failed_issues(
    db: &Database,
    project_id: i64,
    url: &str,
) -> Result<Vec<CheckResult>, String> {
    Ok(load_active_web_scan_issues(db, project_id, url)?
        .into_iter()
        .map(|mut slim| {
            // Omit fields the badge does not consume.
            slim.raw_data = None;
            slim.fix_prompt = None;
            slim.manual_fix = None;
            slim
        })
        .collect())
}

pub(crate) fn build_lightweight_project_signal_snapshot(
    db: &Database,
    project_id: i64,
    url: Option<&str>,
) -> Result<ProjectSignalSnapshot, String> {
    let normalized_url = normalize_snapshot_url(url);
    let cached = db.get_project_signal_snapshot_record(project_id, normalized_url.as_deref())?;
    let first_scan_banner_dismissed = db.is_first_scan_banner_dismissed(project_id)?;
    let (mut code_scan_summary, previous_code_scan_summary) =
        load_relevant_code_scan_summary(db, project_id, normalized_url.as_deref())?;
    // Recompute badge counts from active issues rather than the raw scan summary.
    if let Some(summary) = code_scan_summary.as_mut() {
        // Count keys come from plain columns; the full issue views (with a
        // detail_json parse per issue) are not needed to compute badge counts.
        let keys = db.get_code_scan_issue_count_keys(summary.id)?;
        let inactive: std::collections::HashSet<String> = db
            .get_inactive_check_ids(
                project_id,
                normalized_url.as_deref(),
                chrono::Utc::now().timestamp_millis(),
            )?
            .into_iter()
            .collect();
        let active: Vec<_> = keys
            .into_iter()
            .filter(|key| !inactive.contains(&key.check_id))
            .collect();
        let (grouped, critical, high) = grouped_code_counts_from_keys(&active);
        summary.grouped_issue_count = grouped;
        summary.critical_count = critical;
        summary.high_count = high;
    }
    // On a cold snapshot cache, use configured integrations so navigation does
    // not disappear before the dashboard refreshes.
    let monitoring: ProjectMonitoringSignals = load_cached_json(
        cached
            .as_ref()
            .and_then(|entry| entry.monitoring_json.as_ref()),
    )
    .unwrap_or_else(|| ProjectMonitoringSignals {
        enabled_integrations: enabled_integration_names(db, project_id),
        ..ProjectMonitoringSignals::default()
    });
    let monitoring_refreshed_at = cached
        .as_ref()
        .and_then(|entry| entry.monitoring_refreshed_at.clone());
    let (updates, updates_refreshed_at) = load_cached_project_updates_snapshot(
        db,
        project_id,
        normalized_url.as_deref(),
        cached.as_ref(),
    );
    // Count-only surfaces use canonical active groups, not immutable scan payloads.
    let work_summary =
        build_canonical_issue_work_summary(db, project_id, normalized_url.as_deref())?;

    Ok(ProjectSignalSnapshot {
        project_id,
        environment_url: normalized_url,
        first_scan_banner_dismissed,
        code_scan_summary,
        previous_code_scan_summary,
        code_scan_detail: None,
        monitoring,
        monitoring_refreshed_at,
        updates,
        updates_refreshed_at,
        targets: ProjectAttentionTargets::default(),
        work_summary,
    })
}

pub(crate) fn load_dashboard_code_scan_trend(
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
) -> Result<Vec<CodeScanTrendPoint>, String> {
    let history = db.get_code_scan_history(project_id, 24)?;

    let mut relevant = select_relevant_code_scan_history(history, environment_url);
    relevant.truncate(8);
    relevant.reverse();

    Ok(relevant
        .into_iter()
        .map(|entry| CodeScanTrendPoint {
            score: entry.overall_score,
            timestamp: entry.checked_at,
            issue_count: entry.issue_count,
            critical_count: entry.critical_count,
            high_count: entry.high_count,
        })
        .collect())
}

fn aggregated_status_rank(status: CheckStatus) -> u8 {
    match status {
        CheckStatus::Fail | CheckStatus::Warn => 0,
        CheckStatus::Pass => 1,
        CheckStatus::Skipped => 2,
    }
}

fn aggregated_verdict_rank(status: CheckStatus) -> u8 {
    match status {
        CheckStatus::Fail => 0,
        CheckStatus::Warn => 1,
        CheckStatus::Pass => 2,
        CheckStatus::Skipped => 3,
    }
}

fn aggregated_issue_rank(issue: &CheckResult) -> (u8, u8, u8) {
    let status_rank = aggregated_status_rank(issue.status);
    let severity_rank = if status_rank == 0 {
        issue.severity.sort_rank()
    } else {
        0
    };
    (
        status_rank,
        severity_rank,
        aggregated_verdict_rank(issue.status),
    )
}

/// Merge Web issue aliases by canonical lifecycle ID.
/// Actionable status wins, then severity, without mixing producer fields.
fn merge_aggregated_web_issue(
    by_check_id: &mut std::collections::HashMap<String, CheckResult>,
    mut issue: CheckResult,
) {
    let canonical = crate::core::correlation::resolve_check_id("web_scan", &issue.check_id);
    issue.check_id = canonical.clone();
    match by_check_id.entry(canonical) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(issue);
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            if aggregated_issue_rank(&issue) < aggregated_issue_rank(entry.get()) {
                entry.insert(issue);
            }
        }
    }
}

/// Load the scan state served by the dashboard snapshot.
pub(crate) fn load_dashboard_scan_state(
    db: &Database,
    project_id: i64,
    url: &str,
) -> Result<DashboardScanState, String> {
    let trend = db.get_score_trend_for_project(project_id, url, 20)?;
    let history = db.get_scan_history_for_project(project_id, url, 30)?;
    let latest_scan_id = history.first().map(|entry| entry.id);
    let latest_scan_timestamp = history.first().map(|entry| entry.timestamp.clone());
    let latest_detail = match latest_scan_id {
        Some(scan_id) => Some(
            db.get_scan_detail(scan_id)?
                .ok_or_else(|| format!("scan history references missing scan {scan_id}"))?,
        ),
        None => None,
    };
    let latest_type = history.first().map(|entry| entry.scan_type);
    let previous_scan_id = latest_type
        .and_then(|target| {
            history
                .iter()
                .skip(1)
                .find(|entry| entry.scan_type == target)
                .map(|entry| entry.id)
        })
        .or_else(|| history.get(1).map(|entry| entry.id));
    let previous_detail = match previous_scan_id {
        Some(scan_id) => Some(
            db.get_scan_detail(scan_id)?
                .ok_or_else(|| format!("scan history references missing scan {scan_id}"))?,
        ),
        None => None,
    };

    let mut seen_types = std::collections::HashSet::new();
    let mut latest_per_type = Vec::new();
    for entry in &history {
        if seen_types.insert(entry.scan_type) {
            latest_per_type.push(entry.id);
        }
    }

    let mut by_check_id: std::collections::HashMap<String, CheckResult> =
        std::collections::HashMap::new();
    for scan_id in latest_per_type {
        let detail = db
            .get_scan_detail(scan_id)?
            .ok_or_else(|| format!("scan history references missing scan {scan_id}"))?;
        for issue in detail.issues {
            merge_aggregated_web_issue(&mut by_check_id, issue);
        }
    }

    let mut passed = 0;
    for issue in by_check_id.into_values() {
        match issue.status {
            CheckStatus::Pass => passed += 1,
            CheckStatus::Fail | CheckStatus::Warn => {}
            CheckStatus::Skipped => {}
        }
    }
    let aggregated_failed_issues = load_active_web_scan_issues(db, project_id, url)?;
    let failed = aggregated_failed_issues
        .iter()
        .map(|issue| issue.check_id.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len() as u32;
    let total = passed + failed;

    Ok(DashboardScanState {
        trend,
        latest_scan_id,
        latest_detail,
        previous_detail,
        aggregated_check_counts: DashboardAggregatedCheckCounts {
            passed,
            total,
            failed,
        },
        aggregated_failed_issues,
        latest_scan_timestamp,
    })
}

fn extract_public_environment_host(environment_url: Option<&str>) -> Option<String> {
    environment_url.and_then(|value| {
        url::Url::parse(value).ok().and_then(|parsed| {
            if crate::core::localhost::is_localhost(&parsed) {
                return None;
            }
            parsed
                .host_str()
                .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
        })
    })
}

fn choose_dashboard_integration_host(
    environment_url: Option<&str>,
    project_environment_urls: &[String],
) -> Option<String> {
    extract_public_environment_host(environment_url).or_else(|| {
        project_environment_urls
            .iter()
            .find_map(|url| extract_public_environment_host(Some(url.as_str())))
    })
}

fn integration_type_name(config: &crate::integrations::IntegrationConfig) -> String {
    serde_json::to_string(&config.integration_type)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

fn is_dashboard_reference_integration(integration_type: &str) -> bool {
    matches!(
        integration_type,
        "plausible" | "cloudflare" | "uptimerobot" | "googlesearchconsole" | "bingwebmaster"
    )
}

fn dashboard_integration_cache_scope(integration_type: &str, host_filter: Option<&str>) -> String {
    match integration_type {
        "plausible" => host_filter
            .map(|host| format!("dashboard:{}:30d", host.to_ascii_lowercase()))
            .unwrap_or_else(|| "dashboard:30d".to_string()),
        "uptimerobot" => host_filter
            .map(|host| format!("dashboard:{}", host))
            .unwrap_or_else(|| "dashboard:all".to_string()),
        "googlesearchconsole" => "dashboard:28d".to_string(),
        "bingwebmaster" => "dashboard:30d".to_string(),
        _ => "dashboard:30d".to_string(),
    }
}

pub(crate) async fn load_dashboard_integrations(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    environment_url: Option<&str>,
) -> Vec<crate::integrations::IntegrationData> {
    let Ok(configs) = db.get_integrations(project_id) else {
        return Vec::new();
    };
    let project_environment_urls = db.list_project_envs(project_id).unwrap_or_default();
    let host_filter = choose_dashboard_integration_host(environment_url, &project_environment_urls);

    let mut data = Vec::new();
    for config in configs.into_iter().filter(|config| config.enabled) {
        let typed_integration = config.integration_type.clone();
        let integration_type = integration_type_name(&config);
        if !is_dashboard_reference_integration(&integration_type) {
            continue;
        }

        let cache_scope =
            dashboard_integration_cache_scope(&integration_type, host_filter.as_deref());

        match super::integrations::fetch_cached_integration_data_internal(
            app,
            db,
            project_id,
            &integration_type,
            host_filter.as_deref(),
            &cache_scope,
        )
        .await
        {
            Ok(integration) => data.push(integration),
            Err(error) => data.push(crate::integrations::IntegrationData {
                integration_type: typed_integration,
                data: serde_json::Value::Null,
                fetched_at: chrono::Utc::now().to_rfc3339(),
                error: Some(error),
            }),
        }
    }

    data
}

pub(crate) fn load_dashboard_commits_since_last_scan(
    db: &Database,
    project_id: i64,
    latest_scan_timestamp: Option<&str>,
) -> Vec<git::GitCommit> {
    let Some(since) = latest_scan_timestamp else {
        return Vec::new();
    };
    let Some(project_path) = db.get_project_path(project_id) else {
        return Vec::new();
    };
    git::get_commits_since(&project_path, since)
}

/// Return check IDs excluded from active lists and scoring by lifecycle state.
/// Uses the same grouped state view as the score to keep badge counts aligned.
pub(crate) fn load_dashboard_inactive_check_ids(
    db: &Database,
    project_id: i64,
    env_url: Option<&str>,
    now_ms: i64,
) -> Result<Vec<String>, String> {
    Ok(db.get_inactive_check_ids(project_id, env_url, now_ms)?)
}

#[cfg(test)]
#[path = "project_signal_snapshots_tests.rs"]
mod tests;
