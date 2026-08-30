//! Testable per-page Web Scan loop with cancellation, origin-scope handling,
//! failure bookkeeping, and progress events.

use crate::core::scanner;

use super::web_scan::BrowserRuntime;

#[derive(Debug)]
pub(super) struct PageScanOutput {
    pub(super) result: scanner::ScanResult,
    pub(super) browser_runtime: BrowserRuntime,
}

impl PageScanOutput {
    #[cfg(test)]
    fn transport_only(result: scanner::ScanResult) -> Self {
        Self {
            result,
            browser_runtime: BrowserRuntime::default(),
        }
    }
}

/// What the per-page loop produced: one summary per attempted page, the
/// scores of the pages that actually completed, and the cross-page signals
/// for session analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageLoopStatus {
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug)]
pub(super) struct PageLoopOutcome {
    pub(super) status: PageLoopStatus,
    pub(super) page_results: Vec<scanner::PageScanSummary>,
    /// Scores from successfully scanned pages only. Failed pages appear in
    /// `page_results` with score 0 but must not drag the session average.
    pub(super) completed_scores: Vec<u32>,
    pub(super) session_signals: Vec<crate::core::page_signals::PageSignals>,
    pub(super) session_site_facts: Vec<sitecmd_engine::profile::Observation>,
    pub(super) browser_runtimes: Vec<BrowserRuntime>,
}

/// Average completed pages only; `None` distinguishes an unscored session from
/// a real zero.
pub(super) fn session_average(completed_scores: &[u32]) -> Option<u32> {
    if completed_scores.is_empty() {
        None
    } else {
        Some(completed_scores.iter().sum::<u32>() / completed_scores.len() as u32)
    }
}

/// Run injected per-page scan, persistence, cancellation, and progress effects.
pub(super) async fn run_page_loop<ScanFut, PersistFut>(
    urls: &[String],
    session_id: i64,
    is_cancelled: impl Fn() -> bool,
    mut scan_page: impl FnMut(usize, &str, bool) -> ScanFut,
    mut persist_page: impl FnMut(usize, &str, scanner::ScanResult, BrowserRuntime) -> PersistFut,
    mut emit_progress: impl FnMut(scanner::MultiScanProgress),
) -> Result<PageLoopOutcome, String>
where
    ScanFut: std::future::Future<Output = Result<PageScanOutput, scanner::ScanError>>,
    PersistFut: std::future::Future<Output = Result<i64, String>>,
{
    use scanner::MultiScanProgress;

    let mut outcome = PageLoopOutcome {
        status: PageLoopStatus::Complete,
        page_results: Vec::new(),
        completed_scores: Vec::new(),
        session_signals: Vec::new(),
        session_site_facts: Vec::new(),
        browser_runtimes: Vec::new(),
    };

    for (i, url) in urls.iter().enumerate() {
        if is_cancelled() {
            outcome.status = PageLoopStatus::Cancelled;
            tracing::info!("Multi-scan cancelled after {} pages", i);
            emit_progress(MultiScanProgress {
                page_index: i,
                page_count: urls.len(),
                current_url: url.clone(),
                page_status: "cancelled".into(),
                session_id,
            });
            break;
        }

        emit_progress(MultiScanProgress {
            page_index: i,
            page_count: urls.len(),
            current_url: url.clone(),
            page_status: "scanning".into(),
            session_id,
        });

        tracing::info!(
            "Multi-scan [{}/{}]: {}",
            i + 1,
            urls.len(),
            crate::log_sanitizer::log_safe_url_target(url)
        );

        // Origin-scoped checks (robots, sitemap, DNS, TLS, well-known paths)
        // run on the entry page only; repeating them for every page re-probes
        // the same origin state and dominated multi-scan wall time.
        match scan_page(i, url, i > 0).await {
            Ok(PageScanOutput {
                mut result,
                browser_runtime,
            }) => {
                // Cross-page signals never persist (serde-skipped); pull
                // them off before the result moves into the persist task.
                if let Some(signals) = result.page_signals.take() {
                    outcome.session_signals.push(signals);
                }
                let site_facts = result.site_facts.take();
                let overall_score = result.overall_score;
                let issue_counts = crate::db::insert::grouped_issue_counts(&result.issues);
                let duration_ms = result.duration_ms;

                // Progress is the number of successfully persisted child
                // runs, not the selected-page index. A failed first page
                // followed by one success is 1/2 complete, never 2/2.
                let completed_page_count = outcome.completed_scores.len() + 1;
                let scan_id =
                    persist_page(completed_page_count, url, result, browser_runtime.clone())
                        .await?;

                if let Some(mut facts) = site_facts {
                    facts.scan_id = Some(scan_id);
                    outcome.session_site_facts.push(facts);
                }

                outcome.completed_scores.push(overall_score);
                outcome.browser_runtimes.push(browser_runtime);
                outcome.page_results.push(scanner::PageScanSummary {
                    url: url.clone(),
                    score: overall_score,
                    issues_count: issue_counts.total as usize,
                    issues_critical: issue_counts.critical as usize,
                    issues_high: issue_counts.high as usize,
                    issues_medium: issue_counts.medium as usize,
                    issues_low: issue_counts.low as usize,
                    duration_ms,
                    scan_id,
                });

                emit_progress(MultiScanProgress {
                    page_index: i,
                    page_count: urls.len(),
                    current_url: url.clone(),
                    page_status: "complete".into(),
                    session_id,
                });
            }
            Err(scanner::ScanError::Cancelled) => {
                outcome.status = PageLoopStatus::Cancelled;
                tracing::info!(
                    "Multi-scan cancelled while scanning {}",
                    crate::log_sanitizer::log_safe_url_target(url)
                );
                emit_progress(MultiScanProgress {
                    page_index: i,
                    page_count: urls.len(),
                    current_url: url.clone(),
                    page_status: "cancelled".into(),
                    session_id,
                });
                break;
            }
            Err(e) => {
                tracing::error!(
                    "Multi-scan error for {}: {}",
                    crate::log_sanitizer::log_safe_url_target(url),
                    e
                );
                outcome.page_results.push(scanner::PageScanSummary {
                    url: url.clone(),
                    score: 0,
                    issues_count: 0,
                    issues_critical: 0,
                    issues_high: 0,
                    issues_medium: 0,
                    issues_low: 0,
                    duration_ms: 0,
                    scan_id: -1,
                });

                emit_progress(MultiScanProgress {
                    page_index: i,
                    page_count: urls.len(),
                    current_url: url.clone(),
                    page_status: "error".into(),
                    session_id,
                });
            }
        }
    }

    if outcome.status == PageLoopStatus::Complete
        && outcome.completed_scores.is_empty()
        && !outcome.page_results.is_empty()
    {
        outcome.status = PageLoopStatus::Failed;
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    //! Contracts for cancellation, origin scoping, and failed-page scoring.

    use super::*;
    use crate::core::scanner::ScanType;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn urls(n: usize) -> Vec<String> {
        (0..n)
            .map(|i| format!("https://example.com/page-{i}"))
            .collect()
    }

    fn scan_result(url: &str, score: u32) -> scanner::ScanResult {
        let parsed = url::Url::parse(url).expect("fixture url");
        scanner::ScanResult {
            url: url.to_string(),
            mode: "live".into(),
            scan_type: ScanType::Health,
            overall_score: score,
            categories: vec![],
            issues: vec![],
            detected_stack: None,
            duration_ms: 5,
            timestamp: "2026-07-10T12:00:00Z".into(),
            page_signals: Some(crate::core::page_signals::extract_page_signals(
                &parsed,
                "<html><head><title>t</title></head><body></body></html>",
            )),
            site_facts: None,
        }
    }

    fn statuses(events: &Arc<Mutex<Vec<scanner::MultiScanProgress>>>) -> Vec<(usize, String)> {
        events
            .lock()
            .unwrap()
            .iter()
            .map(|p| (p.page_index, p.page_status.clone()))
            .collect()
    }

    #[tokio::test]
    async fn failed_pages_are_excluded_from_the_session_average() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let persisted = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));

        let events_sink = events.clone();
        let persisted_sink = persisted.clone();
        let outcome = run_page_loop(
            &urls(3),
            7,
            || false,
            |i, url, _| {
                let url = url.to_string();
                async move {
                    if i == 1 {
                        Err(scanner::ScanError::ScanFailed("fixture failure".into()))
                    } else {
                        Ok(PageScanOutput::transport_only(scan_result(
                            &url,
                            if i == 0 { 80 } else { 60 },
                        )))
                    }
                }
            },
            |completed_pages, url, _result, _browser_runtime| {
                let persisted = persisted_sink.clone();
                let url = url.to_string();
                async move {
                    persisted.lock().unwrap().push((completed_pages, url));
                    Ok(100)
                }
            },
            |p| events_sink.lock().unwrap().push(p),
        )
        .await
        .expect("page persistence succeeds");

        // The failed page appears in the summaries (score 0, no scan row)
        // but must not drag the session average down as a zero.
        assert_eq!(outcome.completed_scores, vec![80, 60]);
        assert_eq!(outcome.status, PageLoopStatus::Complete);
        assert_eq!(session_average(&outcome.completed_scores), Some(70));
        assert_eq!(outcome.page_results.len(), 3);
        assert_eq!(outcome.page_results[1].score, 0);
        assert_eq!(outcome.page_results[1].scan_id, -1);

        // Persistence runs only for pages that produced a result.
        let persisted = persisted.lock().unwrap().clone();
        assert_eq!(persisted.len(), 2);
        assert_eq!(persisted[0].0, 1);
        assert_eq!(persisted[1].0, 2);

        let statuses = statuses(&events);
        assert!(statuses.contains(&(1, "error".to_string())));
        assert!(statuses.contains(&(2, "complete".to_string())));
    }

    #[tokio::test]
    async fn page_issue_count_excludes_passing_and_skipped_results() {
        use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

        let mut result = scan_result("https://example.com/page-0", 80);
        result.issues = [
            ("pass", CheckStatus::Pass, Severity::Low),
            ("warn", CheckStatus::Warn, Severity::Medium),
            ("fail", CheckStatus::Fail, Severity::High),
            ("skip", CheckStatus::Skipped, Severity::Low),
        ]
        .into_iter()
        .map(|(check_id, status, severity)| CheckResult {
            check_id: check_id.to_string(),
            category: ScanCategory::Seo,
            severity,
            status,
            title: check_id.to_string(),
            description: String::new(),
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        })
        .collect();

        let outcome = run_page_loop(
            &["https://example.com/page-0".to_string()],
            7,
            || false,
            |_, _, _| {
                let result = result.clone();
                async move { Ok(PageScanOutput::transport_only(result)) }
            },
            |_, _, _, _| async { Ok(1) },
            |_| {},
        )
        .await
        .expect("page scan succeeds");

        assert_eq!(outcome.page_results[0].issues_count, 2);
        assert_eq!(outcome.page_results[0].issues_critical, 0);
        assert_eq!(outcome.page_results[0].issues_high, 1);
        assert_eq!(outcome.page_results[0].issues_medium, 1);
        assert_eq!(outcome.page_results[0].issues_low, 0);
    }

    #[tokio::test]
    async fn origin_scoped_probes_run_on_the_entry_page_only() {
        let skip_flags = Arc::new(Mutex::new(Vec::new()));

        let flags_sink = skip_flags.clone();
        run_page_loop(
            &urls(3),
            7,
            || false,
            |_, url, skip_origin_checks| {
                let flags = flags_sink.clone();
                let url = url.to_string();
                async move {
                    flags.lock().unwrap().push(skip_origin_checks);
                    Ok(PageScanOutput::transport_only(scan_result(&url, 50)))
                }
            },
            |_, _, _, _| async { Ok(1) },
            |_| {},
        )
        .await
        .expect("page persistence succeeds");

        assert_eq!(*skip_flags.lock().unwrap(), vec![false, true, true]);
    }

    #[tokio::test]
    async fn cancellation_between_pages_stops_the_loop() {
        let polls = AtomicUsize::new(0);
        let events = Arc::new(Mutex::new(Vec::new()));

        let events_sink = events.clone();
        let outcome = run_page_loop(
            &urls(3),
            7,
            // First poll (before page 0) allows; every later poll cancels.
            || polls.fetch_add(1, Ordering::SeqCst) >= 1,
            |_, url, _| {
                let url = url.to_string();
                async move { Ok(PageScanOutput::transport_only(scan_result(&url, 90))) }
            },
            |_, _, _, _| async { Ok(1) },
            |p| events_sink.lock().unwrap().push(p),
        )
        .await
        .expect("page persistence succeeds");

        assert_eq!(outcome.page_results.len(), 1);
        assert_eq!(outcome.completed_scores, vec![90]);
        assert_eq!(outcome.status, PageLoopStatus::Cancelled);
        let statuses = statuses(&events);
        assert_eq!(statuses.last(), Some(&(1, "cancelled".to_string())));
    }

    #[tokio::test]
    async fn cancellation_during_a_page_emits_cancelled_and_records_nothing() {
        let events = Arc::new(Mutex::new(Vec::new()));

        let events_sink = events.clone();
        let outcome = run_page_loop(
            &urls(2),
            7,
            || false,
            |_, _, _| async { Err(scanner::ScanError::Cancelled) },
            |_, _, _, _| async { Ok(1) },
            |p| events_sink.lock().unwrap().push(p),
        )
        .await
        .expect("cancelled scan never reaches persistence");

        // A cancelled page is not a failed page: no zero-score summary.
        assert!(outcome.page_results.is_empty());
        assert!(outcome.completed_scores.is_empty());
        assert_eq!(outcome.status, PageLoopStatus::Cancelled);
        let statuses = statuses(&events);
        assert_eq!(
            statuses,
            vec![(0, "scanning".to_string()), (0, "cancelled".to_string())]
        );
    }

    #[tokio::test]
    async fn cross_page_signals_are_collected_before_results_persist() {
        let outcome = run_page_loop(
            &urls(2),
            7,
            || false,
            |_, url, _| {
                let url = url.to_string();
                async move { Ok(PageScanOutput::transport_only(scan_result(&url, 70))) }
            },
            |_, _, result, _| async move {
                // The persist effect must never see the serde-skipped
                // cross-page signals; the loop takes them first.
                assert!(result.page_signals.is_none());
                Ok(1)
            },
            |_| {},
        )
        .await
        .expect("page persistence succeeds");

        assert_eq!(outcome.session_signals.len(), 2);
    }

    #[tokio::test]
    async fn site_fact_observations_are_collected_for_one_session_level_baseline_write() {
        use sitecmd_engine::profile::{FieldValue, Observation, OriginSet};

        let outcome = run_page_loop(
            &urls(2),
            7,
            || false,
            |index, url, _| {
                let url = url.to_string();
                async move {
                    let mut result = scan_result(&url, 70);
                    result.site_facts = Some(Observation {
                        values: vec![FieldValue::ThirdPartyOrigins(OriginSet::from_origins([
                            format!("https://cdn-{index}.test"),
                        ]))],
                        scan_id: None,
                    });
                    Ok(PageScanOutput::transport_only(result))
                }
            },
            |completed_pages, _, result, _| async move {
                assert!(
                    result.site_facts.is_none(),
                    "page persistence must not compare a partial site observation"
                );
                Ok(100 + completed_pages as i64)
            },
            |_| {},
        )
        .await
        .expect("page persistence succeeds");

        assert_eq!(outcome.session_site_facts.len(), 2);
        assert_eq!(outcome.session_site_facts[0].scan_id, Some(101));
        assert_eq!(outcome.session_site_facts[1].scan_id, Some(102));
    }

    #[tokio::test]
    async fn page_persistence_failure_aborts_instead_of_returning_a_fake_scan_id() {
        let error = run_page_loop(
            &urls(1),
            7,
            || false,
            |_, url, _| {
                let url = url.to_string();
                async move { Ok(PageScanOutput::transport_only(scan_result(&url, 90))) }
            },
            |_, _, _, _| async { Err("canonical issue persistence failed".to_string()) },
            |_| {},
        )
        .await
        .expect_err("a persisted page is indivisible from its canonical issue snapshot");

        assert_eq!(error, "canonical issue persistence failed");
    }

    #[tokio::test]
    async fn browser_runtime_reaches_page_persistence() {
        let observed = Arc::new(Mutex::new(None));
        let observed_runtime = observed.clone();

        run_page_loop(
            &["https://example.com/page-0".to_string()],
            7,
            || false,
            |_, url, _| {
                let url = url.to_string();
                async move {
                    Ok(PageScanOutput {
                        result: scan_result(&url, 90),
                        browser_runtime: super::super::web_scan::BrowserRuntime {
                            ran: true,
                            axe_ran: true,
                            build: Some("test-browser".into()),
                        },
                    })
                }
            },
            |_, _, _, browser_runtime| {
                let observed = observed_runtime.clone();
                async move {
                    *observed.lock().unwrap() = Some(browser_runtime);
                    Ok(1)
                }
            },
            |_| {},
        )
        .await
        .expect("page scan succeeds");

        let runtime = observed.lock().unwrap().clone().expect("browser runtime");
        assert!(runtime.ran);
        assert!(runtime.axe_ran);
        assert_eq!(runtime.build.as_deref(), Some("test-browser"));
    }

    #[tokio::test]
    async fn an_all_failed_page_set_has_a_failed_terminal_status() {
        let outcome = run_page_loop(
            &urls(2),
            7,
            || false,
            |_, _, _| async { Err(scanner::ScanError::ScanFailed("fixture failure".into())) },
            |_, _, _, _| async { Ok(1) },
            |_| {},
        )
        .await
        .expect("page failures are recorded in the loop outcome");

        assert_eq!(outcome.status, PageLoopStatus::Failed);
        assert_eq!(outcome.page_results.len(), 2);
        assert!(outcome.completed_scores.is_empty());
    }

    #[test]
    fn empty_session_is_not_scored_rather_than_a_red_zero() {
        assert_eq!(session_average(&[]), None);
        assert_eq!(session_average(&[80, 60]), Some(70));
    }
}
