//! Tests for event timeline CRUD, junction writes, and epoch-ms ordering.

use crate::db::test_helpers::temp_db;
use crate::db::types::{EventSeverity, EventSource, EventType, SiteEvent};

fn make_event(project_id: i64, source_id: &str, occurred_at_ms: i64) -> SiteEvent {
    SiteEvent {
        id: 0,
        project_id,
        event_type: EventType::Deploy,
        severity: EventSeverity::Info,
        occurred_at_ms,
        title: "Test event".to_string(),
        summary: "summary".to_string(),
        detail: None,
        source: EventSource::Internal,
        source_id: Some(source_id.to_string()),
        metadata: None,
        affected_check_ids: None,
    }
}

fn ms(rfc3339: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .expect("valid RFC 3339 test timestamp")
        .timestamp_millis()
}

fn insert_junction(db: &crate::db::Database, event_id: i64, check_id: &str) {
    let check_id = check_id.to_string();
    db.execute(move |conn| {
        conn.execute(
            "INSERT OR IGNORE INTO site_event_check_ids (event_id, check_id) VALUES (?1, ?2)",
            rusqlite::params![event_id, check_id],
        )
        .map_err(|e| e.to_string())
    })
    .expect("execute junction insert")
    .expect("junction insert");
}

#[test]
fn recent_events_returned_via_junction() {
    let db = temp_db();
    let project_id = db
        .upsert_project("test", "https://example.com", None)
        .expect("upsert project");

    let event = make_event(project_id, "deploy-1", ms("2026-05-15T12:00:00Z"));
    let event_id = db.insert_event(&event).expect("insert event");

    insert_junction(&db, event_id, "performance.lcp");
    insert_junction(&db, event_id, "performance.compression");

    // Query with since_ms = 0 to include all events regardless of timestamp
    let since_ms: i64 = 0;
    let result = db
        .get_events_for_check_ids(project_id, &["performance.lcp".to_string()], since_ms)
        .expect("get events");

    assert_eq!(result.len(), 1, "expected 1 check_id key in result map");
    let events = result.get("performance.lcp").expect("events for lcp");
    assert_eq!(events.len(), 1, "expected 1 event for performance.lcp");
    assert_eq!(events[0].title, "Test event");
}

#[test]
fn recent_events_filtered_by_window() {
    let db = temp_db();
    let project_id = db
        .upsert_project("test2", "https://example.com", None)
        .expect("upsert project");

    let event = make_event(project_id, "deploy-old", ms("2025-01-01T00:00:00Z"));
    let event_id = db.insert_event(&event).expect("insert event");
    insert_junction(&db, event_id, "performance.lcp");

    let since_ms: i64 = ms("2026-01-01T00:00:00Z");
    let result = db
        .get_events_for_check_ids(project_id, &["performance.lcp".to_string()], since_ms)
        .expect("get events");

    assert!(
        result.is_empty() || result.get("performance.lcp").is_none_or(|v| v.is_empty()),
        "expected no events when the event occurred before the since window"
    );
}

#[test]
fn recent_events_returns_empty_for_unknown_check_id() {
    let db = temp_db();
    let project_id = db
        .upsert_project("test3", "https://example.com", None)
        .expect("upsert project");

    let since_ms: i64 = 0;
    let result = db
        .get_events_for_check_ids(project_id, &["nonexistent.check".to_string()], since_ms)
        .expect("get events");

    assert!(
        result.is_empty(),
        "expected empty map for unknown check_id, got: {:?}",
        result.keys().collect::<Vec<_>>()
    );
}

#[test]
fn insert_event_writes_junction_rows_when_affected_check_ids_set() {
    let db = temp_db();
    let project_id = db
        .upsert_project("junction-write-test", "https://junction.example.com", None)
        .expect("upsert project");

    let event = SiteEvent {
        id: 0,
        project_id,
        event_type: EventType::Deploy,
        severity: EventSeverity::Info,
        occurred_at_ms: ms("2026-05-15T12:00:00Z"),
        title: "Junction test event".to_string(),
        summary: "verifying junction rows".to_string(),
        detail: None,
        source: EventSource::Internal,
        source_id: Some("junction_test_1".to_string()),
        metadata: None,
        affected_check_ids: Some(vec![
            "performance.lcp".to_string(),
            "performance.compression".to_string(),
        ]),
    };

    let event_id = db.insert_event(&event).expect("insert event");
    assert!(event_id > 0, "expected valid event_id from insert");

    let count = db
        .execute(move |conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM site_event_check_ids WHERE event_id = ?1",
                rusqlite::params![event_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())
        })
        .expect("execute")
        .expect("query count");

    assert_eq!(
        count, 2,
        "expected 2 junction rows for 2 affected_check_ids"
    );
}

#[test]
fn insert_event_no_junction_rows_when_none() {
    let db = temp_db();
    let project_id = db
        .upsert_project(
            "junction-none-test",
            "https://junction-none.example.com",
            None,
        )
        .expect("upsert project");

    let event = SiteEvent {
        id: 0,
        project_id,
        event_type: EventType::Deploy,
        severity: EventSeverity::Info,
        occurred_at_ms: ms("2026-05-15T13:00:00Z"),
        title: "No junction test event".to_string(),
        summary: "verifying no junction rows written".to_string(),
        detail: None,
        source: EventSource::Internal,
        source_id: Some("junction_none_1".to_string()),
        metadata: None,
        affected_check_ids: None,
    };

    let event_id = db.insert_event(&event).expect("insert event");
    assert!(event_id > 0, "expected valid event_id from insert");

    let count = db
        .execute(move |conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM site_event_check_ids WHERE event_id = ?1",
                rusqlite::params![event_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())
        })
        .expect("execute")
        .expect("query count");

    assert_eq!(
        count, 0,
        "expected 0 junction rows when affected_check_ids is None"
    );
}

#[test]
fn get_events_orders_offset_timestamps_chronologically() {
    let db = temp_db();
    let project_id = db
        .upsert_project("offset-order", "https://offset.example.com", None)
        .expect("upsert project");

    let git_wall_time = "2026-07-05T22:00:00-04:00"; // 2026-07-06T02:00:00Z
    let utc_wall_time = "2026-07-05T23:00:00Z";
    assert!(
        git_wall_time < utc_wall_time,
        "precondition: this pair misordered under lexical TEXT comparison"
    );

    let git_event_id = db
        .insert_event(&make_event(project_id, "git-commit", ms(git_wall_time)))
        .expect("insert git event");
    let utc_event_id = db
        .insert_event(&make_event(project_id, "utc-event", ms(utc_wall_time)))
        .expect("insert utc event");

    let events = db
        .get_events(
            project_id,
            0,
            ms("2027-01-01T00:00:00Z"),
            None,
            None,
            None,
            None,
        )
        .expect("get events");

    assert_eq!(events.len(), 2, "both events must fall inside the range");
    assert_eq!(
        events[0].id, git_event_id,
        "the git event happened later (02:00Z next day) and must come first"
    );
    assert_eq!(events[1].id, utc_event_id);
}

#[test]
fn insert_event_row_and_junction_rows_are_atomic() {
    let db = temp_db();
    let project_id = db
        .upsert_project("atomic-test", "https://atomic.example.com", None)
        .expect("upsert project");

    let mut event = make_event(project_id, "atomic-1", ms("2026-06-01T00:00:00Z"));
    event.affected_check_ids = Some(vec!["performance.lcp".to_string()]);
    let event_id = db.insert_event(&event).expect("insert event");
    assert!(event_id > 0);

    let (event_count, junction_count) = db
        .execute(move |conn| {
            let events: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM events WHERE id = ?1",
                    rusqlite::params![event_id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            let junctions: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM site_event_check_ids WHERE event_id = ?1",
                    rusqlite::params![event_id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            Ok::<_, String>((events, junctions))
        })
        .expect("execute")
        .expect("counts");
    assert_eq!((event_count, junction_count), (1, 1));
}

#[test]
fn timestamp_text_to_ms_parses_all_legacy_formats() {
    use crate::db::types::timestamp_text_to_ms;

    // RFC 3339 with fractional seconds and offset
    assert_eq!(
        timestamp_text_to_ms("2026-07-05T09:14:07.500-04:00"),
        Some(ms("2026-07-05T13:14:07.500Z"))
    );
    assert_eq!(
        timestamp_text_to_ms("2026-07-05T13:14:07Z"),
        Some(ms("2026-07-05T13:14:07Z"))
    );
    // Naive T format, treated as UTC
    assert_eq!(
        timestamp_text_to_ms("2026-07-05T13:14:07"),
        Some(ms("2026-07-05T13:14:07Z"))
    );
    // SQLite datetime('now') space format, treated as UTC
    assert_eq!(
        timestamp_text_to_ms("2026-07-05 13:14:07"),
        Some(ms("2026-07-05T13:14:07Z"))
    );
    assert_eq!(timestamp_text_to_ms("not a timestamp"), None);
}
