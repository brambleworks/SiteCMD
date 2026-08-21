//! License validation and offline grace handling.

use super::*;

/// Validate on demand or after the cache ages, with offline grace fallback.
#[tracing::instrument(skip(app, db))]
pub async fn validate_license(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    force: Option<bool>,
) -> Result<LicenseInfo, String> {
    validate_license_with_db(&app, &db, force).await
}

pub(super) async fn validate_license_with_db(
    app: &AppHandle,
    db: &Arc<Database>,
    force: Option<bool>,
) -> Result<LicenseInfo, String> {
    // Retry one discarded verdict and carry it forward as a conservative cap.
    let mut prior_observation: Option<Box<LicenseState>> = None;
    loop {
        let force_live = force.unwrap_or(false) || prior_observation.is_some();
        match validate_license_pass(app, db, force_live, prior_observation.as_deref()).await? {
            ValidationPass::Answered(info) => return Ok(*info),
            ValidationPass::DiscardedWithRetry(observed) => {
                prior_observation = Some(observed);
                tracing::info!(
                    "License state moved during validation; validating live once more for a fresh verdict"
                );
            }
        }
    }
}

/// One validation pass, carrying a discarded verdict into its single retry.
enum ValidationPass {
    /// Boxed for clippy's variant-size lint: both payloads are wide.
    Answered(Box<LicenseInfo>),
    DiscardedWithRetry(Box<LicenseState>),
}

/// Return the less permissive of stored and uncommitted live observations.
pub(super) fn conservative_answer(
    row_answer: LicenseInfo,
    observed_answer: LicenseInfo,
) -> LicenseInfo {
    fn rank(tier: crate::licensing::config::Tier) -> u8 {
        match tier {
            crate::licensing::config::Tier::Free => 0,
            crate::licensing::config::Tier::Core => 1,
            crate::licensing::config::Tier::Pro => 2,
        }
    }
    if rank(observed_answer.tier) < rank(row_answer.tier) {
        observed_answer
    } else {
        let mut answer = row_answer;
        // Preserve the actionable deactivation warning on a tier tie.
        if matches!(
            observed_answer.validation_warning,
            ValidationWarning::InstanceDeactivated
        ) && rank(answer.tier) == rank(observed_answer.tier)
        {
            answer.validation_warning = ValidationWarning::InstanceDeactivated;
        }
        answer
    }
}

/// Cap a grace answer with a carried observation for the same instance.
pub(super) fn capped_by_observation(
    row_answer: LicenseInfo,
    row: &LicenseState,
    prior_observation: Option<&LicenseState>,
) -> LicenseInfo {
    match prior_observation {
        Some(observed) if observed.instance_id == row.instance_id => {
            conservative_answer(row_answer, info_from_state(observed))
        }
        _ => row_answer,
    }
}

async fn validate_license_pass(
    app: &AppHandle,
    db: &Arc<Database>,
    force_live: bool,
    prior_observation: Option<&LicenseState>,
) -> Result<ValidationPass, String> {
    let retry_available = prior_observation.is_none();
    if !config::license_configured() {
        #[cfg(debug_assertions)]
        {
            let cached = load_cached_license_state(db).await?;
            if let Some(info) = cached
                .as_ref()
                .and_then(dev_info_from_state_when_licensing_unconfigured)
            {
                tracing::info!(
                    "LemonSqueezy licensing is not configured - using debug test license"
                );
                return Ok(ValidationPass::Answered(Box::new(info)));
            }
        }

        tracing::warn!("LemonSqueezy licensing is not configured - using Free tier");
        return Ok(ValidationPass::Answered(Box::new(free_info())));
    }

    // Capture before reading so every later write is visible at commit.
    let generation_at_capture = license_write_generation();
    let cached = load_cached_license_state(db).await?;

    let state = match cached {
        Some(s) => s,
        None => {
            tracing::info!("No license key stored - Free tier");
            return Ok(ValidationPass::Answered(Box::new(free_info())));
        }
    };

    // Debug sentinels never round-trip to the provider.
    #[cfg(debug_assertions)]
    if let Some(info) = dev_info_from_state_when_licensing_unconfigured(&state) {
        tracing::info!("Dev license recognized in cache: {} tier", state.tier);
        return Ok(ValidationPass::Answered(Box::new(info)));
    }

    let needs_revalidation = revalidation_required(
        force_live,
        classify_offline_validation(&state.last_validated_at, chrono::Utc::now()),
    );

    if !needs_revalidation {
        tracing::info!("License valid (cached): {} tier", state.tier);
        return Ok(ValidationPass::Answered(Box::new(info_from_state(&state))));
    }

    // Read the key only for live validation; an unreadable key enters grace.
    let license_key = match crate::keyring::get_license_key(app).map(usable_key) {
        Ok(Some(key)) => key,
        Ok(None) => {
            tracing::warn!(
                "License key missing from the secret store; skipping validation and keeping the cached tier (re-enter the key to fix)"
            );
            return offline_validation_or_downgrade_with_cause(&state, GraceCause::KeyUnreadable)
                .map(|info| capped_by_observation(info, &state, prior_observation))
                .map(|info| ValidationPass::Answered(Box::new(info)));
        }
        Err(e) => {
            tracing::warn!(
                "Failed to read license key from the secret store; skipping validation and keeping the cached tier: {}",
                e
            );
            return offline_validation_or_downgrade_with_cause(&state, GraceCause::KeyUnreadable)
                .map(|info| capped_by_observation(info, &state, prior_observation))
                .map(|info| ValidationPass::Answered(Box::new(info)));
        }
    };
    match api::validate(&license_key, &state.instance_id).await {
        Ok(result) => {
            let now = now_iso();
            let validated_instance = state.instance_id.clone();
            let updated = state_refreshed_from_validation_result(state, result, now);
            let saved = updated.clone();
            {
                // Commit only if the validated generation is still current.
                let _generation = LICENSE_MUTATION.lock().await;
                let current = load_cached_license_state(db).await?;
                // Guard both replacement and same-instance write ordering.
                if license_write_generation() != generation_at_capture
                    || current
                        .as_ref()
                        .is_none_or(|row| row.instance_id != validated_instance)
                {
                    tracing::info!(
                        "License state moved during validation; discarding the stale verdict"
                    );
                    if retry_available {
                        return Ok(ValidationPass::DiscardedWithRetry(Box::new(updated)));
                    }
                    return match current.as_ref() {
                        Some(row) => {
                            let row_answer = offline_validation_or_downgrade(row)?;
                            let answer = if row.instance_id == updated.instance_id {
                                conservative_answer(row_answer, info_from_state(&updated))
                            } else {
                                row_answer
                            };
                            Ok(ValidationPass::Answered(Box::new(answer)))
                        }
                        None => Ok(ValidationPass::Answered(Box::new(free_info()))),
                    };
                }
                let db = db.clone();
                let write_result = crate::commands::run_blocking(move || {
                    db.execute(move |conn| store::update_validation(conn, &saved))
                })
                .await;
                // A timed-out dispatch may still land.
                record_license_write();
                write_result???;
            }

            tracing::info!(
                "License re-validated: {} ({})",
                updated.tier,
                updated.status
            );
            Ok(ValidationPass::Answered(Box::new(info_from_state(
                &updated,
            ))))
        }
        Err(e) => {
            tracing::warn!("License validation failed (network?): {}", e);
            // Re-read after the network wait before choosing a grace answer.
            let current = {
                let _generation = LICENSE_MUTATION.lock().await;
                load_cached_license_state(db).await
            };
            match current {
                Ok(Some(row)) => {
                    if row.instance_id != state.instance_id {
                        tracing::info!(
                            "License changed during validation; answering from the installed row rather than the validated one"
                        );
                    }
                    let row_answer = offline_validation_or_downgrade(&row)?;
                    let answer = capped_by_observation(row_answer, &row, prior_observation);
                    Ok(ValidationPass::Answered(Box::new(answer)))
                }
                Ok(None) => {
                    tracing::info!("License removed during validation; answering Free");
                    free_info_result().map(|info| ValidationPass::Answered(Box::new(info)))
                }
                Err(error) => {
                    tracing::warn!(
                        "Could not re-read the license row after a failed validation; answering from the captured state: {error}"
                    );
                    offline_validation_or_downgrade(&state)
                        .map(|info| capped_by_observation(info, &state, prior_observation))
                        .map(|info| ValidationPass::Answered(Box::new(info)))
                }
            }
        }
    }
}

fn free_info_result() -> Result<LicenseInfo, String> {
    Ok(free_info())
}

/// Whether this call must bypass the validation cache.
pub(super) fn revalidation_required(force: bool, offline: OfflineValidationState) -> bool {
    force
        || matches!(
            offline,
            OfflineValidationState::Stale
                | OfflineValidationState::StaleFinalWarning
                | OfflineValidationState::Expired
        )
}

/// Cause used to select grace-period messaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GraceCause {
    /// The validation request failed.
    Network,
    /// The local license key was unreadable.
    KeyUnreadable,
}

/// Select the warning for an offline state and failure cause.
pub(super) fn grace_warning_for(
    offline: OfflineValidationState,
    cause: GraceCause,
) -> ValidationWarning {
    match (offline, cause) {
        (OfflineValidationState::Stale, GraceCause::Network) => ValidationWarning::Stale,
        (OfflineValidationState::StaleFinalWarning, GraceCause::Network) => {
            ValidationWarning::StaleFinalWarning
        }
        (
            OfflineValidationState::Stale | OfflineValidationState::StaleFinalWarning,
            GraceCause::KeyUnreadable,
        ) => ValidationWarning::KeyUnreadable,
        (OfflineValidationState::Fresh | OfflineValidationState::Expired, _) => {
            ValidationWarning::None
        }
    }
}

/// Apply the shared offline grace and downgrade policy to cached state.
pub(super) fn offline_validation_or_downgrade(state: &LicenseState) -> Result<LicenseInfo, String> {
    offline_validation_or_downgrade_with_cause(state, GraceCause::Network)
}

fn offline_validation_or_downgrade_with_cause(
    state: &LicenseState,
    cause: GraceCause,
) -> Result<LicenseInfo, String> {
    let offline = classify_offline_validation(&state.last_validated_at, chrono::Utc::now());
    match offline {
        OfflineValidationState::Fresh => Ok(info_from_state(state)),
        OfflineValidationState::Stale => {
            tracing::info!(
                "Within offline grace period ({:?}); using cached tier with warning: {}",
                cause,
                state.tier
            );
            Ok(info_from_state_with_warning(
                state,
                grace_warning_for(offline, cause),
            ))
        }
        OfflineValidationState::StaleFinalWarning => {
            tracing::warn!(
                "Offline grace period exhausted ({:?}); surfacing warning banner. Cached tier still active: {}",
                cause,
                state.tier
            );
            Ok(info_from_state_with_warning(
                state,
                grace_warning_for(offline, cause),
            ))
        }
        OfflineValidationState::Expired => {
            tracing::warn!("Offline grace AND final-warning windows expired. Downgrading to Free.");
            // Preserve the actionable remote-deactivation warning after downgrade.
            if state.status == INSTANCE_DEACTIVATED_STATUS {
                Ok(info_from_state(state))
            } else {
                Ok(free_info())
            }
        }
    }
}
