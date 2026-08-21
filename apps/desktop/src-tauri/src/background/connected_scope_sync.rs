//! Retries connected scan-scope updates until their revisions are acknowledged.
//!
//! Missing credentials are reported once per state transition; delivery failures
//! use bounded backoff.

use std::collections::BTreeMap;
use std::sync::Arc;

use tauri::AppHandle;

use crate::constants::CONNECTED_SCOPE_RETRY_INTERVAL;
use crate::db::Database;

/// Maximum retry backoff for durable scope delivery, measured in ticks.
const MAX_BACKOFF_TICKS: u32 = 8;

pub async fn run(app: AppHandle, db: Arc<Database>) {
    let mut state = ScopeRetryState::default();
    loop {
        retry_pending(&app, &db, &mut state).await;
        tokio::time::sleep(CONNECTED_SCOPE_RETRY_INTERVAL).await;
    }
}

/// Outcome of one retry tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tick {
    /// The durable queue itself could not be read.
    QueueUnreadable,
    /// Nothing is owed.
    Idle,
    /// Delivery is waiting for a usable credential.
    HoldingForCredential { sites: usize },
    Attempted {
        delivered: usize,
        failed: usize,
        skipped: usize,
    },
}

/// Per-site retry state.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Backoff {
    /// Redacted cause of the last failure.
    cause: String,
    consecutive_failures: u32,
    ticks_remaining: u32,
}

/// What earlier ticks already concluded.
#[derive(Debug, Default)]
struct ScopeRetryState {
    /// A credential hold has already been reported. Cleared when the hold
    /// ends, so it is announced once per episode rather than once a minute.
    credential_hold_reported: bool,
    backoff: BTreeMap<i64, Backoff>,
}

impl ScopeRetryState {
    /// Whether this tick should announce the credential hold. True exactly on
    /// the tick the hold begins.
    fn begin_credential_hold(&mut self) -> bool {
        let announce = !self.credential_hold_reported;
        self.credential_hold_reported = true;
        announce
    }

    /// The hold is over: either a bearer is readable again or nothing is owed.
    /// A later hold is a new episode and is announced on its own.
    fn end_credential_hold(&mut self) {
        self.credential_hold_reported = false;
    }

    /// Whether one site attempts delivery this tick, spending a tick of its
    /// backoff when it does not.
    fn ready(&mut self, site_id: i64) -> bool {
        match self.backoff.get_mut(&site_id) {
            Some(backoff) if backoff.ticks_remaining > 0 => {
                backoff.ticks_remaining -= 1;
                false
            }
            _ => true,
        }
    }

    fn delivered(&mut self, site_id: i64) {
        self.backoff.remove(&site_id);
    }

    /// Record a failed attempt and return the site's failure history.
    fn failed(&mut self, site_id: i64, cause: String) -> Backoff {
        let entry = self.backoff.entry(site_id).or_insert(Backoff {
            cause: String::new(),
            consecutive_failures: 0,
            ticks_remaining: 0,
        });
        entry.cause = cause;
        entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
        // 1, 2, 4, then 8 ticks, and 8 from there on.
        entry.ticks_remaining =
            MAX_BACKOFF_TICKS.min(1 << entry.consecutive_failures.saturating_sub(1).min(3));
        entry.clone()
    }

    /// Forget sites that have left the queue, so a long-running app does not
    /// accumulate history for scopes that were delivered or disconnected.
    fn forget_settled(&mut self, site_ids: &[i64]) {
        self.backoff.retain(|site_id, _| site_ids.contains(site_id));
    }
}

async fn retry_pending<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Arc<Database>,
    state: &mut ScopeRetryState,
) -> Tick {
    let db_read = Arc::clone(db);
    let pending =
        crate::commands::run_blocking(move || db_read.pending_connected_scan_scope_site_ids())
            .await;
    let site_ids = match pending {
        Ok(Ok(site_ids)) => site_ids,
        Ok(Err(_)) | Err(_) => {
            tracing::warn!("Connected scope retry could not read its durable queue");
            return Tick::QueueUnreadable;
        }
    };
    state.forget_settled(&site_ids);
    if site_ids.is_empty() {
        state.end_credential_hold();
        return Tick::Idle;
    }

    // Read the shared installation bearer once; an unactivated installation
    // pauses delivery without repeating per-site keychain warnings.
    match crate::keyring::get_connected_installation_token(app) {
        Ok(Some(_)) => state.end_credential_hold(),
        Ok(None) => {
            if state.begin_credential_hold() {
                tracing::info!(
                    sites = site_ids.len(),
                    "Connected scope delivery is waiting for this installation to be activated"
                );
            }
            return Tick::HoldingForCredential {
                sites: site_ids.len(),
            };
        }
        Err(error) => {
            if state.begin_credential_hold() {
                tracing::warn!(
                    sites = site_ids.len(),
                    cause = %crate::log_sanitizer::bounded_issue_evidence(&error),
                    "Connected scope delivery cannot read this installation's credential"
                );
            }
            return Tick::HoldingForCredential {
                sites: site_ids.len(),
            };
        }
    }

    let mut delivered = 0;
    let mut failed = 0;
    let mut skipped = 0;
    for site_id in site_ids {
        if !state.ready(site_id) {
            skipped += 1;
            continue;
        }
        match crate::commands::sync_connected_scan_scope_for_site(app, db, site_id).await {
            Ok(_) => {
                state.delivered(site_id);
                delivered += 1;
            }
            Err(error) => {
                // Keep the watermark for retry and redact the failure cause
                // before logging potentially sensitive transport details.
                let backoff = state.failed(
                    site_id,
                    crate::log_sanitizer::bounded_issue_evidence(&error),
                );
                failed += 1;
                tracing::warn!(
                    site_id,
                    attempts = backoff.consecutive_failures,
                    waiting_ticks = backoff.ticks_remaining,
                    cause = %backoff.cause,
                    "Connected scope delivery remains pending"
                );
            }
        }
    }
    Tick::Attempted {
        delivered,
        failed,
        skipped,
    }
}

#[cfg(test)]
#[path = "connected_scope_sync_tests.rs"]
mod tests;
