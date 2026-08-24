use super::types::{
    SearchConsoleData, SearchCountry, SearchDailyPoint, SearchDevice, SearchPage, SearchQuery,
};
use super::GSC_API_URL;

/// Fetch Search Console analytics data
#[tracing::instrument(skip(access_token, site_url), fields(period_days))]
pub async fn fetch_analytics(
    access_token: &str,
    site_url: &str,
    period_days: u32,
) -> Result<SearchConsoleData, String> {
    let client = crate::http_client::client();

    let end_date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let start_date = (chrono::Utc::now() - chrono::Duration::days(period_days as i64))
        .format("%Y-%m-%d")
        .to_string();

    let encoded_url = urlencoding::encode(site_url);
    let base_url = format!(
        "{}/sites/{}/searchAnalytics/query",
        GSC_API_URL, encoded_url
    );

    // Query 1: aggregate totals (no dimensions)
    let agg_resp = query_search_analytics(
        client,
        access_token,
        &base_url,
        &serde_json::json!({
            "startDate": start_date,
            "endDate": end_date,
            "rowLimit": 1,
        }),
    )
    .await?;

    let (total_clicks, total_impressions, average_ctr, average_position) =
        parse_aggregate_totals(&agg_resp);

    // Query 2: top queries
    let queries_resp = query_search_analytics(
        client,
        access_token,
        &base_url,
        &serde_json::json!({
            "startDate": start_date,
            "endDate": end_date,
            "dimensions": ["query"],
            "rowLimit": 20,
        }),
    )
    .await?;
    let top_queries = parse_top_queries(&queries_resp);

    // Query 3: top pages
    let pages_resp = query_search_analytics(
        client,
        access_token,
        &base_url,
        &serde_json::json!({
            "startDate": start_date,
            "endDate": end_date,
            "dimensions": ["page"],
            "rowLimit": 20,
        }),
    )
    .await?;
    let top_pages = parse_top_pages(&pages_resp);

    // Query 4: daily timeseries
    let daily_resp = query_search_analytics(
        client,
        access_token,
        &base_url,
        &serde_json::json!({
            "startDate": start_date,
            "endDate": end_date,
            "dimensions": ["date"],
            "rowLimit": 500,
        }),
    )
    .await?;
    let daily = parse_daily(&daily_resp);

    // Query 5: devices
    let devices_resp = query_search_analytics(
        client,
        access_token,
        &base_url,
        &serde_json::json!({
            "startDate": start_date,
            "endDate": end_date,
            "dimensions": ["device"],
            "rowLimit": 5,
        }),
    )
    .await?;
    let devices = parse_devices(&devices_resp);

    // Query 6: countries
    let countries_resp = query_search_analytics(
        client,
        access_token,
        &base_url,
        &serde_json::json!({
            "startDate": start_date,
            "endDate": end_date,
            "dimensions": ["country"],
            "rowLimit": 10,
        }),
    )
    .await?;
    let countries = parse_countries(&countries_resp);

    Ok(SearchConsoleData {
        total_clicks,
        total_impressions,
        average_ctr,
        average_position,
        top_queries,
        top_pages,
        daily,
        devices,
        countries,
    })
}

async fn query_search_analytics(
    client: &reqwest::Client,
    access_token: &str,
    url: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let resp = client
        .post(url)
        .bearer_auth(access_token)
        .json(body)
        .timeout(crate::constants::API_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("Search Console API error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = crate::http_client::read_text_limited(
            resp,
            crate::constants::INTEGRATION_ERROR_BODY_MAX_BYTES,
            crate::constants::BODY_READ_TIMEOUT,
        )
        .await
        .unwrap_or_default();
        return Err(format!("Search Console returned {} - {}", status, body));
    }

    crate::http_client::read_json_limited(
        resp,
        crate::constants::GOOGLE_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await
    .map_err(|e| format!("Search Console parse error: {}", e))
}

#[tracing::instrument(skip(resp, mapper))]
pub(crate) fn parse_rows<T, F>(resp: &serde_json::Value, mapper: F) -> Vec<T>
where
    F: Fn(Vec<String>, &serde_json::Value) -> T,
{
    let empty = vec![];
    resp["rows"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|row| {
            let keys: Vec<String> = row["keys"]
                .as_array()?
                .iter()
                .filter_map(|k| k.as_str().map(|s| s.to_string()))
                .collect();
            Some(mapper(keys, row))
        })
        .collect()
}

/// Pull aggregate totals out of the no-dimensions response. Defaults to
/// `(0, 0, 0.0, 0.0)` for empty / shapeless responses (e.g. brand-new sites
/// with no traffic yet).
#[tracing::instrument(skip(resp))]
pub(crate) fn parse_aggregate_totals(resp: &serde_json::Value) -> (u64, u64, f64, f64) {
    let Some(rows) = resp["rows"].as_array() else {
        return (0, 0, 0.0, 0.0);
    };
    let Some(row) = rows.first() else {
        return (0, 0, 0.0, 0.0);
    };
    (
        row["clicks"].as_f64().unwrap_or(0.0) as u64,
        row["impressions"].as_f64().unwrap_or(0.0) as u64,
        row["ctr"].as_f64().unwrap_or(0.0),
        row["position"].as_f64().unwrap_or(0.0),
    )
}

#[tracing::instrument(skip(resp))]
pub(crate) fn parse_top_queries(resp: &serde_json::Value) -> Vec<SearchQuery> {
    parse_rows(resp, |keys, row| SearchQuery {
        query: keys.first().cloned().unwrap_or_default(),
        clicks: row["clicks"].as_f64().unwrap_or(0.0) as u64,
        impressions: row["impressions"].as_f64().unwrap_or(0.0) as u64,
        ctr: row["ctr"].as_f64().unwrap_or(0.0),
        position: row["position"].as_f64().unwrap_or(0.0),
    })
}

#[tracing::instrument(skip(resp))]
pub(crate) fn parse_top_pages(resp: &serde_json::Value) -> Vec<SearchPage> {
    parse_rows(resp, |keys, row| SearchPage {
        page: keys.first().cloned().unwrap_or_default(),
        clicks: row["clicks"].as_f64().unwrap_or(0.0) as u64,
        impressions: row["impressions"].as_f64().unwrap_or(0.0) as u64,
        ctr: row["ctr"].as_f64().unwrap_or(0.0),
        position: row["position"].as_f64().unwrap_or(0.0),
    })
}

#[tracing::instrument(skip(resp))]
pub(crate) fn parse_daily(resp: &serde_json::Value) -> Vec<SearchDailyPoint> {
    parse_rows(resp, |keys, row| SearchDailyPoint {
        date: keys.first().cloned().unwrap_or_default(),
        clicks: row["clicks"].as_f64().unwrap_or(0.0) as u64,
        impressions: row["impressions"].as_f64().unwrap_or(0.0) as u64,
        ctr: row["ctr"].as_f64().unwrap_or(0.0),
        position: row["position"].as_f64().unwrap_or(0.0),
    })
}

#[tracing::instrument(skip(resp))]
pub(crate) fn parse_devices(resp: &serde_json::Value) -> Vec<SearchDevice> {
    parse_rows(resp, |keys, row| SearchDevice {
        device: keys.first().cloned().unwrap_or_default(),
        clicks: row["clicks"].as_f64().unwrap_or(0.0) as u64,
        impressions: row["impressions"].as_f64().unwrap_or(0.0) as u64,
    })
}

#[tracing::instrument(skip(resp))]
pub(crate) fn parse_countries(resp: &serde_json::Value) -> Vec<SearchCountry> {
    parse_rows(resp, |keys, row| SearchCountry {
        country: keys.first().cloned().unwrap_or_default(),
        clicks: row["clicks"].as_f64().unwrap_or(0.0) as u64,
        impressions: row["impressions"].as_f64().unwrap_or(0.0) as u64,
    })
}
