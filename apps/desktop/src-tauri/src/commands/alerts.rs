use std::sync::Arc;

use tauri::State;

use crate::db::alerts::{AlertFilter, AlertRow};
use crate::db::Database;

use super::run_blocking;

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id, filter = ?filter, since_ms))]
pub async fn get_alerts(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    filter: Option<String>,
    since_ms: Option<i64>,
) -> Result<Vec<AlertRow>, String> {
    let f = match filter.as_deref() {
        Some("unread") | None => AlertFilter::Unread,
        Some("all") => AlertFilter::All,
        Some("viewed") => AlertFilter::Viewed,
        Some("dismissed") => AlertFilter::Dismissed,
        Some(other) => return Err(format!("unknown filter: {}", other)),
    };
    let db = (*db).clone();
    run_blocking(move || db.get_alerts(project_id, f, since_ms))
        .await?
        .map_err(String::from)
}

#[tauri::command]
#[tracing::instrument(skip(db), fields(alert_id))]
pub async fn mark_alert_viewed(db: State<'_, Arc<Database>>, alert_id: i64) -> Result<(), String> {
    let db = (*db).clone();
    run_blocking(move || db.mark_alert_viewed(alert_id, now_ms()))
        .await?
        .map_err(String::from)
}

#[tauri::command]
#[tracing::instrument(skip(db), fields(alert_id))]
pub async fn mark_alert_unread(db: State<'_, Arc<Database>>, alert_id: i64) -> Result<(), String> {
    let db = (*db).clone();
    run_blocking(move || db.mark_alert_unread(alert_id))
        .await?
        .map_err(String::from)
}

#[tauri::command]
#[tracing::instrument(skip(db), fields(alert_id))]
pub async fn dismiss_alert(db: State<'_, Arc<Database>>, alert_id: i64) -> Result<(), String> {
    let db = (*db).clone();
    run_blocking(move || db.dismiss_alert(alert_id, now_ms()))
        .await?
        .map_err(String::from)
}

#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id))]
pub async fn count_unread_alerts(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<crate::db::alerts::UnreadAlertCounts, String> {
    let db = (*db).clone();
    run_blocking(move || db.count_unread_alerts(project_id))
        .await?
        .map_err(String::from)
}

#[tauri::command]
#[tracing::instrument(skip(db, alert_ids))]
pub async fn mark_alerts_viewed_bulk(
    db: State<'_, Arc<Database>>,
    alert_ids: Vec<i64>,
) -> Result<(), String> {
    let db = (*db).clone();
    run_blocking(move || db.mark_alerts_viewed_bulk(alert_ids, now_ms()))
        .await?
        .map_err(String::from)
}
