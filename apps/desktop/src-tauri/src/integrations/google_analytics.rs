//! Google Analytics 4 API client.

use serde::{Deserialize, Serialize};

const GA4_REPORT_URL: &str = "https://analyticsdata.googleapis.com/v1beta/properties";

#[derive(Debug, Serialize, Deserialize)]
pub struct GA4Data {
    pub active_users: u64,
    pub sessions: u64,
    pub pageviews: u64,
    pub bounce_rate: f64,
    pub avg_session_duration: f64,
    pub top_pages: Vec<GA4TopPage>,
    pub top_sources: Vec<GA4TopSource>,
    pub top_countries: Vec<GA4Country>,
    pub daily: Vec<GA4DailyPoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GA4TopPage {
    pub page: String,
    pub views: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GA4TopSource {
    pub source: String,
    pub users: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GA4Country {
    pub country: String,
    pub users: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GA4DailyPoint {
    pub date: String,
    pub users: u64,
    pub sessions: u64,
    pub pageviews: u64,
}

/// Fetch GA4 analytics data for a property
#[tracing::instrument(skip(access_token), fields(property_id = %property_id, period_days))]
pub async fn fetch_analytics(
    access_token: &str,
    property_id: &str,
    period_days: u32,
) -> Result<GA4Data, String> {
    let client = crate::http_client::client();

    let end_date = "today";
    let start_date = format!("{}daysAgo", period_days);

    let agg_body = serde_json::json!({
        "dateRanges": [{"startDate": &start_date, "endDate": end_date}],
        "metrics": [
            {"name": "activeUsers"},
            {"name": "sessions"},
            {"name": "screenPageViews"},
            {"name": "bounceRate"},
            {"name": "averageSessionDuration"},
        ],
    });
    let pages_body = serde_json::json!({
        "dateRanges": [{"startDate": &start_date, "endDate": end_date}],
        "dimensions": [{"name": "pagePath"}],
        "metrics": [{"name": "screenPageViews"}],
        "orderBys": [{"metric": {"metricName": "screenPageViews"}, "desc": true}],
        "limit": 10,
    });
    let sources_body = serde_json::json!({
        "dateRanges": [{"startDate": &start_date, "endDate": end_date}],
        "dimensions": [{"name": "sessionSource"}],
        "metrics": [{"name": "activeUsers"}],
        "orderBys": [{"metric": {"metricName": "activeUsers"}, "desc": true}],
        "limit": 10,
    });
    let countries_body = serde_json::json!({
        "dateRanges": [{"startDate": &start_date, "endDate": end_date}],
        "dimensions": [{"name": "country"}],
        "metrics": [{"name": "activeUsers"}],
        "orderBys": [{"metric": {"metricName": "activeUsers"}, "desc": true}],
        "limit": 10,
    });
    let daily_body = serde_json::json!({
        "dateRanges": [{"startDate": &start_date, "endDate": end_date}],
        "dimensions": [{"name": "date"}],
        "metrics": [
            {"name": "activeUsers"},
            {"name": "sessions"},
            {"name": "screenPageViews"},
        ],
        "orderBys": [{"dimension": {"dimensionName": "date"}}],
    });

    let (agg, pages_resp, sources_resp, countries_resp, daily_resp) = tokio::try_join!(
        run_report(client, access_token, property_id, &agg_body),
        run_report(client, access_token, property_id, &pages_body),
        run_report(client, access_token, property_id, &sources_body),
        run_report(client, access_token, property_id, &countries_body),
        run_report(client, access_token, property_id, &daily_body),
    )?;

    let agg_row = agg["rows"]
        .as_array()
        .and_then(|r| r.first())
        .and_then(|r| r["metricValues"].as_array());

    let (active_users, sessions, pageviews, bounce_rate, avg_duration) = match agg_row {
        Some(vals) => (
            parse_metric_u64(vals, 0),
            parse_metric_u64(vals, 1),
            parse_metric_u64(vals, 2),
            parse_metric_f64(vals, 3),
            parse_metric_f64(vals, 4),
        ),
        None => (0, 0, 0, 0.0, 0.0),
    };

    let top_pages = parse_dimension_metric(&pages_resp, |dim, val| GA4TopPage {
        page: dim,
        views: val.parse().unwrap_or(0),
    });

    let top_sources = parse_dimension_metric(&sources_resp, |dim, val| GA4TopSource {
        source: if dim.is_empty() || dim == "(not set)" {
            "Direct".to_string()
        } else {
            dim
        },
        users: val.parse().unwrap_or(0),
    });

    let top_countries = parse_dimension_metric(&countries_resp, |dim, val| GA4Country {
        country: dim,
        users: val.parse().unwrap_or(0),
    });

    let daily: Vec<GA4DailyPoint> = daily_resp["rows"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|row| {
            let dims = row["dimensionValues"].as_array()?;
            let vals = row["metricValues"].as_array()?;
            let raw_date = dims.first()?.get("value")?.as_str()?;
            let date = if raw_date.len() == 8 {
                format!("{}-{}-{}", &raw_date[..4], &raw_date[4..6], &raw_date[6..8])
            } else {
                raw_date.to_string()
            };
            Some(GA4DailyPoint {
                date,
                users: parse_metric_u64(vals, 0),
                sessions: parse_metric_u64(vals, 1),
                pageviews: parse_metric_u64(vals, 2),
            })
        })
        .collect();

    Ok(GA4Data {
        active_users,
        sessions,
        pageviews,
        bounce_rate,
        avg_session_duration: avg_duration,
        top_pages,
        top_sources,
        top_countries,
        daily,
    })
}

async fn run_report(
    client: &reqwest::Client,
    access_token: &str,
    property_id: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    // Strip "properties/" prefix if already present
    let prop_id = property_id.trim_start_matches("properties/");
    let url = format!("{}/{}:runReport", GA4_REPORT_URL, prop_id);

    let resp = client
        .post(&url)
        .bearer_auth(access_token)
        .json(body)
        .timeout(crate::constants::API_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("GA4 API error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = crate::http_client::read_text_limited(
            resp,
            crate::constants::INTEGRATION_ERROR_BODY_MAX_BYTES,
            crate::constants::BODY_READ_TIMEOUT,
        )
        .await
        .unwrap_or_default();
        return Err(format!("GA4 API returned {} - {}", status, body));
    }

    crate::http_client::read_json_limited(
        resp,
        crate::constants::GOOGLE_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await
    .map_err(|e| format!("GA4 parse error: {}", e))
}

fn parse_metric_u64(vals: &[serde_json::Value], idx: usize) -> u64 {
    vals.get(idx)
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn parse_metric_f64(vals: &[serde_json::Value], idx: usize) -> f64 {
    vals.get(idx)
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0)
}

fn parse_dimension_metric<T, F>(resp: &serde_json::Value, mapper: F) -> Vec<T>
where
    F: Fn(String, String) -> T,
{
    resp["rows"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|row| {
            let dim = row["dimensionValues"]
                .as_array()?
                .first()?
                .get("value")?
                .as_str()?
                .to_string();
            let val = row["metricValues"]
                .as_array()?
                .first()?
                .get("value")?
                .as_str()?
                .to_string();
            Some(mapper(dim, val))
        })
        .collect()
}

/// List all GA4 properties accessible to the authenticated user
#[tracing::instrument(skip(access_token))]
pub async fn list_properties(access_token: &str) -> Result<Vec<GA4Property>, String> {
    let client = crate::http_client::client();

    // Use the GA4 Admin API to list account summaries
    let resp = client
        .get("https://analyticsadmin.googleapis.com/v1beta/accountSummaries")
        .bearer_auth(access_token)
        .timeout(crate::constants::API_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("GA4 Admin API error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = crate::http_client::read_text_limited(
            resp,
            crate::constants::INTEGRATION_ERROR_BODY_MAX_BYTES,
            crate::constants::BODY_READ_TIMEOUT,
        )
        .await
        .unwrap_or_default();
        return Err(format!("GA4 Admin API returned {} - {}", status, body));
    }

    let json: serde_json::Value = crate::http_client::read_json_limited(
        resp,
        crate::constants::GOOGLE_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await
    .map_err(|e| format!("GA4 Admin parse error: {}", e))?;

    let mut properties = Vec::new();
    if let Some(accounts) = json["accountSummaries"].as_array() {
        for account in accounts {
            let account_name = account["displayName"].as_str().unwrap_or("").to_string();
            if let Some(props) = account["propertySummaries"].as_array() {
                for prop in props {
                    let full_name = prop["property"].as_str().unwrap_or(""); // "properties/123456"
                    let property_id = full_name.trim_start_matches("properties/").to_string();
                    let display_name = prop["displayName"].as_str().unwrap_or("").to_string();
                    properties.push(GA4Property {
                        property_id,
                        display_name,
                        account_name: account_name.clone(),
                    });
                }
            }
        }
    }

    Ok(properties)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GA4Property {
    pub property_id: String,
    pub display_name: String,
    pub account_name: String,
}
