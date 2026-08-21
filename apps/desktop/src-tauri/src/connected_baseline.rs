//! Connected verified-good profile reads and acceptance.

use serde::{Deserialize, Serialize};

use crate::connected_service::{ConnectedServiceClient, ConnectedServiceError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConnectedBaselineSource {
    pub source_observation_id: String,
    #[serde(default)]
    pub deployment_ref: Option<String>,
    pub engine_release: String,
    pub contract_digest: String,
    #[serde(default)]
    pub observed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConnectedBaselineField {
    pub field: String,
    #[serde(default)]
    pub good_digest: Option<String>,
    #[serde(default)]
    pub good_origin: Option<String>,
    #[serde(default)]
    pub recorded_at: Option<String>,
    #[serde(default)]
    pub accepted_at: Option<String>,
    #[serde(default)]
    pub frozen: bool,
    #[serde(default)]
    pub observed_digest: Option<String>,
    #[serde(default)]
    pub observed_source: Option<ConnectedBaselineSource>,
    #[serde(default)]
    pub drift_first_seen_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConnectedBaselineProfile {
    pub profile_revision: i64,
    #[serde(default)]
    pub fields: Vec<ConnectedBaselineField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConnectedBaselineAccepted {
    pub profile_revision: i64,
    pub good_digest: String,
}

#[derive(Serialize)]
struct AcceptBaselineRequest<'a> {
    field: &'a str,
    based_on_profile_revision: i64,
    expected_value_digest: &'a str,
}

impl ConnectedServiceClient {
    pub async fn verified_good_profile(
        &self,
        site_id: &str,
    ) -> Result<ConnectedBaselineProfile, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "verified-good"])?;
        self.request(reqwest::Method::GET, url, None, None).await
    }

    pub async fn accept_verified_good(
        &self,
        site_id: &str,
        field: &str,
        revision: i64,
        expected_digest: &str,
        idempotency_key: &str,
    ) -> Result<ConnectedBaselineAccepted, ConnectedServiceError> {
        let body = serde_json::to_string(&AcceptBaselineRequest {
            field,
            based_on_profile_revision: revision,
            expected_value_digest: expected_digest,
        })
        .map_err(|_| {
            crate::connected_service::local_error(
                "serialization_failed",
                "baseline acceptance could not be encoded",
            )
        })?;
        let url = self.url(&["v1", "sites", site_id, "verified-good", "accept"])?;
        self.request(
            reqwest::Method::POST,
            url,
            Some(idempotency_key),
            Some(body),
        )
        .await
    }
}
