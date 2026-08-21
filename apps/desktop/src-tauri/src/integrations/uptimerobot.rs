//! UptimeRobot API v2 client.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UptimeRobotData {
    pub monitors: Vec<MonitorData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitorData {
    pub friendly_name: String,
    pub url: String,
    pub status: u8, // 0=paused, 1=not checked yet, 2=up, 8=seems down, 9=down
    pub status_text: String,
    pub uptime_ratio: f64,     // last 30 days
    pub average_response: u64, // ms
    pub last_downtime: Option<String>,
    pub response_times: Vec<ResponseTimePoint>,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseTimePoint {
    pub datetime: i64,
    pub value: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub log_type: u8, // 1=down, 2=up, 98=started, 99=paused
    pub type_text: String,
    pub datetime: String,
    pub duration: u64, // seconds
    pub reason_code: Option<String>,
    pub reason_detail: Option<String>,
}

/// Map UptimeRobot's monitor status code to the human-readable label.
#[tracing::instrument(skip(status))]
pub(crate) fn status_to_text(status: u8) -> &'static str {
    match status {
        0 => "Paused",
        1 => "Not checked",
        2 => "Up",
        8 => "Seems down",
        9 => "Down",
        _ => "Unknown",
    }
}

/// Map UptimeRobot's log-entry type code to the human-readable label.
#[tracing::instrument(skip(log_type))]
pub(crate) fn log_type_to_text(log_type: u8) -> &'static str {
    match log_type {
        1 => "Down",
        2 => "Up",
        98 => "Started",
        99 => "Paused",
        _ => "Unknown",
    }
}

/// Parse `custom_uptime_ratio` ("98.5-99.1-99.7-99.9") into the FIRST
/// segment as a float (= 1-day window). Returns 0.0 for missing/unparseable.
#[tracing::instrument(skip(raw))]
pub(crate) fn parse_uptime_ratio(raw: &str) -> f64 {
    raw.split('-')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0)
}

/// Decide whether a monitor's URL matches the user's URL filter. Bidirectional
/// `contains` so either side can be the prefix (UptimeRobot may store the
/// monitor as `https://x.com/` while the user filter is `https://x.com`).
#[tracing::instrument(skip(monitor_url, filter), fields(has_filter = filter.is_some()))]
pub(crate) fn monitor_matches_filter(monitor_url: &str, filter: Option<&str>) -> bool {
    match filter {
        None => true,
        Some(f) => monitor_url.contains(f) || f.contains(monitor_url),
    }
}

/// Format a Unix epoch timestamp as RFC 3339, or empty string if invalid.
#[tracing::instrument(fields(ts))]
pub(crate) fn format_unix_rfc3339(ts: u64) -> String {
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

/// Compute the average response time, preferring the explicit field then
/// falling back to the mean of `response_times[*].value`. Returns 0 when
/// neither source is usable.
#[tracing::instrument(skip(monitor))]
pub(crate) fn compute_avg_response(monitor: &serde_json::Value) -> u64 {
    monitor["average_response_time"]
        .as_u64()
        .or_else(|| {
            let times = monitor["response_times"].as_array()?;
            if times.is_empty() {
                return None;
            }
            let sum: u64 = times.iter().filter_map(|t| t["value"].as_u64()).sum();
            Some(sum / times.len() as u64)
        })
        .unwrap_or(0)
}

/// Parse a single monitor JSON object. Returns None if URL or friendly_name
/// is missing (skip rather than poison the result list).
#[tracing::instrument(skip(monitor, url_filter))]
pub(crate) fn parse_monitor(
    monitor: &serde_json::Value,
    url_filter: Option<&str>,
) -> Option<MonitorData> {
    let monitor_url = monitor["url"].as_str()?.to_string();
    if !monitor_matches_filter(&monitor_url, url_filter) {
        return None;
    }

    let status = monitor["status"].as_u64().unwrap_or(0) as u8;
    let uptime_ratio = parse_uptime_ratio(monitor["custom_uptime_ratio"].as_str().unwrap_or("0"));
    let average_response = compute_avg_response(monitor);

    let response_times: Vec<ResponseTimePoint> = monitor["response_times"]
        .as_array()
        .map(|times| {
            times
                .iter()
                .filter_map(|t| {
                    Some(ResponseTimePoint {
                        datetime: t["datetime"].as_i64()?,
                        value: t["value"].as_u64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let logs: Vec<LogEntry> = monitor["logs"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|l| {
                    let log_type = l["type"].as_u64()? as u8;
                    let datetime = l["datetime"]
                        .as_u64()
                        .map(format_unix_rfc3339)
                        .unwrap_or_default();
                    Some(LogEntry {
                        log_type,
                        type_text: log_type_to_text(log_type).to_string(),
                        datetime,
                        duration: l["duration"].as_u64().unwrap_or(0),
                        reason_code: l["reason"]
                            .as_object()
                            .and_then(|r| r.get("code"))
                            .and_then(|c| c.as_str())
                            .map(String::from),
                        reason_detail: l["reason"]
                            .as_object()
                            .and_then(|r| r.get("detail"))
                            .and_then(|d| d.as_str())
                            .map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let last_downtime = monitor["logs"].as_array().and_then(|logs| {
        logs.iter()
            .find(|l| l["type"].as_u64() == Some(1)) // type 1 = down
            .and_then(|l| l["datetime"].as_u64())
            .map(format_unix_rfc3339)
    });

    Some(MonitorData {
        friendly_name: monitor["friendly_name"].as_str()?.to_string(),
        url: monitor_url,
        status,
        status_text: status_to_text(status).to_string(),
        uptime_ratio,
        average_response,
        last_downtime,
        response_times,
        logs,
    })
}

/// Parse a top-level UptimeRobot getMonitors response. Returns Err when the
/// API reports `stat != "ok"`, otherwise returns the list of monitors that
/// match `url_filter`.
#[tracing::instrument(skip(json, url_filter))]
pub(crate) fn parse_response(
    json: &serde_json::Value,
    url_filter: Option<&str>,
) -> Result<UptimeRobotData, String> {
    if json["stat"].as_str() != Some("ok") {
        return Err(format!(
            "UptimeRobot error: {}",
            json["error"]
                .as_object()
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
        ));
    }
    let empty = vec![];
    let monitors: Vec<MonitorData> = json["monitors"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|m| parse_monitor(m, url_filter))
        .collect();
    Ok(UptimeRobotData { monitors })
}

#[tracing::instrument(skip(api_key, url_filter))]
pub async fn fetch_stats(
    api_key: &str,
    url_filter: Option<&str>,
) -> Result<UptimeRobotData, String> {
    let client = crate::http_client::client();

    let body = serde_json::json!({
        "api_key": api_key,
        "format": "json",
        "custom_uptime_ratios": "1-7-30-90",
        "response_times": 1,
        "response_times_limit": 48,
        "response_times_average": 60,
        "logs": 1,
        "logs_limit": 20,
    });

    let resp = client
        .post("https://api.uptimerobot.com/v2/getMonitors")
        .header("Content-Type", "application/json")
        .json(&body)
        .timeout(crate::constants::API_TIMEOUT_SHORT)
        .send()
        .await
        .map_err(|e| format!("UptimeRobot API error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("UptimeRobot API returned {}", resp.status()));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    parse_response(&json, url_filter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_to_text_covers_all_documented_codes() {
        assert_eq!(status_to_text(0), "Paused");
        assert_eq!(status_to_text(1), "Not checked");
        assert_eq!(status_to_text(2), "Up");
        assert_eq!(status_to_text(8), "Seems down");
        assert_eq!(status_to_text(9), "Down");
    }

    #[test]
    fn status_to_text_falls_back_for_unknown_code() {
        // Defensive: UptimeRobot might add codes we don't know about - show
        // "Unknown" rather than crash.
        assert_eq!(status_to_text(3), "Unknown");
        assert_eq!(status_to_text(255), "Unknown");
    }

    #[test]
    fn log_type_to_text_covers_all_documented_codes() {
        assert_eq!(log_type_to_text(1), "Down");
        assert_eq!(log_type_to_text(2), "Up");
        assert_eq!(log_type_to_text(98), "Started");
        assert_eq!(log_type_to_text(99), "Paused");
        assert_eq!(log_type_to_text(0), "Unknown");
    }

    #[test]
    fn parse_uptime_ratio_takes_first_segment() {
        // The 1-day uptime ratio is the leftmost field; the rest are 7d/30d/90d.
        assert!((parse_uptime_ratio("99.5-99.1-99.7-99.9") - 99.5).abs() < 1e-9);
        assert!((parse_uptime_ratio("100.000") - 100.0).abs() < 1e-9);
        assert!((parse_uptime_ratio("0") - 0.0).abs() < 1e-9);
    }

    #[test]
    fn parse_uptime_ratio_handles_garbage() {
        // Non-numeric / empty string / pure dashes - fall back to 0.0 rather
        // than panic. UptimeRobot occasionally returns "" for fresh monitors.
        assert_eq!(parse_uptime_ratio(""), 0.0);
        assert_eq!(parse_uptime_ratio("-"), 0.0);
        assert_eq!(parse_uptime_ratio("not a number"), 0.0);
    }

    #[test]
    fn monitor_matches_filter_passes_all_when_no_filter() {
        assert!(monitor_matches_filter("https://anything.example.com", None));
    }

    #[test]
    fn monitor_matches_filter_uses_bidirectional_contains() {
        // monitor URL contains filter
        assert!(monitor_matches_filter(
            "https://example.com/page",
            Some("example.com")
        ));
        // filter contains monitor URL (UptimeRobot may store the bare host
        // while the user filter has a path).
        assert!(monitor_matches_filter(
            "example.com",
            Some("https://example.com/full/path")
        ));
    }

    #[test]
    fn monitor_matches_filter_rejects_unrelated_urls() {
        assert!(!monitor_matches_filter(
            "https://other.example.com",
            Some("acme.test")
        ));
        assert!(!monitor_matches_filter(
            "https://x.com",
            Some("https://y.com")
        ));
    }

    #[test]
    fn format_unix_rfc3339_known_timestamp() {
        // 1776506400 = 2026-04-18T10:00:00Z (verified empirically against chrono)
        let formatted = format_unix_rfc3339(1776506400);
        assert!(
            formatted.starts_with("2026-04-18T10:00:00"),
            "expected 2026-04-18T10:00:00 prefix, got {}",
            formatted,
        );
    }

    #[test]
    fn format_unix_rfc3339_zero_is_epoch() {
        assert!(format_unix_rfc3339(0).starts_with("1970-01-01T00:00:00"));
    }

    #[test]
    fn compute_avg_response_prefers_explicit_field() {
        let monitor = serde_json::json!({
            "average_response_time": 250u64,
            "response_times": [{"value": 1000u64}], // ignored
        });
        assert_eq!(compute_avg_response(&monitor), 250);
    }

    #[test]
    fn compute_avg_response_falls_back_to_array_mean() {
        let monitor = serde_json::json!({
            "response_times": [
                {"value": 100u64},
                {"value": 200u64},
                {"value": 300u64},
            ],
        });
        assert_eq!(compute_avg_response(&monitor), 200);
    }

    #[test]
    fn compute_avg_response_returns_zero_when_no_data() {
        assert_eq!(compute_avg_response(&serde_json::json!({})), 0);
        assert_eq!(
            compute_avg_response(&serde_json::json!({"response_times": []})),
            0
        );
    }

    #[test]
    fn parse_monitor_extracts_full_payload() {
        let m = serde_json::json!({
            "friendly_name": "Marketing site",
            "url": "https://example.com",
            "status": 2u64,
            "custom_uptime_ratio": "99.95-99.91-99.88-99.80",
            "average_response_time": 240u64,
            "response_times": [
                {"datetime": 1776506400i64, "value": 200u64},
                {"datetime": 1776510000i64, "value": 280u64},
            ],
            "logs": [
                {
                    "type": 1u64, // Down
                    "datetime": 1776506400u64,
                    "duration": 60u64,
                    "reason": {"code": "5xx", "detail": "503 Service Unavailable"}
                },
                {
                    "type": 2u64, // Up
                    "datetime": 1776506460u64,
                    "duration": 0u64,
                }
            ]
        });
        let parsed = parse_monitor(&m, None).expect("parse");
        assert_eq!(parsed.friendly_name, "Marketing site");
        assert_eq!(parsed.url, "https://example.com");
        assert_eq!(parsed.status, 2);
        assert_eq!(parsed.status_text, "Up");
        assert!((parsed.uptime_ratio - 99.95).abs() < 1e-9);
        assert_eq!(parsed.average_response, 240);
        assert_eq!(parsed.response_times.len(), 2);
        assert_eq!(parsed.response_times[0].value, 200);
        assert_eq!(parsed.logs.len(), 2);
        assert_eq!(parsed.logs[0].type_text, "Down");
        assert_eq!(parsed.logs[0].duration, 60);
        assert_eq!(parsed.logs[0].reason_code.as_deref(), Some("5xx"));
        assert_eq!(
            parsed.logs[0].reason_detail.as_deref(),
            Some("503 Service Unavailable")
        );
        assert!(
            parsed.last_downtime.is_some(),
            "last_downtime must be derived from first Down log"
        );
    }

    #[test]
    fn parse_monitor_skips_when_url_filter_excludes() {
        let m = serde_json::json!({"friendly_name": "x", "url": "https://other.example"});
        assert!(parse_monitor(&m, Some("https://target.example")).is_none());
    }

    #[test]
    fn parse_monitor_includes_when_url_filter_matches() {
        let m = serde_json::json!({"friendly_name": "x", "url": "https://example.com"});
        assert!(parse_monitor(&m, Some("example.com")).is_some());
    }

    #[test]
    fn parse_monitor_skips_when_url_missing() {
        let m = serde_json::json!({"friendly_name": "no url"});
        assert!(parse_monitor(&m, None).is_none());
    }

    #[test]
    fn parse_monitor_handles_minimal_payload() {
        // Minimum viable monitor - friendly_name + url. Everything else
        // defaults so the dashboard still renders the row.
        let m = serde_json::json!({
            "friendly_name": "minimal",
            "url": "https://min.example.com",
        });
        let parsed = parse_monitor(&m, None).expect("parse");
        assert_eq!(parsed.status, 0);
        assert_eq!(parsed.status_text, "Paused");
        assert_eq!(parsed.uptime_ratio, 0.0);
        assert_eq!(parsed.average_response, 0);
        assert!(parsed.response_times.is_empty());
        assert!(parsed.logs.is_empty());
        assert!(parsed.last_downtime.is_none());
    }

    #[test]
    fn parse_monitor_extracts_last_downtime_from_first_down_log() {
        // last_downtime should track the FIRST Down log in the array (logs
        // come back newest-first from UptimeRobot).
        let m = serde_json::json!({
            "friendly_name": "x",
            "url": "https://example.com",
            "logs": [
                {"type": 2u64, "datetime": 1776510000u64}, // Up
                {"type": 1u64, "datetime": 1776506400u64}, // Down (first match)
                {"type": 1u64, "datetime": 1776000000u64}, // Older Down
            ]
        });
        let parsed = parse_monitor(&m, None).expect("parse");
        assert!(parsed.last_downtime.is_some());
        // Must be the 1776506400 timestamp (2026-04-18T10:00:00Z), not the
        // older 1776000000 (2026-04-12T10:40:00Z).
        let down = parsed.last_downtime.unwrap();
        assert!(
            down.starts_with("2026-04-18"),
            "expected 2026-04-18, got {}",
            down
        );
    }

    #[test]
    fn parse_response_returns_monitors_on_ok_status() {
        let body = serde_json::json!({
            "stat": "ok",
            "monitors": [
                {"friendly_name": "a", "url": "https://a.example"},
                {"friendly_name": "b", "url": "https://b.example"},
            ]
        });
        let result = parse_response(&body, None).expect("ok");
        assert_eq!(result.monitors.len(), 2);
    }

    #[test]
    fn parse_response_rejects_when_stat_not_ok() {
        let body = serde_json::json!({
            "stat": "fail",
            "error": {"message": "rate_limit_exceeded"}
        });
        let err = parse_response(&body, None).unwrap_err();
        assert!(err.contains("rate_limit_exceeded"), "got: {}", err);
    }

    #[test]
    fn parse_response_uses_unknown_error_when_message_absent() {
        let body = serde_json::json!({"stat": "fail"});
        let err = parse_response(&body, None).unwrap_err();
        assert!(err.contains("Unknown error"), "got: {}", err);
    }

    #[test]
    fn parse_response_returns_empty_monitors_when_field_missing() {
        let body = serde_json::json!({"stat": "ok"});
        let result = parse_response(&body, None).expect("ok");
        assert!(result.monitors.is_empty());
    }

    #[test]
    fn parse_response_filters_monitors_by_url() {
        let body = serde_json::json!({
            "stat": "ok",
            "monitors": [
                {"friendly_name": "match", "url": "https://target.example.com"},
                {"friendly_name": "skip", "url": "https://other.example.com"},
            ]
        });
        let result = parse_response(&body, Some("target.example.com")).expect("ok");
        assert_eq!(result.monitors.len(), 1);
        assert_eq!(result.monitors[0].friendly_name, "match");
    }
}
