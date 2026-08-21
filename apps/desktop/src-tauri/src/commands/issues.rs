use std::collections::HashSet;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::core::types_work_items::{IssueGroup, ScoreSnapshot, VerifiedBy};
use crate::db::{
    Database, GroupDecision, IssueCheckMemory, IssueLifecycle, IssueStateRow, ScoreSnapshotPoint,
};
use crate::scoring::calculator::compute_current_score;

use super::issue_source_capabilities::{
    issue_source_capability, verify_issue_source, IssueVerifyStrategy,
};
use super::{emit_site_score_changed, run_blocking};

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub(super) fn require_issue_env_url(env_url: Option<String>) -> Result<String, String> {
    env_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "env_url is required for issue actions".to_string())
}

#[tauri::command]
#[tracing::instrument(skip(db, env_url), fields(project_id))]
pub async fn get_work_items(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    env_url: Option<String>,
) -> Result<Vec<IssueGroup>, String> {
    let db = (*db).clone();
    run_blocking(move || db.get_work_items_grouped(project_id, env_url.as_deref(), now_ms()))
        .await?
        .map_err(String::from)
}

/// Compute and persist a changed live score when real score evidence exists.
pub(crate) fn compute_and_record_current_score(
    db: &Database,
    project_id: i64,
    env_url: Option<&str>,
    now: i64,
) -> Result<ScoreSnapshot, String> {
    // Unenriched read: the score needs only active issue instances and their
    // promoted confidence fields. Skipping dossier enrichment keeps this path
    // cheaper and prevents presentation-only fields from affecting scoring.
    let groups = db.get_active_issue_groups(project_id, env_url, now)?;
    let snapshot = compute_current_score(&groups, now);
    // Persist score changes at the shared read path, but skip synthetic 100s
    // for projects with no issue or scan evidence.
    let has_score_signal = db.has_persistable_score_signal(project_id, env_url, !groups.is_empty());
    if has_score_signal {
        if let Err(e) = db.record_score_snapshot_if_changed(project_id, env_url, &snapshot) {
            tracing::warn!("failed to persist live score snapshot: {}", e);
        }
    }
    Ok(snapshot)
}

#[tauri::command]
#[tracing::instrument(skip(db, env_url), fields(project_id))]
pub async fn get_current_score(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    env_url: Option<String>,
) -> Result<ScoreSnapshot, String> {
    let db = (*db).clone();
    run_blocking(move || {
        compute_and_record_current_score(&db, project_id, env_url.as_deref(), now_ms())
    })
    .await?
}

/// Recent persisted score history for one project environment, newest first.
#[tauri::command]
#[tracing::instrument(skip(db, env_url), fields(project_id))]
pub async fn get_score_snapshot_history(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    env_url: Option<String>,
) -> Result<Vec<ScoreSnapshotPoint>, String> {
    let db = (*db).clone();
    run_blocking(move || {
        db.get_score_snapshot_history(
            project_id,
            env_url.as_deref(),
            crate::constants::SCORE_SNAPSHOT_HISTORY_LIMIT,
        )
    })
    .await?
    .map_err(|e| format!("failed to load score history: {}", e))
}

/// Read the lifecycle status for one issue.
#[tauri::command]
#[tracing::instrument(skip(db, env_url), fields(project_id, check_id = %check_id))]
pub async fn get_issue_state(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    env_url: Option<String>,
    check_id: String,
) -> Result<Option<IssueStateRow>, String> {
    let db = (*db).clone();
    run_blocking(move || db.get_issue_state(project_id, env_url.as_deref(), &check_id))
        .await?
        .map_err(String::from)
}

/// Return project-wide lifecycle history for one issue check ID.
#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id, check_id = %check_id))]
pub async fn get_issue_check_memory(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    check_id: String,
) -> Result<IssueCheckMemory, String> {
    let db = (*db).clone();
    run_blocking(move || db.get_issue_check_memory(project_id, &check_id))
        .await?
        .map_err(String::from)
}

#[tauri::command]
#[tracing::instrument(skip(db, app, env_url), fields(project_id, check_id = %check_id, snooze_until))]
pub async fn snooze_issue(
    db: State<'_, Arc<Database>>,
    app: AppHandle,
    project_id: i64,
    env_url: Option<String>,
    check_id: String,
    snooze_until: i64,
) -> Result<(), String> {
    let env_url = require_issue_env_url(env_url)?;
    let db = (*db).clone();
    run_blocking(move || {
        db.record_group_decision(
            project_id,
            &env_url,
            &check_id,
            GroupDecision::Snooze {
                until: snooze_until,
            },
            now_ms(),
        )
        .map(|_| ())
    })
    .await??;
    emit_site_score_changed(&app, project_id);
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(db, app, env_url), fields(project_id, check_id = %check_id))]
pub async fn ignore_issue(
    db: State<'_, Arc<Database>>,
    app: AppHandle,
    project_id: i64,
    env_url: Option<String>,
    check_id: String,
) -> Result<(), String> {
    let env_url = require_issue_env_url(env_url)?;
    let db = (*db).clone();
    run_blocking(move || {
        db.record_group_decision(
            project_id,
            &env_url,
            &check_id,
            GroupDecision::Ignore,
            now_ms(),
        )
        .map(|_| ())
    })
    .await??;
    emit_site_score_changed(&app, project_id);
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(db, app, env_url), fields(project_id, check_id = %check_id, reason = %reason))]
pub async fn block_issue(
    db: State<'_, Arc<Database>>,
    app: AppHandle,
    project_id: i64,
    env_url: Option<String>,
    check_id: String,
    reason: String,
) -> Result<(), String> {
    let env_url = require_issue_env_url(env_url)?;
    let db = (*db).clone();
    run_blocking(move || {
        db.record_group_decision(
            project_id,
            &env_url,
            &check_id,
            GroupDecision::Block {
                reason: Some(reason),
            },
            now_ms(),
        )
        .map(|_| ())
    })
    .await??;
    emit_site_score_changed(&app, project_id);
    Ok(())
}

/// Record a user-claimed fix without treating it as scan verification.
/// Later observations may reopen it; verified fixes use [`verify_issue`].
#[tauri::command]
#[tracing::instrument(skip(db, app, env_url), fields(project_id, check_id = %check_id))]
pub async fn mark_issue_fixed(
    db: State<'_, Arc<Database>>,
    app: AppHandle,
    project_id: i64,
    env_url: Option<String>,
    check_id: String,
) -> Result<(), String> {
    let env_url = require_issue_env_url(env_url)?;
    let db = (*db).clone();
    run_blocking(move || {
        db.record_group_decision(
            project_id,
            &env_url,
            &check_id,
            GroupDecision::ClaimFixed,
            now_ms(),
        )
        .map(|_| ())
    })
    .await??;
    emit_site_score_changed(&app, project_id);
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(db, app, env_url), fields(project_id, check_id = %check_id))]
pub async fn reopen_issue(
    db: State<'_, Arc<Database>>,
    app: AppHandle,
    project_id: i64,
    env_url: Option<String>,
    check_id: String,
) -> Result<(), String> {
    let env_url = require_issue_env_url(env_url)?;
    let db = (*db).clone();
    run_blocking(move || {
        db.record_group_decision(
            project_id,
            &env_url,
            &check_id,
            GroupDecision::Reopen,
            now_ms(),
        )
        .map(|_| ())
    })
    .await??;
    emit_site_score_changed(&app, project_id);
    Ok(())
}

/// Verify-relevant snapshot of an active IssueGroup: which sources
/// contributed and, for web-scan instances, which page URL to re-check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebVerifyTarget {
    pub url: String,
    pub producer_check_ids: Vec<String>,
}

pub(crate) struct IssueVerifyInfo {
    pub sources: Vec<String>,
    pub web_targets: Vec<WebVerifyTarget>,
}

/// Look up the active IssueGroup for one check_id and snapshot the fields
/// verification routing needs. `Ok(None)` when no active group exists, e.g.
/// because the issue resolved since the caller last saw it.
pub(crate) fn lookup_issue_verify_info(
    db: &Database,
    project_id: i64,
    env_url: &str,
    check_id: &str,
) -> Result<Option<IssueVerifyInfo>, String> {
    crate::core::code_scan::validate_canonical_check_id(check_id)?;
    let groups = db.get_work_items_grouped(project_id, Some(env_url), now_ms())?;
    let matching_groups: Vec<&IssueGroup> = groups
        .iter()
        .filter(|group| group.check_id == check_id)
        .collect();
    if matching_groups.is_empty() {
        return Ok(None);
    }
    let mut sources = std::collections::BTreeSet::new();
    let mut web_targets: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        std::collections::BTreeMap::new();
    for group in matching_groups {
        sources.extend(group.sources.iter().cloned());
        for instance in group
            .instances
            .iter()
            .filter(|instance| instance.source == "web_scan")
        {
            let url = instance
                .page_url
                .as_deref()
                .or(instance.url.as_deref())
                .unwrap_or(env_url)
                .to_string();
            let producer = instance
                .producer_check_id
                .as_deref()
                .unwrap_or(check_id)
                .to_string();
            web_targets.entry(url).or_default().insert(producer);
        }
    }

    Ok(Some(IssueVerifyInfo {
        sources: sources.into_iter().collect(),
        web_targets: web_targets
            .into_iter()
            .map(|(url, producer_check_ids)| WebVerifyTarget {
                url,
                producer_check_ids: producer_check_ids.into_iter().collect(),
            })
            .collect(),
    }))
}

/// Which sources `verify_issue_sources_for_check` triggered. Empty when no
/// active IssueGroup existed for the check_id (nothing ran).
pub(crate) struct VerifyTrigger {
    pub sources: Vec<String>,
    pub has_pending_sources: bool,
}

/// Deduplicate successful whole-project scans while allowing failures to retry.
fn code_scan_pending(dedup: &HashSet<(i64, String)>, project_id: i64, env_url: &str) -> bool {
    !dedup.contains(&(project_id, env_url.to_string()))
}

/// Re-run the source engines that contributed to an active issue group.
pub(crate) async fn verify_issue_sources_for_check(
    app: &AppHandle,
    db: Arc<Database>,
    project_id: i64,
    env_url: &str,
    check_id: &str,
    code_scan_dedup: &mut HashSet<(i64, String)>,
) -> Result<VerifyTrigger, String> {
    let info = {
        let db = db.clone();
        let env_url = env_url.to_string();
        let check_id = check_id.to_string();
        run_blocking(move || lookup_issue_verify_info(&db, project_id, &env_url, &check_id))
            .await??
    };
    let Some(info) = info else {
        return Ok(VerifyTrigger {
            sources: Vec::new(),
            has_pending_sources: false,
        });
    };

    let mut has_pending_sources = false;
    for source in &info.sources {
        let capability = issue_source_capability(source)
            .ok_or_else(|| format!("verify not implemented for source: {}", source))?;
        if capability.verify == IssueVerifyStrategy::IntegrationPoll {
            has_pending_sources = true;
        }
        // A code scan re-scans the whole repo, refreshing work_items for every
        // code issue, so a sibling attempt for the same project+env this scope
        // needs no rescan. Skip only when a prior scan this scope succeeded.
        if capability.verify == IssueVerifyStrategy::CodeScan
            && !code_scan_pending(code_scan_dedup, project_id, env_url)
        {
            continue;
        }
        if capability.verify == IssueVerifyStrategy::WebScan && !info.web_targets.is_empty() {
            for target in &info.web_targets {
                verify_issue_source(
                    capability,
                    app,
                    db.clone(),
                    project_id,
                    env_url,
                    check_id,
                    Some(&target.url),
                    &target.producer_check_ids,
                )
                .await?;
            }
        } else {
            verify_issue_source(
                capability,
                app,
                db.clone(),
                project_id,
                env_url,
                check_id,
                None,
                &[],
            )
            .await?;
        }
        if capability.verify == IssueVerifyStrategy::CodeScan {
            code_scan_dedup.insert((project_id, env_url.to_string()));
        }
    }

    Ok(VerifyTrigger {
        sources: info.sources,
        has_pending_sources,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum IssueVerificationStatus {
    Verified,
    StillPresent,
    Queued,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct IssueVerificationOutcome {
    pub status: IssueVerificationStatus,
    pub sources: Vec<String>,
}

fn verification_status(
    has_pending_sources: bool,
    issue_still_active: bool,
) -> IssueVerificationStatus {
    if has_pending_sources {
        IssueVerificationStatus::Queued
    } else if issue_still_active {
        IssueVerificationStatus::StillPresent
    } else {
        IssueVerificationStatus::Verified
    }
}

/// Re-verify an issue by re-running the appropriate scan engine(s) for each
/// source that contributed to the active IssueGroup; see
/// [`verify_issue_sources_for_check`] for the routing.
#[tauri::command]
#[tracing::instrument(skip(db, app, env_url), fields(project_id, check_id = %check_id))]
pub async fn verify_issue(
    db: State<'_, Arc<Database>>,
    app: AppHandle,
    project_id: i64,
    env_url: Option<String>,
    check_id: String,
) -> Result<IssueVerificationOutcome, String> {
    let env_url = require_issue_env_url(env_url)?;
    let mut code_scan_dedup = HashSet::new();
    let trigger = verify_issue_sources_for_check(
        &app,
        (*db).clone(),
        project_id,
        &env_url,
        &check_id,
        &mut code_scan_dedup,
    )
    .await?;
    if trigger.sources.is_empty() {
        return Err(format!("no active issue for check_id: {}", check_id));
    }
    let issue_still_active = {
        let db = (*db).clone();
        let env_url = env_url.clone();
        let check_id = check_id.clone();
        run_blocking(move || lookup_issue_verify_info(&db, project_id, &env_url, &check_id))
            .await??
            .is_some()
    };
    let status = verification_status(trigger.has_pending_sources, issue_still_active);
    if status == IssueVerificationStatus::Verified {
        let db = (*db).clone();
        let env_url = env_url.clone();
        let check_id = check_id.clone();
        run_blocking(move || {
            db.set_issue_group_state(
                project_id,
                &env_url,
                &check_id,
                IssueLifecycle::Verified {
                    by: VerifiedBy::LocalScan,
                },
                now_ms(),
            )
        })
        .await??;
        emit_site_score_changed(&app, project_id);
    }
    Ok(IssueVerificationOutcome {
        status,
        sources: trigger.sources,
    })
}

#[cfg(test)]
#[path = "issues_tests.rs"]
mod tests;
