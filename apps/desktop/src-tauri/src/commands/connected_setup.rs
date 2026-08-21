//! Commands for site creation, ownership verification, and initial CI access.

use std::sync::Arc;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::catalog::activation::{self, ActivationOutcome, PendingNonceStore};
use crate::connected_service::ConnectedServiceClient;
use crate::db::Database;

use super::connected::{
    ConnectedCiToken, ConnectedRemoteState, ConnectedSiteChallenge, ConnectedVerification,
};
use super::{run_blocking, sanitize_error};

/// What exchanging the license for a connected-service credential produced.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedServiceActivation {
    pub tier: String,
}

/// Connect nonce, isolated from the catalog activation nonce.
struct KeyringConnectNonceStore<'a> {
    app: &'a AppHandle,
}

impl PendingNonceStore for KeyringConnectNonceStore<'_> {
    fn load(&self) -> Result<Option<activation::PendingActivation>, String> {
        crate::keyring::get_pending_connect_activation(self.app)
    }
    fn save(&self, pending: &activation::PendingActivation) -> Result<(), String> {
        crate::keyring::store_pending_connect_activation(self.app, pending)
    }
    fn clear(&self) {
        if let Err(error) = crate::keyring::delete_pending_connect_activation(self.app) {
            tracing::warn!("pending connect activation nonce could not be cleared: {error}");
        }
    }
}

/// Exchanges the keyring-backed license for an installation token and stores
/// the token for connected commands. The license is sent only to activation.
#[tracing::instrument(skip(app, db))]
pub async fn activate_connected_service(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
) -> Result<ConnectedServiceActivation, String> {
    // Serialized against license changes: exchanging a key that is mid-swap
    // would bind a connect credential to a subscription the desktop is about
    // to forget.
    let _generation = crate::licensing::commands::license_mutation().lock().await;

    let license_key = crate::keyring::get_license_key(&app)
        .map_err(sanitize_error)?
        .ok_or_else(|| {
            "activate your SiteCMD license first; the connected service comes with it".to_string()
        })?;
    let license_key = zeroize::Zeroizing::new(license_key);

    let db_read = Arc::clone(&db);
    let row = match run_blocking(move || db_read.execute(crate::licensing::store::load)).await {
        Ok(Ok(Ok(row))) => row,
        Ok(Ok(Err(error))) => return Err(sanitize_error(error)),
        Ok(Err(error)) => return Err(sanitize_error(error)),
        Err(error) => return Err(error),
    };
    let installation_id = row
        .map(|state| state.instance_id)
        .ok_or_else(|| "this desktop has no license activation to exchange".to_string())?;

    let nonces = KeyringConnectNonceStore { app: &app };
    match activation::obtain_connect_token(license_key.as_str(), &installation_id, &nonces).await {
        Ok(ActivationOutcome::Issued { token, tier }) => {
            // Store first, then clear the nonce: until the token is durably
            // ours, the nonce is the only handle on the credential the
            // service just committed.
            crate::keyring::store_connected_installation_token(&app, &token)
                .map_err(sanitize_error)?;
            PendingNonceStore::clear(&nonces);
            crate::audit_log::record(
                "connect.activate",
                serde_json::json!({ "installation": installation_id }),
                "ok",
            );
            Ok(ConnectedServiceActivation { tier })
        }
        Ok(ActivationOutcome::AlreadyActivated) => {
            crate::audit_log::record(
                "connect.activate",
                serde_json::json!({ "installation": installation_id }),
                "fail",
            );
            Err(
                "the connected service still reports an earlier attempt; try again in a moment"
                    .to_string(),
            )
        }
        Err(error) => {
            crate::audit_log::record(
                "connect.activate",
                serde_json::json!({ "installation": installation_id }),
                "fail",
            );
            Err(error.to_string())
        }
    }
}

/// The token the caller provided, or the one the license exchange stored.
/// The paste path stays for an operator moving a token deliberately; everyone
/// else connects with the license and never sees one.
pub(crate) fn resolve_installation_token(
    app: &AppHandle,
    provided: &str,
) -> Result<zeroize::Zeroizing<String>, String> {
    let trimmed = provided.trim();
    if !trimmed.is_empty() {
        return Ok(zeroize::Zeroizing::new(trimmed.to_string()));
    }
    match crate::keyring::get_connected_installation_token(app).map_err(sanitize_error)? {
        Some(stored) => Ok(zeroize::Zeroizing::new(stored)),
        None => Err(
            "connect with your SiteCMD license first, or paste an installation token".to_string(),
        ),
    }
}

/// Create the remote site before persisting its local binding.
/// If local setup fails, restore local state and delete the unreachable remote
/// site best-effort so a retry cannot collide with it.
#[tracing::instrument(skip(app, db, installation_token), fields(project_id))]
pub async fn create_connected_site(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    url: String,
    installation_token: String,
) -> Result<ConnectedSiteChallenge, String> {
    let installation_token = resolve_installation_token(&app, &installation_token)?;
    let db_read = Arc::clone(&db);
    let env_read = environment_scope_key.clone();
    let existing = run_blocking(move || db_read.get_connected_site(project_id, &env_read))
        .await?
        .map_err(sanitize_error)?;
    if existing.is_some() {
        return Err("this environment is already connected".into());
    }

    // Create the key before the site so site creation can register version 1's
    // commitment. Key bytes never leave the client.
    let mut key_bytes = [0_u8; sitecmd_engine::sync::FINGERPRINT_KEY_LEN];
    getrandom::fill(&mut key_bytes).map_err(|error| format!("OS RNG unavailable: {error}"))?;
    let commitment =
        sitecmd_engine::sync::ProjectFingerprintKey::from_bytes(key_bytes).commitment();

    let client =
        ConnectedServiceClient::configured(installation_token.trim()).map_err(sanitize_error)?;
    let created = client
        .create_site(url.trim(), None, &commitment)
        .await
        .map_err(sanitize_error)?;

    let db_connect = Arc::clone(&db);
    let env_connect = environment_scope_key.clone();
    let site_connect = created.id.clone();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let connected = match run_blocking(move || {
        db_connect.connect_site(project_id, &env_connect, &site_connect, now_ms)
    })
    .await
    {
        Ok(written) => written.map_err(sanitize_error),
        Err(error) => Err(error),
    };
    if let Err(error) = connected {
        let _ = client.delete_site(&created.id).await;
        return Err(error);
    }

    let prior_token = match crate::keyring::get_connected_installation_token(&app) {
        Ok(token) => token,
        Err(error) => {
            let _ = client.delete_site(&created.id).await;
            let _ = db.disconnect_site(project_id, &environment_scope_key);
            return Err(sanitize_error(error));
        }
    };
    let restore = |error: String| -> String {
        if let Some(token) = prior_token.as_deref() {
            let _ = crate::keyring::store_connected_installation_token(&app, token);
        } else {
            let _ = crate::keyring::delete_connected_installation_token(&app);
        }
        let _ = crate::keyring::delete_connected_site_secrets(&app, &db, project_id, &created.id);
        let _ = db.disconnect_site(project_id, &environment_scope_key);
        error
    };
    if let Err(error) =
        crate::keyring::store_connected_installation_token(&app, installation_token.as_str())
    {
        let _ = client.delete_site(&created.id).await;
        return Err(restore(sanitize_error(error)));
    }
    // Store the key matching the registered commitment or roll setup back; a
    // site without that key could never submit stable code identities.
    if let Err(error) =
        crate::keyring::store_project_fingerprint_key(&app, &db, project_id, &created.id, key_bytes)
    {
        let _ = client.delete_site(&created.id).await;
        return Err(restore(error));
    }
    Ok(ConnectedSiteChallenge {
        challenge: created.verification.challenge,
        dns_name: created.verification.dns.name,
        dns_type: created.verification.dns.record_type,
        phase: created.phase,
        site_id: created.id,
        url: created.url,
        well_known_path: created.verification.well_known.path,
    })
}

/// Fetch the authoritative remote verification phase and pending challenge.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn fetch_connected_site_state(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<ConnectedRemoteState, String> {
    let (client, site_id) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let state = client.state(&site_id).await.map_err(sanitize_error)?;
    let scope_revision = state.scope.as_ref().map(|scope| scope.scope_revision);
    let scope_routes = state
        .scope
        .as_ref()
        .map(|scope| scope.routes.clone())
        .unwrap_or_default();
    Ok(ConnectedRemoteState {
        challenge: state
            .verification
            .as_ref()
            .map(|challenge| ConnectedSiteChallenge {
                challenge: challenge.challenge.clone(),
                dns_name: challenge.dns.name.clone(),
                dns_type: challenge.dns.record_type.clone(),
                phase: state.phase.clone(),
                site_id: site_id.clone(),
                url: String::new(),
                well_known_path: challenge.well_known.path.clone(),
            }),
        event_sequence: state.event_sequence,
        phase: state.phase,
        scope_effective_route_count: state.scope_effective_route_count,
        scope_over_plan: state.scope_over_plan,
        scope_over_plan_grace_expires_at: state.scope_over_plan_grace_expires_at,
        scope_overflow_count: state.scope_overflow_count,
        scope_revision,
        scope_route_cap: state.scope_route_cap,
        scope_routes,
        site_id,
        site_allowance_over_plan: state.site_allowance_over_plan,
        site_allowance_over_plan_grace_expires_at: state.site_allowance_over_plan_grace_expires_at,
    })
}

/// Disconnect the remote site while retaining the local binding for resume.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn disconnect_connected_site(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<(), String> {
    let (client, site_id) =
        connected_client(&app, &db, project_id, environment_scope_key.clone()).await?;
    client.delete_site(&site_id).await.map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.disconnect",
        serde_json::json!({ "site": site_id }),
        "ok",
    );
    Ok(())
}

/// The one-time handle a remote erasure answers with.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedErasureReceipt {
    pub job_id: String,
    pub status_token: String,
}

/// Erase the remote site, unlink locally, and return the one-time status receipt.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn erase_connected_site(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<ConnectedErasureReceipt, String> {
    let (client, site_id) =
        connected_client(&app, &db, project_id, environment_scope_key.clone()).await?;
    let started = client.erase_site(&site_id).await.map_err(sanitize_error)?;
    // The remote data is gone; a local binding to it points at nothing. An
    // unlink failure here is reported, but the erasure already happened and
    // the receipt must reach the caller either way.
    let receipt = ConnectedErasureReceipt {
        job_id: started.job_id,
        status_token: started.status_token,
    };
    if let Err(error) =
        super::connected_transfer::unlink_connected_site(app, db, project_id, environment_scope_key)
            .await
    {
        tracing::warn!("local unlink after remote erasure failed: {error}");
    }
    Ok(receipt)
}

/// Ask the service to look for the published challenge.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn verify_connected_site(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    method: String,
) -> Result<ConnectedVerification, String> {
    if method != "dns_txt" && method != "well_known" {
        return Err("choose either the DNS record or the well-known file".into());
    }
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;
    let verification = client
        .verify_site(&site, &method)
        .await
        .map_err(sanitize_error)?;
    Ok(ConnectedVerification {
        phase: verification.phase,
        verified: verification.verified,
    })
}

/// Mint a one-time CI credential without storing it on this machine.
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn mint_connected_ci_token(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
    repository: String,
    workflow_ref: String,
    git_ref: String,
) -> Result<ConnectedCiToken, String> {
    let repository = repository.trim();
    let workflow_ref = workflow_ref.trim();
    let git_ref = git_ref.trim();
    validate_ci_pin_inputs(repository, workflow_ref, git_ref)?;
    let (client, site) = connected_client(&app, &db, project_id, environment_scope_key).await?;

    let (repository, repository_id, workflow_ref) = if workflow_ref.is_empty() {
        (repository.to_string(), None, None)
    } else {
        let github_token =
            super::integrations::github_access_token_for_project(&app, &db, project_id)?;
        let identity = crate::integrations::github::fetch_repository_identity(
            github_token.as_deref(),
            repository,
        )
        .await
        .map_err(sanitize_error)?;
        let canonical_workflow =
            canonical_workflow_ref(repository, &identity.full_name, workflow_ref)?;
        (
            identity.full_name,
            Some(identity.id),
            Some(canonical_workflow),
        )
    };
    let ordering_authority = if let (Some(repository_id), Some(workflow_ref)) =
        (repository_id.as_deref(), workflow_ref.as_deref())
    {
        let authority_id = ci_authority_id(repository_id, workflow_ref, git_ref);
        let state = client.state(&site).await.map_err(sanitize_error)?;
        match state.ordering_authority {
            Some(authority)
                if authority.kind == "publish_attestation"
                    && authority.authority_id == authority_id =>
            {
                Some(authority)
            }
            Some(authority) => {
                return Err(format!(
                    "this environment already uses deployment authority {} ({}); change it explicitly before creating a governing CI workflow",
                    authority.authority_id, authority.kind
                ));
            }
            None => Some(
                client
                    .select_publish_authority(&site, 0, &authority_id)
                    .await
                    .map_err(sanitize_error)?,
            ),
        }
    } else {
        None
    };
    let minted = client
        .mint_ci_token(
            &site,
            (!repository.is_empty()).then_some(repository.as_str()),
            repository_id.as_deref(),
            workflow_ref.as_deref(),
            (!git_ref.is_empty()).then_some(git_ref),
        )
        .await
        .map_err(sanitize_error)?;
    Ok(ConnectedCiToken {
        id: minted.id,
        repository: minted.repository,
        repository_id: minted.repository_id,
        ordering_authority_id: ordering_authority
            .as_ref()
            .map(|authority| authority.authority_id.clone()),
        ordering_authority_epoch: ordering_authority.as_ref().map(|authority| authority.epoch),
        site_id: minted.site,
        token: minted.token,
    })
}

fn ci_authority_id(repository_id: &str, workflow_ref: &str, git_ref: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"sitecmd-publish-authority-v1\0");
    digest.update(repository_id.as_bytes());
    digest.update(b"\0");
    digest.update(workflow_ref.as_bytes());
    digest.update(b"\0");
    digest.update(git_ref.as_bytes());
    format!("github:{repository_id}:{}", hex::encode(digest.finalize()))
}

fn validate_ci_pin_inputs(
    repository: &str,
    workflow_ref: &str,
    git_ref: &str,
) -> Result<(), String> {
    if workflow_ref.is_empty() {
        if git_ref.is_empty() {
            return Ok(());
        }
        return Err("a trusted ref requires a trusted workflow".to_string());
    }
    if repository.is_empty() {
        return Err("a trusted workflow requires its owner/repository".to_string());
    }
    if !git_ref.is_empty() && !git_ref.starts_with("refs/") {
        return Err("a trusted ref is written refs/...".to_string());
    }
    Ok(())
}

fn canonical_workflow_ref(
    requested_repository: &str,
    canonical_repository: &str,
    workflow_ref: &str,
) -> Result<String, String> {
    let relative = if let Some(relative) = workflow_ref.strip_prefix(".github/workflows/") {
        relative
    } else {
        let (workflow_repository, relative) = workflow_ref
            .split_once("/.github/workflows/")
            .ok_or_else(|| {
                "trusted workflow must be .github/workflows/<file>.yml inside this repository"
                    .to_string()
            })?;
        if !workflow_repository.eq_ignore_ascii_case(requested_repository)
            && !workflow_repository.eq_ignore_ascii_case(canonical_repository)
        {
            return Err(
                "trusted workflow must be .github/workflows/<file>.yml inside this repository"
                    .to_string(),
            );
        }
        relative
    };
    if relative.is_empty()
        || relative.contains('/')
        || relative.contains('\\')
        || relative.contains('@')
        || !(relative.ends_with(".yml") || relative.ends_with(".yaml"))
    {
        return Err(
            "trusted workflow must name a .yml or .yaml file under .github/workflows".to_string(),
        );
    }
    Ok(format!(
        "{canonical_repository}/.github/workflows/{relative}"
    ))
}

/// The authenticated client for an environment that is already connected, and
/// the site it is connected to.
pub(super) async fn connected_client(
    app: &AppHandle,
    db: &State<'_, Arc<Database>>,
    project_id: i64,
    environment_scope_key: String,
) -> Result<(ConnectedServiceClient, String), String> {
    let db_read = Arc::clone(db);
    let site = run_blocking(move || db_read.get_connected_site(project_id, &environment_scope_key))
        .await?
        .map_err(sanitize_error)?
        .ok_or_else(|| "this environment is not connected".to_string())?;
    let token = crate::keyring::get_connected_installation_token(app)
        .map_err(sanitize_error)?
        .ok_or_else(|| "no installation token is stored for this machine".to_string())?;
    let client = ConnectedServiceClient::configured(token.trim()).map_err(sanitize_error)?;
    Ok((client, site.site_id))
}

#[cfg(test)]
mod tests {
    use super::{canonical_workflow_ref, ci_authority_id, validate_ci_pin_inputs};

    #[test]
    fn ci_authority_is_stable_and_bound_to_the_exact_workflow_and_ref() {
        let authority = ci_authority_id(
            "1296269",
            "BrambleWorks/SiteCMD/.github/workflows/sitecmd.yml",
            "refs/heads/main",
        );
        assert!(authority.starts_with("github:1296269:"));
        assert_eq!(authority.len(), "github:1296269:".len() + 64);
        assert_eq!(
            authority,
            ci_authority_id(
                "1296269",
                "BrambleWorks/SiteCMD/.github/workflows/sitecmd.yml",
                "refs/heads/main",
            )
        );
        assert_ne!(
            authority,
            ci_authority_id(
                "1296269",
                "BrambleWorks/SiteCMD/.github/workflows/sitecmd.yml",
                "refs/heads/release",
            )
        );
    }

    #[test]
    fn workflow_pin_uses_the_canonical_repository_identity() {
        assert_eq!(
            canonical_workflow_ref(
                "brambleworks/sitecmd",
                "BrambleWorks/SiteCMD",
                ".github/workflows/sitecmd.yml",
            )
            .unwrap(),
            "BrambleWorks/SiteCMD/.github/workflows/sitecmd.yml"
        );
        assert_eq!(
            canonical_workflow_ref(
                "brambleworks/sitecmd",
                "BrambleWorks/SiteCMD",
                "BrambleWorks/SiteCMD/.github/workflows/sitecmd.yml",
            )
            .unwrap(),
            "BrambleWorks/SiteCMD/.github/workflows/sitecmd.yml"
        );
    }

    #[test]
    fn workflow_pin_ref_requires_a_workflow() {
        let error = validate_ci_pin_inputs("brambleworks/sitecmd", "", "refs/heads/main")
            .expect_err("a ref without a workflow must be refused locally");
        assert!(error.contains("trusted ref requires a trusted workflow"));
    }

    #[test]
    fn workflow_pin_must_name_a_direct_workflow_file() {
        assert!(canonical_workflow_ref(
            "brambleworks/sitecmd",
            "BrambleWorks/SiteCMD",
            ".github/workflows/releases/sitecmd.yml",
        )
        .is_err());
    }
}
