//! Desktop connected-service workflow and read-only sync inspector.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::Serialize;
use sha2::{Digest, Sha256};
use sitecmd_engine::sync::{ClientGroupState, DismissalPolicy};
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::connected_service::{ConnectedServiceClient, ConnectedSiteState, GroupMutationEntry};
use crate::connected_workflow::proposed_submission_sequence;
use crate::db::{
    ConnectedSite, ConnectedSubmissionRequest, Database, GroupDecision, PendingMutation,
    PendingRotation,
};

use super::{run_blocking, sanitize_error};

const MAX_GROUP_PAGES: usize = 10;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedStatus {
    pub endpoint_configured: bool,
    pub connected: bool,
    pub site_id: Option<String>,
    pub bootstrapped: bool,
    pub has_installation_token: bool,
    pub has_fingerprint_key: bool,
    pub pending_mutations: usize,
    pub conflicted_mutations: usize,
    /// A local scope revision exists that the connected resource has not yet
    /// acknowledged. The payload is the durable local scope itself.
    pub pending_scope_sync: bool,
    pub last_submission_sequence: i64,
    /// The epoch of this desktop's stored fingerprint key.
    pub fingerprint_key_version: i64,
    /// The version this desktop has claimed but not completed, if any.
    pub pending_key_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedInspection {
    pub payload: String,
    pub connected: bool,
    pub includes_bootstrap: bool,
    pub proposed_submission_sequence: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedSyncResult {
    pub submission_sequence: i64,
    pub event_sequence: i64,
    pub groups_pulled: usize,
    pub mutations_settled: usize,
    pub mutation_conflicts: usize,
    /// Completed rotation key version, signalling that CI must receive the new key.
    pub key_rotation_completed: Option<i64>,
    /// True when sync succeeded but scope delivery remains queued.
    pub scope_delivery_pending: bool,
}

/// What a user has to publish, in the form they will copy. Both routes are
/// returned because domain control and web-server control are different powers
/// and a customer usually has exactly one of them.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedSiteChallenge {
    pub site_id: String,
    pub url: String,
    pub phase: String,
    pub challenge: String,
    pub dns_name: String,
    pub dns_type: String,
    pub well_known_path: String,
}

/// What the service says about a site this desktop is bound to.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedRemoteState {
    pub site_id: String,
    pub phase: String,
    pub event_sequence: i64,
    pub challenge: Option<ConnectedSiteChallenge>,
    pub scope_revision: Option<i64>,
    pub scope_routes: Vec<String>,
    pub scope_effective_route_count: i64,
    pub scope_route_cap: i64,
    pub scope_over_plan: bool,
    pub scope_over_plan_grace_expires_at: Option<String>,
    pub scope_overflow_count: i64,
    pub site_allowance_over_plan: bool,
    pub site_allowance_over_plan_grace_expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedVerification {
    pub phase: String,
    pub verified: bool,
}

/// A minted CI secret. The `token` field is readable exactly once, here, and
/// the desktop deliberately keeps no copy.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedCiToken {
    pub id: String,
    pub site_id: String,
    pub token: String,
    pub repository: Option<String>,
    pub repository_id: Option<String>,
    pub ordering_authority_id: Option<String>,
    pub ordering_authority_epoch: Option<i64>,
}

#[derive(Debug)]
struct PulledState {
    state: ConnectedSiteState,
    groups: usize,
}

fn sync_idempotency_key(installation_id: &str, sequence: i64) -> String {
    let mut digest = Sha256::new();
    digest.update(b"sitecmd-desktop-sync-v1\0");
    digest.update(installation_id.as_bytes());
    digest.update(b"\0");
    digest.update(sequence.to_string().as_bytes());
    format!("sync_{}", hex::encode(digest.finalize()))
}

fn mutation_entry(pending: &PendingMutation) -> GroupMutationEntry {
    let (state, dismissal) = match &pending.decision {
        GroupDecision::Reopen => (ClientGroupState::Active, None),
        GroupDecision::Snooze { until } => (
            ClientGroupState::Dismissed,
            Some(DismissalPolicy::Snoozed { until: *until }),
        ),
        GroupDecision::Ignore => (
            ClientGroupState::Dismissed,
            Some(DismissalPolicy::Ignored {
                reopen_on_reobservation: true,
            }),
        ),
        GroupDecision::Block { reason } => (
            ClientGroupState::Dismissed,
            Some(DismissalPolicy::Blocked {
                reason: reason.clone(),
            }),
        ),
        GroupDecision::ClaimFixed => (ClientGroupState::ClaimedFixed, None),
    };
    GroupMutationEntry {
        check: pending.check_id.clone(),
        based_on_revision: pending.based_on_revision,
        state,
        dismissal,
    }
}

async fn pull_remote_state(
    client: &ConnectedServiceClient,
    db: &Arc<Database>,
    project_id: i64,
    environment_scope_key: &str,
    site_id: &str,
    now_ms: i64,
) -> Result<PulledState, String> {
    let state = client.state(site_id).await.map_err(sanitize_error)?;
    if state.phase.trim().is_empty() || state.event_sequence < 0 || state.state_revision < 0 {
        return Err("connected service returned an invalid site state".into());
    }

    let mut cursor = None;
    let mut seen_cursors = BTreeSet::new();
    let mut revisions = Vec::new();
    for _ in 0..MAX_GROUP_PAGES {
        let page = client
            .groups(site_id, cursor.as_deref())
            .await
            .map_err(sanitize_error)?;
        for group in page.items {
            if group.check.trim().is_empty() || group.state_revision < 0 {
                return Err("connected service returned an invalid group revision".into());
            }
            revisions.push((group.check, group.state_revision));
        }
        let Some(next) = page.next_cursor.filter(|value| !value.is_empty()) else {
            let groups = revisions.len();
            let db_for_pull = Arc::clone(db);
            let env_for_pull = environment_scope_key.to_string();
            let event_sequence = state.event_sequence;
            run_blocking(move || {
                db_for_pull.record_connected_pull(
                    project_id,
                    &env_for_pull,
                    event_sequence,
                    revisions,
                    now_ms,
                )
            })
            .await?
            .map_err(sanitize_error)?;
            return Ok(PulledState { state, groups });
        };
        if !seen_cursors.insert(next.clone()) {
            return Err("connected service repeated a group cursor".into());
        }
        cursor = Some(next);
    }
    Err("connected service returned too many group pages".into())
}

#[tauri::command]
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn get_connected_status(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<ConnectedStatus, String> {
    let db_read = Arc::clone(&db);
    let env_read = environment_scope_key.clone();
    let (site, pending, producer, pending_scope_sync) = run_blocking(move || {
        let site = db_read.get_connected_site(project_id, &env_read)?;
        let pending = db_read.pending_group_mutations(project_id, &env_read)?;
        let producer = db_read.get_producer_identity()?;
        let pending_scope_sync = db_read.connected_scan_scope_pending(project_id, &env_read)?;
        Ok::<_, crate::db::DbError>((site, pending, producer, pending_scope_sync))
    })
    .await?
    .map_err(sanitize_error)?;
    let has_installation_token = crate::keyring::get_connected_installation_token(&app)
        .map_err(sanitize_error)?
        .is_some();
    let has_fingerprint_key = match site.as_ref() {
        Some(site) => {
            crate::keyring::get_project_fingerprint_key(&app, &db, project_id, &site.site_id)
                .map_err(sanitize_error)?
                .is_some()
        }
        None => false,
    };
    Ok(ConnectedStatus {
        endpoint_configured: crate::connected_service::is_configured(),
        connected: site.is_some(),
        site_id: site.as_ref().map(|site| site.site_id.clone()),
        bootstrapped: site
            .as_ref()
            .is_some_and(|site| site.bootstrapped_at.is_some()),
        has_installation_token,
        has_fingerprint_key,
        pending_mutations: pending.len(),
        conflicted_mutations: pending
            .iter()
            .filter(|mutation| mutation.conflict.is_some())
            .count(),
        pending_scope_sync,
        last_submission_sequence: producer
            .map(|identity| identity.last_submission_sequence)
            .unwrap_or(0),
        fingerprint_key_version: site
            .as_ref()
            .map(|site| site.fingerprint_key_version)
            .unwrap_or(1),
        pending_key_version: site.as_ref().and_then(|site| site.pending_key_version),
    })
}

#[tauri::command]
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn inspect_connected_sync(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<ConnectedInspection, String> {
    let db_read = Arc::clone(&db);
    let env_read = environment_scope_key.clone();
    let (site, producer) = run_blocking(move || {
        Ok::<_, crate::db::DbError>((
            db_read.get_connected_site(project_id, &env_read)?,
            db_read.get_producer_identity()?,
        ))
    })
    .await?
    .map_err(sanitize_error)?;
    let connected = site.is_some();
    let site_id = site
        .as_ref()
        .map(|site| site.site_id.clone())
        .unwrap_or_else(|| "site_pending_connection".into());
    let includes_bootstrap = site
        .as_ref()
        .is_none_or(|site| site.bootstrapped_at.is_none());
    let proposed_submission_sequence = proposed_submission_sequence(producer.as_ref())?;
    let fingerprint_key = if connected {
        Some(
            crate::keyring::get_project_fingerprint_key(&app, &db, project_id, &site_id)
                .map_err(sanitize_error)?
                .ok_or_else(|| "the project fingerprint key is missing".to_string())?,
        )
    } else {
        None
    };
    // The preview reflects the epoch a sync would actually use, pending
    // rotation included: an inspector that showed different numbers than the
    // wire would defeat its purpose.
    let fingerprint_key_version = site
        .as_ref()
        .map(|site| site.fingerprint_key_version as u16)
        .unwrap_or(1);
    let pending_rotation = match site.as_ref() {
        Some(site) => pending_rotation_for(&app, &db, project_id, site)?,
        None => None,
    };
    let db_build = Arc::clone(&db);
    let env_build = environment_scope_key;
    let submission = run_blocking(move || {
        db_build
            .build_connected_submission(
                project_id,
                &env_build,
                ConnectedSubmissionRequest {
                    site_id,
                    submission_sequence: proposed_submission_sequence,
                    include_groups: includes_bootstrap,
                    fingerprint_key,
                    fingerprint_key_version,
                    pending_rotation,
                    deployed_commit: None,
                },
            )
            .map_err(sanitize_error)
    })
    .await??;
    let payload = submission.render_for_inspection().map_err(sanitize_error)?;
    Ok(ConnectedInspection {
        payload,
        connected,
        includes_bootstrap,
        proposed_submission_sequence,
    })
}

/// Return a rotation only when both its claim and candidate key exist.
fn pending_rotation_for(
    app: &AppHandle,
    db: &Database,
    project_id: i64,
    site: &ConnectedSite,
) -> Result<Option<PendingRotation>, String> {
    let Some(version) = site.pending_key_version else {
        return Ok(None);
    };
    let version = u16::try_from(version)
        .map_err(|_| "the pending key version is out of range".to_string())?;
    Ok(
        crate::keyring::get_pending_fingerprint_key(app, db, project_id, &site.site_id)
            .map_err(sanitize_error)?
            .map(|key| PendingRotation { key, version }),
    )
}

/// Protected by the external-connector broker: this method transmits the same
/// public wire type and serialization exposed by the inspector, then drains
/// user lifecycle intent.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn sync_connected_site(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<ConnectedSyncResult, String> {
    let db_binding = Arc::clone(&db);
    let env_binding = environment_scope_key.clone();
    let mut site = run_blocking(move || db_binding.get_connected_site(project_id, &env_binding))
        .await?
        .map_err(sanitize_error)?
        .ok_or_else(|| "this environment is not connected".to_string())?;
    let token = crate::keyring::get_connected_installation_token(&app)
        .map_err(sanitize_error)?
        .ok_or_else(|| "the connected installation token is missing".to_string())?;
    let fingerprint_key =
        crate::keyring::get_project_fingerprint_key(&app, &db, project_id, &site.site_id)
            .map_err(sanitize_error)?
            .ok_or_else(|| "the project fingerprint key is missing".to_string())?;
    let client = ConnectedServiceClient::configured(&token).map_err(sanitize_error)?;
    let now_ms = chrono::Utc::now().timestamp_millis();

    let first_pull = pull_remote_state(
        &client,
        &db,
        project_id,
        &environment_scope_key,
        &site.site_id,
        now_ms,
    )
    .await?;
    match first_pull.state.phase.as_str() {
        "connected" if site.bootstrapped_at.is_none() => {
            let db_recover = Arc::clone(&db);
            let env_recover = environment_scope_key.clone();
            run_blocking(move || {
                db_recover.mark_site_bootstrapped(project_id, &env_recover, now_ms)
            })
            .await?
            .map_err(sanitize_error)?;
            site.bootstrapped_at = Some(now_ms);
        }
        "connected" | "pending_bootstrap" => {}
        phase => {
            return Err(format!(
                "connected site cannot sync while its phase is {phase}"
            ));
        }
    }
    if first_pull.state.phase == "pending_bootstrap" && site.bootstrapped_at.is_some() {
        return Err(
            "local and connected bootstrap state disagree; reconnect before syncing".into(),
        );
    }

    let deployed_commit = first_pull
        .state
        .current_deployment
        .as_ref()
        .and_then(|deployment| deployment.commit_sha.clone());
    let db_ticket = Arc::clone(&db);
    let ticket = run_blocking(move || db_ticket.allocate_submission_sequence(now_ms))
        .await?
        .map_err(sanitize_error)?;
    let sequence = ticket.sequence();
    let idempotency_key = sync_idempotency_key(ticket.installation_id(), sequence);
    let include_groups = site.bootstrapped_at.is_none();
    let site_id = site.site_id.clone();
    let fingerprint_key_version = site.fingerprint_key_version as u16;
    let pending_rotation = pending_rotation_for(&app, &db, project_id, &site)?;
    let db_build = Arc::clone(&db);
    let env_build = environment_scope_key.clone();
    let submission = run_blocking(move || {
        db_build
            .build_connected_submission(
                project_id,
                &env_build,
                ConnectedSubmissionRequest {
                    site_id,
                    submission_sequence: sequence,
                    include_groups,
                    fingerprint_key: Some(fingerprint_key),
                    fingerprint_key_version,
                    pending_rotation,
                    deployed_commit,
                },
            )
            .map_err(sanitize_error)
    })
    .await??;
    // Bootstrap scope from observed routes, or the entry route without web evidence.
    // A concurrent installation winning the revision race satisfies the requirement.
    if include_groups && first_pull.state.scope.is_none() {
        let routes =
            crate::connected_workflow::initial_scope_routes(&submission, &environment_scope_key);
        match client.put_scope(&site.site_id, 0, &routes, &[]).await {
            Ok(_) => {}
            Err(error) if error.is_stale_revision() => {}
            Err(error) => return Err(sanitize_error(error)),
        }
    }
    let receipt = client
        .sync_desktop(&site.site_id, &idempotency_key, &submission)
        .await
        .map_err(sanitize_error)?;
    if receipt.event_sequence < 0 || receipt.state_revision < 0 {
        return Err("connected service returned an invalid sync receipt".into());
    }
    // Accepted code evidence completes a matching pending key rotation.
    let key_rotation_completed = submission
        .snapshots
        .code
        .as_ref()
        .map(|code| i64::from(code.versions.fingerprint_key_version))
        .filter(|version| site.pending_key_version == Some(*version));
    if let Some(version) = key_rotation_completed {
        crate::keyring::promote_pending_fingerprint_key(&app, &db, project_id, &site.site_id)
            .map_err(sanitize_error)?;
        let db_promote = Arc::clone(&db);
        let env_promote = environment_scope_key.clone();
        run_blocking(move || db_promote.complete_key_rotation(project_id, &env_promote, version))
            .await?
            .map_err(sanitize_error)?;
        crate::audit_log::record(
            "connect.key_rotation_complete",
            serde_json::json!({ "site": site.site_id, "version": version }),
            "ok",
        );
    }
    if include_groups {
        let db_bootstrap = Arc::clone(&db);
        let env_bootstrap = environment_scope_key.clone();
        run_blocking(move || {
            db_bootstrap.mark_site_bootstrapped(project_id, &env_bootstrap, now_ms)
        })
        .await?
        .map_err(sanitize_error)?;
    }
    let mut event_sequence = receipt.event_sequence;
    if event_sequence > 0 {
        let db_receipt = Arc::clone(&db);
        let env_receipt = environment_scope_key.clone();
        run_blocking(move || {
            db_receipt.record_pulled_event_sequence(
                project_id,
                &env_receipt,
                event_sequence,
                now_ms,
            )
        })
        .await?
        .map_err(sanitize_error)?;
    }

    let db_pending = Arc::clone(&db);
    let env_pending = environment_scope_key.clone();
    let pending =
        run_blocking(move || db_pending.pending_group_mutations(project_id, &env_pending))
            .await?
            .map_err(sanitize_error)?;
    let mut mutations_settled = 0;
    let mut mutation_conflicts = 0;
    for mutation in pending
        .into_iter()
        .filter(|mutation| mutation.conflict.is_none())
    {
        let entry = mutation_entry(&mutation);
        match client
            .mutate_group(&site.site_id, &mutation.idempotency_key, &entry)
            .await
        {
            Ok(mutation_receipt) => {
                if mutation_receipt.event_sequence < 0 || mutation_receipt.state_revision < 0 {
                    return Err("connected service returned an invalid mutation receipt".into());
                }
                event_sequence = event_sequence.max(mutation_receipt.event_sequence);
                let db_settle = Arc::clone(&db);
                let key = mutation.idempotency_key.clone();
                let mutation_id = mutation.id;
                run_blocking(move || {
                    db_settle.settle_group_mutation(
                        mutation_id,
                        &key,
                        mutation_receipt.state_revision,
                        now_ms,
                    )
                })
                .await?
                .map_err(sanitize_error)?;
                mutations_settled += 1;
            }
            Err(error) if error.is_stale_revision() => {
                let conflicts = error.stale_groups(&mutation.check_id);
                let Some(conflict) = conflicts
                    .into_iter()
                    .find(|conflict| conflict.check == mutation.check_id)
                else {
                    return Err("stale mutation response omitted the conflicting group".into());
                };
                let db_conflict = Arc::clone(&db);
                let key = mutation.idempotency_key.clone();
                let mutation_id = mutation.id;
                run_blocking(move || {
                    db_conflict.record_mutation_conflict(
                        mutation_id,
                        &key,
                        &conflict.state,
                        conflict.revision,
                        now_ms,
                    )
                })
                .await?
                .map_err(sanitize_error)?;
                mutation_conflicts += 1;
            }
            Err(error) => return Err(sanitize_error(error)),
        }
    }

    let final_pull = pull_remote_state(
        &client,
        &db,
        project_id,
        &environment_scope_key,
        &site.site_id,
        now_ms,
    )
    .await?;
    event_sequence = event_sequence.max(final_pull.state.event_sequence);

    let scope_delivery_pending =
        deliver_pending_scope(&app, &db, project_id, &environment_scope_key).await;

    Ok(ConnectedSyncResult {
        submission_sequence: sequence,
        event_sequence,
        groups_pulled: first_pull.groups + final_pull.groups,
        mutations_settled,
        mutation_conflicts,
        key_rotation_completed,
        scope_delivery_pending,
    })
}

/// Deliver scope after evidence acceptance without invalidating a completed sync.
async fn deliver_pending_scope<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Arc<Database>,
    project_id: i64,
    environment_scope_key: &str,
) -> bool {
    match try_deliver_pending_scope(app, db, project_id, environment_scope_key).await {
        Ok(pending) => pending,
        Err(error) => {
            tracing::warn!(
                project_id,
                cause = %crate::log_sanitizer::bounded_issue_evidence(&error),
                "Connected scope delivery left pending by an otherwise applied sync"
            );
            true
        }
    }
}

/// Deliver a pending scope revision, skipping network work when already current.
async fn try_deliver_pending_scope<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Arc<Database>,
    project_id: i64,
    environment_scope_key: &str,
) -> Result<bool, String> {
    let db_scope = Arc::clone(db);
    let env_scope = environment_scope_key.to_string();
    let pending_scope = run_blocking(move || {
        if !db_scope.connected_scan_scope_pending(project_id, &env_scope)? {
            return Ok::<_, crate::db::DbError>(None);
        }
        db_scope
            .get_or_create_site_for_project(project_id, &env_scope)
            .map(Some)
    })
    .await?
    .map_err(sanitize_error)?;
    let Some(local_site_id) = pending_scope else {
        return Ok(false);
    };
    let delivered =
        super::scan_scope::sync_connected_scan_scope_for_site(app, db, local_site_id).await?;
    Ok(!delivered.synced)
}

#[cfg(test)]
#[path = "connected_tests.rs"]
mod tests;
