use tauri::State;

// Preserve the command-layer path for the shared cancellation registry.
pub use crate::core::scan_control::ScanControlState;

/// Set the cancel flag to stop an in-progress scan or multi-scan.
#[tauri::command]
#[tracing::instrument(skip(scan_control), fields(scan_request_id))]
pub async fn cancel_scan(
    scan_control: State<'_, ScanControlState>,
    scan_request_id: u64,
) -> Result<(), String> {
    if scan_control.request_cancel(scan_request_id) {
        tracing::info!(
            "Scan cancellation requested for request {}",
            scan_request_id
        );
    } else {
        tracing::info!(
            "Ignoring stale scan cancellation request for request {}",
            scan_request_id
        );
    }
    Ok(())
}
