//! Brokered subscription-owner recovery commands.

use serde::Serialize;
use tauri::AppHandle;
use ts_rs::TS;

use super::connected_providers::installation_client;
use super::sanitize_error;

/// The pending recovery as shown: who asked, when it can complete, and
/// whether the alarm has demonstrably reached an owner-controlled channel.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedRecoveryState {
    pub id: String,
    pub status: String,
    pub requested_by: String,
    pub requested_at: String,
    pub eligible_at: String,
    pub exposure_demonstrated: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ConnectedRecoveryAnswer {
    pub recovery: Option<ConnectedRecoveryState>,
}

fn present(state: crate::connected_recovery::RecoveryState) -> ConnectedRecoveryState {
    ConnectedRecoveryState {
        eligible_at: state.eligible_at,
        exposure_demonstrated: state.exposure_demonstrated,
        id: state.id,
        requested_at: state.requested_at,
        requested_by: state.requested_by,
        status: state.status,
    }
}

/// Request admin recovery from this machine: the path back when every admin
/// device is gone. Converges on an existing pending request.
#[tracing::instrument(skip(app))]
pub async fn request_account_recovery(app: AppHandle) -> Result<ConnectedRecoveryState, String> {
    let client = installation_client(&app)?;
    let requested = client.request_recovery().await.map_err(sanitize_error)?;
    crate::audit_log::record(
        "connect.recovery_request",
        serde_json::json!({ "recovery": requested.recovery.id, "created": requested.created }),
        "ok",
    );
    Ok(present(requested.recovery))
}

/// The pending recovery, or none. A read with no side effects.
#[tracing::instrument(skip(app))]
pub async fn get_account_recovery(app: AppHandle) -> Result<ConnectedRecoveryAnswer, String> {
    let client = installation_client(&app)?;
    let state = client.recovery_state().await.map_err(sanitize_error)?;
    Ok(ConnectedRecoveryAnswer {
        recovery: state.recovery.map(present),
    })
}

/// The banner's acknowledgment: this machine has displayed the alarm.
#[tracing::instrument(skip(app))]
pub async fn acknowledge_account_recovery(
    app: AppHandle,
) -> Result<ConnectedRecoveryAnswer, String> {
    let client = installation_client(&app)?;
    let state = client
        .acknowledge_recovery()
        .await
        .map_err(sanitize_error)?;
    Ok(ConnectedRecoveryAnswer {
        recovery: state.recovery.map(present),
    })
}

/// Cancel the pending recovery: the owner saying "not mine".
#[tracing::instrument(skip(app))]
pub async fn cancel_account_recovery(app: AppHandle) -> Result<(), String> {
    let client = installation_client(&app)?;
    client.cancel_recovery().await.map_err(sanitize_error)?;
    crate::audit_log::record("connect.recovery_cancel", serde_json::json!({}), "ok");
    Ok(())
}
