//! Catalog credential, update, and release-retry loop.
//!
//! Credential acquisition gates manifest fetch, verification, and activation.

use std::sync::{Arc, LazyLock};

use tauri::AppHandle;
use tokio::sync::Notify;

use crate::catalog::{self, activation, ActivationOutcome, CatalogRequest, Channel, FetchError};
use crate::constants::{CATALOG_REFRESH_INITIAL_DELAY, CATALOG_REFRESH_INTERVAL};
use crate::db::Database;

/// A retained permit triggers another tick after the current one finishes.
static IMMEDIATE_TICK: LazyLock<Notify> = LazyLock::new(Notify::new);

/// Serializes credential acquisition, replay recovery, and release.
static CREDENTIAL_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Request a refresh before the next scheduled interval.
pub fn request_immediate_tick() {
    IMMEDIATE_TICK.notify_one();
}

/// Outcome of one catalog refresh tick.
#[derive(Debug, PartialEq, Eq)]
pub enum TickOutcome {
    /// Catalog access does not apply to this installation.
    NotApplicable,
    /// A required credential could not be obtained.
    NoCredential(String),
    /// The installed pack is current.
    Current,
    /// A verified newer pack became active.
    Updated { sequence: u64 },
    /// The token was rejected and cleared for reactivation.
    CredentialRejected,
    /// Retryable transport or verification failure.
    Failed(String),
}

/// Decide from signed sequence state whether a manifest warrants download.
pub fn needs_download(
    manifest_sequence: u64,
    active_sequence: Option<u64>,
    active_needs_repair: bool,
) -> bool {
    match active_sequence {
        // Repair may redownload the active sequence but never an older one.
        Some(active) => {
            manifest_sequence > active || (active_needs_repair && manifest_sequence == active)
        }
        None => true,
    }
}

/// Omit the conditional version when repair requires a full manifest response.
pub fn conditional_version(active_needs_repair: bool, installed: Option<String>) -> Option<String> {
    if active_needs_repair {
        None
    } else {
        installed
    }
}

/// Keychain-backed nonce store for idempotent activation replay.
struct KeyringNonceStore<'a, R: tauri::Runtime> {
    app: &'a AppHandle<R>,
}

impl<R: tauri::Runtime> activation::PendingNonceStore for KeyringNonceStore<'_, R> {
    fn load(&self) -> Result<Option<activation::PendingActivation>, String> {
        crate::keyring::get_pending_activation(self.app)
    }
    fn save(&self, pending: &activation::PendingActivation) -> Result<(), String> {
        crate::keyring::store_pending_activation(self.app, pending)
    }
    fn clear(&self) {
        if let Err(error) = crate::keyring::delete_pending_activation(self.app) {
            tracing::warn!("pending activation nonce could not be cleared: {error}");
        }
    }
}

/// Last conclusive credential refusal exposed through catalog status.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialBlock {
    /// "cap_reached" or the service's refusal code.
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap: Option<u32>,
}

static CREDENTIAL_BLOCK: std::sync::Mutex<Option<CredentialBlock>> = std::sync::Mutex::new(None);

/// Recover the replace-only status value after mutex poisoning.
fn credential_block_lock() -> std::sync::MutexGuard<'static, Option<CredentialBlock>> {
    CREDENTIAL_BLOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn last_credential_block() -> Option<CredentialBlock> {
    credential_block_lock().clone()
}

fn record_credential_block(block: Option<CredentialBlock>) {
    *credential_block_lock() = block;
}

/// Ensure a catalog token is stored without making license activation depend on it.
pub(crate) async fn ensure_credential<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Arc<Database>,
    license_key: &str,
    installation_id: &str,
) -> Result<bool, String> {
    let _guard = CREDENTIAL_LOCK.lock().await;

    // Recheck under the lock to avoid minting over a concurrently stored token.
    match crate::keyring::get_catalog_token(app) {
        Ok(Some(_)) => {
            record_credential_block(None);
            return Ok(true);
        }
        Ok(None) => {}
        Err(error) => return Err(format!("stored catalog token unreadable: {error}")),
    }

    // Verify key and installation as one license generation before minting.
    // A mismatch means the lifecycle operation that changed it owns the remaining work.
    {
        let _generation = crate::licensing::commands::license_mutation().lock().await;
        match crate::keyring::get_license_key(app) {
            Ok(Some(current)) if current == license_key => {}
            Ok(_) => {
                tracing::info!(
                    "License changed while waiting for the credential lock; not activating"
                );
                return Ok(false);
            }
            Err(error) => return Err(format!("license key unreadable: {error}")),
        }

        let row = {
            let db = db.clone();
            match crate::commands::run_blocking(move || db.execute(crate::licensing::store::load))
                .await
            {
                Ok(Ok(Ok(row))) => row,
                Ok(Ok(Err(error))) => return Err(format!("license row unreadable: {error}")),
                Ok(Err(error)) => return Err(format!("license row unreadable: {error}")),
                Err(error) => return Err(format!("license row unreadable: {error}")),
            }
        };
        match row {
            Some(state) if state.instance_id == installation_id => {}
            _ => {
                tracing::info!(
                    "Installation id changed while waiting for the credential lock; not activating"
                );
                return Ok(false);
            }
        }
    }

    let nonces = KeyringNonceStore { app };
    match activation::obtain_token(license_key, installation_id, &nonces).await {
        Ok(ActivationOutcome::Issued { token, .. }) => {
            // Keep the nonce until the issued token is durable.
            crate::keyring::store_catalog_token(app, &token)
                .map_err(|error| format!("catalog token could not be stored: {error}"))?;
            activation::PendingNonceStore::clear(&nonces);
            crate::audit_log::record(
                "catalog.activate",
                serde_json::json!({ "installation": installation_id }),
                "ok",
            );
            record_credential_block(None);
            Ok(true)
        }
        // Release-and-retry already failed, so expose the stranded state.
        Ok(ActivationOutcome::AlreadyActivated) => {
            record_credential_block(Some(CredentialBlock {
                code: "stranded".to_string(),
                active: None,
                cap: None,
            }));
            crate::audit_log::record(
                "catalog.activate",
                serde_json::json!({ "installation": installation_id }),
                "fail",
            );
            Err("catalog credential stranded after release-and-retry".to_string())
        }
        Err(activation::ActivationError::NoEndpointConfigured) => Ok(false),
        Err(activation::ActivationError::CredentialCapReached { active, cap }) => {
            record_credential_block(Some(CredentialBlock {
                code: "cap_reached".to_string(),
                active: Some(active),
                cap: Some(cap),
            }));
            crate::audit_log::record(
                "catalog.activate",
                serde_json::json!({ "installation": installation_id }),
                "fail",
            );
            Err(format!(
                "credential cap reached: {active} of {cap} machines active"
            ))
        }
        Err(activation::ActivationError::Refused { reason }) => {
            record_credential_block(Some(CredentialBlock {
                code: reason.to_string(),
                active: None,
                cap: None,
            }));
            crate::audit_log::record(
                "catalog.activate",
                serde_json::json!({ "installation": installation_id }),
                "fail",
            );
            Err(format!("catalog activation refused: {reason}"))
        }
        Err(error) => {
            crate::audit_log::record(
                "catalog.activate",
                serde_json::json!({ "installation": installation_id }),
                "fail",
            );
            Err(error.to_string())
        }
    }
}

/// Release an installation credential and report how far cleanup progressed.
pub(crate) async fn release_credential<R: tauri::Runtime>(
    app: &AppHandle<R>,
    license_key: &str,
    installation_id: &str,
) -> super::CatalogRelease {
    let _guard = CREDENTIAL_LOCK.lock().await;

    let outcome = match activation::deactivate(license_key, installation_id).await {
        // Zero proves the credential was already absent, not released now.
        Ok(0) => "absent",
        Ok(_) => "ok",
        Err(activation::ActivationError::NoEndpointConfigured) => "unconfigured",
        Err(activation::ActivationError::Refused { reason })
            if activation::known_refusal(&reason) =>
        {
            // Known refusals are terminal but do not prove the seat is absent.
            tracing::warn!(
                "Catalog credential release refused conclusively ({reason}); nothing to retry, and the seat was not proven free"
            );
            "refused"
        }
        Err(error) => {
            tracing::warn!(
                "Catalog credential release failed (clearing token locally anyway): {error}"
            );
            "pending"
        }
    };

    // Preserve release handles when the service could not be reached.
    let outcome = if outcome == "pending" {
        match crate::keyring::store_pending_release(
            app,
            crate::keyring::PendingRelease {
                license_key: license_key.to_string(),
                installation_id: installation_id.to_string(),
                catalog: true,
                lemonsqueezy: false,
            },
        ) {
            Ok(()) => outcome,
            Err(error) => {
                tracing::error!(
                    "Pending catalog release could not be recorded; this seat has no retry handle: {error}"
                );
                "unrecorded"
            }
        }
    } else {
        outcome
    };

    // Local token removal is independent of the upstream seat outcome.
    let local_token_cleared = match crate::keyring::delete_catalog_token(app) {
        Ok(()) => true,
        Err(error) => {
            tracing::error!(
                "Catalog token left in the keyring after deactivation; a later activation will trip over it: {error}"
            );
            false
        }
    };

    crate::audit_log::record(
        "catalog.deactivate",
        serde_json::json!({
            "installation": installation_id,
            "local_token_cleared": local_token_cleared,
        }),
        outcome,
    );
    // A remaining local bearer token keeps deactivation incomplete.
    if !local_token_cleared {
        return super::CatalogRelease::PendingRecorded;
    }
    match outcome {
        "ok" => super::CatalogRelease::Released,
        // Only these outcomes prove no seat remains.
        "absent" | "unconfigured" => super::CatalogRelease::NothingToRelease,
        "refused" => super::CatalogRelease::RefusedUnreleased,
        "unrecorded" => super::CatalogRelease::PendingLost,
        _ => super::CatalogRelease::PendingRecorded,
    }
}

/// Record release handles before local state can disappear.
pub(crate) async fn record_pending_provider_release<R: tauri::Runtime>(
    app: &AppHandle<R>,
    license_key: &str,
    installation_id: &str,
    catalog: bool,
) -> super::CatalogRelease {
    let _guard = CREDENTIAL_LOCK.lock().await;
    match crate::keyring::store_pending_release(
        app,
        crate::keyring::PendingRelease {
            license_key: license_key.to_string(),
            installation_id: installation_id.to_string(),
            catalog,
            lemonsqueezy: true,
        },
    ) {
        Ok(()) => super::CatalogRelease::PendingRecorded,
        Err(error) => {
            tracing::error!(
                "Pending LemonSqueezy release could not be recorded; this seat has no retry handle: {error}"
            );
            super::CatalogRelease::PendingLost
        }
    }
}

/// Retry provider and catalog releases independently on each connected tick.
async fn retry_pending_release<R: tauri::Runtime>(app: &AppHandle<R>) {
    // Snapshot without holding the credential lock across network waits, then
    // subtract only settled entries so concurrent appends survive.
    let pending = {
        let _guard = CREDENTIAL_LOCK.lock().await;
        match crate::keyring::get_pending_releases(app) {
            Ok(pending) if pending.is_empty() => return,
            Ok(pending) => pending,
            Err(error) => {
                tracing::warn!("Pending releases unreadable, will retry: {error}");
                return;
            }
        }
    };

    let mut outcomes = Vec::new();
    for release in pending {
        let mut resolved_catalog = false;
        let mut resolved_lemonsqueezy = false;
        if release.catalog {
            match activation::deactivate(&release.license_key, &release.installation_id).await {
                Ok(_) | Err(activation::ActivationError::NoEndpointConfigured) => {
                    resolved_catalog = true;
                    crate::audit_log::record(
                        "catalog.deactivate",
                        serde_json::json!({ "installation": release.installation_id }),
                        "ok",
                    );
                }
                Err(activation::ActivationError::Refused { reason })
                    if crate::catalog::activation::known_refusal(&reason) =>
                {
                    // Known service refusals are terminal; degraded refusals remain retryable.
                    tracing::warn!(
                        "Pending catalog release refused conclusively ({reason}); settling the record"
                    );
                    resolved_catalog = true;
                    crate::audit_log::record(
                        "catalog.deactivate",
                        serde_json::json!({ "installation": release.installation_id }),
                        "refused",
                    );
                }
                Err(error) => {
                    tracing::warn!("Pending catalog release still failing, will retry: {error}");
                }
            }
        }
        if release.lemonsqueezy {
            // Unconfigured development builds have no provider instance.
            if !crate::licensing::config::license_configured() {
                resolved_lemonsqueezy = true;
            } else {
                match crate::licensing::api::deactivate(
                    &release.license_key,
                    &release.installation_id,
                )
                .await
                {
                    Ok(()) => {
                        resolved_lemonsqueezy = true;
                        crate::audit_log::record(
                            "license.deactivate",
                            serde_json::json!({ "installation": release.installation_id }),
                            "ok",
                        );
                    }
                    Err(error) if crate::licensing::api::deactivate_failure_is_terminal(&error) => {
                        // A terminal absence can be the committed result of a lost response.
                        tracing::info!(
                            "Pending LemonSqueezy release needs no retry ({error}); settling the record"
                        );
                        resolved_lemonsqueezy = true;
                        crate::audit_log::record(
                            "license.deactivate",
                            serde_json::json!({ "installation": release.installation_id }),
                            "gone",
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            "Pending LemonSqueezy release still failing, will retry: {error}"
                        );
                    }
                }
            }
        }
        outcomes.push((release, resolved_catalog, resolved_lemonsqueezy));
    }

    let _guard = CREDENTIAL_LOCK.lock().await;
    let current = match crate::keyring::get_pending_releases(app) {
        Ok(current) => current,
        Err(error) => {
            tracing::warn!("Pending releases unreadable after drain, will retry: {error}");
            return;
        }
    };
    let settled = outcomes.into_iter().fold(
        current,
        |list, (release, resolved_catalog, resolved_lemonsqueezy)| {
            crate::keyring::settle_pending_release(
                list,
                &release,
                resolved_catalog,
                resolved_lemonsqueezy,
            )
        },
    );
    if let Err(error) = crate::keyring::replace_pending_releases(app, &settled) {
        tracing::warn!("Completed releases could not be cleared: {error}");
    }
}

/// Run shortly after launch, on schedule, and after explicit refresh requests.
pub async fn run(app: AppHandle, db: Arc<Database>) {
    tokio::select! {
        _ = tokio::time::sleep(CATALOG_REFRESH_INITIAL_DELAY) => {}
        _ = IMMEDIATE_TICK.notified() => {}
    }
    loop {
        retry_pending_release(&app).await;

        match tick(&app, &db).await {
            TickOutcome::NotApplicable | TickOutcome::Current => {}
            TickOutcome::Updated { sequence } => {
                tracing::info!("Catalog updated to release sequence {sequence}");
                crate::commands::emit_event(&app, "catalog-updated", ());
            }
            TickOutcome::CredentialRejected => {
                tracing::warn!("Catalog credential rejected; re-activating next cycle");
            }
            TickOutcome::NoCredential(reason) => {
                tracing::warn!("Catalog credential unavailable: {reason}");
            }
            TickOutcome::Failed(reason) => {
                tracing::warn!("Catalog refresh failed: {reason}");
            }
        }
        // Refresh status consumers after every terminal tick outcome.
        crate::commands::emit_event(&app, "catalog-refresh-completed", ());
        tokio::select! {
            _ = tokio::time::sleep(CATALOG_REFRESH_INTERVAL) => {}
            _ = IMMEDIATE_TICK.notified() => {}
        }
    }
}

#[path = "catalog_refresh_tick.rs"]
mod tick_pipeline;
use tick_pipeline::tick;

#[cfg(test)]
#[path = "catalog_refresh_tests.rs"]
mod tests;
