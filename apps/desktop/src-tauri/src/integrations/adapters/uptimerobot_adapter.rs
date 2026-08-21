//! Polls UptimeRobot for active outages and correlation enrichments.
//!
//! Outage-start-based IDs keep repeated polls idempotent while separating later outages.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::db::alerts::AlertInput;
use crate::integrations::adapters::{AdapterError, IntegrationAdapter, PollContext, PollOutput};
use crate::integrations::uptimerobot::{LogEntry, MonitorData};

pub struct UptimeRobotAdapter {
    db: Arc<crate::db::Database>,
}

impl UptimeRobotAdapter {
    #[tracing::instrument(skip(db))]
    pub fn new(db: Arc<crate::db::Database>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl IntegrationAdapter for UptimeRobotAdapter {
    fn source(&self) -> &'static str {
        "uptimerobot"
    }

    fn cadence(&self) -> Duration {
        // allow-inline-duration: per-adapter polling cadence.
        Duration::from_secs(60)
    }

    fn is_configured(&self, credentials: &crate::integrations::adapters::Credentials) -> bool {
        credentials.has_api_key()
    }

    async fn poll(&self, ctx: &PollContext) -> Result<PollOutput, AdapterError> {
        let api_key = ctx
            .credentials
            .api_key
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AdapterError::MissingCredentials("uptimerobot".to_string()))?;

        let data = crate::integrations::uptimerobot::fetch_stats(api_key, Some(&ctx.env_url))
            .await
            .map_err(AdapterError::Transport)?;

        write_v3_enrichments(&self.db, ctx.project_id, &data);

        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut alerts: Vec<AlertInput> = Vec::new();

        for monitor in &data.monitors {
            if let Some(alert) =
                build_down_monitor_alert(ctx.project_id, &ctx.env_url, monitor, now_ms)
            {
                alerts.push(alert);
            }
        }

        Ok(PollOutput {
            work_items: vec![],
            alerts,
            partial: false,
            unobserved_signal_prefixes: Vec::new(),
        })
    }
}

fn build_down_monitor_alert(
    project_id: i64,
    fallback_env_url: &str,
    monitor: &MonitorData,
    observed_at: i64,
) -> Option<AlertInput> {
    if monitor.status != 8 && monitor.status != 9 {
        return None;
    }

    let down_log = latest_down_log(&monitor.logs);
    let outage_epoch = down_log
        .map(|log| log.datetime.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let affected_url = if monitor.url.trim().is_empty() {
        fallback_env_url
    } else {
        monitor.url.as_str()
    };
    let monitor_label = if monitor.friendly_name.trim().is_empty() {
        affected_url
    } else {
        monitor.friendly_name.as_str()
    };
    let status_text = monitor.status_text.trim().to_ascii_lowercase();
    let status_text = if status_text.is_empty() {
        "down".to_string()
    } else {
        status_text
    };
    let duration_desc = down_log.map(|log| format_duration(log.duration));
    let description = match duration_desc {
        Some(duration) => format!(
            "UptimeRobot reports {monitor_label} ({affected_url}) is {status_text}. The latest down log has lasted {duration}. Check hosting, DNS, deploy status, and monitor error details before marking it resolved."
        ),
        None => format!(
            "UptimeRobot reports {monitor_label} ({affected_url}) is {status_text}. Check hosting, DNS, deploy status, and monitor error details before marking it resolved."
        ),
    };
    let occurred_at = down_log
        .and_then(|log| {
            chrono::DateTime::parse_from_rfc3339(&log.datetime)
                .ok()
                .map(|dt| dt.timestamp_millis())
        })
        .unwrap_or(observed_at);

    Some(AlertInput {
        project_id,
        env_url: Some(affected_url.to_string()),
        source: "uptimerobot".to_string(),
        alert_id: format!("outage:{affected_url}:{outage_epoch}"),
        severity: "critical".to_string(),
        title: format!("Monitor down: {monitor_label}"),
        description,
        detail_json: serde_json::to_string(&serde_json::json!({
            "alert_type": "uptime_monitor_down",
            "friendly_name": monitor.friendly_name,
            "url": affected_url,
            "status": monitor.status,
            "status_text": monitor.status_text,
            "uptime_ratio": monitor.uptime_ratio,
            "average_response_ms": monitor.average_response,
            "last_downtime": monitor.last_downtime,
            "destination": "integrations"
        }))
        .ok(),
        occurred_at,
        observed_at,
    })
}

fn latest_down_log(logs: &[LogEntry]) -> Option<&LogEntry> {
    logs.iter().find(|log| log.log_type == 1)
}

fn format_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

/// Write UptimeRobot enrichment cache rows from a freshly-fetched data snapshot.
/// Writes recent downtime and TTFB history. The parsed monitor response does not
/// expose certificate expiry data.
pub fn write_v3_enrichments(
    db: &crate::db::Database,
    project_id: i64,
    data: &crate::integrations::uptimerobot::UptimeRobotData,
) {
    use crate::core::correlation::enrichments::write_cache_payload;

    // Select the latest completed downtime; ISO timestamps sort chronologically.
    let recent_downtime_window: Option<(String, String)> = data
        .monitors
        .iter()
        .flat_map(|m| m.logs.iter())
        .filter(|log| log.log_type == 1 && log.duration > 0)
        .max_by(|a, b| a.datetime.cmp(&b.datetime))
        .map(|log| {
            let start = log.datetime.clone();
            let end = chrono::DateTime::parse_from_rfc3339(&log.datetime)
                .map(|dt| (dt + chrono::Duration::seconds(log.duration as i64)).to_rfc3339())
                .unwrap_or_else(|_| start.clone());
            (start, end)
        });

    if let Some((window_start, window_end)) = recent_downtime_window {
        let payload = serde_json::json!({
            "window_start": window_start,
            "window_end": window_end
        });
        if let Err(e) = write_cache_payload(
            db,
            project_id,
            "uptimerobot",
            "recent_downtime",
            &payload.to_string(),
        ) {
            tracing::warn!("uptimerobot: failed to write recent_downtime cache: {}", e);
        }
    }

    // Aggregate the API's bounded response-time window into p75 TTFB.
    let mut ttfb_samples: Vec<u64> = data
        .monitors
        .iter()
        .flat_map(|m| m.response_times.iter().map(|rt| rt.value))
        .collect();
    if !ttfb_samples.is_empty() {
        ttfb_samples.sort_unstable();
        let idx = ((ttfb_samples.len() as f64 * 0.75) as usize).min(ttfb_samples.len() - 1);
        let p75 = ttfb_samples[idx] as u32;
        let payload = serde_json::json!({ "p75_ms": p75, "days": 30 });
        if let Err(e) = write_cache_payload(
            db,
            project_id,
            "uptimerobot",
            "ttfb_history",
            &payload.to_string(),
        ) {
            tracing::warn!("uptimerobot: failed to write ttfb_history cache: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::{temp_db, temp_db_arc, temp_db_with_project};
    use crate::integrations::adapters::Credentials;
    use crate::integrations::uptimerobot::{ResponseTimePoint, UptimeRobotData};

    fn test_adapter() -> UptimeRobotAdapter {
        let db = temp_db_arc();
        UptimeRobotAdapter::new(db.db.clone())
    }

    #[tokio::test]
    async fn returns_missing_credentials_when_creds_empty() {
        let adapter = test_adapter();
        let ctx = PollContext {
            project_id: 1,
            env_url: "https://example.com".into(),
            detected_stack: None,
            credentials: Credentials::empty(),
        };
        let err = adapter.poll(&ctx).await.unwrap_err();
        assert!(matches!(err, AdapterError::MissingCredentials(_)));
    }

    #[tokio::test]
    async fn returns_missing_credentials_when_api_key_empty_string() {
        let adapter = test_adapter();
        let ctx = PollContext {
            project_id: 1,
            env_url: "https://example.com".into(),
            detected_stack: None,
            credentials: Credentials {
                api_key: Some("".to_string()),
                oauth_token: None,
                site_id: None,
                github: None,
                github_unobservable: false,
            },
        };
        let err = adapter.poll(&ctx).await.unwrap_err();
        assert!(matches!(err, AdapterError::MissingCredentials(_)));
    }

    #[test]
    fn cadence_is_60_seconds() {
        assert_eq!(test_adapter().cadence(), Duration::from_secs(60));
    }

    #[test]
    fn source_is_uptimerobot() {
        assert_eq!(test_adapter().source(), "uptimerobot");
    }

    #[test]
    fn is_configured_requires_api_key() {
        let adapter = test_adapter();
        assert!(!adapter.is_configured(&Credentials::empty()));
        assert!(adapter.is_configured(&Credentials {
            api_key: Some("test-key".into()),
            oauth_token: None,
            site_id: None,
            github: None,
            github_unobservable: false,
        }));
    }

    #[test]
    fn down_monitor_alert_uses_monitor_url_and_actionable_metadata() {
        let monitor = MonitorData {
            friendly_name: "Production".into(),
            url: "https://example.com".into(),
            status: 9,
            status_text: "Down".into(),
            uptime_ratio: 99.5,
            average_response: 240,
            last_downtime: Some("2026-05-16T10:00:00+00:00".into()),
            response_times: Vec::new(),
            logs: vec![LogEntry {
                log_type: 1,
                type_text: "Down".into(),
                datetime: "2026-05-16T10:00:00+00:00".into(),
                duration: 3665,
                reason_code: None,
                reason_detail: None,
            }],
        };

        let alert = build_down_monitor_alert(7, "https://staging.example.com", &monitor, 1_000)
            .expect("down monitor alert");

        assert_eq!(alert.env_url.as_deref(), Some("https://example.com"));
        assert!(alert.description.contains("1h 1m"));
        assert!(alert.description.contains("hosting, DNS, deploy status"));
        assert!(alert.detail_json.unwrap().contains("uptime_monitor_down"));
    }

    #[test]
    fn write_v3_enrichments_populates_recent_downtime_and_ttfb() {
        use crate::core::correlation::enrichments::{uptime_recent_downtime, uptime_ttfb_history};

        let db = temp_db_with_project();
        let data = UptimeRobotData {
            monitors: vec![MonitorData {
                friendly_name: "Test".into(),
                url: "https://example.com".into(),
                status: 2,
                status_text: "Up".into(),
                uptime_ratio: 99.9,
                average_response: 200,
                last_downtime: None,
                response_times: vec![
                    ResponseTimePoint {
                        datetime: 1776506400,
                        value: 150,
                    },
                    ResponseTimePoint {
                        datetime: 1776510000,
                        value: 200,
                    },
                    ResponseTimePoint {
                        datetime: 1776513600,
                        value: 180,
                    },
                    ResponseTimePoint {
                        datetime: 1776517200,
                        value: 220,
                    },
                ],
                logs: vec![LogEntry {
                    log_type: 1,
                    type_text: "Down".into(),
                    datetime: "2026-04-18T10:00:00+00:00".into(),
                    duration: 120,
                    reason_code: None,
                    reason_detail: None,
                }],
            }],
        };

        write_v3_enrichments(&db, 1, &data);

        let cache = crate::core::correlation::enrichments::EnrichmentCache::load(&db, 1)
            .expect("load cache");
        let downtime = uptime_recent_downtime("infrastructure.uptime", &cache).expect("no error");
        assert!(
            downtime.is_some(),
            "recent_downtime enrichment should be present"
        );

        let ttfb = uptime_ttfb_history("performance.ttfb", &cache).expect("no error");
        assert!(ttfb.is_some(), "ttfb_history enrichment should be present");
    }

    #[test]
    fn write_v3_enrichments_skips_downtime_when_no_down_logs() {
        use crate::core::correlation::enrichments::{uptime_recent_downtime, EnrichmentCache};

        let db = temp_db();
        let data = UptimeRobotData {
            monitors: vec![MonitorData {
                friendly_name: "Healthy".into(),
                url: "https://example.com".into(),
                status: 2,
                status_text: "Up".into(),
                uptime_ratio: 100.0,
                average_response: 100,
                last_downtime: None,
                response_times: vec![ResponseTimePoint {
                    datetime: 0,
                    value: 100,
                }],
                logs: vec![],
            }],
        };

        write_v3_enrichments(&db, 1, &data);

        let cache = EnrichmentCache::load(&db, 1).expect("load cache");
        let downtime = uptime_recent_downtime("infrastructure.uptime", &cache).expect("no error");
        assert!(
            downtime.is_none(),
            "no down logs means no downtime enrichment"
        );
    }

    #[test]
    fn write_v3_enrichments_computes_p75_correctly() {
        use crate::core::correlation::enrichments::{uptime_ttfb_history, EnrichmentCache};
        use crate::core::types_work_items::Enrichment;

        let db = temp_db_with_project();
        // 4 samples sorted: [100, 150, 200, 300]. p75 index = floor(4*0.75)=3 -> value 300.
        let data = UptimeRobotData {
            monitors: vec![MonitorData {
                friendly_name: "Test".into(),
                url: "https://example.com".into(),
                status: 2,
                status_text: "Up".into(),
                uptime_ratio: 100.0,
                average_response: 0,
                last_downtime: None,
                response_times: vec![
                    ResponseTimePoint {
                        datetime: 1,
                        value: 200,
                    },
                    ResponseTimePoint {
                        datetime: 2,
                        value: 100,
                    },
                    ResponseTimePoint {
                        datetime: 3,
                        value: 300,
                    },
                    ResponseTimePoint {
                        datetime: 4,
                        value: 150,
                    },
                ],
                logs: vec![],
            }],
        };

        write_v3_enrichments(&db, 1, &data);

        let cache = EnrichmentCache::load(&db, 1).expect("load cache");
        let ttfb = uptime_ttfb_history("performance.ttfb", &cache)
            .expect("no error")
            .expect("should exist");
        if let Enrichment::TtfbHistory { p75_ms, days, .. } = ttfb {
            assert_eq!(p75_ms, 300, "p75 of [100,150,200,300] is 300");
            assert_eq!(days, 30);
        } else {
            panic!("unexpected enrichment variant");
        }
    }
}
