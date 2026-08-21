//! Daily retention sweep for stores outside scan-history pruning.
//! Runs at startup and daily; `db::retention` owns delete logic and windows.

use std::sync::Arc;

use crate::commands;
use crate::constants::RETENTION_SWEEP_INTERVAL;
use crate::db::retention::RetentionStats;
use crate::db::Database;

/// Summarize nonempty retention sweeps for logging.
pub fn sweep_summary(stats: &RetentionStats) -> Option<String> {
    if stats.total() == 0 {
        return None;
    }
    Some(format!(
        "Retention sweep removed {} dismissed alert(s), {} old event(s), {} resolved signal item(s)",
        stats.dismissed_alerts, stats.old_events, stats.resolved_signal_items
    ))
}

/// Long-running loop: sweep once at startup, then every
/// `RETENTION_SWEEP_INTERVAL`. Spawned from `lib.rs` under
/// `supervised_loop_async` so a panicking sweep restarts with backoff.
pub async fn run(db: Arc<Database>) {
    loop {
        tick(&db).await;
        tokio::time::sleep(RETENTION_SWEEP_INTERVAL).await;
    }
}

/// Run one sweep on the blocking pool and log the outcome. A failed sweep is
/// logged and retried on the next interval.
async fn tick(db: &Arc<Database>) {
    let db = db.clone();
    let swept = commands::run_blocking(move || {
        db.run_retention_sweep(chrono::Utc::now().timestamp_millis())
    })
    .await;
    match swept {
        Ok(Ok(stats)) => {
            if let Some(summary) = sweep_summary(&stats) {
                tracing::info!("{summary}");
            }
        }
        Ok(Err(e)) => tracing::warn!("Retention sweep failed: {}", e),
        Err(e) => tracing::warn!("Retention sweep task failed: {}", e),
    }
}

#[cfg(test)]
#[path = "retention_sweep_tests.rs"]
mod tests;
