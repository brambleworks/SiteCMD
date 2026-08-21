//! Deploy-provider connections and provider-attested ownership. Provider
//! credentials never reach the desktop.

use serde::Deserialize;

use crate::connected_credentials::DeployTriggerState;
use crate::connected_service::{local_error, ConnectedServiceClient, ConnectedServiceError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProviderExternalAccount {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// A provider connection as reported by the service.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProviderConnectionRow {
    pub id: String,
    pub provider: String,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub activated_at: Option<String>,
    #[serde(default)]
    pub external_account: Option<ProviderExternalAccount>,
    #[serde(default)]
    pub granted_scopes: Option<String>,
    #[serde(default)]
    pub failed_reason: Option<String>,
    #[serde(default)]
    pub revoked_at: Option<String>,
    #[serde(default)]
    pub revoked_reason: Option<String>,
}

/// Browser authorization URL and scopes shown before opening the consent flow.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreatedProviderConnection {
    pub authorize_url: String,
    pub connection: ProviderConnectionRow,
    pub requested_scopes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ProviderConnectionList {
    #[serde(default)]
    items: Vec<ProviderConnectionRow>,
}

/// One project as the provider reports it through the connection.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProviderProject {
    pub external_project_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ProviderProjectList {
    #[serde(default)]
    items: Vec<ProviderProject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RevokedProviderConnection {
    pub connection_id: String,
    pub revoked_at: String,
}

/// Provider-attested ownership and deploy-trigger state.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProviderVerification {
    pub phase: String,
    pub verified: bool,
    #[serde(default)]
    pub deploy_trigger: Option<DeployTriggerState>,
}

impl ConnectedServiceClient {
    /// Start a connection round for one provider.
    pub async fn create_provider_connection(
        &self,
        provider: &str,
    ) -> Result<CreatedProviderConnection, ConnectedServiceError> {
        let body =
            serde_json::to_string(&serde_json::json!({ "provider": provider })).map_err(|_| {
                local_error(
                    "serialization_failed",
                    "provider connection request could not be encoded",
                )
            })?;
        let url = self.url(&["v1", "provider-connections"])?;
        self.request(reqwest::Method::POST, url, None, Some(body))
            .await
    }

    /// Every connection the account holds, terminal states included.
    pub async fn list_provider_connections(
        &self,
    ) -> Result<Vec<ProviderConnectionRow>, ConnectedServiceError> {
        let url = self.url(&["v1", "provider-connections"])?;
        let listing: ProviderConnectionList =
            self.request(reqwest::Method::GET, url, None, None).await?;
        Ok(listing.items)
    }

    /// The projects one active connection can see at the provider.
    pub async fn list_provider_projects(
        &self,
        connection_id: &str,
    ) -> Result<Vec<ProviderProject>, ConnectedServiceError> {
        let url = self.url(&["v1", "provider-connections", connection_id, "projects"])?;
        let listing: ProviderProjectList =
            self.request(reqwest::Method::GET, url, None, None).await?;
        Ok(listing.items)
    }

    /// Revoke a connection: the service deprovisions its deploy triggers
    /// first, while the credential can still act at the provider.
    pub async fn revoke_provider_connection(
        &self,
        connection_id: &str,
    ) -> Result<RevokedProviderConnection, ConnectedServiceError> {
        let url = self.url(&["v1", "provider-connections", connection_id])?;
        self.request(reqwest::Method::DELETE, url, None, None).await
    }

    /// Prove ownership through a provider project's own domain records, which
    /// also binds the project to the site and provisions its deploy trigger.
    pub async fn verify_site_provider(
        &self,
        site_id: &str,
        connection_id: &str,
        external_project_id: &str,
    ) -> Result<ProviderVerification, ConnectedServiceError> {
        let body = serde_json::to_string(&serde_json::json!({
            "connection_id": connection_id,
            "external_project_id": external_project_id,
            "method": "provider",
        }))
        .map_err(|_| {
            local_error(
                "serialization_failed",
                "provider verification request could not be encoded",
            )
        })?;
        let url = self.url(&["v1", "sites", site_id, "verify"])?;
        self.request(reqwest::Method::POST, url, None, Some(body))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_created_connection_carries_the_consent_facts_before_the_browser_opens() {
        let created: CreatedProviderConnection = serde_json::from_str(
            r#"{
                "authorize_url": "https://vercel.com/oauth/authorize?state=st_1",
                "connection": {
                    "id": "pc_1", "provider": "vercel", "status": "pending",
                    "created_at": "2026-08-10T00:00:00Z", "activated_at": null,
                    "external_account": null, "granted_scopes": null,
                    "failed_reason": null, "revoked_at": null, "revoked_reason": null,
                    "superseded_by": null
                },
                "requested_scopes": "read-write projects and deploy hooks"
            }"#,
        )
        .expect("parse");
        assert_eq!(created.connection.status, "pending");
        assert!(!created.requested_scopes.is_empty());
    }

    #[test]
    fn a_verification_answer_reads_with_and_without_the_trigger() {
        let bare: ProviderVerification =
            serde_json::from_str(r#"{"phase": "pending_bootstrap", "verified": true}"#)
                .expect("parse");
        assert!(bare.verified);
        assert!(bare.deploy_trigger.is_none());

        let full: ProviderVerification = serde_json::from_str(
            r#"{
                "phase": "pending_bootstrap", "verified": true,
                "deploy_trigger": {"provider": "netlify", "status": "provisioned",
                                    "external_project_id": "prj_2", "connection_id": "pc_1"}
            }"#,
        )
        .expect("parse");
        assert_eq!(
            full.deploy_trigger.as_ref().map(|t| t.status.as_str()),
            Some("provisioned")
        );
    }
}
