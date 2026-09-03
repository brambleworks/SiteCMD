//! Check-execution phases: the synchronous pass, the isolated solo pass, and
//! the concurrent pass. Split out of `scanner.rs` so the orchestration file
//! stays readable; the phases share the parent module's cancellation helpers
//! and progress emitter.

use super::{await_or_cancel, detector_crash_error, emit_progress, ensure_not_cancelled};
use crate::checks::{AsyncCheck, Check, CheckContext, CheckResult, CheckStatus, Severity};
use crate::constants::CHECK_TIMEOUT;
use crate::core::scanner::{ProgressFn, ScanError};
use futures_util::{future::try_join_all, FutureExt};

/// Run all synchronous checks with progress events
pub(super) fn run_sync_checks<C>(
    checks: &[Box<dyn Check>],
    ctx: &CheckContext,
    is_local: bool,
    progress: Option<&ProgressFn>,
    total: usize,
    checks_done: &mut usize,
    results: &mut Vec<CheckResult>,
    cancel_check: Option<&C>,
) -> Result<(), ScanError>
where
    C: Fn() -> bool + ?Sized,
{
    let mut completed_count = *checks_done;
    let mut completed_results = Vec::new();

    for check in checks {
        let _check_span = tracing::debug_span!(
            "sync_check",
            check_id = check.id(),
            category = ?check.category(),
        )
        .entered();
        ensure_not_cancelled(cancel_check)?;
        if is_local && check.skip_in_predeploy() {
            completed_count += 1;
            emit_progress(
                progress,
                check.id(),
                check.category(),
                "skipped",
                0,
                completed_count,
                total,
            );
            continue;
        }

        emit_progress(
            progress,
            check.id(),
            check.category(),
            "running",
            0,
            completed_count,
            total,
        );

        let check_results =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check.run(ctx))).map_err(
                |_| {
                    tracing::error!(
                        "Sync check {} panicked; aborting incomplete scan",
                        check.id()
                    );
                    detector_crash_error(check.id())
                },
            )?;
        ensure_not_cancelled(cancel_check)?;

        let count = check_results.len();
        completed_results.extend(check_results);
        completed_count += 1;

        emit_progress(
            progress,
            check.id(),
            check.category(),
            "complete",
            count,
            completed_count,
            total,
        );
    }

    results.extend(completed_results);
    *checks_done = completed_count;
    Ok(())
}

/// Run one async check under its timeout and panic guard. A timeout becomes a
/// skipped row; a panic aborts the scan rather than reporting partial results.
async fn run_guarded_async_check(
    check: &dyn AsyncCheck,
    ctx: &CheckContext,
) -> Result<Vec<CheckResult>, ScanError> {
    let id = check.id().to_string();
    let outcome = std::panic::AssertUnwindSafe(tokio::time::timeout(CHECK_TIMEOUT, check.run(ctx)))
        .catch_unwind()
        .await;
    match outcome {
        Ok(Ok(results)) => Ok(results),
        Ok(Err(_)) => {
            tracing::warn!("Async check {} timed out after {:?}", id, CHECK_TIMEOUT);
            Ok(vec![CheckResult {
                check_id: id.clone(),
                category: check.category(),
                title: format!("{} (timed out)", id),
                description: "This check timed out and was skipped.".into(),
                status: CheckStatus::Skipped,
                severity: Severity::Low,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            }])
        }
        Err(_) => {
            tracing::error!("Async check {} panicked; aborting incomplete scan", id);
            Err(detector_crash_error(&id))
        }
    }
}

/// Checks that must not share the network with the rest of the async phase.
///
/// `performance.ttfb` measures how long the origin takes to answer. Running it
/// inside the scanner's own request burst measured the burst instead: the asset
/// sampler alone opens `ASSET_FETCH_CONCURRENCY` connections, and against an
/// origin with a small accept backlog the timing request waited behind them.
/// Loopback fixtures reported 1 to 6 seconds that way, and re-measuring a live
/// origin graded at 1658 ms found 93 to 225 ms. These ids run alone, in order,
/// before anything else starts.
const SOLO_PHASE_CHECK_IDS: &[&str] = &["performance.ttfb"];

/// Run all async checks concurrently with per-check timeouts and progress events
pub(super) async fn run_async_checks<C>(
    checks: &[Box<dyn AsyncCheck>],
    ctx: &CheckContext,
    is_local: bool,
    progress: Option<&ProgressFn>,
    total: usize,
    checks_done: &mut usize,
    results: &mut Vec<CheckResult>,
    cancel_check: Option<&C>,
) -> Result<(), ScanError>
where
    C: Fn() -> bool + ?Sized,
{
    ensure_not_cancelled(cancel_check)?;

    let mut completed_count = *checks_done;
    let is_solo = |check: &dyn AsyncCheck| SOLO_PHASE_CHECK_IDS.contains(&check.id());
    let solo: Vec<&Box<dyn AsyncCheck>> = checks
        .iter()
        .filter(|check| is_solo(check.as_ref()))
        .collect();
    let concurrent: Vec<&Box<dyn AsyncCheck>> = checks
        .iter()
        .filter(|check| !is_solo(check.as_ref()))
        .collect();

    // Buffered, not written straight into `results`: a panic anywhere in this
    // function aborts the whole scan, and no partial row may escape with it.
    let mut solo_results: Vec<CheckResult> = Vec::new();
    for check in &solo {
        if is_local && check.skip_in_predeploy() {
            completed_count += 1;
            emit_progress(
                progress,
                check.id(),
                check.category(),
                "skipped",
                0,
                completed_count,
                total,
            );
            tracing::info!("Skipping {} in pre-deploy mode", check.id());
            continue;
        }
        emit_progress(
            progress,
            check.id(),
            check.category(),
            "running",
            0,
            completed_count,
            total,
        );
        let check_results =
            await_or_cancel(run_guarded_async_check(check.as_ref(), ctx), cancel_check).await??;
        let count = check_results.len();
        solo_results.extend(check_results);
        completed_count += 1;
        emit_progress(
            progress,
            check.id(),
            check.category(),
            "complete",
            count,
            completed_count,
            total,
        );
    }

    let mut runnable: Vec<&Box<dyn AsyncCheck>> = Vec::new();
    for check in concurrent {
        if is_local && check.skip_in_predeploy() {
            completed_count += 1;
            emit_progress(
                progress,
                check.id(),
                check.category(),
                "skipped",
                0,
                completed_count,
                total,
            );
            tracing::info!("Skipping {} in pre-deploy mode", check.id());
        } else {
            emit_progress(
                progress,
                check.id(),
                check.category(),
                "running",
                0,
                completed_count,
                total,
            );
            runnable.push(check);
        }
    }

    let futures: Vec<_> = runnable
        .iter()
        .map(|check| {
            let id = check.id().to_string();
            let cat = check.category();
            let span = tracing::debug_span!("async_check", check_id = %id, category = ?cat);
            use tracing::Instrument;
            async move {
                let result = run_guarded_async_check(check.as_ref(), ctx).await?;
                Ok((id, cat, result))
            }
            .instrument(span)
        })
        .collect();

    let completed = await_or_cancel(try_join_all(futures), cancel_check).await??;

    results.extend(solo_results);
    for (id, cat, check_results) in completed {
        let count = check_results.len();
        results.extend(check_results);
        completed_count += 1;
        emit_progress(
            progress,
            &id,
            cat,
            "complete",
            count,
            completed_count,
            total,
        );
    }
    *checks_done = completed_count;
    Ok(())
}
