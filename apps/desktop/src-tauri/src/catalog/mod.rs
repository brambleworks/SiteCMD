//! Signed remediation catalog that can enrich but not alter findings.

pub mod activation;
pub mod fetch;
pub mod schema;
pub mod store;
pub mod verify;

pub use activation::{ActivationError, ActivationOutcome};
pub use fetch::{CatalogManifest, CatalogRequest, Channel, FetchError};
pub use schema::{CatalogPack, Effort, GuideEntry, SchemaError, SUPPORTED_SCHEMA_VERSION};
pub use store::StoreError;
pub use verify::{VerificationContext, VerifyError};

use std::path::Path;

/// Verify and activate a pack without replacing the active pack on failure.
pub fn verify_and_activate(
    app_data_dir: &Path,
    bytes: &[u8],
    manifest: &CatalogManifest,
    engine_version: &str,
) -> Result<CatalogPack, ActivateError> {
    let context = VerificationContext {
        engine_version,
        active_release_sequence: store::active_release_sequence(app_data_dir),
        active_pack_needs_repair: store::active_pack_needs_repair(app_data_dir),
        expected_content_hash: &manifest.content_hash,
        manifest_release_sequence: manifest.release_sequence,
        manifest_catalog_version: &manifest.catalog_version,
    };
    let signature = &manifest.signature;
    let pack = verify::verify_pack(bytes, signature, &context)?;
    store::activate(app_data_dir, bytes, pack.release_sequence)?;
    Ok(pack)
}

#[derive(Debug, thiserror::Error)]
pub enum ActivateError {
    #[error(transparent)]
    Verify(#[from] VerifyError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Resolve a guide using check-id prefix fallback and ordered variants.
/// Missing entries fall back to bundled content at the caller.
pub fn steps_for(
    pack: &CatalogPack,
    check_id: &str,
    variant_candidates: &[&str],
) -> Option<CatalogSteps> {
    let guide = resolve_guide_entry(pack, check_id)?;
    let steps = variant_candidates
        .iter()
        .find_map(|candidate| guide.frameworks.as_ref().and_then(|f| f.get(*candidate)))
        .unwrap_or(&guide.default)
        .clone();
    Some(CatalogSteps {
        steps,
        effort: guide.effort,
        effort_minutes: guide.effort_minutes,
    })
}

/// Catalog steps and effort metadata.
pub struct CatalogSteps {
    pub steps: Vec<String>,
    pub effort: Effort,
    pub effort_minutes: u32,
}

/// Exact match, then successively shorter dot-prefixes.
fn resolve_guide_entry<'a>(pack: &'a CatalogPack, check_id: &str) -> Option<&'a GuideEntry> {
    if let Some(guide) = pack.guides.get(check_id) {
        return Some(guide);
    }
    let mut parts: Vec<&str> = check_id.split('.').collect();
    while parts.len() > 1 {
        parts.pop();
        if let Some(guide) = pack.guides.get(&parts.join(".")) {
            return Some(guide);
        }
    }
    None
}

#[cfg(test)]
#[path = "packer_contract_tests.rs"]
mod packer_contract_tests;
