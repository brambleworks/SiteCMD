//! Shared cancellation state for interactive and background scans.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Tracks running and cancelled request IDs through a cloneable shared handle.
#[derive(Default, Clone)]
pub struct ScanControlState {
    inner: std::sync::Arc<ScanControlInner>,
}

/// Keep server-generated IDs outside the frontend's low-numbered range.
const SERVER_ALLOCATED_ID_BASE: u64 = 1 << 32;

#[derive(Default)]
struct ScanControlInner {
    next_request_id: AtomicU64,
    running_request_ids: Mutex<HashSet<u64>>,
    cancelled_request_ids: Mutex<HashSet<u64>>,
    /// Cancellations received before the request registers as running.
    pending_cancel_request_ids: Mutex<HashSet<u64>>,
}

/// Keeps one cancellation registration across every child of an execution.
pub(crate) struct ScanRequestGuard {
    state: ScanControlState,
    request_id: u64,
}

impl ScanRequestGuard {
    pub(crate) fn request_id(&self) -> u64 {
        self.request_id
    }
}

impl Drop for ScanRequestGuard {
    fn drop(&mut self) {
        self.state.finish_request(self.request_id);
    }
}

/// Lock a request-ID set, recovering safely from poison.
fn locked(set: &Mutex<HashSet<u64>>) -> std::sync::MutexGuard<'_, HashSet<u64>> {
    set.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl ScanControlState {
    pub(crate) fn begin_request(&self, requested_id: Option<u64>) -> u64 {
        let request_id = requested_id.unwrap_or_else(|| {
            SERVER_ALLOCATED_ID_BASE + self.inner.next_request_id.fetch_add(1, Ordering::SeqCst) + 1
        });
        locked(&self.inner.running_request_ids).insert(request_id);
        // Consume a pre-registration cancellation without leaking it to reused IDs.
        if locked(&self.inner.pending_cancel_request_ids).remove(&request_id) {
            locked(&self.inner.cancelled_request_ids).insert(request_id);
        } else {
            locked(&self.inner.cancelled_request_ids).remove(&request_id);
        }
        request_id
    }

    pub(crate) fn begin_execution(&self, requested_id: Option<u64>) -> ScanRequestGuard {
        ScanRequestGuard {
            state: self.clone(),
            request_id: self.begin_request(requested_id),
        }
    }

    /// Record cancellation intent and report whether the request is running.
    /// Intent for an unregistered id is parked to cover start/cancel races.
    pub(crate) fn request_cancel(&self, request_id: u64) -> bool {
        let is_running = locked(&self.inner.running_request_ids).contains(&request_id);
        if is_running {
            locked(&self.inner.cancelled_request_ids).insert(request_id);
        } else {
            locked(&self.inner.pending_cancel_request_ids).insert(request_id);
        }
        is_running
    }

    pub fn is_cancelled(&self, request_id: u64) -> bool {
        locked(&self.inner.cancelled_request_ids).contains(&request_id)
    }

    pub(crate) fn finish_request(&self, request_id: u64) {
        locked(&self.inner.running_request_ids).remove(&request_id);
        locked(&self.inner.cancelled_request_ids).remove(&request_id);
        // A cancel that raced the end of the run has nothing left to cancel,
        // and leaving it parked would abort the next execution to reuse the id.
        locked(&self.inner.pending_cancel_request_ids).remove(&request_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_request_assigns_unique_monotonic_ids_when_none_supplied() {
        let state = ScanControlState::default();
        let a = state.begin_request(None);
        let b = state.begin_request(None);
        let c = state.begin_request(None);
        assert!(a < b && b < c, "expected monotonic IDs, got {a}, {b}, {c}");
    }

    #[test]
    fn begin_request_uses_supplied_id_verbatim() {
        let state = ScanControlState::default();
        let id = state.begin_request(Some(42));
        assert_eq!(id, 42);
        assert!(!state.is_cancelled(42));
    }

    #[test]
    fn cancel_then_observe_then_finish_clears_state() {
        let state = ScanControlState::default();
        let id = state.begin_request(None);

        assert!(state.request_cancel(id), "running id should be cancellable");
        assert!(state.is_cancelled(id));

        state.finish_request(id);
        // After finish, the cancelled flag must be cleared so a future
        // begin_request that happens to reuse the ID is not silently aborted.
        assert!(!state.is_cancelled(id));
    }

    #[test]
    fn cancel_for_unknown_request_returns_false_and_does_not_set_flag() {
        let state = ScanControlState::default();
        assert!(!state.request_cancel(999));
        // Parked, not applied: nothing is running under this id, so there is
        // nothing for `is_cancelled` to report until one registers.
        assert!(!state.is_cancelled(999));
    }

    #[test]
    fn cancel_before_registration_is_honored_when_the_request_begins() {
        let state = ScanControlState::default();

        assert!(!state.request_cancel(5), "not running yet");
        let id = state.begin_request(Some(5));

        assert!(
            state.is_cancelled(id),
            "a cancel that arrived first must survive registration"
        );
    }

    #[test]
    fn a_consumed_pending_cancel_does_not_apply_twice() {
        let state = ScanControlState::default();
        state.request_cancel(5);
        let id = state.begin_request(Some(5));
        assert!(state.is_cancelled(id));
        state.finish_request(id);

        // The next execution under this id starts clean; the parked intent was
        // spent by the registration above.
        let reused = state.begin_request(Some(5));
        assert!(!state.is_cancelled(reused));
    }

    #[test]
    fn finish_discards_a_cancel_that_raced_the_end_of_the_run() {
        let state = ScanControlState::default();
        let id = state.begin_request(Some(5));
        state.finish_request(id);
        // Arrives too late to cancel anything, and must not be left parked
        // where it would abort an execution that reuses the id.
        state.request_cancel(id);
        state.finish_request(id);

        let reused = state.begin_request(Some(5));
        assert!(!state.is_cancelled(reused));
    }

    #[test]
    fn server_allocated_ids_cannot_collide_with_frontend_ids() {
        let state = ScanControlState::default();
        let server = state.begin_request(None);
        assert!(
            server > SERVER_ALLOCATED_ID_BASE,
            "server ids must sit above the frontend's range, got {server}"
        );

        let frontend = state.begin_request(Some(1));
        state.request_cancel(frontend);
        assert!(state.is_cancelled(frontend));
        assert!(
            !state.is_cancelled(server),
            "cancelling a user scan must not cancel a scheduled one"
        );
    }

    #[test]
    fn double_cancel_is_idempotent() {
        let state = ScanControlState::default();
        let id = state.begin_request(None);
        assert!(state.request_cancel(id));
        assert!(state.request_cancel(id));
        assert!(state.is_cancelled(id));
    }

    #[test]
    fn double_finish_is_idempotent() {
        let state = ScanControlState::default();
        let id = state.begin_request(None);
        state.finish_request(id);
        state.finish_request(id);
        assert!(!state.is_cancelled(id));
    }

    #[test]
    fn begin_after_finish_with_same_id_starts_uncancelled() {
        let state = ScanControlState::default();
        let id = state.begin_request(Some(7));
        state.request_cancel(id);
        state.finish_request(id);

        let id2 = state.begin_request(Some(7));
        assert_eq!(id2, 7);
        assert!(
            !state.is_cancelled(id2),
            "reused id must not inherit cancelled flag from prior run"
        );
    }

    #[test]
    fn cancel_after_finish_does_nothing() {
        let state = ScanControlState::default();
        let id = state.begin_request(None);
        state.finish_request(id);
        assert!(
            !state.request_cancel(id),
            "cancel for finished id should report not-running"
        );
        assert!(!state.is_cancelled(id));
    }

    #[test]
    fn clone_shares_underlying_state() {
        let state = ScanControlState::default();
        let id = state.begin_request(None);
        let mirror = state.clone();
        assert!(state.request_cancel(id));
        assert!(
            mirror.is_cancelled(id),
            "clones must share the same Arc-backed state"
        );
    }

    #[test]
    fn execution_guard_keeps_cancellation_until_the_whole_execution_finishes() {
        let state = ScanControlState::default();
        let guard = state.begin_execution(Some(77));
        let request_id = guard.request_id();

        assert!(state.request_cancel(request_id));
        assert!(state.is_cancelled(request_id));

        // Child boundaries do not call finish_request. The same cancellation
        // flag remains visible to every collector in the execution.
        assert!(state.is_cancelled(request_id));
        drop(guard);

        assert!(!state.is_cancelled(request_id));
        assert!(!state.request_cancel(request_id));
    }
}
