use async_trait::async_trait;
use std::time::Duration;

use crate::db::alerts::AlertInput;
use crate::db::work_items::WorkItemInput;

pub mod cloudflare_adapter;
pub mod ga4_adapter;
pub mod gsc_adapter;
pub mod plausible_adapter;
pub mod psi_adapter;
pub mod updates_adapter;
mod updates_adapter_ssl;
mod updates_dependency_scan;
mod updates_install_scripts;
mod updates_licenses;
pub mod uptimerobot_adapter;

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("credentials missing: {0}")]
    MissingCredentials(String),
    #[error("authentication failed: {0}")]
    AuthFailed(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("rate limited")]
    RateLimited,
    #[error("other: {0}")]
    Other(String),
}

impl AdapterError {
    /// Classify embedded Google HTTP status text for scheduler retry policy.
    pub fn from_google_http_error(msg: impl Into<String>) -> Self {
        let msg = msg.into();
        if msg.contains("401") || msg.contains("403") {
            AdapterError::AuthFailed(msg)
        } else if msg.contains("429") {
            AdapterError::RateLimited
        } else {
            AdapterError::Transport(msg)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Credentials {
    pub api_key: Option<String>,
    pub oauth_token: Option<String>,
    /// Secondary identifier: Plausible site_id, Cloudflare zone_id, GSC site URL, GA4 property_id.
    pub site_id: Option<String>,
    /// Linked repository and keyring-resolved GitHub token.
    pub github: Option<GithubContext>,
    /// Whether configured GitHub context was unreadable this pass.
    /// Prevents missing credentials from false-resolving active CI items.
    pub github_unobservable: bool,
}

#[derive(Debug, Clone)]
pub struct GithubContext {
    pub owner: String,
    pub repo: String,
    pub token: String,
}

impl Credentials {
    #[tracing::instrument]
    pub fn empty() -> Self {
        Self::default()
    }
    #[tracing::instrument(skip(self))]
    pub fn has_api_key(&self) -> bool {
        self.api_key
            .as_deref()
            .map(|s| !s.trim().is_empty() && s != crate::keyring::KEYRING_PLACEHOLDER)
            .unwrap_or(false)
    }
    #[tracing::instrument(skip(self))]
    pub fn has_oauth_token(&self) -> bool {
        self.oauth_token
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }
    #[tracing::instrument(skip(self))]
    pub fn has_site_id(&self) -> bool {
        self.site_id
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct PollContext {
    pub project_id: i64,
    pub env_url: String,
    pub detected_stack: Option<String>,
    pub credentials: Credentials,
}

#[derive(Debug, Clone, Default)]
pub struct PollOutput {
    pub work_items: Vec<WorkItemInput>,
    pub alerts: Vec<AlertInput>,
    /// Whether the entire source was unobservable this poll.
    /// Prevents absent findings from being resolved without evidence.
    pub partial: bool,
    /// Unobservable signal families excluded from absence-based resolution.
    /// Prefer this over `partial` for sources with independent families.
    pub unobserved_signal_prefixes: Vec<String>,
}

#[async_trait]
pub trait IntegrationAdapter: Send + Sync {
    fn source(&self) -> &'static str;
    fn cadence(&self) -> Duration;
    /// Whether this adapter may poll the project. Defaults to fail closed.
    fn is_configured(&self, _credentials: &Credentials) -> bool {
        false
    }
    /// Whether polling varies by environment URL.
    /// Credential-scoped adapters return false to avoid duplicate alerts.
    fn env_scoped(&self) -> bool {
        true
    }
    async fn poll(&self, ctx: &PollContext) -> Result<PollOutput, AdapterError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAdapter;
    #[async_trait]
    impl IntegrationAdapter for MockAdapter {
        fn source(&self) -> &'static str {
            "mock"
        }
        fn cadence(&self) -> Duration {
            Duration::from_secs(60)
        }
        async fn poll(&self, _ctx: &PollContext) -> Result<PollOutput, AdapterError> {
            Ok(PollOutput::default())
        }
    }

    #[tokio::test]
    async fn mock_adapter_polls_successfully() {
        let ctx = PollContext {
            project_id: 1,
            env_url: "https://example.com".into(),
            detected_stack: None,
            credentials: Credentials::empty(),
        };
        let out = MockAdapter.poll(&ctx).await.unwrap();
        assert!(out.work_items.is_empty());
        assert!(out.alerts.is_empty());
    }

    #[test]
    fn adapter_error_variants_display() {
        assert!(AdapterError::MissingCredentials("psi".into())
            .to_string()
            .contains("psi"));
        assert!(AdapterError::AuthFailed("plausible".into())
            .to_string()
            .contains("authentication failed"));
        assert_eq!(AdapterError::RateLimited.to_string(), "rate limited");
    }

    #[test]
    fn google_http_error_classifies_status() {
        assert!(matches!(
            AdapterError::from_google_http_error("GSC query window returned 401 Unauthorized"),
            AdapterError::AuthFailed(_)
        ));
        assert!(matches!(
            AdapterError::from_google_http_error("Search Console returned 403 Forbidden - denied"),
            AdapterError::AuthFailed(_)
        ));
        assert!(matches!(
            AdapterError::from_google_http_error("returned 429 Too Many Requests"),
            AdapterError::RateLimited
        ));
        assert!(matches!(
            AdapterError::from_google_http_error("connection reset by peer"),
            AdapterError::Transport(_)
        ));
    }

    #[test]
    fn poll_output_defaults_to_complete_not_partial() {
        assert!(!PollOutput::default().partial);
    }

    #[test]
    fn credentials_placeholder_api_key_is_not_configured() {
        let credentials = Credentials {
            api_key: Some(crate::keyring::KEYRING_PLACEHOLDER.to_string()),
            oauth_token: None,
            site_id: Some("zone-id".to_string()),
            github: None,
            github_unobservable: false,
        };

        assert!(!credentials.has_api_key());
    }
}
