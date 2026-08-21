use crate::core::scanner::ScheduledScanType;
use crate::db::{Database, ScanSchedule};
use std::sync::Arc;
use tauri::State;

/// Compute next run timestamp from frequency, time_of_day, and day_of_week.
fn compute_next_run(
    frequency: &str,
    time_of_day: &str,
    day_of_week: Option<i32>,
) -> Option<String> {
    compute_next_run_from(
        chrono::Local::now().naive_local(),
        frequency,
        time_of_day,
        day_of_week,
    )
}

/// Compute the next local fire time with an injected clock.
/// The output format must match SQLite `datetime('now', 'localtime')` for
/// lexicographic comparison.
fn compute_next_run_from(
    now: chrono::NaiveDateTime,
    frequency: &str,
    time_of_day: &str,
    day_of_week: Option<i32>,
) -> Option<String> {
    use chrono::{Datelike, Duration, NaiveTime};

    let time = NaiveTime::parse_from_str(time_of_day, "%H:%M").ok()?;
    let today = now.date();

    match frequency {
        "daily" => {
            let candidate = today.and_time(time);
            let next = if candidate <= now {
                candidate + Duration::days(1)
            } else {
                candidate
            };
            Some(next.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        "weekly" => {
            let target_dow = day_of_week.unwrap_or(1) as u32; // default Monday
            let current_dow = today.weekday().num_days_from_sunday();
            let days_ahead = if target_dow > current_dow {
                target_dow - current_dow
            } else if target_dow < current_dow {
                7 - (current_dow - target_dow)
            } else {
                let candidate = today.and_time(time);
                if candidate <= now {
                    7
                } else {
                    0
                }
            };
            let next_date = today + Duration::days(days_ahead as i64);
            let next = next_date.and_time(time);
            Some(next.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        _ => None, // "off" or unknown
    }
}

#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id, environment_id, frequency = %frequency, time_of_day = %time_of_day, day_of_week, scan_type = ?scan_type))]
pub async fn save_scan_schedule(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_id: i64,
    frequency: String,
    time_of_day: String,
    day_of_week: Option<i32>,
    scan_type: Option<ScheduledScanType>,
) -> Result<ScanSchedule, String> {
    let scan_type = scan_type.unwrap_or_default();
    let next_run = compute_next_run(&frequency, &time_of_day, day_of_week);

    {
        let db = (*db).clone();
        let frequency = frequency.clone();
        let time_of_day = time_of_day.clone();
        let next_run = next_run.clone();
        crate::commands::run_blocking(move || {
            db.save_scan_schedule(
                project_id,
                environment_id,
                &frequency,
                &time_of_day,
                day_of_week,
                scan_type,
                next_run,
            )
        })
        .await??;
    }

    Ok(ScanSchedule {
        id: None,
        project_id,
        environment_id,
        frequency,
        time_of_day,
        day_of_week,
        scan_type,
        last_run_at: None,
        next_run_at: next_run,
    })
}

#[tauri::command]
#[tracing::instrument(skip(db), fields(project_id, environment_id, scan_type = ?scan_type))]
pub async fn get_scan_schedule(
    db: State<'_, Arc<Database>>,
    project_id: i64,
    environment_id: i64,
    scan_type: Option<ScheduledScanType>,
) -> Result<Option<ScanSchedule>, String> {
    let scan_type = scan_type.unwrap_or_default();
    let db = (*db).clone();
    crate::commands::run_blocking(move || {
        db.get_scan_schedule(project_id, environment_id, scan_type)
    })
    .await?
    .map_err(String::from)
}

/// Get all schedules that are due (next_run_at <= now and frequency != 'off')
#[tracing::instrument(skip(db))]
pub fn get_due_schedules(db: &Database) -> Result<Vec<(ScanSchedule, String)>, String> {
    db.get_due_schedules().map_err(String::from)
}

#[tracing::instrument(skip(app, url), fields(strategy = %strategy))]
pub async fn get_pagespeed_report(
    app: tauri::AppHandle,
    url: String,
    strategy: String,
) -> Result<crate::integrations::pagespeed::PageSpeedReport, String> {
    let api_key = crate::keyring::get_pagespeed_api_key(&app).ok().flatten();
    crate::integrations::pagespeed::fetch_pagespeed_report(&url, &strategy, api_key.as_deref())
        .await
}

/// Store or clear the optional Google PageSpeed Insights API key (OS keychain).
/// An empty string clears it.
#[tracing::instrument(skip(app, key))]
pub async fn set_pagespeed_api_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        crate::keyring::delete_pagespeed_api_key(&app)
    } else {
        crate::keyring::store_pagespeed_api_key(&app, trimmed)
    }
}

/// Whether a PageSpeed Insights API key is currently stored.
#[tracing::instrument(skip(app))]
pub async fn pagespeed_api_key_is_set(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(crate::keyring::get_pagespeed_api_key(&app)?.is_some())
}

/// Mark a schedule as run and compute the next run time
#[tracing::instrument(skip(db), fields(schedule_id, frequency = %frequency, time_of_day = %time_of_day, day_of_week))]
pub fn mark_schedule_run(
    db: &Database,
    schedule_id: i64,
    frequency: &str,
    time_of_day: &str,
    day_of_week: Option<i32>,
) -> Result<(), String> {
    let next_run = compute_next_run(frequency, time_of_day, day_of_week);
    db.mark_schedule_run(schedule_id, next_run)
        .map_err(String::from)
}

#[cfg(test)]
mod tests {
    use super::compute_next_run_from;
    use chrono::NaiveDateTime;

    // 2026-01-05 is a Monday (num_days_from_sunday == 1).
    fn monday_at(hm: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(&format!("2026-01-05 {hm}:00"), "%Y-%m-%d %H:%M:%S")
            .expect("fixture datetime")
    }

    #[test]
    fn daily_schedules_fire_today_when_the_time_is_still_ahead() {
        let next = compute_next_run_from(monday_at("10:00"), "daily", "11:30", None);
        assert_eq!(next.as_deref(), Some("2026-01-05 11:30:00"));
    }

    #[test]
    fn daily_schedules_roll_to_tomorrow_once_the_time_has_passed() {
        let next = compute_next_run_from(monday_at("10:00"), "daily", "09:00", None);
        assert_eq!(next.as_deref(), Some("2026-01-06 09:00:00"));
        // Exactly-now counts as passed: never schedule a run in the past.
        let at_now = compute_next_run_from(monday_at("10:00"), "daily", "10:00", None);
        assert_eq!(at_now.as_deref(), Some("2026-01-06 10:00:00"));
    }

    #[test]
    fn weekly_schedules_land_on_the_requested_weekday() {
        // Friday (5) from Monday: four days ahead.
        let friday = compute_next_run_from(monday_at("10:00"), "weekly", "09:00", Some(5));
        assert_eq!(friday.as_deref(), Some("2026-01-09 09:00:00"));
        // Sunday (0) is behind Monday in the from-Sunday numbering: wraps
        // to next week's Sunday, never scheduling into the past.
        let sunday = compute_next_run_from(monday_at("10:00"), "weekly", "09:00", Some(0));
        assert_eq!(sunday.as_deref(), Some("2026-01-11 09:00:00"));
    }

    #[test]
    fn weekly_same_day_uses_the_time_to_pick_this_week_or_next() {
        let later_today = compute_next_run_from(monday_at("10:00"), "weekly", "18:00", Some(1));
        assert_eq!(later_today.as_deref(), Some("2026-01-05 18:00:00"));
        let already_passed = compute_next_run_from(monday_at("10:00"), "weekly", "09:00", Some(1));
        assert_eq!(already_passed.as_deref(), Some("2026-01-12 09:00:00"));
    }

    #[test]
    fn weekly_defaults_to_monday_when_no_day_is_stored() {
        let next = compute_next_run_from(monday_at("10:00"), "weekly", "18:00", None);
        assert_eq!(next.as_deref(), Some("2026-01-05 18:00:00"));
    }

    #[test]
    fn off_unknown_frequency_and_bad_time_produce_no_next_run() {
        assert_eq!(
            compute_next_run_from(monday_at("10:00"), "off", "09:00", None),
            None
        );
        assert_eq!(
            compute_next_run_from(monday_at("10:00"), "hourly", "09:00", None),
            None
        );
        assert_eq!(
            compute_next_run_from(monday_at("10:00"), "daily", "9am", None),
            None
        );
    }

    /// get_due_schedules compares next_run_at lexicographically against
    /// SQLite's `datetime('now', 'localtime')`, which formats as
    /// "YYYY-MM-DD HH:MM:SS". A drift to RFC 3339's "T" separator would
    /// corrupt same-day comparisons, so the exact shape is pinned here.
    #[test]
    fn next_run_format_matches_sqlite_datetime_for_lexicographic_compare() {
        let next = compute_next_run_from(monday_at("10:00"), "daily", "11:30", None)
            .expect("daily schedule");
        assert_eq!(next.len(), 19);
        assert_eq!(&next[10..11], " ", "space separator, never RFC 3339's T");
        assert!(next
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == ':' || c == ' '));
    }
}
