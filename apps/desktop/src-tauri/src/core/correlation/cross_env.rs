//! Finds production issues recently observed in non-production environments.

use std::collections::{HashMap, HashSet};

use crate::core::types_work_items::CrossEnvSignal;
use crate::db::Database;

const CROSS_ENV_WINDOW_DAYS: i64 = 14;

pub fn resolve_for_groups(
    db: &Database,
    project_id: i64,
    current_env_url: &str,
    is_current_env_prod: bool,
    check_ids: &HashSet<String>,
) -> Result<HashMap<String, CrossEnvSignal>, String> {
    if !is_current_env_prod {
        return Ok(HashMap::new());
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    let cutoff_ms = now_ms - CROSS_ENV_WINDOW_DAYS * 24 * 60 * 60 * 1000;
    let rows = db
        .get_nonprod_first_seen_by_check(project_id, current_env_url, cutoff_ms)
        .map_err(|e| e.to_string())?;

    rows
        .into_iter()
        .filter(|(check_id, _)| check_ids.contains(check_id))
        .map(|(check_id, earliest)| {
            let days_before = ((now_ms - earliest) / (24 * 60 * 60 * 1000)).max(0);
            let observed_at = chrono::DateTime::from_timestamp_millis(earliest)
                .map(|date| date.to_rfc3339())
                .ok_or_else(|| {
                    format!(
                        "invalid first_seen_at timestamp {earliest} for cross-environment issue {check_id}"
                    )
                })?;
            Ok((
                check_id,
                CrossEnvSignal {
                    staging_observed_at: observed_at,
                    days_before_prod: days_before,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
fn resolve_for_group(
    db: &Database,
    project_id: i64,
    current_env_url: &str,
    is_current_env_prod: bool,
    check_id: &str,
) -> Option<CrossEnvSignal> {
    resolve_for_groups(
        db,
        project_id,
        current_env_url,
        is_current_env_prod,
        &HashSet::from([check_id.to_string()]),
    )
    .ok()?
    .remove(check_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::Severity;
    use crate::db::test_helpers::temp_db;
    use crate::db::work_items::WorkItemInput;
    use crate::db::work_items::WorkItemMetadata;

    fn seed_work_item(
        db: &Database,
        project_id: i64,
        env_url: &str,
        check_id: &str,
        first_seen_ms: i64,
    ) {
        db.upsert_work_items_diff(
            "web_scan",
            project_id,
            env_url,
            vec![WorkItemInput {
                project_id,
                env_url: env_url.into(),
                source: "web_scan".into(),
                signal_id: format!("web_scan:{}:{}", check_id, env_url),
                check_id: check_id.into(),
                category: "performance".into(),
                severity: Severity::High,
                title: "t".into(),
                description: "d".into(),
                detail_json: None,
                scan_ref: None,
                page_url: None,
                fix_prompt: None,
                manual_fix: None,
                why_it_matters: None,
                observed_at: first_seen_ms,
                metadata: WorkItemMetadata::default(),
            }],
            first_seen_ms,
        )
        .expect("upsert_work_items_diff should succeed");
    }

    #[test]
    fn no_signal_when_not_prod() {
        let db = temp_db();
        // Early-return before any DB call: no need to seed anything.
        let signal = resolve_for_group(
            &db,
            1,
            "https://staging.example.com",
            false,
            "performance.lcp",
        );
        assert!(signal.is_none());
    }

    #[test]
    fn surfaces_nonprod_first_seen() {
        let db = temp_db();
        let project_id: i64 = db
            .upsert_project("test", "https://example.com", None)
            .expect("upsert_project");

        // Register environments: prod + staging
        db.add_environment(
            project_id,
            "https://example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("add prod env");
        db.add_environment(
            project_id,
            "https://staging.example.com",
            "Staging",
            "development",
            "manual",
        )
        .expect("add staging env");

        // Seed: staging saw the check 3 days ago
        let now_ms = chrono::Utc::now().timestamp_millis();
        let three_days_ms = now_ms - 3 * 24 * 60 * 60 * 1000;
        seed_work_item(
            &db,
            project_id,
            "https://staging.example.com",
            "performance.lcp",
            three_days_ms,
        );

        let signal = resolve_for_group(
            &db,
            project_id,
            "https://example.com",
            true,
            "performance.lcp",
        );

        assert!(
            signal.is_some(),
            "expected cross-env signal when staging has the check"
        );
        let s = signal.unwrap();
        assert!(s.days_before_prod <= 4, "days_before_prod should be ~3");
        assert!(!s.staging_observed_at.is_empty());
    }

    #[test]
    fn no_signal_when_no_nonprod_data() {
        let db = temp_db();
        let project_id: i64 = db
            .upsert_project("test", "https://example.com", None)
            .expect("upsert_project");

        db.add_environment(
            project_id,
            "https://example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("add prod env");

        let signal = resolve_for_group(
            &db,
            project_id,
            "https://example.com",
            true,
            "performance.lcp",
        );

        assert!(signal.is_none());
    }

    #[test]
    fn no_signal_when_outside_window() {
        let db = temp_db();
        let project_id: i64 = db
            .upsert_project("test", "https://example.com", None)
            .expect("upsert_project");

        db.add_environment(
            project_id,
            "https://example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("add prod env");
        db.add_environment(
            project_id,
            "https://staging.example.com",
            "Staging",
            "development",
            "manual",
        )
        .expect("add staging env");

        // Seed staging 30 days ago (outside the 14-day window)
        let now_ms = chrono::Utc::now().timestamp_millis();
        let thirty_days_ms = now_ms - 30 * 24 * 60 * 60 * 1000;
        seed_work_item(
            &db,
            project_id,
            "https://staging.example.com",
            "performance.lcp",
            thirty_days_ms,
        );

        let signal = resolve_for_group(
            &db,
            project_id,
            "https://example.com",
            true,
            "performance.lcp",
        );

        assert!(
            signal.is_none(),
            "should not surface signal outside the 14-day window"
        );
    }

    #[test]
    fn invalid_first_seen_timestamp_is_an_error_not_empty_evidence() {
        let db = temp_db();
        let project_id = db
            .upsert_project("test", "https://example.com", None)
            .expect("upsert_project");
        db.add_environment(
            project_id,
            "https://example.com",
            "Production",
            "production",
            "manual",
        )
        .expect("add prod env");
        db.add_environment(
            project_id,
            "https://staging.example.com",
            "Staging",
            "staging",
            "manual",
        )
        .expect("add staging env");
        seed_work_item(
            &db,
            project_id,
            "https://staging.example.com",
            "performance.lcp",
            i64::MAX,
        );

        let error = resolve_for_groups(
            &db,
            project_id,
            "https://example.com",
            true,
            &HashSet::from(["performance.lcp".to_string()]),
        )
        .expect_err("invalid timestamps must not produce an empty evidence field");

        assert!(error.contains("invalid first_seen_at timestamp"));
        assert!(error.contains("performance.lcp"));
    }
}
