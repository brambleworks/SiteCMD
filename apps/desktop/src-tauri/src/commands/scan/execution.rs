//! Execution-first orchestration over the existing Web and Code collectors.

use std::{collections::HashSet, sync::Arc};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::core::scan_execution::{
    NewScanExecution, ScanAdmissionClass, ScanComponent, ScanComponentStatus, ScanExecutionError,
    ScanExecutionMode, ScanExecutionRecord, ScanTrigger,
};
use crate::core::scanner::{MultiScanResult, ScanResult, ScanType};
use crate::db::{normalize_env_url, CodeScanResult, Database};

use super::control::ScanControlState;
use super::execution_events::{emit_scan_execution_completed, record_scan_execution_event};

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct RunScanExecutionRequest {
    pub project_id: Option<i64>,
    pub environment_id: Option<i64>,
    pub environment_url: Option<String>,
    pub requested_mode: ScanExecutionMode,
    pub web_focus: Option<ScanType>,
    #[serde(default)]
    pub urls: Vec<String>,
    pub enabled_categories: Option<Vec<String>>,
    pub timeout_secs: Option<u64>,
    pub axe_enabled: Option<bool>,
    #[serde(default)]
    pub inspect_local_databases: bool,
    pub project_path: Option<String>,
    pub scan_request_id: Option<u64>,
    pub retention: Option<u32>,
    pub trigger: ScanTrigger,
    pub idempotency_key: String,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct RunScanExecutionResult {
    pub execution: ScanExecutionRecord,
    pub reused: bool,
    pub web_result: Option<ScanResult>,
    pub multi_result: Option<MultiScanResult>,
    pub code_result: Option<CodeScanResult>,
    pub issue_changes: Option<ScanIssueChanges>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ScanIssueChanges {
    pub previous_open_issues: usize,
    pub open_issues: usize,
    pub new_issues: usize,
    pub resolved_issues: usize,
}

#[derive(Debug, Serialize)]
struct FingerprintV1<'a> {
    project_id: Option<i64>,
    environment_id: Option<i64>,
    environment_url: Option<&'a str>,
    environment_scope_key: &'a str,
    requested_mode: ScanExecutionMode,
    web_focus: Option<ScanType>,
    urls: &'a [String],
    enabled_categories: &'a [String],
    timeout_secs: Option<u64>,
    axe_enabled: bool,
    inspect_local_databases: bool,
    project_path: Option<&'a str>,
    retention: u32,
    trigger: ScanTrigger,
    web_status: Option<ScanComponentStatus>,
    code_status: Option<ScanComponentStatus>,
}

struct ValidatedExecutionPlan {
    project_id: Option<i64>,
    environment_id: Option<i64>,
    environment_url: Option<String>,
    environment_scope_key: String,
    requested_mode: ScanExecutionMode,
    web_focus: Option<ScanType>,
    urls: Vec<String>,
    enabled_categories: Vec<String>,
    timeout_secs: Option<u64>,
    axe_enabled: bool,
    inspect_local_databases: bool,
    project_path: Option<String>,
    retention: u32,
    trigger: ScanTrigger,
    idempotency_key: String,
    web_status: Option<ScanComponentStatus>,
    web_detail: Option<String>,
    code_status: Option<ScanComponentStatus>,
    code_detail: Option<String>,
}

impl ValidatedExecutionPlan {
    fn fingerprint(&self) -> Result<String, String> {
        let payload = FingerprintV1 {
            project_id: self.project_id,
            environment_id: self.environment_id,
            environment_url: self.environment_url.as_deref(),
            environment_scope_key: &self.environment_scope_key,
            requested_mode: self.requested_mode,
            web_focus: self.web_focus,
            urls: &self.urls,
            enabled_categories: &self.enabled_categories,
            timeout_secs: self.timeout_secs,
            axe_enabled: self.axe_enabled,
            inspect_local_databases: self.inspect_local_databases,
            project_path: self.project_path.as_deref(),
            retention: self.retention,
            trigger: self.trigger,
            web_status: self.web_status,
            code_status: self.code_status,
        };
        let bytes = serde_json::to_vec(&payload)
            .map_err(|error| format!("could not fingerprint scan request: {error}"))?;
        let digest = Sha256::digest(bytes);
        Ok(format!("v1:{}", hex::encode(digest)))
    }
}

pub(crate) fn generate_scan_action_key(prefix: &str) -> Result<String, String> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random)
        .map_err(|error| format!("could not create scan action key: {error}"))?;
    Ok(format!("{prefix}:{}", hex::encode(random)))
}

pub(super) fn component_failure_status(message: &str) -> ScanComponentStatus {
    if message.to_ascii_lowercase().contains("cancelled") {
        ScanComponentStatus::Cancelled
    } else {
        ScanComponentStatus::Failed
    }
}

fn incomplete_page_scope_detail(completed_pages: usize, total_pages: usize) -> Option<String> {
    (total_pages > 0 && completed_pages < total_pages)
        .then(|| format!("{completed_pages} of {total_pages} selected pages completed."))
}

fn project_scope_key(project_id: i64) -> String {
    format!("project:{project_id}")
}

async fn validate_plan(
    db: &Arc<Database>,
    request: RunScanExecutionRequest,
) -> Result<ValidatedExecutionPlan, String> {
    let mut enabled_categories = request.enabled_categories.unwrap_or_default();
    enabled_categories.sort();
    enabled_categories.dedup();

    let mut urls = request.urls;
    if urls.is_empty() {
        if let Some(url) = request
            .environment_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
        {
            urls.push(url.to_string());
        }
    }
    let web_requested = request.requested_mode != ScanExecutionMode::Code;
    let code_requested = request.requested_mode != ScanExecutionMode::Web;
    let has_web = web_requested && !urls.is_empty();

    if has_web {
        for url in &urls {
            crate::commands::validate_url_async(url).await?;
        }
    }

    let project_path = if code_requested {
        match request.project_id {
            Some(project_id) => {
                match crate::project_paths::resolve_registered_project_dir(
                    db,
                    project_id,
                    request.project_path.as_deref(),
                ) {
                    Ok(path) => Some(
                        crate::core::code_scan::validate_project_path(&path)
                            .map_err(crate::commands::sanitize_error)?
                            .to_string_lossy()
                            .to_string(),
                    ),
                    Err(error) if request.requested_mode == ScanExecutionMode::Full => {
                        tracing::info!(
                            project_id,
                            "Full execution has no Code capability: {error}"
                        );
                        None
                    }
                    Err(error) => return Err(crate::commands::sanitize_error(error)),
                }
            }
            None if request.requested_mode == ScanExecutionMode::Full => None,
            None => return Err("Code Scan requires a project".into()),
        }
    } else {
        None
    };
    let has_code = code_requested && project_path.is_some();

    if request.requested_mode == ScanExecutionMode::Web && !has_web {
        return Err("Web Scan requires at least one URL".into());
    }
    if request.requested_mode == ScanExecutionMode::Code && !has_code {
        return Err("Code Scan requires a linked source folder".into());
    }
    if !has_web && !has_code {
        return Err("This project has neither a scannable URL nor a linked source folder".into());
    }

    let environment_url = request
        .environment_url
        .filter(|url| !url.trim().is_empty())
        .or_else(|| urls.first().cloned());
    let environment_scope_key = match environment_url.as_deref() {
        Some(url) => normalize_env_url(Some(url)),
        None => project_scope_key(
            request
                .project_id
                .ok_or_else(|| "Code Scan requires a project".to_string())?,
        ),
    };
    let web_status = if web_requested {
        Some(if has_web {
            ScanComponentStatus::Planned
        } else {
            ScanComponentStatus::Skipped
        })
    } else {
        None
    };
    let code_status = if code_requested {
        Some(if has_code {
            ScanComponentStatus::Planned
        } else {
            ScanComponentStatus::Skipped
        })
    } else {
        None
    };

    Ok(ValidatedExecutionPlan {
        project_id: request.project_id,
        environment_id: request.environment_id,
        environment_url,
        environment_scope_key,
        requested_mode: request.requested_mode,
        web_focus: has_web.then_some(request.web_focus.unwrap_or(ScanType::Health)),
        urls,
        enabled_categories,
        timeout_secs: request.timeout_secs,
        axe_enabled: request.axe_enabled.unwrap_or(false),
        inspect_local_databases: request.inspect_local_databases,
        project_path,
        retention: super::policy::scan_retention(request.retention),
        trigger: request.trigger,
        idempotency_key: request.idempotency_key,
        web_status,
        web_detail: (web_requested && !has_web).then(|| "no_environment_url".into()),
        code_status,
        code_detail: (code_requested && !has_code).then(|| "no_source_folder".into()),
    })
}

fn admission_request(
    plan: &ValidatedExecutionPlan,
    request_fingerprint: String,
    admission_class: ScanAdmissionClass,
    now: chrono::DateTime<chrono::Local>,
) -> NewScanExecution {
    NewScanExecution {
        project_id: plan.project_id,
        environment_id: plan.environment_id,
        environment_url: plan.environment_url.clone(),
        environment_scope_key: plan.environment_scope_key.clone(),
        requested_mode: plan.requested_mode,
        web_focus: plan.web_focus,
        trigger: plan.trigger,
        admission_class,
        idempotency_key: plan.idempotency_key.clone(),
        request_fingerprint,
        now_ms: now.timestamp_millis(),
        web_status: plan.web_status,
        web_detail: plan.web_detail.clone(),
        code_status: plan.code_status,
        code_detail: plan.code_detail.clone(),
    }
}

fn apply_execution_retention(db: &Database, plan: &ValidatedExecutionPlan, execution_id: i64) {
    match db.prune_scan_executions_for_scope(
        plan.project_id,
        &plan.environment_scope_key,
        plan.retention,
        crate::db::ScanRetentionWindow::All,
    ) {
        Ok(pruned) if pruned > 0 => tracing::info!(
            execution_id,
            pruned,
            "Pruned old scan executions for the completed scope"
        ),
        Ok(_) => {}
        Err(error) => tracing::warn!(
            execution_id,
            "Failed to apply scan execution retention: {error}"
        ),
    }
}

pub(super) fn load_active_issue_group_ids(
    db: &Database,
    project_id: i64,
    environment_scope_key: &str,
) -> Result<HashSet<String>, crate::db::DbError> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    Ok(db
        .get_active_issue_groups(project_id, Some(environment_scope_key), now_ms)?
        .into_iter()
        .filter(|group| !group.status.is_inactive_for_scoring())
        .map(|group| group.check_id)
        .collect())
}

pub(super) fn build_scan_issue_changes(
    before: &HashSet<String>,
    after: &HashSet<String>,
) -> ScanIssueChanges {
    ScanIssueChanges {
        previous_open_issues: before.len(),
        open_issues: after.len(),
        new_issues: after.difference(before).count(),
        resolved_issues: before.difference(after).count(),
    }
}

#[tracing::instrument(skip(app, db, scan_control, request), fields(mode = %request.requested_mode, trigger = request.trigger.as_str()))]
pub async fn run_scan_execution(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    scan_control: State<'_, ScanControlState>,
    request: RunScanExecutionRequest,
) -> Result<RunScanExecutionResult, String> {
    // The IPC edge is the one place an execution error becomes a string.
    run_scan_execution_internal(app, db.inner().clone(), &scan_control, request)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn run_scan_execution_internal(
    app: AppHandle,
    db: Arc<Database>,
    scan_control: &ScanControlState,
    mut request: RunScanExecutionRequest,
) -> Result<RunScanExecutionResult, ScanExecutionError> {
    request.retention = Some(super::policy::resolve_scan_retention(
        request.retention,
        super::policy::configured_scan_retention(&app),
    ));
    let request_guard = scan_control.begin_execution(request.scan_request_id);
    let scan_request_id = request_guard.request_id();
    let plan = validate_plan(&db, request).await?;
    let fingerprint = plan.fingerprint()?;
    let now = chrono::Local::now();
    let admission = db.admit_scan_execution(
        admission_request(&plan, fingerprint, ScanAdmissionClass::GeneralScan, now),
        crate::constants::SCAN_IDEMPOTENCY_RETRY_WINDOW_SECS,
    )?;
    if admission.reused {
        return Ok(RunScanExecutionResult {
            execution: admission.execution,
            reused: true,
            web_result: None,
            multi_result: None,
            code_result: None,
            issue_changes: None,
        });
    }

    let execution_id = admission.execution.id;
    let mut execution = admission.execution;
    let mut web_result = None;
    let mut multi_result = None;
    let mut code_result = None;
    let issues_before = plan.project_id.and_then(|project_id| {
        match load_active_issue_group_ids(&db, project_id, &plan.environment_scope_key) {
            Ok(issue_ids) => Some(issue_ids),
            Err(error) => {
                tracing::warn!(
                    execution_id,
                    "Could not capture pre-scan issue state: {error}"
                );
                None
            }
        }
    });

    if scan_control.is_cancelled(scan_request_id) {
        execution = db
            .cancel_scan_execution_before_start(
                execution_id,
                "cancelled_by_user".into(),
                chrono::Utc::now().timestamp_millis(),
            )
            .map_err(|error| error.to_string())?;
        apply_execution_retention(&db, &plan, execution_id);
        record_scan_execution_event(&db, &execution);
        emit_scan_execution_completed(&app, &execution, None);
        return Ok(RunScanExecutionResult {
            execution,
            reused: false,
            web_result,
            multi_result,
            code_result,
            issue_changes: None,
        });
    }

    if plan.web_status == Some(ScanComponentStatus::Planned) {
        db.start_scan_execution_component(execution_id, ScanComponent::Web)
            .map_err(|error| error.to_string())?;
        let web_outcome = if plan.urls.len() > 1 {
            super::multi_scan::scan_multi_for_execution(
                app.clone(),
                db.clone(),
                scan_control,
                plan.urls.clone(),
                plan.environment_url.clone(),
                plan.project_id,
                (!plan.enabled_categories.is_empty()).then_some(plan.enabled_categories.clone()),
                plan.timeout_secs,
                Some(plan.axe_enabled),
                plan.web_focus,
                scan_request_id,
                execution_id,
            )
            .await
            .map(|result| {
                let detail = result.incomplete_detail.clone().or_else(|| {
                    incomplete_page_scope_detail(result.completed_pages, result.total_pages)
                });
                multi_result = Some(result);
                detail
            })
            .map_err(|error| error.to_string())
        } else {
            super::web_scan::scan_url_for_execution(
                app.clone(),
                db.clone(),
                scan_control,
                plan.urls[0].clone(),
                plan.environment_url.clone(),
                plan.project_id,
                (!plan.enabled_categories.is_empty()).then_some(plan.enabled_categories.clone()),
                plan.timeout_secs,
                Some(plan.axe_enabled),
                plan.web_focus,
                scan_request_id,
                execution_id,
            )
            .await
            .map(|output| {
                web_result = Some(output.result);
                output.incomplete_detail
            })
            .map_err(|error| error.to_string())
        };
        let (status, detail) = match web_outcome {
            Ok(detail) => (ScanComponentStatus::Complete, detail),
            Err(error) => (component_failure_status(&error), Some(error)),
        };
        execution = db
            .finish_scan_execution_component(
                execution_id,
                ScanComponent::Web,
                status,
                detail,
                chrono::Utc::now().timestamp_millis(),
            )
            .map_err(|error| error.to_string())?;
    }

    if plan.code_status == Some(ScanComponentStatus::Planned) {
        let project_id = plan
            .project_id
            .ok_or_else(|| "Code execution plan is missing its project".to_string())?;
        let (status, detail) = if scan_control.is_cancelled(scan_request_id) {
            (
                ScanComponentStatus::Cancelled,
                Some("cancelled_by_user".to_string()),
            )
        } else {
            db.start_scan_execution_component(execution_id, ScanComponent::Code)
                .map_err(|error| error.to_string())?;
            let code_outcome = super::code_scan::run_code_scan_internal(
                app.clone(),
                db.clone(),
                scan_control,
                project_id,
                plan.project_path.as_deref(),
                plan.environment_url.clone(),
                plan.environment_scope_key.clone(),
                plan.inspect_local_databases,
                scan_request_id,
                execution_id,
            )
            .await;
            match code_outcome {
                Ok(result) => {
                    code_result = Some(result);
                    (ScanComponentStatus::Complete, None)
                }
                Err(error) => {
                    let detail = error.to_string();
                    (component_failure_status(&detail), Some(detail))
                }
            }
        };
        execution = db
            .finish_scan_execution_component(
                execution_id,
                ScanComponent::Code,
                status,
                detail,
                chrono::Utc::now().timestamp_millis(),
            )
            .map_err(|error| error.to_string())?;
    }

    let has_completed_child = execution.web_status == Some(ScanComponentStatus::Complete)
        || execution.code_status == Some(ScanComponentStatus::Complete);
    let issue_changes = match (has_completed_child, plan.project_id, issues_before) {
        (true, Some(project_id), Some(before)) => {
            match load_active_issue_group_ids(&db, project_id, &plan.environment_scope_key) {
                Ok(after) => Some(build_scan_issue_changes(&before, &after)),
                Err(error) => {
                    tracing::warn!(
                        execution_id,
                        "Could not capture post-scan issue state: {error}"
                    );
                    None
                }
            }
        }
        _ => None,
    };
    if has_completed_child {
        if let Some(project_id) = plan.project_id {
            match crate::commands::issues::compute_and_record_current_score(
                &db,
                project_id,
                plan.environment_url.as_deref(),
                chrono::Utc::now().timestamp_millis(),
            ) {
                Ok(_) => {
                    execution = db
                        .link_scan_execution_score_snapshot(
                            execution_id,
                            project_id,
                            plan.environment_url.as_deref(),
                        )
                        .map_err(|error| error.to_string())?;
                }
                Err(error) => tracing::warn!(
                    execution_id,
                    "Execution completed but its SiteCMD Score snapshot could not be linked: {}",
                    error
                ),
            }
        }
    }

    apply_execution_retention(&db, &plan, execution_id);

    record_scan_execution_event(&db, &execution);
    if has_completed_child {
        if let Some(project_id) = execution.project_id {
            crate::commands::emit_site_score_changed(&app, project_id);
        }
    }
    emit_scan_execution_completed(
        &app,
        &execution,
        code_result.as_ref().map(|result| result.id),
    );

    Ok(RunScanExecutionResult {
        execution,
        reused: false,
        web_result,
        multi_result,
        code_result,
        issue_changes,
    })
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
