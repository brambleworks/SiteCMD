//! Subscription-owner recovery requests and exposure acknowledgements.

use serde::Deserialize;

use crate::connected_service::{ConnectedServiceClient, ConnectedServiceError};

/// Pending recovery state reported by the service.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RecoveryState {
    pub id: String,
    pub status: String,
    pub requested_by: String,
    pub requested_at: String,
    pub eligible_at: String,
    pub exposure_demonstrated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RecoveryEnvelope {
    #[serde(default)]
    pub recovery: Option<RecoveryState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RequestedRecovery {
    pub created: bool,
    pub recovery: RecoveryState,
}

impl ConnectedServiceClient {
    /// Ask for admin recovery, or converge on the request that exists.
    pub async fn request_recovery(&self) -> Result<RequestedRecovery, ConnectedServiceError> {
        let url = self.url(&["v1", "account", "recovery"])?;
        self.request(reqwest::Method::POST, url, None, Some("{}".to_string()))
            .await
    }

    /// The pending recovery, or none. A read with no side effects.
    pub async fn recovery_state(&self) -> Result<RecoveryEnvelope, ConnectedServiceError> {
        let url = self.url(&["v1", "account", "recovery"])?;
        self.request(reqwest::Method::GET, url, None, None).await
    }

    /// The explicit, idempotent acknowledgment. Whether it becomes the
    /// exposure fact is the service's single guarded write to decide.
    pub async fn acknowledge_recovery(&self) -> Result<RecoveryEnvelope, ConnectedServiceError> {
        let url = self.url(&["v1", "account", "recovery", "ack"])?;
        self.request(reqwest::Method::POST, url, None, Some("{}".to_string()))
            .await
    }

    /// Cancel the pending recovery: the owner's response to the alarm.
    pub async fn cancel_recovery(&self) -> Result<(), ConnectedServiceError> {
        let url = self.url(&["v1", "account", "recovery"])?;
        self.no_content(reqwest::Method::DELETE, url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_answers_read_with_and_without_a_pending_state() {
        let none: RecoveryEnvelope = serde_json::from_str(r#"{"recovery": null}"#).expect("parse");
        assert!(none.recovery.is_none());

        let pending: RecoveryEnvelope = serde_json::from_str(
            r#"{"recovery": {"id": "rec_1", "status": "pending",
                 "requested_by": "inst_new", "requested_at": "2026-08-10T12:00:00.000Z",
                 "eligible_at": "2026-08-24T12:00:00.000Z", "exposure_demonstrated": false}}"#,
        )
        .expect("parse");
        let state = pending.recovery.expect("pending");
        assert_eq!(state.requested_by, "inst_new");
        assert!(!state.exposure_demonstrated);
    }
}
