//! Local-only recurrence signals across the user's projects.

use std::collections::{HashMap, HashSet};

use crate::core::types_work_items::CrossProjectPattern;
use crate::db::Database;

const CROSS_PROJECT_WINDOW_DAYS: i64 = 90;

pub fn rebuild_index(db: &Database) -> Result<(), String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let cutoff_ms = now_ms - CROSS_PROJECT_WINDOW_DAYS * 24 * 60 * 60 * 1000;

    db.rebuild_cross_project_pattern_index(cutoff_ms, now_ms)
        .map_err(|e| e.to_string())
}

pub fn resolve_patterns(
    db: &Database,
    current_project_id: i64,
    check_ids: &HashSet<String>,
) -> Result<HashMap<String, CrossProjectPattern>, String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let cutoff_ms = now_ms - CROSS_PROJECT_WINDOW_DAYS * 24 * 60 * 60 * 1000;
    let rows = db
        .get_cross_project_check_counts(current_project_id, cutoff_ms)
        .map_err(|e| e.to_string())?;

    rows
        .into_iter()
        .filter(|(check_id, _, _)| check_ids.contains(check_id))
        .map(|(check_id, project_count, latest_ms)| {
            let last_seen_at = chrono::DateTime::from_timestamp_millis(latest_ms)
                .map(|date| date.to_rfc3339())
                .ok_or_else(|| {
                    format!(
                        "invalid first_seen_at timestamp {latest_ms} for cross-project issue {check_id}"
                    )
                })?;
            Ok((
                check_id,
                CrossProjectPattern {
                    project_count,
                    last_seen_at,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
fn resolve_pattern(
    db: &Database,
    check_id: &str,
    current_project_id: i64,
) -> Option<CrossProjectPattern> {
    resolve_patterns(
        db,
        current_project_id,
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
                signal_id: format!("web_scan:{}:{}:{}", check_id, env_url, project_id),
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
    fn rebuild_runs_without_error_on_empty_db() {
        let db = temp_db();
        rebuild_index(&db).expect("rebuild should not error");
    }

    #[test]
    fn resolve_pattern_excludes_current_project() {
        let db = temp_db();

        let p1 = db
            .upsert_project("project1", "https://project1.com", None)
            .expect("upsert project1");
        let p2 = db
            .upsert_project("project2", "https://project2.com", None)
            .expect("upsert project2");

        let now_ms = chrono::Utc::now().timestamp_millis();
        let thirty_days_ms = now_ms - 30 * 24 * 60 * 60 * 1000;

        seed_work_item(
            &db,
            p1,
            "https://project1.com",
            "performance.lcp",
            thirty_days_ms,
        );
        seed_work_item(
            &db,
            p2,
            "https://project2.com",
            "performance.lcp",
            thirty_days_ms,
        );

        // From project 1's perspective: 1 other project has this check_id
        let pattern1 = resolve_pattern(&db, "performance.lcp", p1);
        assert!(
            pattern1.is_some(),
            "project1 should see cross-project pattern"
        );
        assert_eq!(
            pattern1.unwrap().project_count,
            1,
            "should count 1 other project"
        );

        // From project 2's perspective: 1 other project has this check_id
        let pattern2 = resolve_pattern(&db, "performance.lcp", p2);
        assert!(
            pattern2.is_some(),
            "project2 should see cross-project pattern"
        );
        assert_eq!(
            pattern2.unwrap().project_count,
            1,
            "should count 1 other project"
        );
    }

    #[test]
    fn resolve_pattern_returns_none_when_only_current_project() {
        let db = temp_db();

        let p1 = db
            .upsert_project("project1", "https://project1.com", None)
            .expect("upsert project1");

        let now_ms = chrono::Utc::now().timestamp_millis();
        seed_work_item(&db, p1, "https://project1.com", "performance.lcp", now_ms);

        let pattern = resolve_pattern(&db, "performance.lcp", p1);
        assert!(
            pattern.is_none(),
            "should return None when only the current project has the check_id"
        );
    }

    #[test]
    fn resolve_pattern_excludes_outside_window() {
        let db = temp_db();

        let p1 = db
            .upsert_project("project1", "https://project1.com", None)
            .expect("upsert project1");
        let p2 = db
            .upsert_project("project2", "https://project2.com", None)
            .expect("upsert project2");

        let now_ms = chrono::Utc::now().timestamp_millis();
        // Project 2 had the check 200 days ago (outside the 90-day window)
        let two_hundred_days_ms = now_ms - 200 * 24 * 60 * 60 * 1000;

        seed_work_item(&db, p1, "https://project1.com", "performance.lcp", now_ms);
        seed_work_item(
            &db,
            p2,
            "https://project2.com",
            "performance.lcp",
            two_hundred_days_ms,
        );

        let pattern = resolve_pattern(&db, "performance.lcp", p1);
        assert!(
            pattern.is_none(),
            "should return None when other project's data is outside the 90-day window"
        );
    }

    #[test]
    fn rebuild_populates_index_from_work_items() {
        let db = temp_db();

        let p1 = db
            .upsert_project("project1", "https://project1.com", None)
            .expect("upsert project1");
        let p2 = db
            .upsert_project("project2", "https://project2.com", None)
            .expect("upsert project2");

        let now_ms = chrono::Utc::now().timestamp_millis();
        seed_work_item(&db, p1, "https://project1.com", "performance.lcp", now_ms);
        seed_work_item(&db, p2, "https://project2.com", "performance.lcp", now_ms);

        rebuild_index(&db).expect("rebuild should succeed");

        // Verify the index was populated by querying it directly
        let count: i64 = db
            .execute(|conn| {
                conn.query_row(
                    "SELECT project_count FROM cross_project_pattern_index WHERE check_id = 'performance.lcp'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0)
            })
            .expect("execute should succeed");

        assert_eq!(
            count, 2,
            "index should record 2 projects for performance.lcp"
        );
    }

    #[test]
    fn invalid_latest_timestamp_is_an_error_not_empty_evidence() {
        let db = temp_db();
        let current_project = db
            .upsert_project("current", "https://current.example", None)
            .expect("upsert current project");
        let other_project = db
            .upsert_project("other", "https://other.example", None)
            .expect("upsert other project");
        seed_work_item(
            &db,
            other_project,
            "https://other.example",
            "performance.lcp",
            i64::MAX,
        );

        let error = resolve_patterns(
            &db,
            current_project,
            &HashSet::from(["performance.lcp".to_string()]),
        )
        .expect_err("invalid timestamps must not produce an empty evidence field");

        assert!(error.contains("invalid first_seen_at timestamp"));
        assert!(error.contains("performance.lcp"));
    }
}
