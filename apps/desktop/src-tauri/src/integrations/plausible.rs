//! Plausible Stats API v2 client.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PlausibleData {
    pub visitors: u64,
    pub pageviews: u64,
    pub bounce_rate: f64,
    pub visit_duration: f64,
    pub top_pages: Vec<TopPage>,
    pub top_sources: Vec<TopSource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TopPage {
    pub page: String,
    pub visitors: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TopSource {
    pub source: String,
    pub visitors: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlausibleTimeseries {
    pub period: String,
    pub points: Vec<TimeseriesPoint>,
    pub aggregate: PlausibleData,
    pub top_pages: Vec<TopPage>,
    pub top_sources: Vec<TopSource>,
    pub countries: Vec<CountryData>,
    pub devices: Vec<DeviceData>,
    pub browsers: Vec<BrowserData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeseriesPoint {
    pub date: String,
    pub visitors: u64,
    pub pageviews: u64,
    pub bounce_rate: f64,
    pub visit_duration: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CountryData {
    pub country: String,
    pub visitors: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceData {
    pub device: String,
    pub visitors: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserData {
    pub browser: String,
    pub visitors: u64,
}

const QUERY_URL: &str = "https://plausible.io/api/v2/query";

// The API returns metric values in this positional order.
const CORE_METRICS: [&str; 4] = ["visitors", "pageviews", "bounce_rate", "visit_duration"];

/// POST a Stats API v2 query and return the parsed JSON body.
async fn query(
    client: &reqwest::Client,
    api_key: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let resp = client
        .post(QUERY_URL)
        .bearer_auth(api_key)
        .json(&body)
        .timeout(crate::constants::API_TIMEOUT_SHORT)
        .send()
        .await
        .map_err(|e| format!("Plausible API error: {}", e))?;

    if !resp.status().is_success() {
        // Surface Plausible's error body (e.g. "site could not be found",
        // plan/feature gating) instead of a bare status - the status alone
        // can't tell a wrong site_id from an access problem.
        let status = resp.status();
        let body = crate::http_client::read_text_limited(
            resp,
            crate::constants::INTEGRATION_ERROR_BODY_MAX_BYTES,
            crate::constants::BODY_READ_TIMEOUT,
        )
        .await
        .unwrap_or_default();
        return Err(format!(
            "Plausible API returned {}: {}",
            status,
            body.trim()
        ));
    }

    crate::http_client::read_json_limited(
        resp,
        crate::constants::PLAUSIBLE_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await
    .map_err(|e| format!("Plausible parse error: {}", e))
}

// API v2 aligns dimension and metric arrays with their request order.

fn metric_u64(row: &serde_json::Value, idx: usize) -> u64 {
    row["metrics"][idx].as_u64().unwrap_or(0)
}

fn metric_f64(row: &serde_json::Value, idx: usize) -> f64 {
    row["metrics"][idx].as_f64().unwrap_or(0.0)
}

fn dimension_str(row: &serde_json::Value, idx: usize) -> Option<String> {
    row["dimensions"][idx].as_str().map(|s| s.to_string())
}

struct AggResult {
    visitors: u64,
    pageviews: u64,
    bounce_rate: f64,
    visit_duration: f64,
}

/// Parse an aggregate response (no dimensions -> a single totals row).
fn parse_aggregate(json: &serde_json::Value) -> AggResult {
    let row = &json["results"][0];
    AggResult {
        visitors: metric_u64(row, 0),
        pageviews: metric_u64(row, 1),
        bounce_rate: metric_f64(row, 2),
        visit_duration: metric_f64(row, 3),
    }
}

/// Parse a `time:day`-dimensioned response into daily points.
fn parse_timeseries(json: &serde_json::Value) -> Vec<TimeseriesPoint> {
    json["results"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    Some(TimeseriesPoint {
                        date: dimension_str(row, 0)?,
                        visitors: metric_u64(row, 0),
                        pageviews: metric_u64(row, 1),
                        bounce_rate: metric_f64(row, 2),
                        visit_duration: metric_f64(row, 3),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a single-dimension, single-metric (visitors) breakdown. `build` turns
/// the dimension label + visitor count into the caller's row type.
fn parse_breakdown<T>(json: &serde_json::Value, build: impl Fn(String, u64) -> T) -> Vec<T> {
    json["results"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| Some(build(dimension_str(row, 0)?, metric_u64(row, 0))))
                .collect()
        })
        .unwrap_or_default()
}

/// Build a single-dimension visitors breakdown query body.
fn breakdown_query(
    site_id: &str,
    date_range: serde_json::Value,
    dimension: &str,
    limit: u32,
) -> serde_json::Value {
    serde_json::json!({
        "site_id": site_id,
        "metrics": ["visitors"],
        "date_range": date_range,
        "dimensions": [dimension],
        "order_by": [["visitors", "desc"]],
        "pagination": { "limit": limit },
    })
}

// Public API

#[tracing::instrument(skip(api_key), fields(site_id = %site_id))]
pub async fn fetch_stats(api_key: &str, site_id: &str) -> Result<PlausibleData, String> {
    let client = crate::http_client::client();
    let (agg, pages, sources) = tokio::join!(
        fetch_aggregate(client, api_key, site_id, "30d".into()),
        fetch_breakdown(client, api_key, site_id, "30d".into(), "event:page", 5),
        fetch_breakdown_sources(client, api_key, site_id, "30d".into(), 5),
    );
    let agg = agg?;

    Ok(PlausibleData {
        visitors: agg.visitors,
        pageviews: agg.pageviews,
        bounce_rate: agg.bounce_rate,
        visit_duration: agg.visit_duration,
        top_pages: pages,
        top_sources: sources,
    })
}

/// Full analytics data with timeseries, breakdowns, geo, devices.
#[tracing::instrument(skip(api_key), fields(site_id = %site_id, period = %period))]
pub async fn fetch_analytics(
    api_key: &str,
    site_id: &str,
    period: &str,
) -> Result<PlausibleTimeseries, String> {
    let client = crate::http_client::client();
    let range = date_range(period);

    let (agg, timeseries, pages, sources, countries, devices, browsers) = tokio::join!(
        fetch_aggregate(client, api_key, site_id, range.clone()),
        fetch_timeseries(client, api_key, site_id, range.clone()),
        fetch_breakdown(client, api_key, site_id, range.clone(), "event:page", 10),
        fetch_breakdown_sources(client, api_key, site_id, range.clone(), 10),
        fetch_breakdown_countries(client, api_key, site_id, range.clone()),
        fetch_breakdown_devices(client, api_key, site_id, range.clone()),
        fetch_breakdown_browsers(client, api_key, site_id, range.clone()),
    );

    let agg = agg?;
    let timeseries = timeseries?;

    Ok(PlausibleTimeseries {
        period: period.to_string(),
        points: timeseries,
        aggregate: PlausibleData {
            visitors: agg.visitors,
            pageviews: agg.pageviews,
            bounce_rate: agg.bounce_rate,
            visit_duration: agg.visit_duration,
            top_pages: vec![],
            top_sources: vec![],
        },
        top_pages: pages,
        top_sources: sources,
        countries,
        devices,
        browsers,
    })
}

/// Fetch top pages for a UTC window ending `end_date_offset_days` before today.
pub async fn fetch_top_pages_for_window(
    api_key: &str,
    site_id: &str,
    period_days: u32,
    end_date_offset_days: u32,
) -> Result<Vec<TopPage>, String> {
    let client = crate::http_client::client();
    let end = chrono::Utc::now() - chrono::Duration::days(end_date_offset_days as i64);
    let start = end - chrono::Duration::days(period_days.saturating_sub(1) as i64);
    // v2 takes a custom window as a [start, end] ISO-date array.
    let range = serde_json::json!([
        start.format("%Y-%m-%d").to_string(),
        end.format("%Y-%m-%d").to_string(),
    ]);
    let body = breakdown_query(site_id, range, "event:page", 50);
    let json = query(client, api_key, body).await?;
    Ok(parse_breakdown(&json, |page, visitors| TopPage {
        page,
        visitors,
    }))
}

// Internal helpers

/// Map a UI period to a v2 `date_range`, defaulting to 30 days.
fn date_range(period: &str) -> serde_json::Value {
    const VALID: [&str; 11] = [
        "day", "24h", "7d", "28d", "30d", "91d", "month", "6mo", "12mo", "year", "all",
    ];
    let normalized = if VALID.contains(&period) {
        period
    } else {
        "30d"
    };
    serde_json::Value::String(normalized.to_string())
}

async fn fetch_aggregate(
    client: &reqwest::Client,
    api_key: &str,
    site_id: &str,
    date_range: serde_json::Value,
) -> Result<AggResult, String> {
    let body = serde_json::json!({
        "site_id": site_id,
        "metrics": CORE_METRICS,
        "date_range": date_range,
    });
    let json = query(client, api_key, body).await?;
    Ok(parse_aggregate(&json))
}

async fn fetch_timeseries(
    client: &reqwest::Client,
    api_key: &str,
    site_id: &str,
    date_range: serde_json::Value,
) -> Result<Vec<TimeseriesPoint>, String> {
    let body = serde_json::json!({
        "site_id": site_id,
        "metrics": CORE_METRICS,
        "date_range": date_range,
        "dimensions": ["time:day"],
    });
    let json = query(client, api_key, body).await?;
    Ok(parse_timeseries(&json))
}

async fn fetch_breakdown(
    client: &reqwest::Client,
    api_key: &str,
    site_id: &str,
    date_range: serde_json::Value,
    dimension: &str,
    limit: u32,
) -> Vec<TopPage> {
    let body = breakdown_query(site_id, date_range, dimension, limit);
    match query(client, api_key, body).await {
        Ok(json) => parse_breakdown(&json, |page, visitors| TopPage { page, visitors }),
        Err(_) => vec![],
    }
}

async fn fetch_breakdown_sources(
    client: &reqwest::Client,
    api_key: &str,
    site_id: &str,
    date_range: serde_json::Value,
    limit: u32,
) -> Vec<TopSource> {
    let body = breakdown_query(site_id, date_range, "visit:source", limit);
    match query(client, api_key, body).await {
        Ok(json) => parse_breakdown(&json, |source, visitors| TopSource { source, visitors }),
        Err(_) => vec![],
    }
}

async fn fetch_breakdown_countries(
    client: &reqwest::Client,
    api_key: &str,
    site_id: &str,
    date_range: serde_json::Value,
) -> Vec<CountryData> {
    let body = breakdown_query(site_id, date_range, "visit:country", 10);
    match query(client, api_key, body).await {
        Ok(json) => parse_breakdown(&json, |country, visitors| CountryData { country, visitors }),
        Err(_) => vec![],
    }
}

async fn fetch_breakdown_devices(
    client: &reqwest::Client,
    api_key: &str,
    site_id: &str,
    date_range: serde_json::Value,
) -> Vec<DeviceData> {
    let body = breakdown_query(site_id, date_range, "visit:device", 10);
    match query(client, api_key, body).await {
        Ok(json) => parse_breakdown(&json, |device, visitors| DeviceData { device, visitors }),
        Err(_) => vec![],
    }
}

async fn fetch_breakdown_browsers(
    client: &reqwest::Client,
    api_key: &str,
    site_id: &str,
    date_range: serde_json::Value,
) -> Vec<BrowserData> {
    let body = breakdown_query(site_id, date_range, "visit:browser", 8);
    match query(client, api_key, body).await {
        Ok(json) => parse_breakdown(&json, |browser, visitors| BrowserData { browser, visitors }),
        Err(_) => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aggregate_metrics_positionally() {
        let json = serde_json::json!({
            "results": [{ "dimensions": [], "metrics": [1234, 5678, 45.2, 92.3] }],
            "meta": {}
        });
        let agg = parse_aggregate(&json);
        assert_eq!(agg.visitors, 1234);
        assert_eq!(agg.pageviews, 5678);
        assert!((agg.bounce_rate - 45.2).abs() < f64::EPSILON);
        assert!((agg.visit_duration - 92.3).abs() < f64::EPSILON);
    }

    #[test]
    fn aggregate_with_no_data_is_all_zero_not_a_panic() {
        let agg = parse_aggregate(&serde_json::json!({ "results": [] }));
        assert_eq!(agg.visitors, 0);
        assert_eq!(agg.pageviews, 0);
        assert_eq!(agg.bounce_rate, 0.0);
    }

    #[test]
    fn parses_timeseries_rows_by_day() {
        let json = serde_json::json!({
            "results": [
                { "dimensions": ["2026-06-01"], "metrics": [10, 20, 30.0, 40.0] },
                { "dimensions": ["2026-06-02"], "metrics": [11, 22, 33.0, 44.0] }
            ]
        });
        let points = parse_timeseries(&json);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].date, "2026-06-01");
        assert_eq!(points[0].visitors, 10);
        assert_eq!(points[0].pageviews, 20);
        assert_eq!(points[1].date, "2026-06-02");
        assert_eq!(points[1].visitors, 11);
    }

    #[test]
    fn parses_breakdown_dimension_label_and_visitors() {
        let json = serde_json::json!({
            "results": [
                { "dimensions": ["/"], "metrics": [500] },
                { "dimensions": ["/pricing"], "metrics": [200] }
            ]
        });
        let pages = parse_breakdown(&json, |page, visitors| TopPage { page, visitors });
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].page, "/");
        assert_eq!(pages[0].visitors, 500);
        assert_eq!(pages[1].page, "/pricing");
        assert_eq!(pages[1].visitors, 200);
    }

    #[test]
    fn date_range_passes_valid_ui_periods_through_and_defaults_the_rest() {
        assert_eq!(date_range("7d"), serde_json::json!("7d"));
        assert_eq!(date_range("12mo"), serde_json::json!("12mo"));
        assert_eq!(date_range("all"), serde_json::json!("all"));
        // Unknown / legacy values fall back to the 30-day default.
        assert_eq!(date_range("90d"), serde_json::json!("30d"));
        assert_eq!(date_range(""), serde_json::json!("30d"));
    }
}
