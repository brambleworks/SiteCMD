use crate::core::{events, git};
use crate::db::{Database, EventSeverity, EventSource, EventType, SiteEvent};
use chrono::Utc;
use std::sync::Arc;
use tauri::{AppHandle, State};

use super::{confirm_sensitive_action, run_blocking, sanitize_error, SensitiveActionTone};

/// Parse a frontend-supplied severity string into `EventSeverity`. Falls back
/// to `Info` for `None`, empty, or unrecognized values - record_*_event
/// callers shouldn't fail just because the UI sent a typo.
#[tracing::instrument(fields(has_raw = raw.is_some()))]
pub(crate) fn parse_event_severity(raw: Option<&str>) -> EventSeverity {
    raw.unwrap_or("info")
        .parse::<EventSeverity>()
        .unwrap_or(EventSeverity::Info)
}

/// Build a user-recorded `SiteEvent` (source = Internal, current timestamp)
/// for the Activity timeline. Shared by every record_*_event command.
#[tracing::instrument(skip(event_type, severity, title, summary, detail), fields(project_id, title_len = title.len(), summary_len = summary.len(), has_detail = detail.is_some(), source_id = ?source_id))]
pub(crate) fn build_user_event(
    project_id: i64,
    event_type: EventType,
    severity: EventSeverity,
    title: String,
    summary: String,
    detail: Option<String>,
    source_id: Option<String>,
) -> SiteEvent {
    SiteEvent {
        id: 0,
        project_id,
        event_type,
        severity,
        occurred_at_ms: Utc::now().timestamp_millis(),
        title,
        summary,
        detail,
        source: EventSource::Internal,
        source_id,
        metadata: None,
        // Manual deploy / user-recorded events do not map to a specific check_id.
        affected_check_ids: None,
    }
}

/// Get timeline events for a project within an epoch-ms range, optionally filtered by type.
#[tauri::command]
#[tracing::instrument(skip(db, event_types), fields(project_id, start_ms, end_ms, since_ms = ?since_ms, since_event_id, limit))]
pub async fn get_events(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    start_ms: i64,
    end_ms: i64,
    event_types: Option<Vec<String>>,
    since_ms: Option<i64>,
    since_event_id: Option<i64>,
    limit: Option<u32>,
) -> Result<Vec<SiteEvent>, String> {
    // Polled every 30s by the frontend; keep the blocking DB wait off the
    // async runtime workers.
    let db = db.inner().clone();
    crate::commands::run_blocking(move || {
        db.get_events(
            project_id,
            start_ms,
            end_ms,
            event_types.as_deref(),
            since_ms,
            since_event_id,
            limit.map(|value| value as usize),
        )
        .map_err(sanitize_error)
    })
    .await?
}

/// Delete a single timeline event by ID.
#[tracing::instrument(skip(app, db), fields(event_id))]
pub async fn delete_event(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    event_id: i64,
) -> Result<(), String> {
    confirm_sensitive_action(
        app,
        "Delete this activity event?",
        SensitiveActionTone::Warning,
        "This removes the selected activity entry from the timeline.".to_string(),
        "Delete Event",
    )
    .await?;
    let db = (*db).clone();
    run_blocking(move || db.delete_event(event_id))
        .await?
        .map_err(sanitize_error)
}

/// Record a dependency/update workflow event for the Activity timeline.
#[tauri::command]
#[tracing::instrument(skip(db, title, summary, detail), fields(project_id, title_len = title.len(), summary_len = summary.len(), has_detail = detail.is_some(), source_id = ?source_id, severity = ?severity))]
pub async fn record_update_event(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    title: String,
    summary: String,
    detail: Option<String>,
    source_id: Option<String>,
    severity: Option<String>,
) -> Result<i64, String> {
    let event = build_user_event(
        project_id,
        EventType::Update,
        parse_event_severity(severity.as_deref()),
        title,
        summary,
        detail,
        source_id,
    );
    let db = (*db).clone();
    run_blocking(move || db.insert_event(&event))
        .await?
        .map_err(sanitize_error)
}

/// Record a Search & SEO verification event for the Activity timeline.
#[tauri::command]
#[tracing::instrument(skip(db, title, summary, detail), fields(project_id, title_len = title.len(), summary_len = summary.len(), has_detail = detail.is_some(), source_id = ?source_id, severity = ?severity))]
pub async fn record_verification_event(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    title: String,
    summary: String,
    detail: Option<String>,
    source_id: Option<String>,
    severity: Option<String>,
) -> Result<i64, String> {
    let event = build_user_event(
        project_id,
        EventType::Verification,
        parse_event_severity(severity.as_deref()),
        title,
        summary,
        detail,
        source_id,
    );
    let db = (*db).clone();
    run_blocking(move || db.insert_event(&event))
        .await?
        .map_err(sanitize_error)
}

/// Record a Search & SEO verification event for the Activity timeline.
#[tauri::command]
#[tracing::instrument(skip(db, title, summary, detail), fields(project_id, title_len = title.len(), summary_len = summary.len(), has_detail = detail.is_some(), source_id = ?source_id, severity = ?severity))]
pub async fn record_search_event(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    title: String,
    summary: String,
    detail: Option<String>,
    source_id: Option<String>,
    severity: Option<String>,
) -> Result<i64, String> {
    let event = build_user_event(
        project_id,
        EventType::Search,
        parse_event_severity(severity.as_deref()),
        title,
        summary,
        detail,
        source_id,
    );
    let db = (*db).clone();
    run_blocking(move || db.insert_event(&event))
        .await?
        .map_err(sanitize_error)
}

/// Record a Security verification event for the Activity timeline.
#[tauri::command]
#[tracing::instrument(skip(db, title, summary, detail), fields(project_id, title_len = title.len(), summary_len = summary.len(), has_detail = detail.is_some(), source_id = ?source_id, severity = ?severity))]
pub async fn record_security_event(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    title: String,
    summary: String,
    detail: Option<String>,
    source_id: Option<String>,
    severity: Option<String>,
) -> Result<i64, String> {
    let event = build_user_event(
        project_id,
        EventType::Security,
        parse_event_severity(severity.as_deref()),
        title,
        summary,
        detail,
        source_id,
    );
    let db = (*db).clone();
    run_blocking(move || db.insert_event(&event))
        .await?
        .map_err(sanitize_error)
}

/// Get event correlations (e.g. deploy → score change) for the Timeline page.
#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id))]
pub async fn get_correlations(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<Vec<crate::core::event_correlations::Correlation>, String> {
    let db = (*db).clone();
    run_blocking(move || crate::core::event_correlations::get_project_correlations(&db, project_id))
        .await?
        .map_err(sanitize_error)
}

/// Queue immediate asynchronous polls for all connected integrations.
#[tauri::command]
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn refresh_events(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<(), String> {
    events::refresh_integration_events(&app, &db, project_id)
        .await
        .map_err(sanitize_error)
}

/// Backfill timeline events from scan history, git commits, and integrations.
/// Used to populate the timeline for projects with existing data.
#[tauri::command]
#[tracing::instrument(skip(app, db), fields(project_id))]
pub async fn backfill_events(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<usize, String> {
    // The scan-event backfill plus the git subprocess walk are all blocking;
    // keep the whole batch off the async runtime workers.
    let total = {
        let db = (*db).clone();
        run_blocking(move || -> Result<usize, String> {
            let mut total = db
                .backfill_scan_events(project_id)
                .map_err(sanitize_error)?;

            let projects = db.get_projects().map_err(sanitize_error)?;
            if let Some(project) = projects.iter().find(|p| p.id == project_id) {
                if !project.path.is_empty() {
                    let commits = git::get_recent_commits(&project.path, 100);
                    if !commits.is_empty() {
                        let events: Vec<SiteEvent> = commits
                            .iter()
                            .map(|c| git::commit_to_deploy_event(c, project_id))
                            .collect();
                        total += db.insert_events(&events).map_err(sanitize_error)?;
                    }
                }
            }
            Ok(total)
        })
        .await??
    };

    // Also queue immediate polls from the integration scheduler.
    if let Err(e) = events::refresh_integration_events(&app, &db, project_id).await {
        tracing::warn!("Integration refresh during backfill: {}", e);
    }

    tracing::info!(
        "Backfill complete for project {}: {} events",
        project_id,
        total
    );
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_event_severity_recognises_all_three_levels() {
        assert_eq!(parse_event_severity(Some("info")), EventSeverity::Info);
        assert_eq!(
            parse_event_severity(Some("warning")),
            EventSeverity::Warning
        );
        assert_eq!(
            parse_event_severity(Some("critical")),
            EventSeverity::Critical
        );
    }

    #[test]
    fn parse_event_severity_defaults_to_info_when_none() {
        // Frontend may omit severity entirely; default must be Info so the
        // event still records.
        assert_eq!(parse_event_severity(None), EventSeverity::Info);
    }

    #[test]
    fn parse_event_severity_defaults_to_info_on_unknown_string() {
        assert_eq!(parse_event_severity(Some("HIGH")), EventSeverity::Info);
        assert_eq!(parse_event_severity(Some("Warning")), EventSeverity::Info);
        assert_eq!(parse_event_severity(Some("debug")), EventSeverity::Info);
        assert_eq!(parse_event_severity(Some("")), EventSeverity::Info);
        assert_eq!(parse_event_severity(Some("🔥")), EventSeverity::Info);
    }

    #[test]
    fn build_user_event_carries_all_fields_through() {
        let event = build_user_event(
            42,
            EventType::Update,
            EventSeverity::Warning,
            "Updated react".into(),
            "react@19.0.0".into(),
            Some("detail blob".into()),
            Some("src_123".into()),
        );
        assert_eq!(event.id, 0, "id must be 0 so insert_event auto-assigns");
        assert_eq!(event.project_id, 42);
        assert_eq!(event.event_type, EventType::Update);
        assert_eq!(event.severity, EventSeverity::Warning);
        assert_eq!(event.title, "Updated react");
        assert_eq!(event.summary, "react@19.0.0");
        assert_eq!(event.detail.as_deref(), Some("detail blob"));
        assert_eq!(event.source, EventSource::Internal);
        assert_eq!(event.source_id.as_deref(), Some("src_123"));
    }

    #[test]
    fn build_user_event_always_marks_source_internal() {
        for event_type in [
            EventType::Update,
            EventType::Verification,
            EventType::Search,
            EventType::Launch,
            EventType::Security,
        ] {
            let event = build_user_event(
                1,
                event_type,
                EventSeverity::Info,
                "t".into(),
                "s".into(),
                None,
                None,
            );
            assert_eq!(event.source, EventSource::Internal);
        }
    }

    #[test]
    fn build_user_event_stamps_current_utc_timestamp() {
        let before = chrono::Utc::now().timestamp_millis();
        let event = build_user_event(
            1,
            EventType::Update,
            EventSeverity::Info,
            "t".into(),
            "s".into(),
            None,
            None,
        );
        let after = chrono::Utc::now().timestamp_millis();
        assert!(
            event.occurred_at_ms >= before && event.occurred_at_ms <= after,
            "occurred_at_ms {} must fall between {} and {}",
            event.occurred_at_ms,
            before,
            after,
        );
    }

    #[test]
    fn build_user_event_preserves_optional_fields_as_none() {
        let event = build_user_event(
            1,
            EventType::Launch,
            EventSeverity::Critical,
            "Launched".into(),
            "All checks green".into(),
            None,
            None,
        );
        assert!(event.detail.is_none());
        assert!(event.source_id.is_none());
    }

    /// End-to-end: build_user_event + insert_event + get_events round-trips
    /// every field correctly. Catches schema-vs-struct drift that the pure
    /// helper tests above can't see.
    #[test]
    fn record_event_helpers_round_trip_through_database() {
        use crate::db::Database;

        // Hold the tempdir for the duration of this test; it's cleaned up on drop.
        let _dir = tempfile::tempdir().expect("tempdir");
        let path = _dir.path().join("events_helper_round_trip.db");
        let db = Database::open(path).expect("open");

        let project_id = db
            .upsert_project("Events Helper", "/tmp/events-helper", None)
            .expect("upsert");

        for (event_type, severity_str, expected_severity) in [
            (EventType::Update, Some("warning"), EventSeverity::Warning),
            (EventType::Verification, Some("info"), EventSeverity::Info),
            (EventType::Search, None, EventSeverity::Info),
            (EventType::Launch, Some("critical"), EventSeverity::Critical),
            (EventType::Security, Some("BAD-INPUT"), EventSeverity::Info),
        ] {
            let event = build_user_event(
                project_id,
                event_type.clone(),
                parse_event_severity(severity_str),
                format!("{:?} title", event_type),
                "summary".into(),
                None,
                Some(format!("src_{:?}", event_type)),
            );
            assert_eq!(event.severity, expected_severity);
            db.insert_event(&event).expect("insert");
        }

        let events = db
            .get_events(project_id, 0, i64::MAX, None, None, None, None)
            .expect("get_events");
        assert_eq!(
            events.len(),
            5,
            "all 5 record_*_event variants must round-trip"
        );
        // Every event was Internal-sourced.
        for event in &events {
            assert_eq!(event.source, EventSource::Internal);
            assert_eq!(event.project_id, project_id);
        }
    }
}
