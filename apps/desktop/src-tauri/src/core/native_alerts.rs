//! Alerts from SiteCMD scan regressions, blockers, and failures.

use crate::checks::{CheckStatus, Severity};
use crate::core::code_scan::CodeScanError;
use crate::core::scanner::{ScanError, ScanResult};
use crate::db::alerts::AlertInput;
use crate::db::{CodeScanSummary, Database};

const SITECMD_ALERT_SOURCE: &str = "sitecmd";
const SCORE_DROP_THRESHOLD: i32 = 10;
const CRITICAL_SCORE_DROP_THRESHOLD: i32 = 20;

/// Suppress generic scan alerts when a deploy-regression alert already covers them.
#[tracing::instrument(skip(db, result, env_url), fields(project_id, scan_id))]
pub fn emit_web_scan_alerts(
    db: &Database,
    project_id: i64,
    env_url: &str,
    scan_id: i64,
    result: &ScanResult,
    suppress_regression_overlap: bool,
) {
    let previous = db
        .get_scan_history_for_project(project_id, env_url, 2)
        .ok()
        .and_then(|history| history.into_iter().find(|entry| entry.id != scan_id));
    let critical_count = active_critical_web_issue_count(result) as u32;
    let high_count = active_high_web_issue_count(result) as u32;
    let issue_count = active_web_issue_count(result) as u32;
    let occurred_at = timestamp_ms(&result.timestamp);
    let observed_at = now_ms();

    if let Some(previous) = previous {
        if suppress_regression_overlap {
            return;
        }
        let score_drop = previous.overall_score as i32 - result.overall_score as i32;
        if score_drop >= SCORE_DROP_THRESHOLD {
            let severity =
                if score_drop >= CRITICAL_SCORE_DROP_THRESHOLD || result.overall_score < 50 {
                    "critical"
                } else {
                    "warn"
                };
            upsert_native_alert(
                db,
                AlertInput {
                    project_id,
                    env_url: Some(env_url.to_string()),
                    source: SITECMD_ALERT_SOURCE.to_string(),
                    alert_id: format!("web-score-drop:{scan_id}"),
                    severity: severity.to_string(),
                    title: format!("Web Scan diagnostics fell by {score_drop} points"),
                    description: format!(
                        "The latest Web Scan diagnostic score was {} after the previous diagnostic score was {}. Review the newest Issues before assuming the site is stable.",
                        result.overall_score, previous.overall_score
                    ),
                    detail_json: Some(
                        serde_json::json!({
                            "alert_type": "web_score_drop",
                            "scan_id": scan_id,
                            "previous_score": previous.overall_score,
                            "current_score": result.overall_score,
                            "score_drop": score_drop,
                            "issues_total": issue_count,
                            "critical_issues": critical_count,
                            "high_issues": high_count,
                            "url": env_url,
                            "destination": "issues"
                        })
                        .to_string(),
                    ),
                    occurred_at,
                    observed_at,
                },
            );
        }

        if critical_count > previous.issues_critical {
            let new_critical = critical_count - previous.issues_critical;
            upsert_native_alert(
                db,
                AlertInput {
                    project_id,
                    env_url: Some(env_url.to_string()),
                    source: SITECMD_ALERT_SOURCE.to_string(),
                    alert_id: format!("web-critical-increase:{scan_id}"),
                    severity: "critical".to_string(),
                    title: format!(
                        "{} new critical Web Scan {}",
                        new_critical,
                        if new_critical == 1 { "finding" } else { "findings" }
                    ),
                    description: format!(
                        "Critical Web Scan findings increased from {} to {}. This is an alert because the site changed in a way that can affect launch or production safety.",
                        previous.issues_critical, critical_count
                    ),
                    detail_json: Some(
                        serde_json::json!({
                            "alert_type": "web_critical_increase",
                            "scan_id": scan_id,
                            "previous_critical_issues": previous.issues_critical,
                            "current_critical_issues": critical_count,
                            "new_critical_issues": new_critical,
                            "url": env_url,
                            "destination": "issues"
                        })
                        .to_string(),
                    ),
                    occurred_at,
                    observed_at,
                },
            );
        }
    } else if critical_count > 0 {
        upsert_native_alert(
            db,
            AlertInput {
                project_id,
                env_url: Some(env_url.to_string()),
                source: SITECMD_ALERT_SOURCE.to_string(),
                alert_id: format!("web-first-critical:{scan_id}"),
                severity: "critical".to_string(),
                title: format!(
                    "First Web Scan found {} critical {}",
                    critical_count,
                    if critical_count == 1 { "finding" } else { "findings" }
                ),
                description:
                    "SiteCMD created a baseline and found critical Web Scan findings. Future alerts will focus on regressions and meaningful changes."
                        .to_string(),
                detail_json: Some(
                    serde_json::json!({
                        "alert_type": "web_first_critical",
                        "scan_id": scan_id,
                        "critical_issues": critical_count,
                        "high_issues": high_count,
                        "issues_total": issue_count,
                        "url": env_url,
                        "destination": "issues"
                    })
                    .to_string(),
                ),
                occurred_at,
                observed_at,
            },
        );
    }
}

/// Emit Code Scan alerts while suppressing duplicate deploy-regression notices.
#[tracing::instrument(skip(db, previous, env_url), fields(project_id, scan_id))]
pub fn emit_code_scan_alerts(
    db: &Database,
    project_id: i64,
    env_url: Option<&str>,
    scan_id: i64,
    checked_at: &str,
    overall_score: u32,
    issue_count: u32,
    critical_count: u32,
    high_count: u32,
    previous: Option<&CodeScanSummary>,
    suppress_regression_overlap: bool,
) {
    let occurred_at = timestamp_ms(checked_at);
    let observed_at = now_ms();
    let env_url_owned = env_url.map(str::to_string);

    if let Some(previous) = previous {
        if suppress_regression_overlap {
            return;
        }
        if critical_count > previous.critical_count {
            let new_critical = critical_count - previous.critical_count;
            upsert_native_alert(
                db,
                AlertInput {
                    project_id,
                    env_url: env_url_owned,
                    source: SITECMD_ALERT_SOURCE.to_string(),
                    alert_id: format!("code-critical-increase:{scan_id}"),
                    severity: "critical".to_string(),
                    title: format!(
                        "{} new critical Code Scan {}",
                        new_critical,
                        if new_critical == 1 { "finding" } else { "findings" }
                    ),
                    description: format!(
                        "Critical Code Scan findings increased from {} to {}. This is a change alert, separate from the durable issue list.",
                        previous.critical_count, critical_count
                    ),
                    detail_json: Some(
                        serde_json::json!({
                            "alert_type": "code_critical_increase",
                            "code_scan_id": scan_id,
                            "previous_critical_issues": previous.critical_count,
                            "current_critical_issues": critical_count,
                            "new_critical_issues": new_critical,
                            "url": env_url,
                            "destination": "issues"
                        })
                        .to_string(),
                    ),
                    occurred_at,
                    observed_at,
                },
            );
        }
    } else if critical_count > 0 {
        upsert_native_alert(
            db,
            AlertInput {
                project_id,
                env_url: env_url_owned,
                source: SITECMD_ALERT_SOURCE.to_string(),
                alert_id: format!("code-first-critical:{scan_id}"),
                severity: "critical".to_string(),
                title: format!(
                    "First Code Scan found {} critical {}",
                    critical_count,
                    if critical_count == 1 { "finding" } else { "findings" }
                ),
                description:
                    "SiteCMD created a code baseline and found critical findings. Future alerts will focus on regressions and meaningful changes."
                        .to_string(),
                detail_json: Some(
                    serde_json::json!({
                        "alert_type": "code_first_critical",
                        "code_scan_id": scan_id,
                        "critical_issues": critical_count,
                        "high_issues": high_count,
                        "issues_total": issue_count,
                        "url": env_url,
                        "destination": "issues"
                    })
                    .to_string(),
                ),
                occurred_at,
                observed_at,
            },
        );
    }
}

#[tracing::instrument(skip(db, error, env_url), fields(project_id, scan_kind))]
pub fn emit_scan_failure_alert(
    db: &Database,
    project_id: i64,
    env_url: Option<&str>,
    scan_kind: &str,
    error: &str,
) {
    let observed_at = now_ms();
    let alert_key = scan_kind.to_lowercase().replace(' ', "-");
    upsert_native_alert(
        db,
        AlertInput {
            project_id,
            env_url: env_url.map(str::to_string),
            source: SITECMD_ALERT_SOURCE.to_string(),
            alert_id: format!("{alert_key}-failed:{observed_at}"),
            severity: "warn".to_string(),
            title: format!("{scan_kind} failed"),
            description: format!(
                "{scan_kind} could not complete. This does not belong in the fix list until SiteCMD can inspect the site or project again."
            ),
            detail_json: Some(
                serde_json::json!({
                    "alert_type": "scan_failed",
                    "scan_kind": scan_kind,
                    "error": error,
                    "url": env_url,
                    "destination": "activity"
                })
                .to_string(),
            ),
            occurred_at: observed_at,
            observed_at,
        },
    );
}

pub fn is_user_cancelled_scan(error: &ScanError) -> bool {
    matches!(error, ScanError::Cancelled)
}

pub fn is_user_cancelled_code_scan(error: &CodeScanError) -> bool {
    matches!(error, CodeScanError::Cancelled)
}

fn upsert_native_alert(db: &Database, input: AlertInput) {
    if let Err(error) = db.upsert_alert(input) {
        tracing::warn!("failed to upsert native alert: {}", error);
    }
}

fn active_web_issue_count(result: &ScanResult) -> usize {
    result
        .issues
        .iter()
        .filter(|issue| matches!(issue.status, CheckStatus::Fail | CheckStatus::Warn))
        .count()
}

fn active_critical_web_issue_count(result: &ScanResult) -> usize {
    result
        .issues
        .iter()
        .filter(|issue| {
            matches!(issue.status, CheckStatus::Fail | CheckStatus::Warn)
                && matches!(issue.severity, Severity::Critical)
        })
        .count()
}

fn active_high_web_issue_count(result: &ScanResult) -> usize {
    result
        .issues
        .iter()
        .filter(|issue| {
            matches!(issue.status, CheckStatus::Fail | CheckStatus::Warn)
                && matches!(issue.severity, Severity::High)
        })
        .count()
}

fn timestamp_ms(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or_else(|_| now_ms())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{CheckResult, ScanCategory};
    use crate::db::alerts::AlertFilter;
    use crate::db::test_helpers::{temp_db, TestDb};

    fn test_db() -> TestDb {
        let db = temp_db();
        let project_id = db
            .upsert_project("test", "/tmp/sitecmd-native-alert-test", None)
            .expect("project");
        db.add_environment(
            project_id,
            "https://example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("environment");
        db
    }

    fn scan(score: u32, timestamp: &str, critical: usize) -> ScanResult {
        let mut issues = Vec::new();
        for index in 0..critical {
            issues.push(CheckResult {
                check_id: format!("security.critical-{index}"),
                category: ScanCategory::Security,
                title: "Critical finding".into(),
                description: "Critical finding".into(),
                status: CheckStatus::Fail,
                severity: Severity::Critical,
                fix_prompt: None,
                manual_fix: None,
                raw_data: None,
                confidence: crate::checks::IssueConfidence::High,
                confidence_reason: None,
                why_it_matters: None,
            });
        }
        ScanResult {
            page_signals: None,
            site_facts: None,
            url: "https://example.com".into(),
            mode: "live".into(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: score,
            categories: Vec::new(),
            issues,
            detected_stack: None,
            duration_ms: 100,
            timestamp: timestamp.into(),
        }
    }

    #[test]
    fn web_score_drop_emits_native_alert() {
        let db = test_db();
        let project_id = 1;
        let url = "https://example.com";
        let site_id = db.get_or_create_site(url).expect("site");
        let first = scan(92, "2026-05-01T00:00:00Z", 0);
        db.save_scan(site_id, &first).expect("first scan");

        let second = scan(70, "2026-05-02T00:00:00Z", 0);
        let second_id = db.save_scan(site_id, &second).expect("second scan");
        emit_web_scan_alerts(&db, project_id, url, second_id, &second, false);

        let alerts = db
            .get_alerts(project_id, AlertFilter::Unread, None)
            .expect("alerts");
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].source, SITECMD_ALERT_SOURCE);
        assert!(alerts[0].alert_id.starts_with("web-score-drop:"));
    }

    #[test]
    fn web_alerts_are_suppressed_when_blame_fired() {
        let db = test_db();
        let project_id = 1;
        let url = "https://example.com";
        let site_id = db.get_or_create_site(url).expect("site");
        let first = scan(92, "2026-05-01T00:00:00Z", 0);
        db.save_scan(site_id, &first).expect("first scan");

        // Score drop AND critical increase: both generic alerts would fire,
        // but the deploy-regression alert subsumes them for this scan.
        let second = scan(70, "2026-05-02T00:00:00Z", 2);
        let second_id = db.save_scan(site_id, &second).expect("second scan");
        emit_web_scan_alerts(&db, project_id, url, second_id, &second, true);

        let alerts = db
            .get_alerts(project_id, AlertFilter::Unread, None)
            .expect("alerts");
        assert!(alerts.is_empty());
    }

    #[test]
    fn code_critical_increase_emits_native_alert() {
        let db = test_db();
        let previous = CodeScanSummary {
            id: 1,
            project_id: 1,
            environment_url: Some("https://example.com".into()),
            overall_score: 90,
            issue_count: 1,
            grouped_issue_count: 1,
            critical_count: 0,
            high_count: 1,
            duration_ms: 10,
            checked_at: "2026-05-01T00:00:00Z".into(),
            framework: None,
            top_domain: None,
            top_domain_count: 0,
            domain_summaries: Vec::new(),
        };

        emit_code_scan_alerts(
            &db,
            1,
            Some("https://example.com"),
            2,
            "2026-05-02T00:00:00Z",
            90,
            2,
            1,
            1,
            Some(&previous),
            false,
        );

        let alerts = db.get_alerts(1, AlertFilter::Unread, None).expect("alerts");
        assert_eq!(alerts.len(), 1);
        assert!(alerts[0].alert_id.starts_with("code-critical-increase:"));
    }

    #[test]
    fn code_critical_increase_suppressed_when_blame_fired() {
        let db = test_db();
        let previous = CodeScanSummary {
            id: 1,
            project_id: 1,
            environment_url: Some("https://example.com".into()),
            overall_score: 90,
            issue_count: 1,
            grouped_issue_count: 1,
            critical_count: 0,
            high_count: 1,
            duration_ms: 10,
            checked_at: "2026-05-01T00:00:00Z".into(),
            framework: None,
            top_domain: None,
            top_domain_count: 0,
            domain_summaries: Vec::new(),
        };

        emit_code_scan_alerts(
            &db,
            1,
            Some("https://example.com"),
            2,
            "2026-05-02T00:00:00Z",
            90,
            2,
            1,
            1,
            Some(&previous),
            true,
        );

        let alerts = db.get_alerts(1, AlertFilter::Unread, None).expect("alerts");
        assert!(alerts.is_empty());
    }

    #[test]
    fn code_score_drop_without_new_critical_findings_does_not_emit_native_alert() {
        let db = test_db();
        let previous = CodeScanSummary {
            id: 1,
            project_id: 1,
            environment_url: Some("https://example.com".into()),
            overall_score: 95,
            issue_count: 1,
            grouped_issue_count: 1,
            critical_count: 0,
            high_count: 1,
            duration_ms: 10,
            checked_at: "2026-05-01T00:00:00Z".into(),
            framework: None,
            top_domain: None,
            top_domain_count: 0,
            domain_summaries: Vec::new(),
        };

        emit_code_scan_alerts(
            &db,
            1,
            Some("https://example.com"),
            2,
            "2026-05-02T00:00:00Z",
            25,
            12,
            0,
            12,
            Some(&previous),
            false,
        );

        let alerts = db.get_alerts(1, AlertFilter::Unread, None).expect("alerts");
        assert!(alerts.is_empty());
    }
}
