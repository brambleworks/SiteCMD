//! Catalog pack provenance, integrity, compatibility, and rollback checks.
//!
//! Verify the signature before parsing untrusted JSON; schema limits remain
//! necessary even for signed content.

use minisign_verify::{PublicKey, Signature};
use sha2::{Digest, Sha256};

use super::schema::{CatalogPack, SchemaError};
use crate::constants::CATALOG_MAX_PACK_BYTES;

/// Build-time catalog key, intentionally distinct from the updater key.
/// Missing keys make pack verification fail closed.
const CATALOG_PUBLIC_KEY: Option<&str> = option_env!("SITECMD_CATALOG_PUBLIC_KEY");

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error("no catalog signing key is configured in this build")]
    NoSigningKeyConfigured,
    #[error("catalog signing key is malformed: {0}")]
    MalformedSigningKey(String),
    #[error("catalog signature is malformed: {0}")]
    MalformedSignature(String),
    #[error("catalog signature does not verify against the embedded key")]
    SignatureMismatch,
    #[error("catalog pack is {found} bytes, above the {CATALOG_MAX_PACK_BYTES} byte limit")]
    PackTooLarge { found: usize },
    #[error("catalog content hash mismatch: manifest says {expected}, bytes hash to {actual}")]
    ContentHashMismatch { expected: String, actual: String },
    #[error("catalog is not valid JSON: {0}")]
    MalformedJson(String),
    #[error(transparent)]
    Schema(#[from] SchemaError),
    #[error("catalog needs engine {required} or newer, this engine is {current}")]
    EngineTooOld { required: String, current: String },
    #[error(
        "catalog release sequence {offered} is not newer than the active {active}; refusing rollback"
    )]
    RollbackRefused { offered: u64, active: u64 },
    #[error("catalog manifest {field} is {manifest}, but the signed pack says {pack}")]
    ManifestDisagrees {
        field: &'static str,
        manifest: String,
        pack: String,
    },
}

/// What the caller already trusts, used to judge an offered pack.
pub struct VerificationContext<'a> {
    /// Version of the running engine, for the minimum-engine check.
    pub engine_version: &'a str,
    /// Release sequence of the currently active pack, if any. `None` means no
    /// pack has ever activated, so any sequence is acceptable.
    pub active_release_sequence: Option<u64>,
    /// Whether the rollback floor stands without the pack it was raised for
    /// (corrupt, missing, or readable but behind the floor). Permits the
    /// equal-sequence repair download; never relaxes the strictly-lower
    /// rollback refusal.
    pub active_pack_needs_repair: bool,
    /// Content hash the manifest claims, lowercase hex SHA-256.
    pub expected_content_hash: &'a str,
    /// Unsigned manifest sequence, required to match the signed pack.
    pub manifest_release_sequence: u64,
    /// Catalog version the manifest advertised, checked against the signed pack
    /// for the same reason.
    pub manifest_catalog_version: &'a str,
}

/// Verify an offered pack end to end. Returns the parsed pack only when every
/// check passes; any failure leaves the caller's active pack untouched.
pub fn verify_pack(
    bytes: &[u8],
    signature: &str,
    context: &VerificationContext<'_>,
) -> Result<CatalogPack, VerifyError> {
    // Size first: everything below reads the whole buffer, so bound it before
    // hashing or parsing rather than after.
    if bytes.len() > CATALOG_MAX_PACK_BYTES {
        return Err(VerifyError::PackTooLarge { found: bytes.len() });
    }

    let key_source = CATALOG_PUBLIC_KEY.ok_or(VerifyError::NoSigningKeyConfigured)?;
    let public_key = PublicKey::from_base64(key_source.trim())
        .map_err(|error| VerifyError::MalformedSigningKey(error.to_string()))?;
    let signature = decode_wrapped_signature(signature)?;
    public_key
        .verify(bytes, &signature, false)
        .map_err(|_| VerifyError::SignatureMismatch)?;

    // The hash is redundant with the signature for tamper detection, and is
    // checked anyway: it catches a manifest and pack that were signed
    // separately and then paired wrongly, which a signature alone cannot see.
    let actual_hash = sha256_hex(bytes);
    if !actual_hash.eq_ignore_ascii_case(context.expected_content_hash) {
        return Err(VerifyError::ContentHashMismatch {
            expected: context.expected_content_hash.to_string(),
            actual: actual_hash,
        });
    }

    let pack: CatalogPack = serde_json::from_slice(bytes)
        .map_err(|error| VerifyError::MalformedJson(error.to_string()))?;
    pack.validate()?;

    // Confirm every unsigned manifest projection against the signed pack.
    if pack.release_sequence != context.manifest_release_sequence {
        return Err(VerifyError::ManifestDisagrees {
            field: "release_sequence",
            manifest: context.manifest_release_sequence.to_string(),
            pack: pack.release_sequence.to_string(),
        });
    }
    if pack.catalog_version != context.manifest_catalog_version {
        return Err(VerifyError::ManifestDisagrees {
            field: "catalog_version",
            manifest: context.manifest_catalog_version.to_string(),
            pack: pack.catalog_version.clone(),
        });
    }

    if !engine_meets_minimum(context.engine_version, &pack.minimum_engine_version) {
        return Err(VerifyError::EngineTooOld {
            required: pack.minimum_engine_version.clone(),
            current: context.engine_version.to_string(),
        });
    }

    if let Some(active) = context.active_release_sequence {
        if rollback_refused(
            pack.release_sequence,
            active,
            context.active_pack_needs_repair,
        ) {
            return Err(VerifyError::RollbackRefused {
                offered: pack.release_sequence,
                active,
            });
        }
    }

    Ok(pack)
}

/// Rejects lower release sequences and equal-sequence replays. An equal
/// sequence is allowed only to repair a missing, corrupt, or stale active pack.
fn rollback_refused(offered: u64, active: u64, active_pack_needs_repair: bool) -> bool {
    offered < active || (offered == active && !active_pack_needs_repair)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Decode the base64-wrapped minisign file emitted by the catalog packer.
fn decode_wrapped_signature(wrapped: &str) -> Result<Signature, VerifyError> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;

    let inner = STANDARD
        .decode(wrapped.trim())
        .map_err(|error| VerifyError::MalformedSignature(format!("outer wrapping: {error}")))?;
    let text = String::from_utf8(inner)
        .map_err(|error| VerifyError::MalformedSignature(format!("outer wrapping: {error}")))?;
    Signature::decode(&text).map_err(|error| VerifyError::MalformedSignature(error.to_string()))
}

/// Compare numeric `major.minor.patch`, ignoring suffixes and rejecting
/// unparseable versions.
fn engine_meets_minimum(engine: &str, minimum: &str) -> bool {
    match (version_triple(engine), version_triple(minimum)) {
        (Some(engine), Some(minimum)) => engine >= minimum,
        _ => false,
    }
}

fn version_triple(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let mut next = || parts.next().and_then(|part| part.parse::<u64>().ok());
    let triple = (next()?, next()?, next()?);
    // A trailing component means this is not a `major.minor.patch` at all, and
    // silently ignoring it would accept "1.2.3.4" as 1.2.3.
    parts.next().is_none().then_some(triple)
}

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;
