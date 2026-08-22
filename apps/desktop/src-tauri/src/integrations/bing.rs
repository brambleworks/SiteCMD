//! Bing Webmaster Tools API client.
//! API docs: https://learn.microsoft.com/en-us/bingwebmaster/
//! Auth: API key from Bing Webmaster Tools Settings > API Access

use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://ssl.bing.com/webmaster/api.svc/json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingSearchData {
    pub total_clicks: u64,
    pub total_impressions: u64,
    pub avg_position: f64,
    pub daily_stats: Vec<BingDailyStat>,
    pub top_queries: Vec<BingQueryStat>,
    pub top_pages: Vec<BingPageStat>,
    pub crawl_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingDailyStat {
    pub date: String,
    pub clicks: u64,
    pub impressions: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingQueryStat {
    pub query: String,
    pub clicks: u64,
    pub impressions: u64,
    pub avg_position: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingPageStat {
    pub url: String,
    pub clicks: u64,
    pub impressions: u64,
    pub avg_position: f64,
}

/// Bing's API requires a trailing slash on the site URL - otherwise it
/// returns no data. Pure helper, idempotent.
#[tracing::instrument(skip(site_url))]
pub(crate) fn normalize_site_url(site_url: &str) -> String {
    if site_url.ends_with('/') {
        site_url.to_string()
    } else {
        format!("{}/", site_url)
    }
}

/// Parse the GetRankAndTrafficStats response. Returns the most recent 30 days
/// of daily stats (newest first) along with cumulative click/impression totals.
#[tracing::instrument(skip(value))]
pub(crate) fn parse_traffic_stats(value: &serde_json::Value) -> (Vec<BingDailyStat>, u64, u64) {
    let mut daily: Vec<BingDailyStat> = Vec::new();
    let mut total_clicks: u64 = 0;
    let mut total_impressions: u64 = 0;
    let Some(entries) = value["d"].as_array() else {
        return (daily, total_clicks, total_impressions);
    };
    for entry in entries.iter().rev().take(30) {
        let clicks = entry["Clicks"].as_u64().unwrap_or(0);
        let impressions = entry["Impressions"].as_u64().unwrap_or(0);
        let date = parse_bing_date(entry["Date"].as_str().unwrap_or(""));
        total_clicks += clicks;
        total_impressions += impressions;
        daily.push(BingDailyStat {
            date,
            clicks,
            impressions,
        });
    }
    (daily, total_clicks, total_impressions)
}

/// Aggregate the per-date GetQueryStats response into top-20 unique queries
/// sorted by click count (descending). Bing's `AvgClickPosition` is divided
/// by 10 (their values are 10x).
#[tracing::instrument(skip(value))]
pub(crate) fn aggregate_query_stats(value: &serde_json::Value) -> Vec<BingQueryStat> {
    let Some(entries) = value["d"].as_array() else {
        return Vec::new();
    };
    let mut by_query: std::collections::HashMap<String, (u64, u64, f64, u64)> =
        std::collections::HashMap::new();
    for entry in entries {
        let query = entry["Query"].as_str().unwrap_or("").to_string();
        let clicks = entry["Clicks"].as_u64().unwrap_or(0);
        let impressions = entry["Impressions"].as_u64().unwrap_or(0);
        let position = entry["AvgClickPosition"].as_f64().unwrap_or(0.0) / 10.0;
        let e = by_query.entry(query).or_insert((0, 0, 0.0, 0));
        e.0 += clicks;
        e.1 += impressions;
        e.2 += position;
        e.3 += 1;
    }
    let mut queries: Vec<BingQueryStat> = by_query
        .into_iter()
        .map(
            |(query, (clicks, impressions, pos_sum, count))| BingQueryStat {
                query,
                clicks,
                impressions,
                avg_position: if count > 0 {
                    pos_sum / count as f64
                } else {
                    0.0
                },
            },
        )
        .collect();
    queries.sort_by_key(|b| std::cmp::Reverse(b.clicks));
    queries.into_iter().take(20).collect()
}

/// Aggregate the per-date GetPageStats response. Bing's PageStats endpoint
/// reuses the `Query` field for the URL (sic). Top-20 by clicks descending.
#[tracing::instrument(skip(value))]
pub(crate) fn aggregate_page_stats(value: &serde_json::Value) -> Vec<BingPageStat> {
    let Some(entries) = value["d"].as_array() else {
        return Vec::new();
    };
    let mut by_url: std::collections::HashMap<String, (u64, u64, f64, u64)> =
        std::collections::HashMap::new();
    for entry in entries {
        let url = entry["Query"].as_str().unwrap_or("").to_string();
        let clicks = entry["Clicks"].as_u64().unwrap_or(0);
        let impressions = entry["Impressions"].as_u64().unwrap_or(0);
        let position = entry["AvgClickPosition"].as_f64().unwrap_or(0.0) / 10.0;
        let e = by_url.entry(url).or_insert((0, 0, 0.0, 0));
        e.0 += clicks;
        e.1 += impressions;
        e.2 += position;
        e.3 += 1;
    }
    let mut pages: Vec<BingPageStat> = by_url
        .into_iter()
        .map(
            |(url, (clicks, impressions, pos_sum, count))| BingPageStat {
                url,
                clicks,
                impressions,
                avg_position: if count > 0 {
                    pos_sum / count as f64
                } else {
                    0.0
                },
            },
        )
        .collect();
    pages.sort_by_key(|b| std::cmp::Reverse(b.clicks));
    pages.into_iter().take(20).collect()
}

/// Mean of the per-query `avg_position` values. Returns 0.0 when there are
/// no queries (defaults to "no data" rather than NaN).
#[tracing::instrument(skip(queries))]
pub(crate) fn compute_avg_position(queries: &[BingQueryStat]) -> f64 {
    if queries.is_empty() {
        return 0.0;
    }
    let sum: f64 = queries.iter().map(|q| q.avg_position).sum();
    sum / queries.len() as f64
}

/// Fetch search performance data from Bing Webmaster Tools API
#[tracing::instrument(skip(api_key, site_url))]
pub async fn fetch_search_stats(api_key: &str, site_url: &str) -> Result<BingSearchData, String> {
    let client = crate::http_client::client();
    let site = normalize_site_url(site_url);

    // Put the key in query parameters and strip URLs from reqwest errors so
    // credential-bearing request URLs never reach logs.
    let mut daily_stats: Vec<BingDailyStat> = Vec::new();
    let mut total_clicks: u64 = 0;
    let mut total_impressions: u64 = 0;

    match client
        .get(format!("{}/GetRankAndTrafficStats", API_BASE))
        .query(&[("apikey", api_key), ("siteUrl", site.as_str())])
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(data) = crate::http_client::read_json_limited::<serde_json::Value>(
                resp,
                crate::constants::BING_API_RESPONSE_MAX_BYTES,
                crate::constants::BODY_READ_TIMEOUT,
            )
            .await
            {
                let (d, c, i) = parse_traffic_stats(&data);
                daily_stats = d;
                total_clicks = c;
                total_impressions = i;
            }
        }
        Ok(resp) => {
            let status = resp.status();
            let body = crate::http_client::read_text_limited(
                resp,
                crate::constants::INTEGRATION_ERROR_BODY_MAX_BYTES,
                crate::constants::BODY_READ_TIMEOUT,
            )
            .await
            .unwrap_or_default();
            tracing::warn!(
                "Bing traffic stats returned {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            );
        }
        Err(e) => {
            tracing::warn!("Bing traffic stats request failed: {}", e.without_url());
        }
    }

    let mut top_queries: Vec<BingQueryStat> = Vec::new();

    match client
        .get(format!("{}/GetQueryStats", API_BASE))
        .query(&[("apikey", api_key), ("siteUrl", site.as_str())])
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(data) = crate::http_client::read_json_limited::<serde_json::Value>(
                resp,
                crate::constants::BING_API_RESPONSE_MAX_BYTES,
                crate::constants::BODY_READ_TIMEOUT,
            )
            .await
            {
                top_queries = aggregate_query_stats(&data);
            }
        }
        Ok(_) | Err(_) => {}
    }

    let mut top_pages: Vec<BingPageStat> = Vec::new();

    match client
        .get(format!("{}/GetPageStats", API_BASE))
        .query(&[("apikey", api_key), ("siteUrl", site.as_str())])
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(data) = crate::http_client::read_json_limited::<serde_json::Value>(
                resp,
                crate::constants::BING_API_RESPONSE_MAX_BYTES,
                crate::constants::BODY_READ_TIMEOUT,
            )
            .await
            {
                top_pages = aggregate_page_stats(&data);
            }
        }
        Ok(_) | Err(_) => {}
    }

    let avg_position = compute_avg_position(&top_queries);

    tracing::info!(
        "Bing stats for {}: {} clicks, {} impressions, {:.1} avg position, {} queries, {} pages",
        site_url,
        total_clicks,
        total_impressions,
        avg_position,
        top_queries.len(),
        top_pages.len()
    );

    Ok(BingSearchData {
        total_clicks,
        total_impressions,
        avg_position,
        daily_stats,
        top_queries,
        top_pages,
        crawl_errors: 0, // Not available from these endpoints
    })
}

/// Parse Bing's date format: "/Date(1234567890000)/" → "2024-01-15"
fn parse_bing_date(date_str: &str) -> String {
    if let Some(start) = date_str.find("/Date(") {
        let num_start = start + 6;
        if let Some(end) = date_str[num_start..].find(')') {
            let ms_str = &date_str[num_start..num_start + end];
            // Handle timezone offset like "+0000"
            let ms_part = if let Some(plus_idx) = ms_str.find('+') {
                &ms_str[..plus_idx]
            } else if let Some(minus_idx) = ms_str.rfind('-') {
                if minus_idx > 0 {
                    &ms_str[..minus_idx]
                } else {
                    ms_str
                }
            } else {
                ms_str
            };
            if let Ok(ms) = ms_part.parse::<i64>() {
                if let Some(dt) = chrono::DateTime::from_timestamp(ms / 1000, 0) {
                    return dt.format("%Y-%m-%d").to_string();
                }
            }
        }
    }
    date_str.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_site_url_appends_trailing_slash() {
        assert_eq!(
            normalize_site_url("https://example.com"),
            "https://example.com/"
        );
    }

    #[test]
    fn normalize_site_url_is_idempotent_when_slash_present() {
        assert_eq!(
            normalize_site_url("https://example.com/"),
            "https://example.com/"
        );
        assert_eq!(normalize_site_url("/"), "/");
    }

    #[test]
    fn parse_bing_date_extracts_date_from_dotnet_format() {
        // Bing wraps timestamps as `/Date(<ms>)/` (a .NET serialization
        // artifact). 1700000000000ms = 2023-11-14.
        let parsed = parse_bing_date("/Date(1700000000000)/");
        assert_eq!(parsed, "2023-11-14");
    }

    #[test]
    fn parse_bing_date_strips_timezone_suffix() {
        // Some responses include "+0000" or similar offset.
        let parsed = parse_bing_date("/Date(1700000000000+0000)/");
        assert_eq!(parsed, "2023-11-14");
    }

    #[test]
    fn parse_bing_date_returns_input_on_unrecognized_format() {
        // If the wrapper is missing or malformed, return the raw string so
        // the user sees something rather than an empty date.
        assert_eq!(parse_bing_date("2024-01-15"), "2024-01-15");
        assert_eq!(parse_bing_date(""), "");
        assert_eq!(parse_bing_date("garbage"), "garbage");
    }

    #[test]
    fn parse_bing_date_returns_input_on_unparseable_number() {
        assert_eq!(
            parse_bing_date("/Date(not-a-number)/"),
            "/Date(not-a-number)/"
        );
    }

    #[test]
    fn parse_traffic_stats_collects_daily_and_totals() {
        let body = serde_json::json!({
            "d": [
                {"Date": "/Date(1700000000000)/", "Clicks": 10u64, "Impressions": 100u64},
                {"Date": "/Date(1700086400000)/", "Clicks": 20u64, "Impressions": 200u64},
            ]
        });
        let (daily, clicks, impressions) = parse_traffic_stats(&body);
        // Iteration is reversed (newest first), so the second entry comes first.
        assert_eq!(daily.len(), 2);
        assert_eq!(daily[0].clicks, 20);
        assert_eq!(daily[1].clicks, 10);
        assert_eq!(clicks, 30);
        assert_eq!(impressions, 300);
    }

    #[test]
    fn parse_traffic_stats_caps_at_30_days() {
        let entries: Vec<serde_json::Value> = (0..50)
            .map(|i| {
                serde_json::json!({
                    "Date": format!("/Date({}000)/", 1700000000 + i * 86400),
                    "Clicks": 1u64,
                    "Impressions": 10u64,
                })
            })
            .collect();
        let (daily, clicks, impressions) = parse_traffic_stats(&serde_json::json!({"d": entries}));
        assert_eq!(daily.len(), 30, "must cap the daily window at 30");
        assert_eq!(clicks, 30);
        assert_eq!(impressions, 300);
    }

    #[test]
    fn parse_traffic_stats_returns_empty_when_d_missing() {
        let (daily, clicks, impressions) = parse_traffic_stats(&serde_json::json!({}));
        assert!(daily.is_empty());
        assert_eq!(clicks, 0);
        assert_eq!(impressions, 0);
    }

    #[test]
    fn parse_traffic_stats_defaults_missing_fields_to_zero() {
        let body = serde_json::json!({"d": [{"Date": "/Date(1700000000000)/"}]});
        let (daily, clicks, impressions) = parse_traffic_stats(&body);
        assert_eq!(daily.len(), 1);
        assert_eq!(daily[0].clicks, 0);
        assert_eq!(daily[0].impressions, 0);
        assert_eq!(clicks, 0);
        assert_eq!(impressions, 0);
    }

    #[test]
    fn aggregate_query_stats_merges_per_date_rows_for_same_query() {
        // The API returns one row per (query, date). The aggregation
        // collapses these into one entry per query with summed totals.
        let body = serde_json::json!({
            "d": [
                {"Query": "sitecmd", "Clicks": 10u64, "Impressions": 100u64, "AvgClickPosition": 25.0},
                {"Query": "sitecmd", "Clicks": 5u64, "Impressions": 50u64, "AvgClickPosition": 30.0},
                {"Query": "other", "Clicks": 3u64, "Impressions": 20u64, "AvgClickPosition": 80.0},
            ]
        });
        let queries = aggregate_query_stats(&body);
        assert_eq!(queries.len(), 2);
        // Sorted by clicks desc → "sitecmd" with 15 first.
        assert_eq!(queries[0].query, "sitecmd");
        assert_eq!(queries[0].clicks, 15);
        assert_eq!(queries[0].impressions, 150);
        // avg_position = mean(2.5, 3.0) = 2.75 (raw values are /10).
        assert!(
            (queries[0].avg_position - 2.75).abs() < 1e-9,
            "avg_position = {}",
            queries[0].avg_position
        );
    }

    #[test]
    fn aggregate_query_stats_divides_position_by_ten() {
        let body = serde_json::json!({
            "d": [{"Query": "x", "Clicks": 1u64, "Impressions": 10u64, "AvgClickPosition": 47.0}]
        });
        let queries = aggregate_query_stats(&body);
        assert_eq!(queries.len(), 1);
        assert!((queries[0].avg_position - 4.7).abs() < 1e-9);
    }

    #[test]
    fn aggregate_query_stats_sorts_by_clicks_descending() {
        let body = serde_json::json!({
            "d": [
                {"Query": "low", "Clicks": 1u64},
                {"Query": "high", "Clicks": 100u64},
                {"Query": "mid", "Clicks": 50u64},
            ]
        });
        let queries = aggregate_query_stats(&body);
        let names: Vec<&str> = queries.iter().map(|q| q.query.as_str()).collect();
        assert_eq!(names, vec!["high", "mid", "low"]);
    }

    #[test]
    fn aggregate_query_stats_caps_at_top_20() {
        let entries: Vec<serde_json::Value> = (0..30)
            .map(|i| serde_json::json!({"Query": format!("q{}", i), "Clicks": (i + 1) as u64}))
            .collect();
        let queries = aggregate_query_stats(&serde_json::json!({"d": entries}));
        assert_eq!(queries.len(), 20);
        // Top entry = q29 (highest clicks).
        assert_eq!(queries[0].query, "q29");
    }

    #[test]
    fn aggregate_query_stats_returns_empty_when_d_missing() {
        assert!(aggregate_query_stats(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn aggregate_page_stats_uses_query_field_as_url() {
        // Bing reuses the "Query" field for the URL on the PageStats
        // endpoint - sic. Document the quirk via test.
        let body = serde_json::json!({
            "d": [
                {"Query": "https://example.com/", "Clicks": 50u64, "Impressions": 500u64, "AvgClickPosition": 10.0},
                {"Query": "https://example.com/about", "Clicks": 30u64, "Impressions": 300u64, "AvgClickPosition": 20.0},
            ]
        });
        let pages = aggregate_page_stats(&body);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].url, "https://example.com/");
        assert_eq!(pages[0].clicks, 50);
        assert!((pages[0].avg_position - 1.0).abs() < 1e-9);
    }

    #[test]
    fn aggregate_page_stats_merges_duplicates_and_sorts() {
        let body = serde_json::json!({
            "d": [
                {"Query": "/a", "Clicks": 10u64, "AvgClickPosition": 10.0},
                {"Query": "/a", "Clicks": 20u64, "AvgClickPosition": 20.0},
                {"Query": "/b", "Clicks": 100u64, "AvgClickPosition": 5.0},
            ]
        });
        let pages = aggregate_page_stats(&body);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].url, "/b");
        assert_eq!(pages[0].clicks, 100);
        assert_eq!(pages[1].url, "/a");
        assert_eq!(pages[1].clicks, 30);
        assert!((pages[1].avg_position - 1.5).abs() < 1e-9);
    }

    #[test]
    fn compute_avg_position_returns_zero_for_empty() {
        assert_eq!(compute_avg_position(&[]), 0.0);
    }

    #[test]
    fn compute_avg_position_averages_query_positions() {
        let queries = vec![
            BingQueryStat {
                query: "a".into(),
                clicks: 0,
                impressions: 0,
                avg_position: 1.0,
            },
            BingQueryStat {
                query: "b".into(),
                clicks: 0,
                impressions: 0,
                avg_position: 2.0,
            },
            BingQueryStat {
                query: "c".into(),
                clicks: 0,
                impressions: 0,
                avg_position: 3.0,
            },
        ];
        assert!((compute_avg_position(&queries) - 2.0).abs() < 1e-9);
    }
}
