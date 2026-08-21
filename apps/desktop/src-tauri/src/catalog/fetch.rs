//! Catalog delivery client with a closed request-data boundary.

use serde::Deserialize;

use crate::constants::CATALOG_MAX_PACK_BYTES;

/// Base URL of the catalog service, embedded at build time. Absent in
/// development builds, where every fetch refuses rather than falling back to
/// some default host.
const CATALOG_ENDPOINT: Option<&str> = option_env!("SITECMD_CATALOG_ENDPOINT");

/// Whether this binary includes a catalog endpoint. This distinguishes an
/// unsupported build from one whose first download is still pending.
pub fn endpoint_configured() -> bool {
    CATALOG_ENDPOINT.is_some()
}

/// Which catalog stream to read. A closed set: the channel reaches the service
/// as a path segment, so an open string would let a caller shape the request
/// URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Stable,
}

impl Channel {
    fn as_str(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
        }
    }
}

/// Complete catalog request payload; fields remain private to prevent widening.
pub struct CatalogRequest {
    /// Opaque, server-issued. Not the purchase license key, which never
    /// reaches the catalog service.
    token: String,
    client_version: String,
    installed_catalog_version: Option<String>,
    channel: Channel,
}

impl CatalogRequest {
    pub fn new(
        token: impl Into<String>,
        client_version: impl Into<String>,
        installed_catalog_version: Option<String>,
        channel: Channel,
    ) -> Self {
        Self {
            token: token.into(),
            client_version: client_version.into(),
            installed_catalog_version,
            channel,
        }
    }

    /// The query pairs this request sends. Returned as a concrete list rather
    /// than an open map so the test can assert the complete set, and so a
    /// caller cannot append to it.
    fn query_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = vec![("client_version", self.client_version.clone())];
        if let Some(installed) = &self.installed_catalog_version {
            pairs.push(("catalog_version", installed.clone()));
        }
        pairs
    }
}

/// Catalog metadata returned by the service. Fetch locations come only from
/// the build-time endpoint, and unknown fields remain forward compatible.
/// Pack signatures and verification floors provide authenticity.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CatalogManifest {
    pub catalog_version: String,
    pub release_sequence: u64,
    pub published_at: String,
    pub content_hash: String,
    pub signature: String,
    pub minimum_engine_version: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("no catalog endpoint is configured in this build")]
    NoEndpointConfigured,
    #[error("catalog endpoint is not a valid URL: {0}")]
    MalformedEndpoint(String),
    #[error("catalog request failed: {0}")]
    Transport(String),
    #[error("catalog service rejected the entitlement token")]
    Unauthorized,
    #[error("catalog service returned HTTP {status}")]
    UnexpectedStatus { status: u16 },
    #[error("catalog manifest is malformed: {0}")]
    MalformedManifest(String),
    #[error("catalog pack exceeded the {CATALOG_MAX_PACK_BYTES} byte limit")]
    PackTooLarge,
}

fn base_url() -> Result<url::Url, FetchError> {
    let raw = CATALOG_ENDPOINT.ok_or(FetchError::NoEndpointConfigured)?;
    let parsed = url::Url::parse(raw.trim())
        .map_err(|error| FetchError::MalformedEndpoint(error.to_string()))?;
    if parsed.scheme() != "https" {
        return Err(FetchError::MalformedEndpoint(
            "catalog endpoint must be https".to_string(),
        ));
    }
    Ok(parsed)
}

/// Manifest update plus the service-authoritative credential tier.
///
/// `manifest` is `None` when the installed catalog is current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestAnswer {
    pub manifest: Option<CatalogManifest>,
    pub server_tier: Option<String>,
}

/// The credential tier named by a manifest response's headers, if any.
fn tier_header(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get("x-catalog-tier")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

/// Ask the service what the current pack is.
pub async fn fetch_manifest(request: &CatalogRequest) -> Result<ManifestAnswer, FetchError> {
    let mut url = base_url()?;
    url.path_segments_mut()
        .map_err(|_| FetchError::MalformedEndpoint("endpoint cannot have path segments".into()))?
        .extend(["v1", "catalog", request.channel.as_str(), "manifest"]);
    for (key, value) in request.query_pairs() {
        url.query_pairs_mut().append_pair(key, &value);
    }

    let response = crate::http_client::credentialed_service_client()
        .get(url)
        .bearer_auth(&request.token)
        .send()
        .await
        .map_err(|error| FetchError::Transport(error.to_string()))?;

    let server_tier = tier_header(&response);
    match response.status().as_u16() {
        // A current catalog still carries the authoritative tier header.
        304 => {
            return Ok(ManifestAnswer {
                manifest: None,
                server_tier,
            })
        }
        200 => {}
        401 | 403 => return Err(FetchError::Unauthorized),
        status => return Err(FetchError::UnexpectedStatus { status }),
    }

    let body = crate::http_client::read_text_limited(
        response,
        CATALOG_MAX_PACK_BYTES as u64,
        crate::constants::API_TIMEOUT_SHORT,
    )
    .await
    .map_err(|error| FetchError::Transport(error.to_string()))?;
    serde_json::from_str(&body)
        .map(|manifest| ManifestAnswer {
            manifest: Some(manifest),
            server_tier,
        })
        .map_err(|error| FetchError::MalformedManifest(error.to_string()))
}

/// Download a pack from the build-time endpoint for later manifest verification.
pub async fn fetch_pack(
    request: &CatalogRequest,
    content_hash: &str,
) -> Result<Vec<u8>, FetchError> {
    let mut url = base_url()?;
    url.path_segments_mut()
        .map_err(|_| FetchError::MalformedEndpoint("endpoint cannot have path segments".into()))?
        .extend(["v1", "catalog", request.channel.as_str(), "pack"]);
    url.query_pairs_mut()
        .append_pair("content_hash", content_hash);

    // Pack downloads need a longer total deadline than manifest requests.
    let response = crate::http_client::credentialed_service_client()
        .get(url)
        .timeout(crate::constants::CATALOG_DOWNLOAD_TIMEOUT)
        .bearer_auth(&request.token)
        .send()
        .await
        .map_err(|error| FetchError::Transport(error.to_string()))?;

    match response.status().as_u16() {
        200 => {}
        401 | 403 => return Err(FetchError::Unauthorized),
        status => return Err(FetchError::UnexpectedStatus { status }),
    }

    crate::http_client::read_body_limited(
        response,
        CATALOG_MAX_PACK_BYTES as u64,
        crate::constants::CATALOG_DOWNLOAD_TIMEOUT,
    )
    .await
    .map_err(|error| match error {
        // Preserve transport failures instead of mislabeling them as size errors.
        crate::http_client::BodyReadError::TooLarge { .. } => FetchError::PackTooLarge,
        other => FetchError::Transport(other.to_string()),
    })
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
