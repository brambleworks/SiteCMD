//! Long-running application loops composed above `core` and `commands`.

pub mod agent_request_watcher;
pub mod catalog_refresh;
pub mod connected_scope_sync;
pub mod fix_attempt_watcher;
pub mod retention_sweep;
pub mod scan_scheduler;

/// Catalog credential release outcome, ordered from strongest to weakest claim.
/// Callers must distinguish proven absence, refusal, and lost retry state.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CatalogRelease {
    /// The service released the seat.
    Released,
    /// The service proved no matching activation exists.
    NothingToRelease,
    /// Release failed with a durable retry tombstone.
    PendingRecorded,
    /// The service refused without proving the seat absent.
    RefusedUnreleased,
    /// Release and retry persistence both failed.
    PendingLost,
}
