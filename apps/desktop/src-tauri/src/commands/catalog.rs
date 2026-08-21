//! Resolves remediation guidance from the signed catalog or bundled baseline.
//!
//! Missing catalogs fall back silently; corrupt stored packs remain errors.

use serde::Serialize;

use crate::catalog;

/// Resolved remediation guide plus source metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedGuide {
    pub steps: Vec<String>,
    pub source: GuideSource,
    /// Catalog effort metadata, absent for bundled guides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<catalog::Effort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort_minutes: Option<u32>,
    /// Catalog release the steps came from, when they came from a catalog.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuideSource {
    /// From the active intelligence catalog.
    Catalog,
    /// From the corpus compiled into this build.
    Bundled,
}

/// Resolve guidance for a check, preferring the active catalog.
///
/// Returns `None` only when neither source has anything, which the caller
/// renders as "no guidance for this check" rather than as a failure.
pub fn resolve_guide(
    app_data_dir: &std::path::Path,
    check_id: &str,
    variant_candidates: &[&str],
    bundled: Option<Vec<String>>,
) -> Option<ResolvedGuide> {
    // A corrupt pack must not silently fall through to the bundled corpus: a
    // subscriber would quietly start reading stale content with no signal.
    // `load_active` distinguishes absent from unreadable, and only absent
    // takes the fallback.
    match catalog::store::load_active(app_data_dir) {
        Ok(Some(pack)) => {
            if let Some(resolved) = catalog::steps_for(&pack, check_id, variant_candidates) {
                return Some(ResolvedGuide {
                    steps: resolved.steps,
                    source: GuideSource::Catalog,
                    effort: Some(resolved.effort),
                    effort_minutes: Some(resolved.effort_minutes),
                    catalog_version: Some(pack.catalog_version),
                });
            }
            // An active catalog with nothing for this check is normal: the
            // catalog enriches, it does not replace the engine's coverage.
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!("active catalog pack is unreadable, using bundled guidance: {error}");
        }
    }

    bundled.map(|steps| ResolvedGuide {
        steps,
        source: GuideSource::Bundled,
        effort: None,
        effort_minutes: None,
        catalog_version: None,
    })
}

/// What the UI needs to describe catalog state honestly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStatus {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// Set when a pack exists on disk but cannot be read. Distinct from
    /// `active: false`, which just means no pack has ever activated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Last catalog-credential block unrelated to licensing. The degraded
    /// `"refused"` code is retryable; other refusals are conclusive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_block: Option<crate::background::catalog_refresh::CredentialBlock>,
    /// Whether this build includes a catalog endpoint, distinguishing an
    /// unavailable feature from a download that has not completed yet.
    pub endpoint_configured: bool,
}

#[tauri::command]
#[tracing::instrument(skip(app))]
pub async fn get_catalog_status(app: tauri::AppHandle) -> Result<CatalogStatus, String> {
    let dir = app_data_dir(&app)?;
    let credential_block = crate::background::catalog_refresh::last_credential_block();
    let endpoint_configured = catalog::fetch::endpoint_configured();
    Ok(match catalog::store::load_active(&dir) {
        Ok(Some(pack)) => CatalogStatus {
            active: true,
            catalog_version: Some(pack.catalog_version),
            published_at: Some(pack.published_at),
            error: None,
            credential_block,
            endpoint_configured,
        },
        Ok(None) => CatalogStatus {
            active: false,
            catalog_version: None,
            published_at: None,
            error: None,
            credential_block,
            endpoint_configured,
        },
        Err(error) => CatalogStatus {
            active: false,
            catalog_version: None,
            published_at: None,
            error: Some(error.to_string()),
            credential_block,
            endpoint_configured,
        },
    })
}

/// Request an immediate catalog refresh after a license action.
#[tauri::command]
#[tracing::instrument]
pub async fn retry_catalog_refresh() {
    crate::background::catalog_refresh::request_immediate_tick();
}

/// Resolve catalog guidance before the caller's bundled fallback.
#[tauri::command]
#[tracing::instrument(skip(app, bundled))]
pub async fn resolve_fix_guide(
    app: tauri::AppHandle,
    check_id: String,
    variant_candidates: Option<Vec<String>>,
    bundled: Option<Vec<String>>,
) -> Result<Option<ResolvedGuide>, String> {
    let dir = app_data_dir(&app)?;
    let candidates: Vec<&str> = variant_candidates
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect();
    Ok(resolve_guide(&dir, &check_id, &candidates, bundled))
}

fn app_data_dir(_app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    crate::app_identity::default_storage_dir()
        .ok_or_else(|| "no application data directory is available".to_string())
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
