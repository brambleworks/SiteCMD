//! License deactivation and upstream release tracking.

use super::*;
use crate::background::CatalogRelease;

/// Whether deactivation can release upstream seats or only clear local state.
pub(super) enum DeactivateKeySource {
    /// Release the provider instance and catalog credential.
    Key(String),
    /// No usable key exists for an upstream release.
    LocalOnly,
}

pub(super) fn deactivate_key_source(
    read: Result<Option<String>, String>,
) -> Result<DeactivateKeySource, String> {
    match read.map(usable_key) {
        Ok(Some(key)) => Ok(DeactivateKeySource::Key(key)),
        Ok(None) => Ok(DeactivateKeySource::LocalOnly),
        Err(error) => Err(format!(
            "The license key could not be read from the keychain, so this machine's \
             activation cannot be released. Unlock the keychain and try again. ({error})"
        )),
    }
}

/// Warn before a local-only clear destroys the remaining upstream release handle.
pub(super) fn confirmation_body(key_source: &DeactivateKeySource) -> &'static str {
    match key_source {
        DeactivateKeySource::Key(_) => {
            "This unlinks this machine from the current license and clears the local license state."
        }
        DeactivateKeySource::LocalOnly => {
            "No license key is stored on this machine, so its activation CANNOT be released \
             and the seat stays used until support frees it. To release it instead, cancel, \
             enter your license key, and activate - then deactivate."
        }
    }
}

/// Unlink locally before releasing upstream seats so a concurrent refresh
/// cannot mint a credential after the final release.
#[tracing::instrument(skip(app, db))]
pub async fn deactivate_license(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let cached = load_cached_license_state(&db).await?;

    let state = match cached {
        Some(s) => s,
        None => {
            return Err("No active license to deactivate".to_string());
        }
    };

    // Resolve key availability before asking the user to confirm.
    let key_source = deactivate_key_source(crate::keyring::get_license_key(&app))?;

    crate::commands::confirm_sensitive_action(
        app.clone(),
        "Deactivate this license?",
        crate::commands::SensitiveActionTone::Warning,
        confirmation_body(&key_source).to_string(),
        "Deactivate License",
    )
    .await
    .map_err(|refusal| match refusal {
        crate::commands::SensitiveActionError::Declined => {
            tracing::info!("License deactivation declined");
            activation_error(LicenseActivationErrorCode::Cancelled)
        }
        crate::commands::SensitiveActionError::Failed(error) => error,
    })?;

    tracing::info!("Deactivating license key");

    let key_delete_failure;
    {
        // Recheck the generation after the dialog before clearing anything.
        let _generation = LICENSE_MUTATION.lock().await;
        let current = load_cached_license_state(&db).await?;
        let captured_key = match &key_source {
            DeactivateKeySource::Key(key) => Some(key.as_str()),
            DeactivateKeySource::LocalOnly => None,
        };
        let current_key = usable_key(crate::keyring::get_license_key(&app).map_err(|error| {
            format!("Could not re-read the license key; nothing was unlinked: {error}")
        })?);
        if !same_license_generation(
            &state.instance_id,
            current.as_ref(),
            captured_key,
            current_key.as_deref(),
        ) {
            return Err(
                "The license changed while the confirmation was open; nothing was unlinked. \
                 Review the current license and try again."
                    .to_string(),
            );
        }

        {
            // A dispatch timeout may still commit. Record a release tombstone only
            // when the row may be gone, or it could revoke a working license.
            let db_for_reread = (*db).clone();
            let db = (*db).clone();
            let clear_attempt =
                crate::commands::run_blocking(move || db.execute(store::clear)).await;
            record_license_write();
            let (cleared, row_may_be_gone) = match clear_attempt {
                Ok(Ok(inner)) => (inner, false),
                Ok(Err(crate::db::DbError::WorkerUnavailable)) => (
                    Err("the database worker is no longer accepting operations".to_string()),
                    false,
                ),
                Ok(Err(dispatch)) => (
                    Err(format!("database dispatch failed: {dispatch}")),
                    // Timeouts and terminated workers cannot prove whether the clear ran.
                    true,
                ),
                Err(join) => (Err(join), true),
            };
            if let Err(error) = cleared {
                // A surviving matching row proves the clear did not commit.
                let row_may_be_gone = if row_may_be_gone {
                    let db = db_for_reread;
                    let instance = state.instance_id.clone();
                    match crate::commands::run_blocking(move || db.execute(store::load)).await {
                        Ok(Ok(Ok(Some(row)))) if row.instance_id == instance => {
                            tracing::warn!(
                                "Clear reported an ambiguous failure but the row is still \
                                 present; recording no release"
                            );
                            false
                        }
                        _ => true,
                    }
                } else {
                    false
                };
                if row_may_be_gone {
                    let recorded = match &key_source {
                        DeactivateKeySource::Key(license_key) => Some(
                            crate::background::catalog_refresh::record_pending_provider_release(
                                &app,
                                license_key,
                                &state.instance_id,
                                true,
                            )
                            .await,
                        ),
                        DeactivateKeySource::LocalOnly => None,
                    };
                    return Err(format!(
                        "This machine's license could not be unlinked locally ({error}). {}",
                        ambiguous_clear_clause(recorded)
                    ));
                }
                return Err(format!(
                    "This machine's license could not be unlinked locally ({error}). Nothing was \
                     changed and this machine is still licensed; try again once the app is less \
                     busy."
                ));
            }
        }
        key_delete_failure = match crate::keyring::delete_license_key(&app) {
            Ok(()) => None,
            Err(e) => {
                tracing::warn!(
                    "Failed to clear license key from keyring (local state already cleared): {}",
                    e
                );
                Some(e)
            }
        };
    }
    tracing::info!("Local license state cleared");

    let upstream_release = match &key_source {
        DeactivateKeySource::Key(license_key) => {
            // Failed releases leave a tombstone for the next connected tick.
            let catalog = crate::background::catalog_refresh::release_credential(
                &app,
                license_key,
                &state.instance_id,
            )
            .await;

            // Track the provider and catalog seats independently.
            let provider = match api::deactivate(license_key, &state.instance_id).await {
                Ok(()) => {
                    tracing::info!("License deactivated with LS API");
                    CatalogRelease::Released
                }
                Err(e) if api::deactivate_failure_proves_absence(&e) => {
                    tracing::info!("LS instance needs no release ({}); nothing recorded", e);
                    CatalogRelease::NothingToRelease
                }
                Err(e) if api::deactivate_failure_is_terminal(&e) => {
                    // Terminal refusal does not prove the seat is free.
                    tracing::warn!(
                        "LS deactivation refused conclusively ({}); nothing recorded, and the seat was not proven free",
                        e
                    );
                    CatalogRelease::RefusedUnreleased
                }
                Err(e) => {
                    tracing::warn!("LS deactivation API failed; recording for retry: {}", e);
                    crate::background::catalog_refresh::record_pending_provider_release(
                        &app,
                        license_key,
                        &state.instance_id,
                        false,
                    )
                    .await
                }
            };
            // Report the least-complete outcome across both seats.
            match worst_release(catalog, provider) {
                CatalogRelease::Released => "ok",
                CatalogRelease::NothingToRelease => "gone",
                CatalogRelease::PendingRecorded => "pending",
                CatalogRelease::RefusedUnreleased => "refused",
                CatalogRelease::PendingLost => "lost",
            }
        }
        DeactivateKeySource::LocalOnly => {
            tracing::warn!(
                "No license key stored; unlinked locally without releasing upstream slots"
            );
            "none"
        }
    };
    crate::audit_log::record(
        "license.deactivate",
        serde_json::json!({
            "installation": state.instance_id,
            "upstream_release": upstream_release,
        }),
        match upstream_release {
            "pending" => "pending",
            "lost" | "refused" | "none" => "lost",
            _ => "ok",
        },
    );

    deactivation_result(
        key_delete_failure,
        match upstream_release {
            "ok" => UpstreamRelease::Released,
            "gone" => UpstreamRelease::NothingOwed,
            "none" => UpstreamRelease::None,
            "refused" => UpstreamRelease::RefusedUnreleased,
            "lost" => UpstreamRelease::Lost,
            _ => UpstreamRelease::Pending,
        },
    )
}

/// Describe recovery when a failed local clear may already have committed.
fn ambiguous_clear_clause(recorded: Option<CatalogRelease>) -> &'static str {
    match recorded {
        Some(CatalogRelease::PendingRecorded) => {
            "Its activation slots were recorded for release and will be freed automatically; \
             try again once the app is less busy."
        }
        // Any other result means no retry tombstone exists.
        Some(_) => {
            "Its activation slots could NOT be recorded for release, so nothing here will free \
             them; if another machine cannot activate, contact support."
        }
        None => {
            "No license key was stored, so nothing was contacted upstream and no activation \
             slots were recorded for release; if another machine cannot activate, contact support."
        }
    }
}

/// Return the release outcome requiring the strongest recovery claim.
pub(super) fn worst_release(a: CatalogRelease, b: CatalogRelease) -> CatalogRelease {
    if release_rank(a) >= release_rank(b) {
        a
    } else {
        b
    }
}

fn release_rank(release: CatalogRelease) -> u8 {
    match release {
        CatalogRelease::Released => 0,
        CatalogRelease::NothingToRelease => 1,
        CatalogRelease::PendingRecorded => 2,
        CatalogRelease::RefusedUnreleased => 3,
        CatalogRelease::PendingLost => 4,
    }
}

/// Prefix for a completed unlink that still requires cleanup.
pub(crate) const DEACTIVATION_KEYCHAIN_REMNANT: &str = "unlinked_with_keychain_remnant: ";

/// Report cleanup remaining after the local unlink has completed.
pub(super) fn deactivation_result(
    key_delete_failure: Option<String>,
    upstream_release: UpstreamRelease,
) -> Result<(), String> {
    match key_delete_failure {
        // A stranded seat is cleanup debt even when the keyring delete succeeded.
        None if leaves_a_stranded_seat(upstream_release) => Err(format!(
            "{DEACTIVATION_KEYCHAIN_REMNANT}This machine was unlinked {}.{}",
            released_clause(upstream_release),
            remaining_work(upstream_release)
        )),
        None => Ok(()),
        Some(error) => Err(format!(
            "{DEACTIVATION_KEYCHAIN_REMNANT}This machine was unlinked {}, but the \
                 license key could not be removed from the keychain ({error}). Remove it from \
                 the OS keychain manually.{}",
            released_clause(upstream_release),
            remaining_work(upstream_release)
        )),
    }
}

/// Describe only what the combined upstream outcome establishes.
fn released_clause(upstream_release: UpstreamRelease) -> &'static str {
    match upstream_release {
        UpstreamRelease::Released => "and its activations released",
        UpstreamRelease::NothingOwed => {
            "and the licensing service reported nothing left to release"
        }
        UpstreamRelease::Pending => {
            "and its activations recorded for release, which SiteCMD completes \
             automatically the next time it is online"
        }
        UpstreamRelease::RefusedUnreleased => {
            "but the licensing service refused to release at least one of its activations"
        }
        UpstreamRelease::Lost => {
            "without releasing at least one of its activations, which could not be recorded \
             for a retry either"
        }
        UpstreamRelease::None => "locally",
    }
}

/// Whether no automatic path can reclaim at least one upstream seat.
fn leaves_a_stranded_seat(upstream_release: UpstreamRelease) -> bool {
    matches!(
        upstream_release,
        UpstreamRelease::Lost | UpstreamRelease::RefusedUnreleased | UpstreamRelease::None
    )
}

/// Describe any user action still required after deactivation.
fn remaining_work(upstream_release: UpstreamRelease) -> &'static str {
    match upstream_release {
        UpstreamRelease::Lost => {
            " Nothing here will retry that release, so if another machine cannot activate, \
             contact support to free the seat."
        }
        UpstreamRelease::RefusedUnreleased => {
            " A refused release is not retried, so if another machine cannot activate, \
             contact support to free the seat."
        }
        UpstreamRelease::None => {
            " No license key was stored, so this machine's activation could not be released \
             and nothing here will retry it; if another machine cannot activate, contact \
             support to free the seat."
        }
        _ => " Nothing else here needs doing.",
    }
}

/// Combined provider and catalog release outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpstreamRelease {
    /// Both seats were released.
    Released,
    /// Both seats are already absent.
    NothingOwed,
    /// At least one release has a retry tombstone.
    Pending,
    /// At least one seat was conclusively refused and remains allocated.
    RefusedUnreleased,
    /// At least one owed release has no retry handle.
    Lost,
    /// No key was available for either upstream release.
    None,
}
