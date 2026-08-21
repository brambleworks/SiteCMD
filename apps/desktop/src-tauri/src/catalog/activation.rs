//! Exchange purchase licenses for opaque catalog credentials.
//!
//! License keys reach only the build-time activation service. Closed request
//! structs and shape tests keep ordinary catalog traffic key-free.

use serde::Serialize;

/// Base URL of the activation service, embedded at build time.
const ACTIVATION_ENDPOINT: Option<&str> = option_env!("SITECMD_ACTIVATION_ENDPOINT");

/// Closed outbound activation payload.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivateBody<'a> {
    license_key: &'a str,
    installation_id: &'a str,
    nonce: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeactivateBody<'a> {
    license_key: &'a str,
    installation_id: &'a str,
}

/// Successful activation outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationOutcome {
    /// A fresh, one-time credential.
    Issued { token: String, tier: String },
    /// The nonce was consumed but the client has no token.
    AlreadyActivated,
}

#[derive(Debug, thiserror::Error)]
pub enum ActivationError {
    #[error("no activation endpoint is configured in this build")]
    NoEndpointConfigured,
    #[error("activation endpoint is not a valid URL: {0}")]
    MalformedEndpoint(String),
    #[error("activation request failed: {0}")]
    Transport(String),
    /// Conclusive service refusal using a closed error code.
    #[error("activation refused: {reason}")]
    Refused { reason: String },
    /// Every credential slot is in use.
    #[error("credential limit reached: {active} of {cap} in use")]
    CredentialCapReached { active: u32, cap: u32 },
    /// Retryable service failure.
    #[error("activation service unavailable")]
    ServiceUnavailable,
    #[error("activation response is malformed: {0}")]
    MalformedResponse(String),
    /// The request was not sent because its nonce was not durable.
    #[error("activation nonce could not be persisted: {0}")]
    NonceNotPersisted(String),
    /// The request was not sent because pending nonce state was unreadable.
    #[error("activation nonce store is unreadable: {0}")]
    NonceStoreUnreadable(String),
}

impl ActivationError {
    /// Whether retrying can help without changing user input.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ActivationError::Transport(_)
                | ActivationError::ServiceUnavailable
                | ActivationError::NonceNotPersisted(_)
                | ActivationError::NonceStoreUnreadable(_)
        )
    }
}

fn base_url() -> Result<url::Url, ActivationError> {
    let raw = ACTIVATION_ENDPOINT.ok_or(ActivationError::NoEndpointConfigured)?;
    let parsed = url::Url::parse(raw.trim())
        .map_err(|error| ActivationError::MalformedEndpoint(error.to_string()))?;
    if parsed.scheme() != "https" {
        return Err(ActivationError::MalformedEndpoint(
            "activation endpoint must be https".to_string(),
        ));
    }
    Ok(parsed)
}

/// Mint a random idempotency nonce for one activation attempt.
pub fn fresh_nonce() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS RNG unavailable");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Unresolved nonce bound to a hashed license identity and installation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingActivation {
    pub nonce: String,
    /// SHA-256 of the license key, lowercase hex.
    pub license_key_hash: String,
    pub installation_id: String,
}

impl PendingActivation {
    pub fn mint(license_key: &str, installation_id: &str) -> Self {
        Self {
            nonce: fresh_nonce(),
            license_key_hash: crate::catalog::verify::sha256_hex(license_key.as_bytes()),
            installation_id: installation_id.to_string(),
        }
    }

    /// Whether this attempt belongs to the given identity.
    pub fn is_for(&self, license_key: &str, installation_id: &str) -> bool {
        self.license_key_hash == crate::catalog::verify::sha256_hex(license_key.as_bytes())
            && self.installation_id == installation_id
    }
}

/// Durable nonce storage for replay after an ambiguous response.
/// Unreadable or unwritable state prevents dispatch rather than risking a new slot.
pub trait PendingNonceStore: Sync {
    fn load(&self) -> Result<Option<PendingActivation>, String>;
    fn save(&self, pending: &PendingActivation) -> Result<(), String>;
    fn clear(&self);
}

/// Clear a nonce only when the response proves no credential holds a slot.
/// Issued, malformed, and degraded responses remain unresolved until the caller
/// durably stores the token or a later replay settles the attempt.
pub fn nonce_disposition(outcome: &Result<ActivationOutcome, ActivationError>) -> NonceAction {
    match outcome {
        Err(ActivationError::Refused { reason }) if known_refusal(reason) => NonceAction::Clear,
        Err(ActivationError::CredentialCapReached { .. }) => NonceAction::Clear,
        _ => NonceAction::Keep,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceAction {
    Keep,
    Clear,
}

async fn post_json<T: Serialize>(
    path: &[&str],
    body: &T,
) -> Result<(u16, String), ActivationError> {
    let mut url = base_url()?;
    url.path_segments_mut()
        .map_err(|_| {
            ActivationError::MalformedEndpoint("endpoint cannot have path segments".into())
        })?
        .extend(path);

    let response = crate::http_client::credentialed_service_client()
        .post(url)
        .json(body)
        .send()
        .await
        .map_err(|error| ActivationError::Transport(error.to_string()))?;

    let status = response.status().as_u16();
    let text = crate::http_client::read_text_limited(
        response,
        crate::constants::CATALOG_ACTIVATION_MAX_RESPONSE_BYTES,
        crate::constants::API_TIMEOUT_SHORT,
    )
    .await
    .map_err(|error| ActivationError::Transport(error.to_string()))?;
    Ok((status, text))
}

/// One activation attempt with the caller's nonce.
pub async fn activate(
    license_key: &str,
    installation_id: &str,
    nonce: &str,
) -> Result<ActivationOutcome, ActivationError> {
    let (status, body) = post_json(
        &["v1", "activate"],
        &ActivateBody {
            license_key,
            installation_id,
            nonce,
        },
    )
    .await?;
    parse_activate_response(status, &body)
}

/// Activate the connected service with the same closed exchange contract.
pub async fn activate_connect(
    license_key: &str,
    installation_id: &str,
    nonce: &str,
) -> Result<ActivationOutcome, ActivationError> {
    let (status, body) = post_json(
        &["v1", "connect", "activate"],
        &ActivateBody {
            license_key,
            installation_id,
            nonce,
        },
    )
    .await?;
    parse_activate_response(status, &body)
}

/// Release an installation credential idempotently.
pub async fn deactivate(license_key: &str, installation_id: &str) -> Result<u32, ActivationError> {
    let (status, body) = post_json(
        &["v1", "deactivate"],
        &DeactivateBody {
            license_key,
            installation_id,
        },
    )
    .await?;
    parse_deactivate_response(status, &body)
}

/// Obtain a token, replaying a pending nonce and repairing one stranded attempt.
pub async fn obtain_token(
    license_key: &str,
    installation_id: &str,
    nonces: &dyn PendingNonceStore,
) -> Result<ActivationOutcome, ActivationError> {
    let nonce = resolve_nonce(license_key, installation_id, nonces)?;

    let first = activate(license_key, installation_id, &nonce).await;
    let outcome = match first {
        Ok(ActivationOutcome::AlreadyActivated) => {
            // Keep the pending nonce until its stranded credential is released.
            deactivate(license_key, installation_id).await?;
            let retry = PendingActivation::mint(license_key, installation_id);
            nonces
                .save(&retry)
                .map_err(ActivationError::NonceNotPersisted)?;
            activate(license_key, installation_id, &retry.nonce).await
        }
        other => other,
    };

    if nonce_disposition(&outcome) == NonceAction::Clear {
        nonces.clear();
    }
    outcome
}

/// Reuse a matching durable nonce or persist a fresh one before dispatch.
fn resolve_nonce(
    license_key: &str,
    installation_id: &str,
    nonces: &dyn PendingNonceStore,
) -> Result<String, ActivationError> {
    match nonces
        .load()
        .map_err(ActivationError::NonceStoreUnreadable)?
    {
        Some(pending) if pending.is_for(license_key, installation_id) => Ok(pending.nonce),
        other => {
            if other.is_some() {
                nonces.clear();
            }
            let minted = PendingActivation::mint(license_key, installation_id);
            nonces
                .save(&minted)
                .map_err(ActivationError::NonceNotPersisted)?;
            Ok(minted.nonce)
        }
    }
}

/// Obtain a connected token, replacing one stranded attempt at most once.
pub async fn obtain_connect_token(
    license_key: &str,
    installation_id: &str,
    nonces: &dyn PendingNonceStore,
) -> Result<ActivationOutcome, ActivationError> {
    let nonce = resolve_nonce(license_key, installation_id, nonces)?;

    let first = activate_connect(license_key, installation_id, &nonce).await;
    let outcome = match first {
        Ok(ActivationOutcome::AlreadyActivated) => {
            let retry = PendingActivation::mint(license_key, installation_id);
            nonces
                .save(&retry)
                .map_err(ActivationError::NonceNotPersisted)?;
            activate_connect(license_key, installation_id, &retry.nonce).await
        }
        other => other,
    };

    if nonce_disposition(&outcome) == NonceAction::Clear {
        nonces.clear();
    }
    outcome
}

/// Closed refusal vocabulary safe for logs and user-facing errors.
const KNOWN_REFUSALS: [&str; 6] = [
    "malformed_request",
    "invalid_license",
    "wrong_store",
    "unknown_variant",
    "subscription_inactive",
    "credential_cap_reached",
];

/// Whether a refusal is authoritative rather than a degraded fallback.
pub fn known_refusal(reason: &str) -> bool {
    KNOWN_REFUSALS.contains(&reason)
}

fn refusal_code(reported: Option<&str>) -> String {
    match reported {
        Some(code) if KNOWN_REFUSALS.contains(&code) => code.to_string(),
        _ => "refused".to_string(),
    }
}

/// Pure; tested directly against the service's documented responses.
pub fn parse_activate_response(
    status: u16,
    body: &str,
) -> Result<ActivationOutcome, ActivationError> {
    #[derive(serde::Deserialize)]
    struct Body {
        token: Option<String>,
        tier: Option<String>,
        replayed: Option<bool>,
        error: Option<String>,
        #[serde(rename = "activeCredentials")]
        active_credentials: Option<u32>,
        cap: Option<u32>,
    }
    // Classify transient statuses before parsing potentially non-JSON edge responses.
    if status == 429 || (500..600).contains(&status) {
        return Err(ActivationError::ServiceUnavailable);
    }
    let parsed: Body = serde_json::from_str(body)
        .map_err(|error| ActivationError::MalformedResponse(error.to_string()))?;

    match status {
        200 => {
            if parsed.replayed == Some(true) {
                return Ok(ActivationOutcome::AlreadyActivated);
            }
            match (parsed.token, parsed.tier) {
                (Some(token), Some(tier)) => Ok(ActivationOutcome::Issued { token, tier }),
                _ => Err(ActivationError::MalformedResponse(
                    "success response carried no token".to_string(),
                )),
            }
        }
        // Only the complete service-defined 409 shape proves a credential cap.
        409 => match (
            parsed.error.as_deref(),
            parsed.active_credentials,
            parsed.cap,
        ) {
            (Some("credential_cap_reached"), Some(active), Some(cap)) => {
                Err(ActivationError::CredentialCapReached { active, cap })
            }
            _ => Err(ActivationError::Refused {
                reason: "refused".to_string(),
            }),
        },
        400 | 403 => Err(ActivationError::Refused {
            reason: refusal_code(parsed.error.as_deref()),
        }),
        other => Err(ActivationError::MalformedResponse(format!(
            "unexpected status {other}"
        ))),
    }
}

/// Pure; tested directly.
pub fn parse_deactivate_response(status: u16, body: &str) -> Result<u32, ActivationError> {
    #[derive(serde::Deserialize)]
    struct Body {
        released: Option<u32>,
        error: Option<String>,
    }
    // Classify transient statuses before parsing potentially non-JSON edge responses.
    if status == 429 || (500..600).contains(&status) {
        return Err(ActivationError::ServiceUnavailable);
    }
    let parsed: Body = serde_json::from_str(body)
        .map_err(|error| ActivationError::MalformedResponse(error.to_string()))?;
    match status {
        // A present count is required; zero validly means no matching credential.
        200 => parsed.released.ok_or_else(|| {
            ActivationError::MalformedResponse(
                "deactivation response carried no released count".to_string(),
            )
        }),
        400 | 403 => Err(ActivationError::Refused {
            reason: refusal_code(parsed.error.as_deref()),
        }),
        other => Err(ActivationError::MalformedResponse(format!(
            "unexpected status {other}"
        ))),
    }
}

/// Serialize the exact outbound activation shape for contract tests.
#[cfg(test)]
pub fn activate_body_json(license_key: &str, installation_id: &str, nonce: &str) -> String {
    serde_json::to_string(&ActivateBody {
        license_key,
        installation_id,
        nonce,
    })
    .expect("serializing a closed struct cannot fail")
}

#[cfg(test)]
#[path = "activation_tests.rs"]
mod tests;
