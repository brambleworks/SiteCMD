//! Fingerprint-key rotation claims and aborts. Completion occurs through the
//! ordinary sync protocol.

use serde::Deserialize;

use crate::connected_service::{local_error, ConnectedServiceClient, ConnectedServiceError};

/// The claim the service holds after a successful rotation start.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RotationClaim {
    pub version: i64,
    pub commitment: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClaimedRotation {
    pub claim: RotationClaim,
}

/// The pending claim named by a `409 rotation_in_progress`, read from the
/// refusal's details: the machine holding it may be the account's other
/// desktop mid-rotation, and naming it is what makes the abort route usable.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PendingElsewhere {
    pub version: i64,
    #[serde(default)]
    pub claimed_by: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

pub enum ClaimOutcome {
    Claimed(RotationClaim),
    AlreadyPending(PendingElsewhere),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AbortedRotation {
    pub status: String,
}

impl ConnectedServiceClient {
    /// Claim the next key version, carrying the candidate's commitment. A
    /// rotation already pending is an outcome, not an error: the caller
    /// shows whose it is and offers the abort.
    pub async fn claim_key_rotation(
        &self,
        site_id: &str,
        commitment: &str,
    ) -> Result<ClaimOutcome, ConnectedServiceError> {
        let body = serde_json::to_string(&serde_json::json!({ "commitment": commitment }))
            .map_err(|_| {
                local_error(
                    "serialization_failed",
                    "rotation claim could not be encoded",
                )
            })?;
        let url = self.url(&["v1", "sites", site_id, "key-rotations"])?;
        match self
            .request::<ClaimedRotation>(reqwest::Method::POST, url, None, Some(body))
            .await
        {
            Ok(claimed) => Ok(ClaimOutcome::Claimed(claimed.claim)),
            Err(error) if error.code == "rotation_in_progress" => {
                let pending = error
                    .details
                    .as_ref()
                    .and_then(|details| details.get("pending"))
                    .and_then(|pending| {
                        serde_json::from_value::<PendingElsewhere>(pending.clone()).ok()
                    })
                    .ok_or(error)?;
                Ok(ClaimOutcome::AlreadyPending(pending))
            }
            Err(error) => Err(error),
        }
    }

    /// Clear the pending claim: the machine-lost case, so any assigned
    /// installation may. Repeating it converges on `no_pending_claim`.
    pub async fn abort_key_rotation(
        &self,
        site_id: &str,
    ) -> Result<AbortedRotation, ConnectedServiceError> {
        let url = self.url(&["v1", "sites", site_id, "key-rotations", "abort"])?;
        self.request(reqwest::Method::POST, url, None, Some("{}".to_string()))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pending_refusal_reads_the_claim_it_names() {
        let pending: PendingElsewhere = serde_json::from_str(
            r#"{"claimed_by": "inst_other", "commitment": "ab", "expires_at": "2026-08-13T12:00:00Z", "version": 3}"#,
        )
        .expect("parse");
        assert_eq!(pending.version, 3);
        assert_eq!(pending.claimed_by.as_deref(), Some("inst_other"));
    }
}
