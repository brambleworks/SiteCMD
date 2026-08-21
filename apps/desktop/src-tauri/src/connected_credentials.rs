//! Connected-site CI and webhook credentials. Secrets appear only in mint,
//! rotate, and reconnect responses.

use serde::Deserialize;

use crate::connected_service::{ConnectedServiceClient, ConnectedServiceError};

/// Listed CI or webhook credential, including revoked tombstones.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SiteCredentialRow {
    pub id: String,
    /// `"ci"` or `"webhook"`, exactly as the service names them.
    pub kind: String,
    pub created_at: String,
    pub created_by: String,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub last_used_at: Option<String>,
    #[serde(default)]
    pub revoked_at: Option<String>,
    #[serde(default)]
    pub secret_fingerprint: Option<String>,
    #[serde(default)]
    pub secret_generation: Option<i64>,
    #[serde(default)]
    pub rotation_overlap_until: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SiteCredentialList {
    #[serde(default)]
    items: Vec<SiteCredentialRow>,
}

/// The minted webhook secret, readable exactly once here. It is derived, not
/// stored, so losing this answer means rotating.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MintedWebhookSecret {
    pub id: String,
    pub secret: String,
    pub secret_fingerprint: String,
    pub secret_generation: i64,
    pub created_at: String,
}

/// A rotated secret and the previous generation's overlap deadline.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RotatedWebhookSecret {
    pub id: String,
    pub secret: String,
    pub secret_fingerprint: String,
    pub secret_generation: i64,
    #[serde(default)]
    pub rotation_overlap_until: Option<String>,
}

/// One-time webhook secret returned only by a real reconnect transition.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReconnectWebhookSecret {
    pub id: String,
    pub secret: String,
    pub secret_generation: i64,
}

/// Provider deploy-trigger state without secret material.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeployTriggerState {
    pub provider: String,
    pub status: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub external_project_id: Option<String>,
    #[serde(default)]
    pub provisioned_at: Option<String>,
}

/// A resumed site. `webhook_secret` is present only when this call performed
/// the real transition and the disconnect had killed a secret to remint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReconnectedSite {
    pub phase: String,
    pub reconnected: bool,
    #[serde(default)]
    pub webhook_secret: Option<ReconnectWebhookSecret>,
    #[serde(default)]
    pub deploy_trigger: Option<DeployTriggerState>,
}

impl ConnectedServiceClient {
    /// Every credential the site holds, both kinds, tombstones included.
    pub async fn list_site_credentials(
        &self,
        site_id: &str,
    ) -> Result<Vec<SiteCredentialRow>, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "tokens"])?;
        let listing: SiteCredentialList =
            self.request(reqwest::Method::GET, url, None, None).await?;
        Ok(listing.items)
    }

    /// Mint one webhook secret per site; live secrets must use the rotation path.
    pub async fn mint_webhook_secret(
        &self,
        site_id: &str,
    ) -> Result<MintedWebhookSecret, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "tokens", "webhook"])?;
        self.request(reqwest::Method::POST, url, None, Some("{}".to_string()))
            .await
    }

    /// Rotate the webhook secret. CI tokens answer `409 not_rotatable`: they
    /// are reminted and revoked, never rotated.
    pub async fn rotate_site_credential(
        &self,
        site_id: &str,
        token_id: &str,
    ) -> Result<RotatedWebhookSecret, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "tokens", token_id, "rotate"])?;
        self.request(reqwest::Method::POST, url, None, Some("{}".to_string()))
            .await
    }

    /// Revoke either credential kind by its public handle. Repeating it is
    /// success, so callers retry without a read first.
    pub async fn revoke_site_credential(
        &self,
        site_id: &str,
        token_id: &str,
    ) -> Result<(), ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "tokens", token_id])?;
        self.no_content(reqwest::Method::DELETE, url).await
    }

    /// Resume a disconnected site inside its retention window.
    pub async fn reconnect_site(
        &self,
        site_id: &str,
    ) -> Result<ReconnectedSite, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "reconnect"])?;
        self.request(reqwest::Method::POST, url, None, Some("{}".to_string()))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_listing_reads_both_kinds_and_never_models_a_secret() {
        let listing: SiteCredentialList = serde_json::from_str(
            r#"{"items": [{
                "id": "cit_1",
                "kind": "ci",
                "created_at": "2026-08-01T00:00:00Z",
                "created_by": "inst_a",
                "repository": "acme/site",
                "last_used_at": "2026-08-09T00:00:00Z",
                "revoked_at": null
            }, {
                "id": "swh_1",
                "kind": "webhook",
                "created_at": "2026-08-02T00:00:00Z",
                "created_by": "inst_a",
                "revoked_at": "2026-08-08T00:00:00Z",
                "rotation_overlap_until": null,
                "secret_fingerprint": "sha256:0123456789abcdef",
                "secret_generation": 3,
                "secret": "must-never-be-modeled"
            }]}"#,
        )
        .expect("parse");
        assert_eq!(listing.items[0].kind, "ci");
        assert_eq!(listing.items[0].repository.as_deref(), Some("acme/site"));
        let hook = &listing.items[1];
        assert_eq!(hook.secret_generation, Some(3));
        assert!(hook.revoked_at.is_some());
        // The row type has no secret field to deserialize into: even a
        // service defect that echoed one could not reach a caller here.
        assert!(!format!("{hook:?}").contains("must-never-be-modeled"));
    }

    #[test]
    fn a_reconnect_answer_reads_with_and_without_the_reminted_secret() {
        let bare: ReconnectedSite =
            serde_json::from_str(r#"{"phase": "watching", "reconnected": true}"#).expect("parse");
        assert!(bare.webhook_secret.is_none());
        assert!(bare.deploy_trigger.is_none());

        let full: ReconnectedSite = serde_json::from_str(
            r#"{
                "phase": "watching",
                "reconnected": true,
                "webhook_secret": {"id": "swh_1", "secret": "sitecmd_whs_x", "secret_generation": 2},
                "deploy_trigger": {"provider": "vercel", "status": "provisioned",
                                   "external_project_id": "prj_9", "connection_id": "pc_1"}
            }"#,
        )
        .expect("parse");
        assert_eq!(
            full.webhook_secret.as_ref().map(|s| s.secret_generation),
            Some(2)
        );
        assert_eq!(
            full.deploy_trigger.as_ref().map(|t| t.status.as_str()),
            Some("provisioned")
        );
    }
}
