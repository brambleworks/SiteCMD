//! Catalog download, verification, and installation.

use super::*;

pub(super) async fn tick(app: &AppHandle, db: &Arc<Database>) -> TickOutcome {
    // Entitled license first: no license or a Free tier means the catalog
    // does not apply to this install.
    let state = {
        let db = db.clone();
        match crate::commands::run_blocking(move || db.execute(crate::licensing::store::load)).await
        {
            Ok(Ok(Ok(Some(state)))) => state,
            Ok(Ok(Ok(None))) => {
                // Clear credential state when no license remains.
                record_credential_block(None);
                return TickOutcome::NotApplicable;
            }
            Ok(Ok(Err(error))) => return TickOutcome::Failed(error),
            Ok(Err(error)) => return TickOutcome::Failed(error.to_string()),
            Err(error) => return TickOutcome::Failed(error.to_string()),
        }
    };

    // Use the licensing authority's effective tier so expired or disabled
    // licenses cannot keep fetching paid catalogs.
    if crate::licensing::access::effective_tier_from_state(&state)
        == crate::licensing::config::Tier::Free
    {
        record_credential_block(None);
        return TickOutcome::NotApplicable;
    }

    let token = match current_or_new_token(app, db, &state).await {
        Ok(Some(token)) => token,
        Ok(None) => return TickOutcome::NotApplicable,
        Err(reason) => return TickOutcome::NoCredential(reason),
    };

    let data_dir = match crate::app_identity::default_storage_dir() {
        Some(dir) => dir,
        None => return TickOutcome::Failed("no application data directory".to_string()),
    };
    let active_sequence = catalog::store::active_release_sequence(&data_dir);
    let active_needs_repair = catalog::store::active_pack_needs_repair(&data_dir);
    let installed_version = conditional_version(
        active_needs_repair,
        catalog::store::load_active(&data_dir)
            .ok()
            .flatten()
            .map(|pack| pack.catalog_version),
    );

    let request = CatalogRequest::new(
        token.clone(),
        env!("CARGO_PKG_VERSION").to_string(),
        installed_version,
        Channel::Stable,
    );

    let answer = match catalog::fetch::fetch_manifest(&request).await {
        Ok(answer) => answer,
        Err(FetchError::NoEndpointConfigured) => return TickOutcome::NotApplicable,
        Err(FetchError::Unauthorized) => {
            clear_rejected_token(app, &token).await;
            return TickOutcome::CredentialRejected;
        }
        Err(error) => return TickOutcome::Failed(error.to_string()),
    };

    // Adopt the server-authoritative tier for this license instance.
    if let Some(server_tier) = answer.server_tier.as_deref() {
        crate::licensing::commands::adopt_server_tier(db, &state.instance_id, server_tier).await;
    }

    let manifest = match answer.manifest {
        Some(manifest) => manifest,
        None => return TickOutcome::Current,
    };

    if !needs_download(
        manifest.release_sequence,
        active_sequence,
        active_needs_repair,
    ) {
        return TickOutcome::Current;
    }

    let bytes = match catalog::fetch::fetch_pack(&request, &manifest.content_hash).await {
        Ok(bytes) => bytes,
        // A token can be revoked between manifest and pack requests. Clear it on
        // either 401 so the next tick can reacquire credentials immediately.
        Err(FetchError::Unauthorized) => {
            clear_rejected_token(app, &token).await;
            return TickOutcome::CredentialRejected;
        }
        Err(error) => return TickOutcome::Failed(error.to_string()),
    };

    match catalog::verify_and_activate(&data_dir, &bytes, &manifest, env!("CARGO_PKG_VERSION")) {
        // The signed pack's sequence, not the manifest's. Verification has just
        // proved the two agree, so this is the same number, read from the side
        // that carries a signature.
        Ok(pack) => TickOutcome::Updated {
            sequence: pack.release_sequence,
        },
        Err(error) => TickOutcome::Failed(error.to_string()),
    }
}

/// Clear a rejected credential only if it still matches the presented token.
/// The credential lock prevents deleting a concurrent replacement.
async fn clear_rejected_token(app: &AppHandle, presented: &str) {
    let _guard = CREDENTIAL_LOCK.lock().await;
    match crate::keyring::get_catalog_token(app) {
        Ok(Some(current)) if current == presented => {
            if let Err(error) = crate::keyring::delete_catalog_token(app) {
                tracing::warn!("Failed to clear rejected catalog token: {error}");
            }
        }
        Ok(_) => {
            // Replaced or already gone: not this tick's to delete.
        }
        Err(error) => {
            tracing::warn!(
                "Could not verify the rejected catalog token; leaving the keyring untouched: {error}"
            );
        }
    }
}

/// The stored token, or a fresh one from the license when none is stored.
/// `Ok(None)` means this build has no activation endpoint.
async fn current_or_new_token(
    app: &AppHandle,
    db: &Arc<Database>,
    state: &crate::licensing::store::LicenseState,
) -> Result<Option<String>, String> {
    // Only confirmed absence may mint; unreadable credentials still hold a slot.
    match crate::keyring::get_catalog_token(app) {
        Ok(Some(token)) => return Ok(Some(token)),
        Ok(None) => {}
        Err(error) => return Err(format!("stored catalog token unreadable: {error}")),
    }
    // Reject empty keychain entries before requesting a credential.
    let license_key = crate::licensing::commands::usable_key(
        crate::keyring::get_license_key(app)
            .map_err(|error| format!("license key unavailable: {error}"))?,
    )
    .ok_or_else(|| "no license key in keyring".to_string())?;
    match ensure_credential(app, db, &license_key, &state.instance_id).await? {
        true => crate::keyring::get_catalog_token(app)
            .map_err(|error| format!("stored token unreadable: {error}")),
        false => Ok(None),
    }
}
