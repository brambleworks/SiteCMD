use crate::checks::CheckResult;
use crate::core::git;
use crate::core::scanner::ScanResult;
use crate::db::{
    Database, ProjectSignalSnapshot, ProjectWorkQueue, ProjectWorkSummary, SearchRegressionSignal,
};
use crate::scoring::calculator::compute_current_score;
use std::sync::Arc;
use tauri::{AppHandle, State};

use super::project_maintenance_items::build_project_maintenance_items;
use super::project_signal_snapshots::{
    build_lightweight_project_signal_snapshot, get_project_signal_snapshot_internal,
    load_dashboard_code_scan_trend, load_dashboard_commits_since_last_scan,
    load_dashboard_inactive_check_ids, load_dashboard_integrations, load_dashboard_scan_state,
    load_nav_badge_failed_issues,
};
use super::project_signal_state::normalize_snapshot_url;
use super::project_work_items::{build_issue_work_entries, build_update_work_entries};
use super::{run_blocking, sanitize_error};
use crate::core::project_snapshot::build_project_work_queue;

/// Get all projects with issue counts for the Sites Overview page.
#[tauri::command]
#[tracing::instrument(skip(db))]
pub async fn get_all_projects_summary(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<ProjectSummary>, String> {
    // Sync DB reads throughout; run off the async runtime workers so a held
    // DB worker can't park an IPC thread.
    let db = db.inner().clone();
    crate::commands::run_blocking(move || build_all_projects_summary(&db)).await?
}

fn build_all_projects_summary(db: &Database) -> Result<Vec<ProjectSummary>, String> {
    let projects = db.get_projects().map_err(sanitize_error)?;
    let urls: Vec<String> = projects
        .iter()
        .filter_map(|p| p.environments.first().map(|e| e.url.clone()))
        .collect();
    let issue_counts = db.get_latest_issue_counts_batch(&urls)?;
    let mut summaries = Vec::new();
    for p in projects {
        let primary_url = p.environments.first().map(|e| e.url.clone());
        let (issues_critical, issues_high) = primary_url
            .as_deref()
            .map(|u| issue_counts.get(u).copied().unwrap_or((0, 0)))
            .unwrap_or((0, 0));
        let latest_score = p.environments.first().and_then(|e| e.latest_score);
        let last_scanned = p
            .environments
            .first()
            .and_then(|e| e.last_scanned_at.clone());
        summaries.push(ProjectSummary {
            id: p.id,
            name: p.name,
            framework: p.framework,
            primary_url: primary_url.unwrap_or_default(),
            latest_score,
            last_scanned_at: last_scanned,
            issues_critical,
            issues_high,
            environment_count: p.environments.len() as u32,
        });
    }
    Ok(summaries)
}

#[tauri::command]
#[tracing::instrument(
    skip(app, db, url),
    fields(project_id, force_refresh, include_code_scan_detail)
)]
pub async fn get_project_signal_snapshot(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    url: Option<String>,
    force_refresh: Option<bool>,
    include_code_scan_detail: Option<bool>,
) -> Result<ProjectSignalSnapshot, String> {
    get_project_signal_snapshot_internal(
        &app,
        &db,
        project_id,
        url.as_deref(),
        force_refresh.unwrap_or(false),
        true,
        include_code_scan_detail.unwrap_or(true),
    )
    .await
    .map_err(sanitize_error)
}

#[tauri::command]
#[tracing::instrument(skip(app, db), fields(force_refresh))]
pub async fn get_all_projects_work_summary(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    force_refresh: Option<bool>,
) -> Result<Vec<TodayProjectWorkSummary>, String> {
    // Initial reads off the async runtime workers (get_projects runs a
    // latest-score subquery per environment).
    let projects = {
        let db = (*db).clone();
        run_blocking(move || db.get_projects())
            .await?
            .map_err(sanitize_error)?
    };
    let allow_refresh = force_refresh.unwrap_or(false);

    let snapshot_futures: Vec<_> = projects
        .iter()
        .map(|project| {
            let primary_url = project
                .environments
                .first()
                .map(|env| env.url.clone())
                .unwrap_or_default();
            let env_url = if primary_url.is_empty() {
                None
            } else {
                Some(primary_url.clone())
            };
            let app_ref = &app;
            let db_ref = &db;
            async move {
                get_project_signal_snapshot_internal(
                    app_ref,
                    db_ref,
                    project.id,
                    env_url.as_deref(),
                    allow_refresh,
                    allow_refresh,
                    false,
                )
                .await
            }
        })
        .collect();
    let snapshots = futures_util::future::join_all(snapshot_futures).await;

    // Batch the per-environment issue counts into one DB round-trip instead of
    // one query per project on the single writer thread (mirrors the lighter
    // get_all_projects_summary path).
    let issue_count_urls: Vec<String> = projects
        .iter()
        .filter_map(|project| project.environments.first().map(|env| env.url.clone()))
        .collect();
    let issue_counts = {
        let db = (*db).clone();
        run_blocking(move || db.get_latest_issue_counts_batch(&issue_count_urls)).await??
    };

    let mut summaries = Vec::new();
    for (project, snapshot_result) in projects.into_iter().zip(snapshots) {
        let snapshot = snapshot_result.map_err(sanitize_error)?;
        let primary_url = project
            .environments
            .first()
            .map(|env| env.url.clone())
            .unwrap_or_default();
        let (issues_critical, issues_high) =
            issue_counts.get(&primary_url).copied().unwrap_or((0, 0));
        let latest_score = project
            .environments
            .first()
            .and_then(|env| env.latest_score);
        let last_scanned_at = project
            .environments
            .first()
            .and_then(|env| env.last_scanned_at.clone());
        let site_score_snapshot = if primary_url.is_empty() {
            None
        } else {
            let now_ms = chrono::Utc::now().timestamp_millis();
            // Compute the score from unenriched groups and offload the database work;
            // dashboard summaries do not consume correlation enrichment.
            let groups_db = (*db).clone();
            let groups_url = primary_url.clone();
            let groups_project_id = project.id;
            run_blocking(move || {
                let groups = groups_db.get_active_issue_groups(
                    groups_project_id,
                    Some(&groups_url),
                    now_ms,
                )?;
                // Share the score authority's no-signal predicate so overview and
                // snapshot persistence cannot diverge.
                if !groups_db.has_persistable_score_signal(
                    groups_project_id,
                    Some(&groups_url),
                    !groups.is_empty(),
                ) {
                    return Ok::<_, String>(None);
                }
                let score = compute_current_score(&groups, now_ms);
                // Write-on-change history for the headline number (E2); a
                // persistence failure must not break the overview.
                if let Err(e) = groups_db.record_score_snapshot_if_changed(
                    groups_project_id,
                    Some(&groups_url),
                    &score,
                ) {
                    tracing::warn!("failed to persist live score snapshot: {}", e);
                }
                Ok(Some(score))
            })
            .await??
        };
        let site_score = site_score_snapshot
            .as_ref()
            .map(|snapshot| snapshot.overall.round().clamp(0.0, 100.0) as u32);
        let site_issue_count = site_score_snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot.critical_count
                    + snapshot.high_count
                    + snapshot.medium_count
                    + snapshot.low_count
            })
            .unwrap_or(0) as u32;
        let site_critical_count = site_score_snapshot
            .as_ref()
            .map(|snapshot| snapshot.critical_count)
            .unwrap_or(0) as u32;
        let site_high_count = site_score_snapshot
            .as_ref()
            .map(|snapshot| snapshot.high_count)
            .unwrap_or(0) as u32;
        let top_guardrail_issue = match snapshot
            .code_scan_summary
            .as_ref()
            .map(|summary| summary.id)
        {
            Some(scan_id) => {
                let db = (*db).clone();
                run_blocking(move || db.get_top_code_scan_issue_view(scan_id)).await??
            }
            None => None,
        };

        summaries.push(TodayProjectWorkSummary {
            id: project.id,
            name: project.name,
            framework: project.framework,
            primary_url,
            latest_score,
            site_score,
            site_issue_count,
            site_critical_count,
            site_high_count,
            last_scanned_at,
            issues_critical,
            issues_high,
            environment_count: project.environments.len() as u32,
            project_path: if project.path.is_empty() {
                None
            } else {
                Some(project.path)
            },
            primary_security_issue_id: snapshot.targets.security_issue_id.clone(),
            primary_security_focus: snapshot.targets.security_focus.clone(),
            enabled_integrations: snapshot.monitoring.enabled_integrations.clone(),
            security_update_count: snapshot
                .updates
                .as_ref()
                .map(|report| {
                    report
                        .updates
                        .iter()
                        .filter(|update| update.is_security)
                        .count() as u32
                })
                .unwrap_or(0),
            pending_update_count: snapshot
                .updates
                .as_ref()
                .map(|report| report.updates.len() as u32)
                .unwrap_or(0),
            search_regression: snapshot.monitoring.search_regression.clone(),
            integration_failure_count: snapshot.monitoring.integration_failure_count,
            stale_integration_count: snapshot.monitoring.stale_integration_count,
            guardrail_critical_count: snapshot
                .code_scan_summary
                .as_ref()
                .map(|summary| summary.critical_count)
                .unwrap_or(0),
            guardrail_high_count: snapshot
                .code_scan_summary
                .as_ref()
                .map(|summary| summary.high_count)
                .unwrap_or(0),
            top_guardrail_issue,
            top_guardrail_domain: snapshot
                .code_scan_summary
                .as_ref()
                .and_then(|summary| summary.top_domain),
            top_guardrail_domain_count: snapshot
                .code_scan_summary
                .as_ref()
                .map(|summary| summary.top_domain_count)
                .unwrap_or(0),
            guardrails_checked_at: snapshot
                .code_scan_summary
                .as_ref()
                .map(|summary| summary.checked_at.clone()),
            code_scan_checked_at: snapshot
                .code_scan_summary
                .as_ref()
                .map(|summary| summary.checked_at.clone()),
            work_summary: snapshot.work_summary,
        });
    }

    Ok(summaries)
}

#[tauri::command]
#[tracing::instrument(skip(db, url), fields(project_id))]
pub async fn invalidate_project_signal_snapshot(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    url: Option<String>,
) -> Result<(), String> {
    let db = (*db).clone();
    run_blocking(move || db.invalidate_project_signal_snapshots(project_id, url.as_deref()))
        .await?
        .map_err(sanitize_error)
}

#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id))]
pub async fn dismiss_first_scan_banner(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<(), String> {
    let db = (*db).clone();
    run_blocking(move || db.dismiss_first_scan_banner(project_id))
        .await?
        .map_err(sanitize_error)
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectSummary {
    pub id: i64,
    pub name: String,
    pub framework: Option<String>,
    pub primary_url: String,
    pub latest_score: Option<u32>,
    pub last_scanned_at: Option<String>,
    pub issues_critical: u32,
    pub issues_high: u32,
    pub environment_count: u32,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DashboardAggregatedCheckCounts {
    pub passed: u32,
    pub total: u32,
    pub failed: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct CodeScanTrendPoint {
    pub score: u32,
    pub timestamp: String,
    pub issue_count: u32,
    pub critical_count: u32,
    pub high_count: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DashboardSnapshot {
    pub project_id: i64,
    pub environment_url: Option<String>,
    pub trend: Vec<crate::db::ScoreTrendPoint>,
    pub code_trend: Vec<CodeScanTrendPoint>,
    pub latest_scan_id: Option<i64>,
    pub latest_detail: Option<ScanResult>,
    pub previous_detail: Option<ScanResult>,
    pub aggregated_check_counts: DashboardAggregatedCheckCounts,
    pub aggregated_failed_issues: Vec<CheckResult>,
    pub commits_since_last_scan: Vec<git::GitCommit>,
    pub issue_links: Vec<crate::db::IssueLink>,
    pub inactive_check_ids: Vec<String>,
    pub signals: ProjectSignalSnapshot,
    pub work_queue: ProjectWorkQueue,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DashboardReferenceSignals {
    pub integrations: Vec<crate::integrations::IntegrationData>,
    pub last_ci_run: Option<crate::integrations::github::WorkflowRun>,
    pub psi_report: Option<crate::integrations::pagespeed::PageSpeedReport>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ProjectNavBadgeSnapshot {
    pub project_id: i64,
    pub environment_url: Option<String>,
    pub aggregated_failed_issues: Vec<CheckResult>,
    pub inactive_check_ids: Vec<String>,
    pub signals: ProjectSignalSnapshot,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct TodayProjectWorkSummary {
    pub id: i64,
    pub name: String,
    pub framework: Option<String>,
    pub primary_url: String,
    pub latest_score: Option<u32>,
    pub site_score: Option<u32>,
    pub site_issue_count: u32,
    pub site_critical_count: u32,
    pub site_high_count: u32,
    pub last_scanned_at: Option<String>,
    pub issues_critical: u32,
    pub issues_high: u32,
    pub environment_count: u32,
    pub project_path: Option<String>,
    pub primary_security_issue_id: Option<String>,
    pub primary_security_focus: Option<String>,
    pub enabled_integrations: Vec<String>,
    pub security_update_count: u32,
    pub pending_update_count: u32,
    pub search_regression: Option<SearchRegressionSignal>,
    pub integration_failure_count: u32,
    pub stale_integration_count: u32,
    pub guardrail_critical_count: u32,
    pub guardrail_high_count: u32,
    pub top_guardrail_issue: Option<crate::core::code_scan::CodeIssueView>,
    pub top_guardrail_domain: Option<crate::core::code_scan::CodeScanDomain>,
    pub top_guardrail_domain_count: u32,
    pub guardrails_checked_at: Option<String>,
    pub code_scan_checked_at: Option<String>,
    pub work_summary: ProjectWorkSummary,
}

#[tauri::command]
#[tracing::instrument(skip(app, db, url), fields(project_id, force_refresh))]
pub async fn get_dashboard_snapshot(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    url: String,
    force_refresh: Option<bool>,
) -> Result<DashboardSnapshot, String> {
    let normalized_url = normalize_snapshot_url(Some(&url)).unwrap_or(url);
    let environment_url = Some(normalized_url.clone());
    let db_reads = (*db).clone();
    let normalized_url_for_reads = normalized_url.clone();
    let environment_url_for_reads = environment_url.clone();
    let (scan_state, code_trend, issue_links, inactive_check_ids, commits_since_last_scan) =
        run_blocking(move || -> Result<_, String> {
            let scan_state =
                load_dashboard_scan_state(&db_reads, project_id, &normalized_url_for_reads)?;
            let code_trend = load_dashboard_code_scan_trend(
                &db_reads,
                project_id,
                environment_url_for_reads.as_deref(),
            )?;
            let issue_links = db_reads.get_issue_links(project_id)?;
            let inactive_check_ids = load_dashboard_inactive_check_ids(
                &db_reads,
                project_id,
                environment_url_for_reads.as_deref(),
                chrono::Utc::now().timestamp_millis(),
            )?;
            let commits_since_last_scan = load_dashboard_commits_since_last_scan(
                &db_reads,
                project_id,
                scan_state.latest_scan_timestamp.as_deref(),
            );

            Ok((
                scan_state,
                code_trend,
                issue_links,
                inactive_check_ids,
                commits_since_last_scan,
            ))
        })
        .await??;

    let signals = get_project_signal_snapshot_internal(
        &app,
        &db,
        project_id,
        environment_url.as_deref(),
        force_refresh.unwrap_or(false),
        force_refresh.unwrap_or(false),
        true,
    )
    .await?;
    let db_work = (*db).clone();
    let environment_url_for_work = environment_url.clone();
    let latest_detail_for_work = scan_state.latest_detail.clone();
    let code_scan_summary_for_work = signals.code_scan_summary.clone();
    let updates_for_work = signals.updates.clone();
    let updates_refreshed_at_for_work = signals.updates_refreshed_at.clone();
    let monitoring_for_work = signals.monitoring.clone();
    let (issue_work_items, maintenance_work_items) = run_blocking(move || -> Result<_, String> {
        let env_url = environment_url_for_work.as_deref();
        // Same derivation as refresh_project_work_state: lifecycle from
        // project_issue_states, verify-in-flight from fix_attempts, security
        // updates from the snapshot loaded above. No persisted queue rows.
        let mut issue_work_items = build_issue_work_entries(&db_work, project_id, env_url)?;
        issue_work_items.extend(build_update_work_entries(
            &db_work,
            project_id,
            env_url,
            updates_for_work.as_ref(),
        )?);
        let maintenance_work_items = build_project_maintenance_items(
            &db_work,
            project_id,
            env_url,
            latest_detail_for_work.as_ref(),
            code_scan_summary_for_work.as_ref(),
            updates_refreshed_at_for_work.as_deref(),
            &monitoring_for_work,
        )?;
        Ok((issue_work_items, maintenance_work_items))
    })
    .await??;
    let work_queue = build_project_work_queue(&issue_work_items, maintenance_work_items);

    Ok(DashboardSnapshot {
        project_id,
        environment_url,
        trend: scan_state.trend,
        code_trend,
        latest_scan_id: scan_state.latest_scan_id,
        latest_detail: scan_state.latest_detail,
        previous_detail: scan_state.previous_detail,
        aggregated_check_counts: scan_state.aggregated_check_counts,
        aggregated_failed_issues: scan_state.aggregated_failed_issues,
        commits_since_last_scan,
        issue_links,
        inactive_check_ids,
        signals,
        work_queue,
    })
}

#[tauri::command]
#[tracing::instrument(skip(app, db, url), fields(project_id, include_psi))]
pub async fn get_dashboard_reference_signals(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    url: String,
    include_psi: Option<bool>,
) -> Result<DashboardReferenceSignals, String> {
    let normalized_url = normalize_snapshot_url(Some(&url)).unwrap_or(url);
    let environment_url = Some(normalized_url.clone());
    let load_psi = include_psi.unwrap_or(false);

    let (integrations, github) = tokio::join!(
        load_dashboard_integrations(&app, &db, project_id, environment_url.as_deref()),
        super::integrations::fetch_github_data_internal(&app, &db, project_id),
    );

    let psi_report = if load_psi {
        let api_key = crate::keyring::get_pagespeed_api_key(&app).ok().flatten();
        match crate::integrations::pagespeed::fetch_pagespeed_report(
            &normalized_url,
            "mobile",
            api_key.as_deref(),
        )
        .await
        {
            Ok(report) => Some(report),
            Err(error) => {
                tracing::warn!(
                    "Failed to fetch PageSpeed reference signal for project {} ({}): {}",
                    project_id,
                    normalized_url,
                    error
                );
                None
            }
        }
    } else {
        None
    };

    Ok(DashboardReferenceSignals {
        integrations,
        last_ci_run: github
            .ok()
            .and_then(|data| data.workflow_runs.into_iter().next()),
        psi_report,
    })
}

#[tauri::command]
#[tracing::instrument(skip(_app, db, _force_refresh, url), fields(project_id))]
pub async fn get_project_nav_badge_snapshot(
    _app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    url: String,
    _force_refresh: Option<bool>,
) -> Result<ProjectNavBadgeSnapshot, String> {
    let normalized_url = normalize_snapshot_url(Some(&url)).unwrap_or(url);
    let environment_url = Some(normalized_url.clone());
    let db = (*db).clone();
    run_blocking(move || -> Result<ProjectNavBadgeSnapshot, String> {
        let inactive_check_ids = load_dashboard_inactive_check_ids(
            &db,
            project_id,
            environment_url.as_deref(),
            chrono::Utc::now().timestamp_millis(),
        )?;

        let signals =
            build_lightweight_project_signal_snapshot(&db, project_id, environment_url.as_deref())?;

        Ok(ProjectNavBadgeSnapshot {
            project_id,
            environment_url,
            aggregated_failed_issues: load_nav_badge_failed_issues(
                &db,
                project_id,
                &normalized_url,
            )?,
            inactive_check_ids,
            signals,
        })
    })
    .await?
}
