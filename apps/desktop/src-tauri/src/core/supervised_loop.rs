//! Panic recovery for background schedulers.

use std::time::Duration;

/// Run a periodic synchronous tick with bounded exponential panic backoff.
pub async fn supervised_loop<F>(name: &'static str, period: Duration, mut tick: F)
where
    F: FnMut() + Send + 'static,
{
    let mut backoff = crate::constants::SUPERVISED_INITIAL_BACKOFF;
    loop {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut tick));
        match result {
            Ok(()) => {
                backoff = crate::constants::SUPERVISED_INITIAL_BACKOFF;
                tokio::time::sleep(period).await;
            }
            Err(payload) => {
                let msg = panic_message(payload.as_ref());
                tracing::error!(
                    scheduler = name,
                    backoff_ms = backoff.as_millis() as u64,
                    "Scheduler tick panicked: {msg}; restarting in {backoff:?}"
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(crate::constants::SUPERVISED_MAX_BACKOFF);
            }
        }
    }
}

/// Supervise a factory-produced async loop with panic backoff.
/// The future owns its normal pacing; the supervisor sleeps only after failure.
pub async fn supervised_loop_async<F, Fut>(name: &'static str, mut factory: F)
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    use futures_util::FutureExt;

    let mut backoff = crate::constants::SUPERVISED_INITIAL_BACKOFF;
    loop {
        let fut = factory();
        // `AssertUnwindSafe` is sound here: any state the future captures is
        // already isolated by the supervisor restart contract - on panic we
        // drop and rebuild via `factory`.
        let result = std::panic::AssertUnwindSafe(fut).catch_unwind().await;
        match result {
            Ok(()) => {
                tracing::warn!(
                    scheduler = name,
                    "Scheduler future returned cleanly; restarting"
                );
                backoff = crate::constants::SUPERVISED_INITIAL_BACKOFF;
                // Brief pause so a future that returns instantly doesn't spin.
                tokio::time::sleep(crate::constants::SUPERVISED_INITIAL_BACKOFF).await;
            }
            Err(payload) => {
                let msg = panic_message(payload.as_ref());
                tracing::error!(
                    scheduler = name,
                    backoff_ms = backoff.as_millis() as u64,
                    "Scheduler future panicked: {msg}; restarting in {backoff:?}"
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(crate::constants::SUPERVISED_MAX_BACKOFF);
            }
        }
    }
}

/// Format a `catch_unwind` payload as a human-readable string.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}
