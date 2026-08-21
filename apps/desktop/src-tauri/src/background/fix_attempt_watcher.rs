//! Background verification of agent fix attempts. External agents cannot mark
//! their own attempts verified.

use std::sync::Arc;

use tauri::AppHandle;

use crate::commands::issue_source_capabilities::{issue_source_capability, IssueVerifyStrategy};
use crate::commands::issues::verify_issue_sources_for_check;
use crate::commands::{
    emit_event, emit_site_score_changed, send_actionable_desktop_notification,
    ActionableDesktopNotificationRequest,
};
use crate::constants::FIX_ATTEMPT_POLL_INTERVAL;
use crate::core::types_work_items::VerifiedBy;
use crate::db::{Database, FixAttemptRow, IssueLifecycle, FIX_ATTEMPT_EXPIRY_MS};

/// Integration sources (psi, gsc,...) verify via a queued out-of-band poll,
/// so an active issue is not a failure until this grace window has passed
/// since verification started.
pub const INTEGRATION_GRACE_MS: i64 = 10 * 60 * 1000;

/// How long a remote-environment web attempt waits for a deploy before
/// failing for real. Matches the attempt expiry window so an abandoned wait
/// and an abandoned attempt mean the same thing.
pub const REMOTE_WEB_DEPLOY_WAIT_MS: i64 = FIX_ATTEMPT_EXPIRY_MS;

/// While awaiting a deploy, re-run the live check this often. Each recheck
/// touches the attempt so the stale-expiry sweeper leaves it alone for the
/// duration of the wait.
pub const DEPLOY_RECHECK_INTERVAL_MS: i64 = 10 * 60 * 1000;

const STILL_FAILING_DETAIL: &str =
    "SiteCMD re-ran the check after the agent finished and it is still failing.";

/// Whether verification must wait for deployment to a remote environment.
pub fn is_remote_web_attempt(check_id: &str, producer_rule: Option<&str>, env_url: &str) -> bool {
    producer_rule.is_none()
        && !check_id.starts_with("code_scan.")
        && url::Url::parse(env_url)
            .map(|parsed| !crate::core::localhost::is_localhost(&parsed))
            .unwrap_or(false)
}

/// Build retry detail with deploy context or the exact code location when available.
fn still_failing_detail(attempt: &FixAttemptRow) -> String {
    if is_remote_web_attempt(
        &attempt.check_id,
        attempt.producer_rule.as_deref(),
        &attempt.env_url,
    ) {
        return format!(
            "SiteCMD kept re-checking {} and the check never passed. If your \
             agent changed source files, the fix is not live until you deploy. \
             Deploy, then try again and SiteCMD will verify it.",
            attempt.env_url
        );
    }
    match (
        attempt.producer_rule.as_deref(),
        attempt.target_relative_path.as_deref(),
    ) {
        (Some(rule), Some(path)) => format!(
            "SiteCMD re-ran the code scan after the agent finished and {path} \
             still fails the {rule} check."
        ),
        _ => STILL_FAILING_DETAIL.to_string(),
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Outcome {
    Verified,
    Failed,
}

/// Decide verification from issue state, source timing, and deployment grace.
pub fn decide_outcome(
    issue_still_active: bool,
    sources_settle_immediately: bool,
    remote_web: bool,
    verify_started_ms: i64,
    now_ms: i64,
) -> Option<Outcome> {
    if !issue_still_active {
        return Some(Outcome::Verified);
    }
    if remote_web {
        if now_ms - verify_started_ms > REMOTE_WEB_DEPLOY_WAIT_MS {
            return Some(Outcome::Failed);
        }
        return None;
    }
    if sources_settle_immediately {
        return Some(Outcome::Failed);
    }
    if now_ms - verify_started_ms > INTEGRATION_GRACE_MS {
        return Some(Outcome::Failed);
    }
    None
}

/// Extracts a title from the fix-brief heading.
pub fn issue_title_from_brief(brief_md: &str) -> Option<String> {
    let title = brief_md
        .lines()
        .next()?
        .strip_prefix("# SiteCMD Fix Brief: ")?
        .trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

/// Whether every source verifies inline without an integration grace window.
pub fn sources_settle_immediately(sources: &[String]) -> bool {
    sources.iter().all(|source| {
        matches!(
            issue_source_capability(source).map(|capability| capability.verify),
            Some(IssueVerifyStrategy::WebScan) | Some(IssueVerifyStrategy::CodeScan)
        )
    })
}

fn emit_changed(app: &AppHandle) {
    emit_event(app, "fix-attempt-updated", ());
}

/// Long-running loop: tick every `FIX_ATTEMPT_POLL_INTERVAL`. Spawned from
/// `lib.rs` under `supervised_loop_async` so a panicking tick restarts with
/// backoff.
pub async fn run(db: Arc<Database>, app: AppHandle) {
    loop {
        tick(&db, &app).await;
        tokio::time::sleep(FIX_ATTEMPT_POLL_INTERVAL).await;
    }
}

async fn tick(db: &Arc<Database>, app: &AppHandle) {
    let now = now_ms();

    match db.expire_stale_fix_attempts(now - FIX_ATTEMPT_EXPIRY_MS, now) {
        Ok(expired) if expired > 0 => emit_changed(app),
        Ok(_) => {}
        Err(e) => tracing::warn!("fix attempt watcher: expire stale attempts: {e}"),
    }

    // Coalesce whole-repository code verification across attempts in this tick.
    let mut code_scan_dedup = std::collections::HashSet::new();
    for attempt in list_attempts(db, "verify_requested") {
        start_verification(db, app, &attempt, &mut code_scan_dedup).await;
    }

    // Settle after starting verification so inline outcomes resolve in this tick.
    for attempt in list_attempts(db, "verifying") {
        settle_attempt(db, app, &attempt).await;
    }
}

fn list_attempts(db: &Database, status: &'static str) -> Vec<FixAttemptRow> {
    db.list_fix_attempts_in_status(&[status])
        .unwrap_or_else(|e| {
            tracing::warn!("fix attempt watcher: list '{status}' attempts: {e}");
            Vec::new()
        })
}

/// Move one `verify_requested` attempt to `verifying` (stamping
/// verify_started_at) and trigger verification for the issue's sources. A
/// trigger error fails the attempt rather than leaving it stuck.
async fn start_verification(
    db: &Arc<Database>,
    app: &AppHandle,
    attempt: &FixAttemptRow,
    code_scan_dedup: &mut std::collections::HashSet<(i64, String)>,
) {
    if let Err(e) = db.set_fix_attempt_status(attempt.id, "verifying", None, None, now_ms()) {
        tracing::warn!(
            "fix attempt watcher: start verifying attempt {}: {e}",
            attempt.id
        );
        return;
    }
    emit_changed(app);

    let triggered = verify_issue_sources_for_check(
        app,
        db.clone(),
        attempt.project_id,
        &attempt.env_url,
        &attempt.check_id,
        code_scan_dedup,
    )
    .await;
    if let Err(e) = triggered {
        let detail = format!("Verification could not run: {e}");
        if let Err(e) =
            db.set_fix_attempt_status(attempt.id, "verify_failed", None, Some(&detail), now_ms())
        {
            tracing::warn!("fix attempt watcher: fail attempt {}: {e}", attempt.id);
        }
        emit_changed(app);
    }
}

/// Settle a decidable attempt, updating lifecycle state and notifying on success.
async fn settle_attempt(db: &Arc<Database>, app: &AppHandle, attempt: &FixAttemptRow) {
    let now = now_ms();
    let outcome = match evaluate_attempt(db, attempt, now) {
        Ok(outcome) => outcome,
        Err(e) => {
            tracing::warn!("fix attempt watcher: evaluate attempt {}: {e}", attempt.id);
            return;
        }
    };
    match outcome {
        None => {
            // Periodic live rechecks verify deployed fixes and keep the attempt
            // ahead of stale expiry; trigger failures retry next interval.
            if is_remote_web_attempt(
                &attempt.check_id,
                attempt.producer_rule.as_deref(),
                &attempt.env_url,
            ) && now - attempt.updated_at >= DEPLOY_RECHECK_INTERVAL_MS
            {
                if let Err(e) = db.touch_fix_attempt(attempt.id, now) {
                    tracing::warn!("fix attempt watcher: touch attempt {}: {e}", attempt.id);
                }
                // Deploy rechecks are remote web attempts (never code scans),
                // so a throwaway dedup set is fine here.
                let mut code_scan_dedup = std::collections::HashSet::new();
                if let Err(e) = verify_issue_sources_for_check(
                    app,
                    db.clone(),
                    attempt.project_id,
                    &attempt.env_url,
                    &attempt.check_id,
                    &mut code_scan_dedup,
                )
                .await
                {
                    tracing::warn!(
                        "fix attempt watcher: deploy recheck for attempt {}: {e}",
                        attempt.id
                    );
                }
            }
        }
        Some(Outcome::Verified) => {
            if let Err(e) = db.set_fix_attempt_status(attempt.id, "verified", None, None, now) {
                tracing::warn!("fix attempt watcher: verify attempt {}: {e}", attempt.id);
                return;
            }
            if let Err(e) = db.set_issue_group_state(
                attempt.project_id,
                &attempt.env_url,
                &attempt.check_id,
                IssueLifecycle::Verified {
                    by: VerifiedBy::LocalScan,
                },
                now,
            ) {
                tracing::warn!(
                    "fix attempt watcher: set issue state for attempt {}: {e}",
                    attempt.id
                );
            }
            emit_site_score_changed(app, attempt.project_id);
            emit_changed(app);
            notify_verified(app, attempt).await;
        }
        Some(Outcome::Failed) => {
            let detail = still_failing_detail(attempt);
            if let Err(e) =
                db.set_fix_attempt_status(attempt.id, "verify_failed", None, Some(&detail), now)
            {
                tracing::warn!("fix attempt watcher: fail attempt {}: {e}", attempt.id);
                return;
            }
            emit_changed(app);
        }
    }
}

/// Best-effort desktop notification for a verified fix, reusing the same
/// delivery path as the frontend's `send_actionable_desktop_notification`
/// command (mac_notification_sys on macOS, the notification plugin elsewhere).
/// A delivery failure is logged and never blocks settling.
async fn notify_verified(app: &AppHandle, attempt: &FixAttemptRow) {
    let body =
        issue_title_from_brief(&attempt.brief_md).unwrap_or_else(|| attempt.check_id.clone());
    let request = ActionableDesktopNotificationRequest {
        id: None,
        title: "Issue fixed and verified".to_string(),
        body,
        click_target: None,
        actions: Vec::new(),
    };
    if let Err(e) = send_actionable_desktop_notification(app.clone(), request).await {
        tracing::warn!(
            "fix attempt watcher: desktop notification for attempt {}: {e}",
            attempt.id
        );
    }
}

/// Gather active-state and source-settlement inputs for `decide_outcome`.
/// Code occurrences match by canonical group and stable relative path.
fn evaluate_attempt(
    db: &Database,
    attempt: &FixAttemptRow,
    now: i64,
) -> Result<Option<Outcome>, String> {
    let issue_still_active = db.is_fix_attempt_target_active(
        attempt.project_id,
        &attempt.env_url,
        &attempt.check_id,
        &attempt.target_kind,
        attempt.target_relative_path.as_deref(),
        attempt.target_line,
    )?;
    if !issue_still_active {
        // An inactive issue is verified regardless of sources, so skip even
        // the targeted source lookup below.
        return Ok(Some(Outcome::Verified));
    }
    // Poll only distinct producer sources; full dossier enrichment is unnecessary.
    let sources =
        db.get_active_issue_sources(attempt.project_id, &attempt.env_url, &attempt.check_id)?;
    let verify_started = attempt.verify_started_at.unwrap_or(attempt.updated_at);
    Ok(decide_outcome(
        issue_still_active,
        sources_settle_immediately(&sources),
        is_remote_web_attempt(
            &attempt.check_id,
            attempt.producer_rule.as_deref(),
            &attempt.env_url,
        ),
        verify_started,
        now,
    ))
}

#[cfg(test)]
#[path = "fix_attempt_watcher_tests.rs"]
mod tests;
