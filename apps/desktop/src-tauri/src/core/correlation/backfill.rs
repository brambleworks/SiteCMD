//! Retroactive enrichment writes when a user connects a new integration.
//! Walks recent work_items, calls relevant enrichers, materializes results
//! into `historical_enrichments` (created in migration 032).

use std::sync::Arc;

use crate::core::correlation::integration_hints::INTEGRATION_HINTS;
use crate::db::Database;
use crate::integrations::IntegrationType;

const BACKFILL_WINDOW_DAYS: i64 = 90;

pub async fn run(
    integration_type: IntegrationType,
    project_id: i64,
    db: Arc<Database>,
) -> Result<(), String> {
    let cutoff_ms =
        chrono::Utc::now().timestamp_millis() - BACKFILL_WINDOW_DAYS * 24 * 60 * 60 * 1000;

    // Get (work_item_id, check_id) for items in window.
    let items: Vec<(i64, String)> = db
        .get_recent_work_item_check_ids(project_id, cutoff_ms)
        .map_err(|error| error.to_string())?;

    // Use Debug repr of the enum as the integration label stored in historical_enrichments.
    let integration_label = format!("{:?}", integration_type);
    let enrichment_cache =
        crate::core::correlation::enrichments::EnrichmentCache::load(&db, project_id)?;

    // Compute in memory, then persist all enrichment rows in one transaction.
    let mut pending: Vec<(i64, String)> = Vec::new();
    for (work_item_id, check_id) in items {
        for hint in INTEGRATION_HINTS {
            if hint.integration != integration_type || hint.check_id != check_id {
                continue;
            }
            let Some(enricher) = hint.enricher else {
                continue;
            };
            let Some(payload) = enricher(&check_id, &enrichment_cache)? else {
                continue;
            };
            let json = serde_json::to_string(&payload)
                .map_err(|error| format!("serialize historical enrichment: {error}"))?;
            pending.push((work_item_id, json));
        }
    }

    if pending.is_empty() {
        return Ok(());
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    db.insert_historical_enrichments(&integration_label, pending, now_ms)
        .map_err(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::Severity;
    use crate::core::correlation::enrichments::write_cache_payload;
    use crate::db::test_helpers::temp_db_arc;
    use crate::db::work_items::WorkItemInput;
    use crate::db::work_items::WorkItemMetadata;
    use rusqlite::params;

    fn seed_work_item(db: &Database, project_id: i64, check_id: &str, first_seen_at: i64) -> i64 {
        let env_url = "https://example.com".to_string();
        let check_id_owned = check_id.to_string();
        db.upsert_work_items_diff(
            "web_scan",
            project_id,
            &env_url,
            vec![WorkItemInput {
                project_id,
                env_url: env_url.clone(),
                source: "web_scan".into(),
                signal_id: format!("web_scan:{}:{}", check_id, env_url),
                check_id: check_id_owned,
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
                observed_at: first_seen_at,
                metadata: WorkItemMetadata::default(),
            }],
            first_seen_at,
        )
        .expect("upsert_work_items_diff");

        // Return the inserted work_item id.
        db.execute(move |conn| {
            conn.query_row(
                "SELECT id FROM work_items WHERE project_id = ?1 AND check_id = ?2",
                params![project_id, "performance.lcp"],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
        })
        .unwrap_or(0)
    }

    #[tokio::test]
    async fn backfill_writes_historical_enrichments_when_cache_has_data() {
        let tdb = temp_db_arc();
        let db = tdb.db.clone();

        let project_id: i64 = db
            .upsert_project("backfill-test", "https://example.com", None)
            .expect("upsert project");

        let now_ms = chrono::Utc::now().timestamp_millis();
        // Seed a recent work_item (within 90-day window).
        seed_work_item(&db, project_id, "performance.lcp", now_ms - 1000);

        // Seed integration_enrichment_cache with fresh data for gsc/field_lcp.
        write_cache_payload(
            &db,
            project_id,
            "gsc",
            "field_lcp",
            r#"{"p75_ms":2800,"url":"https://example.com/"}"#,
        )
        .expect("write cache");

        run(IntegrationType::GoogleSearchConsole, project_id, db.clone())
            .await
            .expect("backfill");

        let count: i64 = db
            .execute(|conn| {
                conn.query_row("SELECT COUNT(*) FROM historical_enrichments", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0)
            })
            .unwrap_or(0);
        assert!(
            count >= 1,
            "backfill should write at least one historical_enrichments row; got {count}"
        );
    }

    #[tokio::test]
    async fn backfill_skips_items_outside_window() {
        let tdb = temp_db_arc();
        let db = tdb.db.clone();

        let project_id: i64 = db
            .upsert_project("backfill-old", "https://old.example.com", None)
            .expect("upsert project");

        // Seed a work_item older than 90 days.
        let old_ms = chrono::Utc::now().timestamp_millis() - 200 * 24 * 60 * 60 * 1000;
        seed_work_item(&db, project_id, "performance.lcp", old_ms);

        // Fresh cache data available, but item is outside window.
        write_cache_payload(
            &db,
            project_id,
            "gsc",
            "field_lcp",
            r#"{"p75_ms":2800,"url":"https://old.example.com/"}"#,
        )
        .expect("write cache");

        run(IntegrationType::GoogleSearchConsole, project_id, db.clone())
            .await
            .expect("backfill");

        let count: i64 = db
            .execute(|conn| {
                conn.query_row("SELECT COUNT(*) FROM historical_enrichments", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0)
            })
            .unwrap_or(0);
        assert_eq!(
            count, 0,
            "items outside the 90-day window should be skipped"
        );
    }

    #[tokio::test]
    async fn malformed_cache_payload_is_an_error_not_a_skipped_enrichment() {
        let tdb = temp_db_arc();
        let db = tdb.db.clone();
        let project_id = db
            .upsert_project("backfill-invalid", "https://example.com", None)
            .expect("upsert project");
        seed_work_item(
            &db,
            project_id,
            "performance.lcp",
            chrono::Utc::now().timestamp_millis(),
        );
        write_cache_payload(
            &db,
            project_id,
            "gsc",
            "field_lcp",
            r#"{"p75_ms":"not-a-number","url":"https://example.com/"}"#,
        )
        .expect("write cache");

        let error = run(IntegrationType::GoogleSearchConsole, project_id, db)
            .await
            .expect_err("malformed enrichment evidence must not be skipped");

        assert!(error.contains("invalid gsc/field_lcp enrichment payload"));
    }
}
