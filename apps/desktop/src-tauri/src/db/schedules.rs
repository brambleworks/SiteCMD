//! Scan schedule CRUD.

use super::DbError;
use rusqlite::{named_params, params};

use super::from_row::FromRow;
use super::helpers::parse_required_enum;
use super::types::ScanSchedule;
use super::Database;
use crate::core::scanner::ScheduledScanType;

impl Database {
    /// Save (upsert) a scan schedule.
    #[tracing::instrument(skip(self), fields(project_id, environment_id, frequency = %frequency, time_of_day = %time_of_day, day_of_week, scan_type = %scan_type, next_run_at = ?next_run_at))]
    pub fn save_scan_schedule(
        &self,
        project_id: i64,
        environment_id: i64,
        frequency: &str,
        time_of_day: &str,
        day_of_week: Option<i32>,
        scan_type: ScheduledScanType,
        next_run_at: Option<String>,
    ) -> Result<(), DbError> {
        let frequency = frequency.to_string();
        let time_of_day = time_of_day.to_string();
        self.execute(move |conn| {
            conn.execute(
                // Supply both timestamps explicitly and preserve created_at on updates.
                "INSERT INTO scan_schedules (project_id, environment_id, frequency, time_of_day, day_of_week, scan_type, next_run_at, created_at, updated_at)
                 VALUES (:project_id, :environment_id, :frequency, :time_of_day, :day_of_week, :scan_type, :next_run_at, datetime('now'), datetime('now'))
                 ON CONFLICT(project_id, environment_id, scan_type) DO UPDATE SET
                   frequency = excluded.frequency,
                   time_of_day = excluded.time_of_day,
                   day_of_week = excluded.day_of_week,
                   next_run_at = excluded.next_run_at,
                   updated_at = datetime('now')",
                named_params! {
                    ":project_id": project_id,
                    ":environment_id": environment_id,
                    ":frequency": frequency,
                    ":time_of_day": time_of_day,
                    ":day_of_week": day_of_week,
                    ":scan_type": scan_type.as_str(),
                    ":next_run_at": next_run_at,
                },
            )
            ?;
            Ok(())
        })?
    }

    /// Get a scan schedule by project, environment, and scan type.
    #[tracing::instrument(skip(self), fields(project_id, environment_id, scan_type = %scan_type))]
    pub fn get_scan_schedule(
        &self,
        project_id: i64,
        environment_id: i64,
        scan_type: ScheduledScanType,
    ) -> Result<Option<ScanSchedule>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, environment_id, frequency, time_of_day,
                            day_of_week, scan_type, last_run_at, next_run_at
                     FROM scan_schedules
                     WHERE project_id = ?1 AND environment_id = ?2 AND scan_type = ?3",
            )?;

            let result = stmt.query_row(
                params![project_id, environment_id, scan_type.as_str()],
                ScanSchedule::from_row,
            );

            match result {
                Ok(s) => Ok(Some(s)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })?
    }

    /// Get all schedules that are due (next_run_at <= now and frequency != 'off').
    /// Returns each schedule paired with its environment URL.
    #[tracing::instrument(skip(self))]
    pub fn get_due_schedules(&self) -> Result<Vec<(ScanSchedule, String)>, DbError> {
        self.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT s.id, s.project_id, s.environment_id, s.frequency, s.time_of_day,
                            s.day_of_week, COALESCE(s.scan_type, 'health') AS scan_type,
                            s.last_run_at, s.next_run_at, e.url AS env_url
                     FROM scan_schedules s
                     JOIN environments e ON e.id = s.environment_id
                     WHERE s.frequency != 'off'
                       AND s.next_run_at IS NOT NULL
                       AND s.next_run_at <= datetime('now', 'localtime')",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok((
                    ScanSchedule {
                        id: row.get("id")?,
                        project_id: row.get("project_id")?,
                        environment_id: row.get("environment_id")?,
                        frequency: row.get("frequency")?,
                        time_of_day: row.get("time_of_day")?,
                        day_of_week: row.get("day_of_week")?,
                        scan_type: parse_required_enum(
                            6,
                            "scan_schedules.scan_type",
                            &row.get::<_, String>("scan_type")?,
                        )?,
                        last_run_at: row.get("last_run_at")?,
                        next_run_at: row.get("next_run_at")?,
                    },
                    row.get::<_, String>("env_url")?,
                ))
            })?;

            let mut result = Vec::new();
            for row in rows {
                result.push(row?);
            }
            Ok(result)
        })?
    }

    /// Mark a schedule as run and set the next run time.
    #[tracing::instrument(skip(self), fields(schedule_id, next_run_at = ?next_run_at))]
    pub fn mark_schedule_run(
        &self,
        schedule_id: i64,
        next_run_at: Option<String>,
    ) -> Result<(), DbError> {
        self.execute(move |conn| {
            conn.execute(
                "UPDATE scan_schedules
                 SET last_run_at = datetime('now', 'localtime'),
                     next_run_at = ?1,
                     updated_at = datetime('now')
                 WHERE id = ?2",
                params![next_run_at, schedule_id],
            )?;
            Ok(())
        })?
    }
}
