//! Statistical anomaly detection over stored signal history.

use serde::{Deserialize, Serialize};

use crate::db::types::{EventSeverity, EventSource, EventType, SiteEvent};
use crate::db::Database;

const BASELINE_WINDOW_DAYS: i64 = 30;
const MIN_BASELINE_SAMPLES: i64 = 7;
const Z_THRESHOLD_WARNING: f32 = 3.0;
const Z_THRESHOLD_CRITICAL: f32 = 5.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AnomalyScore {
    pub z: f32,
    pub current: f64,
    pub mean: f64,
    pub stddev: f64,
}

/// Fetcher function type: given a database handle and project_id, returns the
/// current value of an anomaly signal (or None if unavailable).
type SignalFetcher = fn(&Database, i64) -> Option<f64>;

/// Registered signal fetchers. Signal history may also be populated directly.
pub const ANOMALY_SIGNALS: &[(&str, SignalFetcher)] = &[
    ("traffic.daily_visitors", current_none),
    ("performance.lcp_p75", current_none),
    ("performance.inp_p75", current_none),
    ("seo.indexed_count", current_none),
    ("infrastructure.4xx_rate", current_none),
    ("infrastructure.5xx_rate", current_none),
    ("infrastructure.uptime_pct", current_none),
    ("security.bot_traffic_pct", current_none),
];

fn current_none(_db: &Database, _project_id: i64) -> Option<f64> {
    None
}

pub fn record_signals(db: &Database, _project_id: i64) -> Result<(), String> {
    // No signal fetchers are registered yet; callers populate history directly.
    db.execute(move |_conn| Result::<(), rusqlite::Error>::Ok(()))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

pub fn recompute_baselines(db: &Database, project_id: i64) -> Result<(), String> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let cutoff_ms = now_ms - BASELINE_WINDOW_DAYS * 24 * 60 * 60 * 1000;

    for (signal_key, _) in ANOMALY_SIGNALS {
        let series = db
            .get_signal_history(project_id, signal_key, cutoff_ms)
            .map_err(|error| error.to_string())?;
        if (series.len() as i64) < MIN_BASELINE_SAMPLES {
            continue;
        }
        if series.iter().any(|(_, value)| !value.is_finite()) {
            return Err(format!(
                "non-finite anomaly history value for project {project_id}, signal {signal_key}"
            ));
        }
        let n = series.len() as f64;
        let mean: f64 = series.iter().map(|(_, v)| v).sum::<f64>() / n;
        let var: f64 = series.iter().map(|(_, v)| (v - mean).powi(2)).sum::<f64>() / n;
        let stddev = var.sqrt();
        let sample_count = series.len() as i64;
        db.upsert_signal_baseline(
            project_id,
            signal_key,
            BASELINE_WINDOW_DAYS,
            mean,
            stddev,
            sample_count,
            now_ms,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn detect(
    db: &Database,
    project_id: i64,
    signal_key: &str,
    current: f64,
) -> Result<Option<AnomalyScore>, String> {
    if !current.is_finite() {
        return Err(format!(
            "non-finite current anomaly value for project {project_id}, signal {signal_key}"
        ));
    }
    let result: Option<(f64, f64, i64)> = db
        .get_signal_baseline(project_id, signal_key, BASELINE_WINDOW_DAYS)
        .map_err(|error| error.to_string())?;

    let Some((mean, stddev, count)) = result else {
        return Ok(None);
    };
    if !mean.is_finite() || !stddev.is_finite() || stddev < 0.0 {
        return Err(format!(
            "invalid anomaly baseline for project {project_id}, signal {signal_key}"
        ));
    }
    if count < MIN_BASELINE_SAMPLES || stddev < f64::EPSILON {
        return Ok(None);
    }
    let z = ((current - mean) / stddev) as f32;
    if z.abs() > Z_THRESHOLD_WARNING {
        Ok(Some(AnomalyScore {
            z,
            current,
            mean,
            stddev,
        }))
    } else {
        Ok(None)
    }
}

pub fn signal_to_check_ids(signal_key: &str) -> Vec<&'static str> {
    match signal_key {
        "traffic.daily_visitors" => vec!["analytics.traffic-drop"],
        "performance.lcp_p75" => vec!["performance.lcp"],
        "performance.inp_p75" => vec!["performance.inp"],
        "seo.indexed_count" => vec!["seo.indexing.not-indexed"],
        "infrastructure.4xx_rate" => vec!["infrastructure.client-errors"],
        "infrastructure.5xx_rate" => vec!["infrastructure.server-errors"],
        "infrastructure.uptime_pct" => vec!["infrastructure.uptime"],
        "security.bot_traffic_pct" => vec!["security.bot-traffic"],
        _ => vec![],
    }
}

pub fn run_detection(db: &Database, project_id: i64) -> Result<(), String> {
    for (signal_key, _) in ANOMALY_SIGNALS {
        // Get the most recent history point for this signal.
        let latest: Option<(i64, f64)> = db
            .get_latest_signal_point(project_id, signal_key)
            .map_err(|error| error.to_string())?;
        let Some((ts_ms, current)) = latest else {
            continue;
        };
        if chrono::DateTime::from_timestamp_millis(ts_ms).is_none() {
            return Err(format!(
                "invalid anomaly timestamp {ts_ms} for project {project_id}, signal {signal_key}"
            ));
        }
        let Some(score) = detect(db, project_id, signal_key, current)? else {
            continue;
        };

        let check_ids: Vec<String> = signal_to_check_ids(signal_key)
            .iter()
            .map(|s| s.to_string())
            .collect();
        let severity = if score.z.abs() > Z_THRESHOLD_CRITICAL {
            EventSeverity::Critical
        } else {
            EventSeverity::Warning
        };
        let metadata = Some(
            serde_json::to_string(&score)
                .map_err(|error| format!("serialize anomaly evidence: {error}"))?,
        );

        let event = SiteEvent {
            id: 0,
            project_id,
            event_type: EventType::Anomaly,
            severity,
            occurred_at_ms: ts_ms,
            title: format!(
                "{} anomaly: {:.2}\u{03c3} from baseline",
                signal_key, score.z
            ),
            summary: format!(
                "Current {:.2}, baseline {:.2} +/- {:.2}",
                current, score.mean, score.stddev
            ),
            detail: None,
            source: EventSource::Internal,
            // Use signal_key + ts_ms as dedup key so re-running detection is idempotent.
            source_id: Some(format!("anomaly_{}_{}", signal_key, ts_ms)),
            metadata,
            affected_check_ids: if check_ids.is_empty() {
                None
            } else {
                Some(check_ids)
            },
        };
        db.insert_event(&event).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::temp_db_with_project;
    use rusqlite::params;

    fn seed_history(db: &Database, project_id: i64, signal_key: &str, values: Vec<(i64, f64)>) {
        for (ts_ms, value) in values {
            let sk = signal_key.to_string();
            db.execute(move |conn| {
                conn.execute(
                    "INSERT INTO signal_history (project_id, signal_key, ts_ms, value) VALUES (?, ?, ?, ?)",
                    params![project_id, sk, ts_ms, value],
                )?;
                Result::<(), rusqlite::Error>::Ok(())
            })
            .unwrap()
            .unwrap();
        }
    }

    #[test]
    fn baseline_writes_one_row_per_signal_with_enough_samples() {
        let db = temp_db_with_project();
        let now = chrono::Utc::now().timestamp_millis();
        // 10 samples for one signal, only 3 for another.
        let mut a = Vec::new();
        for i in 0..10 {
            a.push((now - i * 60_000, 100.0 + i as f64));
        }
        seed_history(&db, 1, "traffic.daily_visitors", a);
        let mut b = Vec::new();
        for i in 0..3 {
            b.push((now - i * 60_000, 50.0));
        }
        seed_history(&db, 1, "performance.lcp_p75", b);

        recompute_baselines(&db, 1).unwrap();

        // signal_baselines should have exactly one row (traffic.daily_visitors).
        let count: i64 = db
            .execute(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM signal_baselines WHERE project_id = 1",
                    [],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())
            })
            .unwrap()
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn detect_fires_above_three_sigma() {
        let db = temp_db_with_project();
        // Set a known baseline: mean=100, stddev=10, count=10.
        db.execute(|conn| {
            conn.execute(
                "INSERT INTO signal_baselines (project_id, signal_key, window_days, mean, stddev, sample_count, updated_at)
                 VALUES (1, 'traffic.daily_visitors', 30, 100.0, 10.0, 10, 0)",
                [],
            )?;
            Result::<(), rusqlite::Error>::Ok(())
        })
        .unwrap()
        .unwrap();

        // current = 140 => z = (140 - 100) / 10 = 4 => above threshold.
        let score = detect(&db, 1, "traffic.daily_visitors", 140.0).unwrap();
        assert!(score.is_some());
        assert!((score.unwrap().z - 4.0).abs() < 0.01);
    }

    #[test]
    fn detect_skips_below_threshold() {
        let db = temp_db_with_project();
        db.execute(|conn| {
            conn.execute(
                "INSERT INTO signal_baselines (project_id, signal_key, window_days, mean, stddev, sample_count, updated_at)
                 VALUES (1, 'traffic.daily_visitors', 30, 100.0, 10.0, 10, 0)",
                [],
            )?;
            Result::<(), rusqlite::Error>::Ok(())
        })
        .unwrap()
        .unwrap();
        // current = 110 => z = 1 => below threshold.
        let score = detect(&db, 1, "traffic.daily_visitors", 110.0).unwrap();
        assert!(score.is_none());
    }

    #[test]
    fn detect_skips_insufficient_baseline_samples() {
        let db = temp_db_with_project();
        // Only 5 samples - below the MIN_BASELINE_SAMPLES threshold of 7.
        db.execute(|conn| {
            conn.execute(
                "INSERT INTO signal_baselines (project_id, signal_key, window_days, mean, stddev, sample_count, updated_at)
                 VALUES (1, 'traffic.daily_visitors', 30, 100.0, 10.0, 5, 0)",
                [],
            )?;
            Result::<(), rusqlite::Error>::Ok(())
        })
        .unwrap()
        .unwrap();
        let score = detect(&db, 1, "traffic.daily_visitors", 200.0).unwrap();
        assert!(
            score.is_none(),
            "should return None when sample_count < MIN_BASELINE_SAMPLES"
        );
    }

    #[test]
    fn run_detection_emits_anomaly_event_with_check_ids() {
        let db = temp_db_with_project();
        let project_id = db
            .upsert_project("anomaly-test", "/tmp/anomaly", None)
            .expect("upsert project");
        let now = chrono::Utc::now().timestamp_millis();
        // Seed baseline directly.
        db.execute(move |conn| {
            conn.execute(
                "INSERT INTO signal_baselines (project_id, signal_key, window_days, mean, stddev, sample_count, updated_at)
                 VALUES (?1, 'traffic.daily_visitors', 30, 100.0, 10.0, 10, 0)",
                rusqlite::params![project_id],
            )?;
            Result::<(), rusqlite::Error>::Ok(())
        })
        .unwrap()
        .unwrap();
        // Seed a current value 4 sigma off baseline.
        seed_history(
            &db,
            project_id,
            "traffic.daily_visitors",
            vec![(now, 140.0)],
        );

        run_detection(&db, project_id).unwrap();

        // Verify: one Anomaly event exists.
        let event_count: i64 = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM events WHERE project_id = ?1 AND event_type = 'anomaly'",
                    rusqlite::params![project_id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())
            })
            .unwrap()
            .unwrap();
        assert_eq!(event_count, 1);

        // Verify: junction row for the mapped check_id.
        let junction_count: i64 = db
            .execute(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM site_event_check_ids
                     WHERE check_id = 'analytics.traffic-drop'",
                    [],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())
            })
            .unwrap()
            .unwrap();
        assert_eq!(junction_count, 1);
    }

    #[test]
    fn run_detection_idempotent() {
        let db = temp_db_with_project();
        let project_id = db
            .upsert_project("anomaly-idempotent", "/tmp/anomaly-idem", None)
            .expect("upsert project");
        let now = chrono::Utc::now().timestamp_millis();
        db.execute(move |conn| {
            conn.execute(
                "INSERT INTO signal_baselines (project_id, signal_key, window_days, mean, stddev, sample_count, updated_at)
                 VALUES (?1, 'traffic.daily_visitors', 30, 100.0, 10.0, 10, 0)",
                rusqlite::params![project_id],
            )?;
            Result::<(), rusqlite::Error>::Ok(())
        })
        .unwrap()
        .unwrap();
        seed_history(
            &db,
            project_id,
            "traffic.daily_visitors",
            vec![(now, 140.0)],
        );

        // Running detection twice should still produce exactly one event (INSERT OR IGNORE via source_id dedup).
        run_detection(&db, project_id).unwrap();
        run_detection(&db, project_id).unwrap();

        let event_count: i64 = db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM events WHERE project_id = ?1 AND event_type = 'anomaly'",
                    rusqlite::params![project_id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())
            })
            .unwrap()
            .unwrap();
        assert_eq!(event_count, 1, "second run should be deduped by source_id");
    }

    #[test]
    fn malformed_history_value_is_an_error_not_a_dropped_sample() {
        let db = temp_db_with_project();
        let now = chrono::Utc::now().timestamp_millis();
        db.execute(move |conn| {
            conn.execute(
                "INSERT INTO signal_history (project_id, signal_key, ts_ms, value)
                 VALUES (1, 'traffic.daily_visitors', ?1, 'not-a-number')",
                params![now],
            )?;
            Result::<(), rusqlite::Error>::Ok(())
        })
        .expect("database worker")
        .expect("seed malformed history row");

        let error = recompute_baselines(&db, 1)
            .expect_err("malformed signal evidence must fail baseline recomputation");
        assert!(error.contains("Invalid column type") || error.contains("invalid column type"));
    }

    #[test]
    fn baseline_storage_failure_is_an_error_not_no_baseline() {
        let db = temp_db_with_project();
        db.execute(|conn| conn.execute("DROP TABLE signal_baselines", []).map(|_| ()))
            .expect("database worker")
            .expect("drop signal_baselines");

        let error = detect(&db, 1, "traffic.daily_visitors", 140.0)
            .expect_err("storage failure must not be reported as a missing baseline");
        assert!(error.contains("signal_baselines"));
    }

    #[test]
    fn invalid_latest_timestamp_is_an_error_not_an_anomaly_event() {
        let db = temp_db_with_project();
        seed_history(&db, 1, "traffic.daily_visitors", vec![(i64::MAX, 140.0)]);

        let error = run_detection(&db, 1)
            .expect_err("invalid timestamps must not enter persisted anomaly evidence");
        assert!(error.contains("invalid anomaly timestamp"));
        assert!(error.contains("traffic.daily_visitors"));
    }

    #[test]
    fn signal_to_check_ids_maps_known_signals() {
        assert_eq!(
            signal_to_check_ids("traffic.daily_visitors"),
            vec!["analytics.traffic-drop"]
        );
        assert_eq!(
            signal_to_check_ids("performance.lcp_p75"),
            vec!["performance.lcp"]
        );
        assert!(signal_to_check_ids("unknown.signal").is_empty());
    }
}
