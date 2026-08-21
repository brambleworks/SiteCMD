//! Narrow transport contract for deployment-ordering setup and CI cursors.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use super::{local_error, ConnectedServiceClient, ConnectedServiceError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConnectedCurrentDeployment {
    #[serde(default)]
    pub commit_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConnectedOrderingAuthority {
    pub kind: String,
    pub authority_id: String,
    pub epoch: i64,
    #[serde(default)]
    pub current_deployment_id: Option<String>,
    #[serde(default)]
    pub publish_sequence: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiSubmissionAttestation {
    GithubOidc,
    Unattested,
}

fn legacy_submission_attestation() -> CiSubmissionAttestation {
    // Older connected-service deployments predate this discriminator and
    // always expected the CLI to request OIDC inside GitHub Actions. Preserve
    // that fail-loud behavior during a rolling service/client deployment.
    CiSubmissionAttestation::GithubOidc
}

#[derive(Debug, Serialize)]
struct OrderingAuthorityPutRequest<'a> {
    based_on_epoch: i64,
    kind: &'a str,
    authority_id: &'a str,
    seed_publish_sequence: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct OrderingAuthorityReceipt {
    ordering_authority: ConnectedOrderingAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CiDeploymentHead {
    #[serde(default)]
    pub current_deployment_id: Option<String>,
    #[serde(default = "legacy_submission_attestation")]
    pub submission_attestation: CiSubmissionAttestation,
    #[serde(default)]
    pub ordering_authority: Option<ConnectedOrderingAuthority>,
}

impl ConnectedServiceClient {
    pub async fn select_publish_authority(
        &self,
        site_id: &str,
        based_on_epoch: i64,
        authority_id: &str,
    ) -> Result<ConnectedOrderingAuthority, ConnectedServiceError> {
        let body = serde_json::to_string(&OrderingAuthorityPutRequest {
            authority_id,
            based_on_epoch,
            kind: "publish_attestation",
            seed_publish_sequence: 0,
        })
        .map_err(|_| {
            local_error(
                "serialization_failed",
                "deployment authority request could not be encoded",
            )
        })?;
        let url = self.url(&["v1", "sites", site_id, "ordering-authority"])?;
        let receipt: OrderingAuthorityReceipt =
            self.request(Method::PUT, url, None, Some(body)).await?;
        Ok(receipt.ordering_authority)
    }

    pub async fn ci_deployment_head(
        &self,
        site_id: &str,
    ) -> Result<CiDeploymentHead, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "deployment-head"])?;
        self.request(Method::GET, url, None, None).await
    }
}
