//! License activation and replacement planning.

use super::*;

#[derive(Debug)]
pub(super) enum ActivationConfirmationError {
    Declined,
    Failed,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ActivationPersistenceError {
    Changed,
    Storage,
}

#[async_trait::async_trait]
pub(super) trait ActivationPorts: Send + Sync {
    async fn read_state(&self) -> Result<Option<LicenseState>, String>;
    fn read_key(&self) -> Result<Option<String>, String>;
    async fn revalidate(&self) -> Result<LicenseInfo, String>;
    async fn confirm_replacement(
        &self,
        current_tier: &str,
    ) -> Result<(), ActivationConfirmationError>;
    fn instance_name(&self) -> String;
    async fn activate(&self, key: &str, instance_name: &str) -> Result<api::LicenseResult, String>;
    async fn release_instance(&self, key: &str, instance_id: &str);
    async fn release_credential(&self, key: &str, instance_id: &str, phase: &str);
    fn delete_catalog_token(&self) -> Result<(), String>;
    async fn persist_generation(
        &self,
        expected_state: Option<&LicenseState>,
        expected_key: Option<&str>,
        state: &LicenseState,
    ) -> Result<(), ActivationPersistenceError>;
    fn tier_for_variant(&self, variant_id: u64) -> Tier;
    fn now_iso(&self) -> String;
    async fn ensure_credential(&self, key: &str, instance_id: &str) -> Result<(), String>;
    fn request_catalog_refresh(&self);
    fn audit(&self, detail: serde_json::Value, outcome: &'static str);
}

pub(super) async fn activate_license_with_ports<P: ActivationPorts>(
    ports: &P,
    key: String,
) -> Result<LicenseInfo, String> {
    let key = normalize_license_key(&key);
    if key.is_empty() {
        return Err(activation_error(LicenseActivationErrorCode::KeyRequired));
    }

    let existing_state = ports
        .read_state()
        .await
        .map_err(|_| activation_error(LicenseActivationErrorCode::Incomplete))?;
    let existing_key = usable_key(
        ports
            .read_key()
            .map_err(|_| activation_error(LicenseActivationErrorCode::Incomplete))?,
    );
    let same_key = existing_key
        .as_deref()
        .is_some_and(|old_key| !license_replacement_required(old_key, &key));
    let plan = activation_plan(
        existing_state.is_some(),
        existing_key.is_some(),
        same_key,
        existing_state
            .as_ref()
            .is_some_and(|state| info_from_state(state).is_active),
    );
    if matches!(plan, ActivationPlan::AlreadyActive) {
        let Some(state) = existing_state.as_ref() else {
            tracing::error!("AlreadyActive reached with no stored state; nothing was changed");
            return Err(activation_error(LicenseActivationErrorCode::Incomplete));
        };
        tracing::info!("License already active; leaving the existing activation in place");
        if let Err(error) = ports.ensure_credential(&key, &state.instance_id).await {
            tracing::warn!(
                "Catalog credential unavailable; the refresh loop retries what is retryable: {error}"
            );
        }
        ports.request_catalog_refresh();
        return ports.revalidate().await.map_err(|error| {
            tracing::warn!(
                "Revalidation of the already-active key stopped before completion: {error}"
            );
            activation_error(LicenseActivationErrorCode::Incomplete)
        });
    }
    if matches!(plan, ActivationPlan::Teardown { confirm: true }) {
        let current_tier = existing_state
            .as_ref()
            .map(|state| state.tier.to_string())
            .unwrap_or_else(|| "paid".to_string());
        match ports.confirm_replacement(&current_tier).await {
            Ok(()) => {}
            Err(ActivationConfirmationError::Declined) => {
                return Err(activation_error(LicenseActivationErrorCode::Cancelled));
            }
            Err(ActivationConfirmationError::Failed) => {
                return Err(activation_error(LicenseActivationErrorCode::Incomplete));
            }
        }
        let row_now = ports
            .read_state()
            .await
            .map_err(|_| activation_error(LicenseActivationErrorCode::Incomplete))?;
        let key_now = usable_key(
            ports
                .read_key()
                .map_err(|_| activation_error(LicenseActivationErrorCode::Incomplete))?,
        );
        if !snapshot_unchanged(
            existing_state
                .as_ref()
                .map(|state| state.instance_id.as_str()),
            row_now.as_ref(),
            existing_key.as_deref(),
            key_now.as_deref(),
        ) {
            tracing::info!(
                "License changed while the replacement dialog was open; nothing was changed"
            );
            return Err(activation_error(
                LicenseActivationErrorCode::ChangedDuringActivation,
            ));
        }
    }

    let key_fingerprint = license_key_fingerprint(&key);
    let audit_detail = license_activation_audit_detail(&key_fingerprint);

    let instance_name = ports.instance_name();
    tracing::info!("Activating license key (instance: {})", instance_name);
    let mut minted = ports.activate(&key, &instance_name).await;
    let mut predecessor_instance_released = false;
    if mint_refusal_code(&minted)
        .is_some_and(|code| own_seat_retry_applies(&plan, same_key, existing_state.is_some(), code))
    {
        if let Some(own) = existing_state.as_ref() {
            tracing::info!(
                "Activation limit reached re-activating this license; releasing this machine's own instance {} and retrying",
                own.instance_id
            );
            ports.release_instance(&key, &own.instance_id).await;
            predecessor_instance_released = true;
            minted = ports.activate(&key, &instance_name).await;
        }
    }
    let result = match minted {
        Ok(result) => result,
        Err(error) => {
            ports.audit(audit_detail.clone(), "fail");
            return Err(activation_error_from_raw(&error));
        }
    };
    if !result.valid {
        ports.audit(audit_detail.clone(), "fail");
        if let Some(instance_id) = result.instance_id.as_deref() {
            ports.release_instance(&key, instance_id).await;
        }
        return Err(provider_refusal_error(
            result.error.as_deref().unwrap_or("Activation failed"),
        ));
    }
    let instance_id = match result.instance_id {
        Some(instance_id) => instance_id,
        None => {
            ports.audit(audit_detail.clone(), "fail");
            return Err(activation_error(
                LicenseActivationErrorCode::MissingInstanceId,
            ));
        }
    };
    let tier = ports.tier_for_variant(result.variant_id);
    if tier == Tier::Free {
        ports.release_instance(&key, &instance_id).await;
        ports.audit(audit_detail.clone(), "fail");
        return Err(activation_error(LicenseActivationErrorCode::VariantUnknown));
    }
    let now = ports.now_iso();
    let state = LicenseState {
        license_key: key.clone(),
        instance_id: instance_id.clone(),
        variant_id: result.variant_id,
        tier,
        status: result.status,
        last_validated_at: now.clone(),
        activated_at: now,
        expires_at: result.expires_at,
    };

    match &plan {
        ActivationPlan::Teardown { .. } => {
            if let (Some(old_state), Some(old_key)) =
                (existing_state.as_ref(), existing_key.as_deref())
            {
                ports
                    .release_credential(old_key, &old_state.instance_id, "the predecessor teardown")
                    .await;
                if !predecessor_instance_released {
                    ports
                        .release_instance(old_key, &old_state.instance_id)
                        .await;
                }
            }
        }
        ActivationPlan::Fresh => {
            if let Some(orphaned) = existing_state.as_ref() {
                ports
                    .release_credential(
                        &state.license_key,
                        &orphaned.instance_id,
                        "the orphaned-row reclaim",
                    )
                    .await;
                if !predecessor_instance_released {
                    ports
                        .release_instance(&state.license_key, &orphaned.instance_id)
                        .await;
                }
            }
            if let Err(error) = ports.delete_catalog_token() {
                tracing::warn!(
                    "Stray catalog token could not be cleared; refusing to activate: {error}"
                );
                ports
                    .release_instance(&state.license_key, &state.instance_id)
                    .await;
                ports.audit(audit_detail, "fail");
                return Err(activation_error(LicenseActivationErrorCode::Incomplete));
            }
        }
        ActivationPlan::AlreadyActive => unreachable!("returned above"),
    }

    match ports
        .persist_generation(existing_state.as_ref(), existing_key.as_deref(), &state)
        .await
    {
        Ok(()) => {}
        Err(ActivationPersistenceError::Changed) => {
            tracing::info!(
                "License changed during activation; releasing this attempt's instance and leaving the winner installed"
            );
            ports.release_instance(&key, &instance_id).await;
            ports.audit(audit_detail, "fail");
            return Err(activation_error(
                LicenseActivationErrorCode::ChangedDuringActivation,
            ));
        }
        Err(ActivationPersistenceError::Storage) => {
            ports.release_instance(&key, &instance_id).await;
            ports.audit(audit_detail, "fail");
            return Err(activation_error(LicenseActivationErrorCode::Incomplete));
        }
    }
    if matches!(plan, ActivationPlan::Teardown { .. }) {
        if let (Some(old_state), Some(old_key)) = (existing_state.as_ref(), existing_key.as_deref())
        {
            ports
                .release_credential(old_key, &old_state.instance_id, "the raced-tick sweep")
                .await;
        }
    }
    if let Err(error) = ports.ensure_credential(&key, &instance_id).await {
        tracing::warn!(
            "Catalog activation unavailable; the refresh loop retries what is retryable: {error}"
        );
    }
    ports.request_catalog_refresh();
    tracing::info!("License activated: {} tier", tier);
    ports.audit(
        serde_json::json!({
            "key_fingerprint": key_fingerprint,
            "tier": tier.to_string(),
        }),
        "ok",
    );
    Ok(info_from_state(&state))
}

struct DesktopActivationPorts {
    app: AppHandle,
    db: Arc<Database>,
}

#[async_trait::async_trait]
impl ActivationPorts for DesktopActivationPorts {
    async fn read_state(&self) -> Result<Option<LicenseState>, String> {
        load_cached_license_state(&self.db).await.map_err(|error| {
            tracing::warn!(
                "License row unreadable; refusing to activate until the database answers: {error}"
            );
            error
        })
    }

    fn read_key(&self) -> Result<Option<String>, String> {
        crate::keyring::get_license_key(&self.app).map_err(|error| {
            tracing::warn!(
                "License key unreadable; refusing to activate until the keychain answers: {error}"
            );
            error
        })
    }

    async fn revalidate(&self) -> Result<LicenseInfo, String> {
        super::validation::validate_license_with_db(&self.app, &self.db, None).await
    }

    async fn confirm_replacement(
        &self,
        current_tier: &str,
    ) -> Result<(), ActivationConfirmationError> {
        crate::commands::confirm_sensitive_action(
            self.app.clone(),
            "Replace Active License?",
            crate::commands::SensitiveActionTone::Warning,
            format!(
                "SiteCMD already has an active {current_tier} license. Activating this different key will unlink the previous activation and replace the local license."
            ),
            "Replace License",
        )
        .await
        .map_err(|refusal| match refusal {
            crate::commands::SensitiveActionError::Declined => {
                tracing::info!("License replacement declined");
                ActivationConfirmationError::Declined
            }
            crate::commands::SensitiveActionError::Failed(error) => {
                tracing::warn!("License replacement dialog failed: {error}");
                ActivationConfirmationError::Failed
            }
        })
    }

    fn instance_name(&self) -> String {
        api::machine_instance_name()
    }

    async fn activate(&self, key: &str, instance_name: &str) -> Result<api::LicenseResult, String> {
        api::activate(key, instance_name).await
    }

    async fn release_instance(&self, key: &str, instance_id: &str) {
        release_orphaned_instance(&self.app, key, instance_id).await;
    }

    async fn release_credential(&self, key: &str, instance_id: &str, phase: &str) {
        release_predecessor_credential(&self.app, key, instance_id, phase).await;
    }

    fn delete_catalog_token(&self) -> Result<(), String> {
        crate::keyring::delete_catalog_token(&self.app)
    }

    async fn persist_generation(
        &self,
        expected_state: Option<&LicenseState>,
        expected_key: Option<&str>,
        state: &LicenseState,
    ) -> Result<(), ActivationPersistenceError> {
        let _generation = LICENSE_MUTATION.lock().await;
        let row_now = load_cached_license_state(&self.db).await.map_err(|error| {
            tracing::error!("license.activate could not re-read the license row: {error}");
            ActivationPersistenceError::Storage
        })?;
        let key_now = crate::keyring::get_license_key(&self.app).map_err(|error| {
            tracing::error!("license.activate could not re-read the license key: {error}");
            ActivationPersistenceError::Storage
        })?;
        let key_now = usable_key(key_now);
        if !snapshot_unchanged(
            expected_state.map(|row| row.instance_id.as_str()),
            row_now.as_ref(),
            expected_key,
            key_now.as_deref(),
        ) {
            return Err(ActivationPersistenceError::Changed);
        }

        if let Err(error) = crate::keyring::store_license_key(&self.app, &state.license_key) {
            tracing::error!("license.activate keyring store failed: {error}");
            return Err(ActivationPersistenceError::Storage);
        }

        let saved = state.clone();
        let attempted_instance = state.instance_id.clone();
        let save_result = {
            let db = self.db.clone();
            let attempt = crate::commands::run_blocking(move || {
                db.execute(move |conn| store::save(conn, &saved))
            })
            .await;
            record_license_write();
            match attempt {
                Ok(Ok(inner)) => inner,
                Ok(Err(dispatch)) => Err(format!("database dispatch failed: {dispatch}")),
                Err(join) => Err(join),
            }
        };
        if let Err(error) = save_result {
            tracing::error!("license.activate save failed: {error}");
            let key_restore = match expected_key {
                Some(old_key) => crate::keyring::store_license_key(&self.app, old_key),
                None => crate::keyring::delete_license_key(&self.app),
            };
            if let Err(restore_error) = key_restore {
                tracing::error!(
                    "license.activate keyring restore after save failure also failed: {restore_error}"
                );
            }

            let db = self.db.clone();
            let restore = row_now;
            let compensation = crate::commands::run_blocking(move || {
                db.execute(move |conn| {
                    let current = store::load(conn)?;
                    if current.as_ref().map(|row| row.instance_id.as_str())
                        != Some(attempted_instance.as_str())
                    {
                        return Ok(());
                    }
                    match restore.as_ref() {
                        Some(previous) => store::save(conn, previous),
                        None => store::clear(conn),
                    }
                })
            })
            .await;
            record_license_write();
            let compensated = match compensation {
                Ok(Ok(inner)) => inner,
                Ok(Err(dispatch)) => Err(format!("database dispatch failed: {dispatch}")),
                Err(join) => Err(join),
            };
            if let Err(restore_error) = compensated {
                tracing::error!(
                    "license.activate could not queue the row restore after a failed save: {restore_error}"
                );
            }
            return Err(ActivationPersistenceError::Storage);
        }

        Ok(())
    }

    fn tier_for_variant(&self, variant_id: u64) -> Tier {
        Tier::from_variant_id(variant_id)
    }

    fn now_iso(&self) -> String {
        now_iso()
    }

    async fn ensure_credential(&self, key: &str, instance_id: &str) -> Result<(), String> {
        crate::background::catalog_refresh::ensure_credential(&self.app, &self.db, key, instance_id)
            .await
            .map(|_| ())
    }

    fn request_catalog_refresh(&self) {
        crate::background::catalog_refresh::request_immediate_tick();
    }

    fn audit(&self, detail: serde_json::Value, outcome: &'static str) {
        crate::audit_log::record("license.activate", detail, outcome);
    }
}

/// Activate a license key, returning typed errors safe for the frontend.
#[tracing::instrument(skip(app, db, key))]
pub async fn activate_license(
    app: AppHandle,
    key: String,
    db: State<'_, Arc<Database>>,
) -> Result<LicenseInfo, String> {
    if let Err(error) = config::require_license_configured() {
        tracing::error!("license.activate refused: {error}");
        return Err(activation_error(LicenseActivationErrorCode::Incomplete));
    }

    let ports = DesktopActivationPorts {
        app,
        db: db.inner().clone(),
    };
    activate_license_with_ports(&ports, key).await
}

/// Decide whether activation can revalidate or must replace existing handles.
/// Upstream activation is not idempotent, so `AlreadyActive` must not mint.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ActivationPlan {
    /// Revalidate the installed active license without minting.
    AlreadyActive,
    /// Release usable predecessor handles before saving the new activation.
    Teardown { confirm: bool },
    /// No usable predecessor handles exist.
    Fresh,
}

pub(super) fn activation_plan(
    has_state: bool,
    has_key: bool,
    same_key: bool,
    is_active: bool,
) -> ActivationPlan {
    if has_state && has_key && same_key && is_active {
        return ActivationPlan::AlreadyActive;
    }
    if has_state && has_key {
        return ActivationPlan::Teardown {
            confirm: !same_key && is_active,
        };
    }
    ActivationPlan::Fresh
}

pub(super) fn license_replacement_required(existing_key: &str, incoming_key: &str) -> bool {
    license_key_fingerprint(existing_key) != license_key_fingerprint(incoming_key)
}

pub(super) fn license_key_fingerprint(key: &str) -> String {
    let digest = Sha256::digest(key.trim().as_bytes());
    format!("sha256:{}", hex::encode(&digest[..8]))
}

pub(super) fn license_activation_audit_detail(key_fingerprint: &str) -> serde_json::Value {
    serde_json::json!({ "key_fingerprint": key_fingerprint })
}

/// Release a predecessor credential and report an unrecoverable lost retry handle.
async fn release_predecessor_credential(
    app: &AppHandle,
    license_key: &str,
    instance_id: &str,
    phase: &str,
) {
    let outcome =
        crate::background::catalog_refresh::release_credential(app, license_key, instance_id).await;
    if outcome == crate::background::CatalogRelease::PendingLost {
        tracing::error!(
            "Catalog credential for instance {} could not be released during {} and its retry \
             could not be recorded; this seat is stranded until support frees it",
            instance_id,
            phase
        );
    }
}

#[cfg(test)]
#[path = "license_lifecycle_activation_tests.rs"]
mod behavior_tests;
