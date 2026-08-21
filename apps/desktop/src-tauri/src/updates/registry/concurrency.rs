//! Semaphore-bounded package-registry fan-out shared by all ecosystems.

use crate::updates::types::InstalledPackage;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Successful findings plus the count of unobserved package fetches.
/// Any failure makes absent packages unproven.
pub(crate) struct RegistryFanOut<T> {
    pub results: Vec<T>,
    pub failed: usize,
}

/// Fetches package metadata with bounded network concurrency.
///
/// `Ok(None)` is an observed clean result; errors and panics increment `failed`
/// so outages cannot resolve existing findings.
pub(crate) async fn check_registry_updates<T, Fut, F>(
    packages: &[InstalledPackage],
    concurrency_limit: usize,
    fetch: F,
) -> RegistryFanOut<T>
where
    T: Send + 'static,
    Fut: Future<Output = Result<Option<T>, String>> + Send + 'static,
    F: Fn(InstalledPackage) -> Fut,
{
    let semaphore = Arc::new(Semaphore::new(concurrency_limit));
    let mut handles = Vec::with_capacity(packages.len());

    for pkg in packages {
        let permit = semaphore.clone();
        let future = fetch(pkg.clone());
        handles.push(tokio::spawn(async move {
            let _permit = permit.acquire().await;
            future.await
        }));
    }

    let mut results = Vec::new();
    let mut failed = 0usize;
    for handle in handles {
        match handle.await {
            Ok(Ok(Some(value))) => results.push(value),
            Ok(Ok(None)) => {}
            Ok(Err(e)) => {
                tracing::warn!("updates: registry fetch failed: {}", e);
                failed += 1;
            }
            Err(e) => {
                tracing::warn!("updates: registry fetch task died: {}", e);
                failed += 1;
            }
        }
    }
    RegistryFanOut { results, failed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updates::types::Ecosystem;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn packages(count: usize) -> Vec<InstalledPackage> {
        (0..count)
            .map(|index| InstalledPackage {
                name: format!("pkg-{index}"),
                version: "1.0.0".into(),
                ecosystem: Ecosystem::Npm,
                source: "package-lock.json".into(),
                is_dev: false,
                workspace_members: Vec::new(),
            })
            .collect()
    }

    #[tokio::test]
    async fn honors_the_concurrency_cap_and_runs_every_item() {
        // N a multiple of the limit so the size-`limit` barrier drains in whole
        // waves without a short final wave deadlocking.
        const LIMIT: usize = 3;
        const N: usize = 9;

        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_in_flight = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(tokio::sync::Barrier::new(LIMIT));

        let in_flight_c = in_flight.clone();
        let max_c = max_in_flight.clone();
        let completed_c = completed.clone();
        let barrier_c = barrier.clone();

        let results = tokio::time::timeout(
            Duration::from_secs(5),
            check_registry_updates(&packages(N), LIMIT, move |pkg| {
                let in_flight = in_flight_c.clone();
                let max_in_flight = max_c.clone();
                let completed = completed_c.clone();
                let barrier = barrier_c.clone();
                async move {
                    let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    max_in_flight.fetch_max(current, Ordering::SeqCst);
                    barrier.wait().await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    completed.fetch_add(1, Ordering::SeqCst);

                    let index: usize = pkg.name.strip_prefix("pkg-").unwrap().parse().unwrap();
                    if index.is_multiple_of(2) {
                        Ok(Some(index))
                    } else {
                        Ok(None)
                    }
                }
            }),
        )
        .await
        .expect("fan-out did not deadlock within the timeout");

        // Cap honored exactly: never more than LIMIT concurrent, and the cap
        // was genuinely reached (barrier could not have released otherwise).
        assert_eq!(max_in_flight.load(Ordering::SeqCst), LIMIT);
        // Every package's task ran to completion.
        assert_eq!(completed.load(Ordering::SeqCst), N);
        // Only the `Ok(Some)` results survive; `Ok(None)` drops, nothing failed.
        assert_eq!(results.failed, 0);
        let mut collected = results.results;
        collected.sort_unstable();
        assert_eq!(collected, vec![0, 2, 4, 6, 8]);
    }

    #[tokio::test]
    async fn empty_input_yields_no_results() {
        let fan_out: RegistryFanOut<usize> =
            check_registry_updates(&packages(0), 4, |_pkg| async move { Ok(Some(1)) }).await;
        assert!(fan_out.results.is_empty());
        assert_eq!(fan_out.failed, 0);
    }

    #[tokio::test]
    async fn failed_fetches_are_counted_without_dropping_successes() {
        let fan_out = check_registry_updates(&packages(4), 4, |pkg| async move {
            let index: usize = pkg.name.strip_prefix("pkg-").unwrap().parse().unwrap();
            match index {
                0 => Ok(Some(index)),
                1 => Ok(None),
                _ => Err("registry returned status 503".to_string()),
            }
        })
        .await;

        assert_eq!(
            fan_out.results,
            vec![0],
            "the observed finding must survive"
        );
        assert_eq!(fan_out.failed, 2, "every failed fetch must be counted");
    }

    #[tokio::test]
    async fn panicked_fetch_task_counts_as_failed() {
        // A parser/client panic mid-fetch leaves that package unobserved, so
        // it must degrade the sweep exactly like a transport error.
        let fan_out: RegistryFanOut<usize> =
            check_registry_updates(&packages(1), 1, |_pkg| async move {
                if true {
                    panic!("boom");
                }
                Ok(None)
            })
            .await;
        assert!(fan_out.results.is_empty());
        assert_eq!(fan_out.failed, 1);
    }
}
