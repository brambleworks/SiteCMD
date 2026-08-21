use super::types::{QueryRegression, SearchQuery};

/// Return sufficiently visible queries whose impressions fell across adjacent windows.
#[tracing::instrument(
    skip(oauth_token, site_url),
    fields(days_per_window, drop_threshold_pct, min_impressions)
)]
pub async fn fetch_query_comparison(
    oauth_token: &str,
    site_url: &str,
    days_per_window: u32,
    drop_threshold_pct: f64,
    min_impressions: i64,
) -> Result<Vec<QueryRegression>, String> {
    use chrono::{Duration as CDur, Utc};

    let today = Utc::now().date_naive();
    let current_end = today;
    let current_start = today - CDur::days(days_per_window as i64 - 1);
    let previous_end = current_start - CDur::days(1);
    let previous_start = previous_end - CDur::days(days_per_window as i64 - 1);

    let current = fetch_query_window(
        oauth_token,
        site_url,
        &current_start.to_string(),
        &current_end.to_string(),
    )
    .await?;
    let previous = fetch_query_window(
        oauth_token,
        site_url,
        &previous_start.to_string(),
        &previous_end.to_string(),
    )
    .await?;

    let prev_map: std::collections::HashMap<String, (u64, u64, f64, f64)> = previous
        .into_iter()
        .map(|q| {
            (
                q.query.clone(),
                (q.impressions, q.clicks, q.ctr, q.position),
            )
        })
        .collect();

    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut regressions = Vec::new();
    for cur in current {
        let Some((p_imp, p_clk, p_ctr, p_pos)) = prev_map.get(&cur.query).copied() else {
            continue;
        };
        if (p_imp as i64) < min_impressions {
            continue;
        }
        if p_imp == 0 {
            continue;
        }
        let drop_pct = 1.0 - (cur.impressions as f64 / p_imp as f64);
        if drop_pct < drop_threshold_pct {
            continue;
        }
        regressions.push(QueryRegression {
            query: cur.query.clone(),
            previous_impressions: p_imp as i64,
            current_impressions: cur.impressions as i64,
            previous_clicks: p_clk as i64,
            current_clicks: cur.clicks as i64,
            previous_ctr: p_ctr,
            current_ctr: cur.ctr,
            previous_position: p_pos,
            current_position: cur.position,
            detected_at: now_ms,
        });
    }
    Ok(regressions)
}

async fn fetch_query_window(
    oauth_token: &str,
    site_url: &str,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<SearchQuery>, String> {
    let encoded_site = urlencoding::encode(site_url);
    let endpoint = format!(
        "https://searchconsole.googleapis.com/v1/sites/{}/searchAnalytics/query",
        encoded_site
    );
    let client = crate::http_client::client();
    let resp = client
        .post(&endpoint)
        .bearer_auth(oauth_token)
        .json(&serde_json::json!({
            "startDate": start_date,
            "endDate": end_date,
            "dimensions": ["query"],
            "rowLimit": 500
        }))
        .send()
        .await
        .map_err(|e| format!("GSC query window request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GSC query window returned {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let rows = json
        .get("rows")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(rows
        .into_iter()
        .map(|r| SearchQuery {
            query: r
                .pointer("/keys/0")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            clicks: r.get("clicks").and_then(|v| v.as_f64()).unwrap_or(0.0) as u64,
            impressions: r.get("impressions").and_then(|v| v.as_f64()).unwrap_or(0.0) as u64,
            ctr: r.get("ctr").and_then(|v| v.as_f64()).unwrap_or(0.0),
            position: r.get("position").and_then(|v| v.as_f64()).unwrap_or(0.0),
        })
        .collect())
}
