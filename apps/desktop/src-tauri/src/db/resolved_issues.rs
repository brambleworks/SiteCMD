//! Resolved Web finding events for one project environment.
//! Missing causal scan IDs remain `None`; queries return newest-first up to `limit`.

use super::DbError;
use rusqlite::params;
use serde::Serialize;

use super::helpers::normalize_url;
use super::Database;
use ts_rs::TS;

/// One lifecycle event for a Web finding that is no longer active.
#[derive(Debug, Clone, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct ResolvedIssue {
    pub check_id: String,
    pub title: String,
    pub category: String,
    pub severity: String,
    pub resolved_scan_id: Option<i64>,
    pub resolved_at: String,
    pub first_seen_scan_id: Option<i64>,
    pub first_seen_at: String,
    pub duration_hours: Option<f64>,
    pub recurrence_count: u32,
}

impl Database {
    /// Return resolved Web issue lifecycles for a URL, newest first.
    /// Scan IDs come only from exact full-scan provenance; other rows return
    /// `None` rather than an inferred ID.
    #[tracing::instrument(skip(self, url), fields(project_id, limit))]
    pub fn get_resolved_issues(
        &self,
        project_id: i64,
        url: String,
        limit: u32,
    ) -> Result<Vec<ResolvedIssue>, DbError> {
        self.execute(move |conn| {
            let (normalized, url_slash) = normalize_url(&url);

            let mut stmt = conn.prepare(
                "SELECT
                        wi.check_id,
                        wi.title,
                        wi.category,
                        wi.severity,
                        wi.first_seen_at,
                        wi.resolved_at,
                        wi.first_seen_scan_ref,
                        wi.resolved_scan_ref,
                        ROW_NUMBER() OVER (
                            PARTITION BY wi.check_id
                            ORDER BY wi.resolved_at ASC, wi.id ASC
                        ) AS recurrence_count
                     FROM work_items wi
                     WHERE wi.source = 'web_scan'
                       AND wi.project_id = ?1
                       AND wi.resolved_at IS NOT NULL
                       AND (wi.env_url = ?2 OR wi.env_url = ?3)
                     ORDER BY wi.resolved_at DESC
                     LIMIT ?4",
            )?;

            #[allow(clippy::type_complexity)]
            let raw: Vec<(
                String,
                String,
                String,
                String,
                i64,
                i64,
                Option<i64>,
                Option<i64>,
                i64,
            )> = stmt
                .query_map(params![project_id, normalized, url_slash, limit], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if raw.is_empty() {
                return Ok(vec![]);
            }

            let mut events = Vec::with_capacity(raw.len());
            for (
                check_id,
                title,
                category,
                severity,
                first_seen_ms,
                resolved_ms,
                first_seen_scan_id,
                resolved_scan_id,
                recurrence_count,
            ) in raw
            {
                let recurrence_count = u32::try_from(recurrence_count).map_err(|_| {
                    DbError::Other("resolved issue recurrence count exceeds u32".into())
                })?;
                let first_seen_at = ms_to_rfc3339(first_seen_ms)?;
                let resolved_at = ms_to_rfc3339(resolved_ms)?;
                let duration_hours = (resolved_ms >= first_seen_ms)
                    .then_some((resolved_ms - first_seen_ms) as f64 / 3_600_000.0);

                events.push(ResolvedIssue {
                    check_id,
                    title,
                    category,
                    severity,
                    resolved_scan_id,
                    resolved_at,
                    first_seen_scan_id,
                    first_seen_at,
                    duration_hours,
                    recurrence_count,
                });
            }

            Ok(events)
        })?
    }
}

/// Convert a millisecond epoch timestamp to RFC 3339 without inventing a
/// string for values outside chrono's supported range.
fn ms_to_rfc3339(ms: i64) -> Result<String, DbError> {
    use chrono::{TimeZone, Utc};
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|timestamp| timestamp.to_rfc3339())
        .ok_or_else(|| DbError::Other(format!("invalid lifecycle timestamp: {ms}")))
}

#[cfg(test)]
mod tests {
    use crate::checks::{CheckResult, CheckStatus, ScanCategory, Severity};
    use crate::core::scanner::ScanResult;
    use crate::db::test_helpers::temp_db;
    use crate::db::work_items::WorkItemInput;
    use crate::db::work_items::WorkItemMetadata;
    use crate::db::Database;

    /// Create a project + environment and return (project_id, site_id).
    fn setup_project(db: &Database, url: &str) -> (i64, i64) {
        let project_id = db
            .upsert_project("Resolved Test", "/tmp/resolved", None)
            .expect("upsert project");
        db.add_environment(project_id, url, "production", "production", "manual")
            .expect("add env");
        let site_id = db.get_or_create_site(url).expect("site");
        (project_id, site_id)
    }

    /// Save a scan and persist its failing checks as work_items.
    /// `pass_csp` items are excluded (work_items only stores failing checks).
    fn save_scan_with_work_items(
        db: &Database,
        site_id: i64,
        _project_id: i64,
        _url: &str,
        scan: &ScanResult,
        _now_ms: i64,
    ) -> i64 {
        db.save_scan(site_id, scan).expect("save_scan")
    }

    fn make_scan(url: &str, timestamp: &str, issues: Vec<CheckResult>) -> ScanResult {
        ScanResult {
            page_signals: None,
            site_facts: None,
            url: url.to_string(),
            mode: "full".to_string(),
            scan_type: crate::core::scanner::ScanType::Health,
            overall_score: 80,
            categories: vec![],
            issues,
            detected_stack: None,
            duration_ms: 500,
            timestamp: timestamp.to_string(),
        }
    }

    fn fail_csp() -> CheckResult {
        CheckResult {
            check_id: "security.csp".to_string(),
            category: ScanCategory::Security,
            title: "Missing CSP".to_string(),
            description: "".to_string(),
            status: CheckStatus::Fail,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    }

    fn pass_csp() -> CheckResult {
        CheckResult {
            check_id: "security.csp".to_string(),
            category: ScanCategory::Security,
            title: "Missing CSP".to_string(),
            description: "".to_string(),
            status: CheckStatus::Pass,
            severity: Severity::High,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        }
    }

    const MS_APR18: i64 = 1_776_470_400_000;
    const MS_APR19: i64 = 1_776_556_800_000;
    const MS_APR20: i64 = 1_776_643_200_000;
    const MS_APR21: i64 = 1_776_729_600_000;

    #[test]
    fn returns_empty_when_fewer_than_two_scans() {
        let db = temp_db();
        let url = "https://example.com";
        let (project_id, site_id) = setup_project(&db, url);
        let scan = make_scan(url, "2026-04-18T00:00:00Z", vec![fail_csp()]);
        save_scan_with_work_items(&db, site_id, project_id, url, &scan, MS_APR18);

        let resolved = db
            .get_resolved_issues(project_id, url.to_string(), 50)
            .expect("query");

        assert!(
            resolved.is_empty(),
            "single scan with still-active issue cannot produce resolved events"
        );
    }

    #[test]
    fn detects_issue_resolved_between_two_scans() {
        let db = temp_db();
        let url = "https://example.com";
        let (project_id, site_id) = setup_project(&db, url);

        // Scan 1: CSP fails.
        let scan1 = make_scan(url, "2026-04-18T00:00:00Z", vec![fail_csp()]);
        let first_scan_id =
            save_scan_with_work_items(&db, site_id, project_id, url, &scan1, MS_APR18);

        // Scan 2: CSP passes - empty observed list resolves the work_item at MS_APR19.
        let scan2 = make_scan(url, "2026-04-19T00:00:00Z", vec![pass_csp()]);
        let resolved_scan_id =
            save_scan_with_work_items(&db, site_id, project_id, url, &scan2, MS_APR19);

        let resolved = db
            .get_resolved_issues(project_id, url.to_string(), 50)
            .expect("query");

        assert_eq!(resolved.len(), 1, "one resolution event expected");
        let r = &resolved[0];
        assert_eq!(r.check_id, "security.csp");
        assert_eq!(r.recurrence_count, 1);
        assert_eq!(r.first_seen_scan_id, Some(first_scan_id));
        assert_eq!(r.resolved_scan_id, Some(resolved_scan_id));
        // first_seen_at and resolved_at are derived from ms epoch on work_items
        assert!(
            r.first_seen_at.contains("2026-04-18"),
            "first seen on Apr 18, got {}",
            r.first_seen_at
        );
        assert!(
            r.resolved_at.contains("2026-04-19"),
            "resolved on Apr 19, got {}",
            r.resolved_at
        );
        assert!(r.duration_hours.is_some());
        let hours = r.duration_hours.unwrap();
        assert!((hours - 24.0).abs() < 0.1, "expected ~24h, got {}", hours);
    }

    #[test]
    fn first_seen_scan_id_is_not_replaced_by_last_persistent_failure() {
        let db = temp_db();
        let url = "https://persistent-then-fixed.example.com";
        let (project_id, site_id) = setup_project(&db, url);

        let first = make_scan(url, "2026-04-18T00:00:00Z", vec![fail_csp()]);
        let first_id = save_scan_with_work_items(&db, site_id, project_id, url, &first, MS_APR18);
        let still_failing = make_scan(url, "2026-04-19T00:00:00Z", vec![fail_csp()]);
        let last_failure_id =
            save_scan_with_work_items(&db, site_id, project_id, url, &still_failing, MS_APR19);
        let fixed = make_scan(url, "2026-04-20T00:00:00Z", vec![pass_csp()]);
        let resolved_id =
            save_scan_with_work_items(&db, site_id, project_id, url, &fixed, MS_APR20);

        let resolved = db
            .get_resolved_issues(project_id, url.to_string(), 50)
            .expect("query");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].first_seen_scan_id, Some(first_id));
        assert_ne!(resolved[0].first_seen_scan_id, Some(last_failure_id));
        assert_eq!(resolved[0].resolved_scan_id, Some(resolved_id));
    }

    #[test]
    fn counts_recurrence_for_flaky_issues() {
        // fail -> pass -> fail -> pass produces 2 resolved events.
        let db = temp_db();
        let url = "https://flaky.example.com";
        let (project_id, site_id) = setup_project(&db, url);

        // Scan 1: fail (inserts work_item with first_seen_at=MS_APR18)
        let s1 = make_scan(url, "2026-04-18T00:00:00Z", vec![fail_csp()]);
        save_scan_with_work_items(&db, site_id, project_id, url, &s1, MS_APR18);

        let s2 = make_scan(url, "2026-04-19T00:00:00Z", vec![pass_csp()]);
        save_scan_with_work_items(&db, site_id, project_id, url, &s2, MS_APR19);

        let s3 = make_scan(url, "2026-04-20T00:00:00Z", vec![fail_csp()]);
        save_scan_with_work_items(&db, site_id, project_id, url, &s3, MS_APR20);

        // Scan 4: pass -> resolves the second work_item (resolved_at=MS_APR21)
        let s4 = make_scan(url, "2026-04-21T00:00:00Z", vec![pass_csp()]);
        save_scan_with_work_items(&db, site_id, project_id, url, &s4, MS_APR21);

        let resolved = db
            .get_resolved_issues(project_id, url.to_string(), 50)
            .expect("query");

        assert_eq!(resolved.len(), 2, "two resolution events expected");
        // Sorted newest-first by resolved_at.
        assert!(
            resolved[0].resolved_at.contains("2026-04-21"),
            "first (newest) resolved Apr 21, got {}",
            resolved[0].resolved_at
        );
        assert!(
            resolved[1].resolved_at.contains("2026-04-19"),
            "second resolved Apr 19, got {}",
            resolved[1].resolved_at
        );
        // Recurrence counts: newest has count 2, older has count 1
        assert_eq!(resolved[0].recurrence_count, 2);
        assert_eq!(resolved[1].recurrence_count, 1);

        let newest_only = db
            .get_resolved_issues(project_id, url.to_string(), 1)
            .expect("limited query");
        assert_eq!(newest_only.len(), 1);
        assert_eq!(
            newest_only[0].recurrence_count, 2,
            "recurrence ordinal must be computed before applying the result limit"
        );
    }

    #[test]
    fn persistent_failure_produces_no_resolution_events() {
        // A check that fails in every scan must never count as resolved.
        let db = temp_db();
        let url = "https://persistent.example.com";
        let (project_id, site_id) = setup_project(&db, url);

        for (ts, now_ms) in [
            ("2026-04-18T00:00:00Z", MS_APR18),
            ("2026-04-19T00:00:00Z", MS_APR19),
            ("2026-04-20T00:00:00Z", MS_APR20),
        ] {
            let s = make_scan(url, ts, vec![fail_csp()]);
            save_scan_with_work_items(&db, site_id, project_id, url, &s, now_ms);
        }

        let resolved = db
            .get_resolved_issues(project_id, url.to_string(), 50)
            .expect("query");

        assert!(
            resolved.is_empty(),
            "check failing in every scan cannot resolve"
        );
    }

    #[test]
    fn tracks_multiple_check_ids_independently() {
        // Two distinct check_ids failing simultaneously must not cross-pollute.
        // CSP resolves in scan 2; title resolves in scan 3.
        let db = temp_db();
        let url = "https://multi.example.com";
        let (project_id, site_id) = setup_project(&db, url);

        let fail_title = CheckResult {
            check_id: "seo.title".to_string(),
            category: ScanCategory::Seo,
            title: "Missing title".to_string(),
            description: "".to_string(),
            status: CheckStatus::Fail,
            severity: Severity::Medium,
            fix_prompt: None,
            manual_fix: None,
            raw_data: None,
            confidence: crate::checks::IssueConfidence::High,
            confidence_reason: None,
            why_it_matters: None,
        };
        let pass_title = CheckResult {
            status: CheckStatus::Pass,
            ..fail_title.clone()
        };

        // Scan 1: both fail.
        let s1 = make_scan(
            url,
            "2026-04-18T00:00:00Z",
            vec![fail_csp(), fail_title.clone()],
        );
        save_scan_with_work_items(&db, site_id, project_id, url, &s1, MS_APR18);
        // Scan 2: csp passes (resolved at MS_APR19), title still fails.
        let s2 = make_scan(
            url,
            "2026-04-19T00:00:00Z",
            vec![pass_csp(), fail_title.clone()],
        );
        save_scan_with_work_items(&db, site_id, project_id, url, &s2, MS_APR19);
        // Scan 3: both pass (title resolved at MS_APR20).
        let s3 = make_scan(url, "2026-04-20T00:00:00Z", vec![pass_csp(), pass_title]);
        save_scan_with_work_items(&db, site_id, project_id, url, &s3, MS_APR20);

        let resolved = db
            .get_resolved_issues(project_id, url.to_string(), 50)
            .expect("query");

        assert_eq!(resolved.len(), 2);
        let by_id: std::collections::HashMap<_, _> =
            resolved.iter().map(|r| (r.check_id.as_str(), r)).collect();
        let csp = by_id.get("security.csp").expect("csp resolved");
        assert!(
            csp.resolved_at.contains("2026-04-19"),
            "csp resolved Apr 19, got {}",
            csp.resolved_at
        );
        assert_eq!(csp.recurrence_count, 1);
        let title = by_id.get("seo.title").expect("title resolved");
        assert!(
            title.resolved_at.contains("2026-04-20"),
            "title resolved Apr 20, got {}",
            title.resolved_at
        );
        assert_eq!(title.recurrence_count, 1);
    }

    #[test]
    fn a_check_that_reported_a_verdict_resolves_the_finding_it_no_longer_reports() {
        let db = temp_db();
        let url = "https://missing.example.com";
        let (project_id, site_id) = setup_project(&db, url);

        let scan1 = make_scan(url, "2026-04-18T00:00:00Z", vec![fail_csp()]);
        save_scan_with_work_items(&db, site_id, project_id, url, &scan1, MS_APR18);

        let scan2 = make_scan(url, "2026-04-19T00:00:00Z", vec![pass_csp()]);
        save_scan_with_work_items(&db, site_id, project_id, url, &scan2, MS_APR19);

        let resolved = db
            .get_resolved_issues(project_id, url.to_string(), 50)
            .expect("query");

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].check_id, "security.csp");
        assert_eq!(resolved[0].recurrence_count, 1);
    }

    #[test]
    fn a_run_that_reported_nothing_resolves_nothing() {
        let db = temp_db();
        let url = "https://silent.example.com";
        let (project_id, site_id) = setup_project(&db, url);

        let scan1 = make_scan(url, "2026-04-18T00:00:00Z", vec![fail_csp()]);
        save_scan_with_work_items(&db, site_id, project_id, url, &scan1, MS_APR18);
        let scan2 = make_scan(url, "2026-04-19T00:00:00Z", vec![]);
        save_scan_with_work_items(&db, site_id, project_id, url, &scan2, MS_APR19);

        let resolved = db
            .get_resolved_issues(project_id, url.to_string(), 50)
            .expect("query");

        assert!(resolved.is_empty(), "{resolved:?}");
    }

    #[test]
    fn resolved_history_isolated_by_project_even_when_urls_match() {
        let db = temp_db();
        let url = "https://shared.example.com";
        let (first_project_id, _) = setup_project(&db, url);
        let second_project_id = db
            .upsert_project("Other Resolved Test", "/tmp/resolved-other", None)
            .expect("upsert second project");
        db.add_environment(second_project_id, url, "production", "production", "manual")
            .expect("add second environment");

        for (project_id, signal_id, title) in [
            (first_project_id, "web_scan:first", "First project issue"),
            (second_project_id, "web_scan:second", "Second project issue"),
        ] {
            let input = WorkItemInput {
                project_id,
                env_url: url.to_string(),
                source: "web_scan".to_string(),
                signal_id: signal_id.to_string(),
                check_id: "security.csp".to_string(),
                category: "security".to_string(),
                severity: Severity::High,
                title: title.to_string(),
                description: "project-scoped finding".to_string(),
                detail_json: None,
                scan_ref: None,
                page_url: None,
                fix_prompt: None,
                manual_fix: None,
                why_it_matters: None,
                observed_at: 1_000,
                metadata: WorkItemMetadata::default(),
            };
            db.upsert_work_items_diff("web_scan", project_id, url, vec![input], 1_000)
                .expect("insert project finding");
            db.upsert_work_items_diff("web_scan", project_id, url, vec![], 2_000)
                .expect("resolve project finding");
        }

        let resolved = db
            .get_resolved_issues(first_project_id, url.to_string(), 50)
            .expect("query first project");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].title, "First project issue");
        assert_eq!(resolved[0].first_seen_scan_id, None);
        assert_eq!(resolved[0].resolved_scan_id, None);
    }
}
