use crate::{
    checks::{CheckStatus, Severity},
    core::{
        normalized_scan::ScanRunKind,
        scanner::{MultiScanResult, ScanResult},
    },
};

#[derive(Debug, Clone, Copy)]
pub(super) struct WebScanIssueCounts {
    pub(super) total: usize,
    pub(super) critical: usize,
    pub(super) high: usize,
}

#[derive(Debug)]
pub(super) struct ScheduledWebCompletion {
    pub(super) scan_id: Option<i64>,
    pub(super) score: u32,
    pub(super) counts: WebScanIssueCounts,
    pub(super) timestamp: String,
    pub(super) regression_scan_ids: Vec<i64>,
    pub(super) scope_complete: bool,
    pub(super) completed_pages: usize,
    pub(super) total_pages: usize,
    pub(super) incomplete_detail: Option<String>,
    pub(super) comparison_eligible: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PreviousWebCompletion {
    pub(super) run_id: i64,
    pub(super) score: u32,
    pub(super) critical: usize,
}

pub(super) fn scheduled_web_run_kind(scope_urls: &[String]) -> ScanRunKind {
    if scope_urls.len() > 1 {
        ScanRunKind::MultiParent
    } else {
        ScanRunKind::Single
    }
}

pub(super) fn summarize_scheduled_web_result(
    web_result: Option<&ScanResult>,
    multi_result: Option<&MultiScanResult>,
    incomplete_detail: Option<&str>,
    web_scan_id: Option<i64>,
    web_session_id: Option<i64>,
    fallback_timestamp: &str,
) -> Option<ScheduledWebCompletion> {
    if let Some(result) = web_result {
        let scope_complete = incomplete_detail.is_none();
        return Some(ScheduledWebCompletion {
            scan_id: web_scan_id,
            score: result.overall_score,
            counts: web_scan_issue_counts(result),
            timestamp: result.timestamp.clone(),
            regression_scan_ids: web_scan_id.into_iter().collect(),
            scope_complete,
            completed_pages: 1,
            total_pages: 1,
            incomplete_detail: incomplete_detail.map(str::to_owned),
            comparison_eligible: scope_complete,
        });
    }

    multi_result.map(|result| {
        let mut counts = result.page_results.iter().fold(
            WebScanIssueCounts {
                total: 0,
                critical: 0,
                high: 0,
            },
            |mut counts, page| {
                counts.total += page.issues_count;
                counts.critical += page.issues_critical;
                counts.high += page.issues_high;
                counts
            },
        );
        for issue in result
            .site_issues
            .iter()
            .filter(|issue| matches!(issue.status, CheckStatus::Fail | CheckStatus::Warn))
        {
            counts.total += 1;
            if matches!(issue.severity, Severity::Critical) {
                counts.critical += 1;
            }
            if matches!(issue.severity, Severity::High) {
                counts.high += 1;
            }
        }

        let incomplete_detail = result
            .incomplete_detail
            .as_deref()
            .or(incomplete_detail)
            .map(str::to_owned);
        let scope_complete = result.total_pages > 0
            && result.completed_pages == result.total_pages
            && incomplete_detail.is_none();
        ScheduledWebCompletion {
            scan_id: web_session_id.or(Some(result.session_id)),
            score: result.overall_score,
            counts,
            timestamp: fallback_timestamp.to_string(),
            regression_scan_ids: result
                .page_results
                .iter()
                .map(|page| page.scan_id)
                .filter(|scan_id| *scan_id > 0)
                .collect(),
            scope_complete,
            completed_pages: result.completed_pages,
            total_pages: result.total_pages,
            incomplete_detail,
            comparison_eligible: scope_complete,
        }
    })
}

fn web_scan_issue_counts(result: &ScanResult) -> WebScanIssueCounts {
    result
        .issues
        .iter()
        .filter(|issue| matches!(issue.status, CheckStatus::Fail | CheckStatus::Warn))
        .fold(
            WebScanIssueCounts {
                total: 0,
                critical: 0,
                high: 0,
            },
            |mut counts, issue| {
                counts.total += 1;
                if matches!(issue.severity, Severity::Critical) {
                    counts.critical += 1;
                }
                if matches!(issue.severity, Severity::High) {
                    counts.high += 1;
                }
                counts
            },
        )
}
