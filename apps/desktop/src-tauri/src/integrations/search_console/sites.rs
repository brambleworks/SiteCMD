use super::types::GSCSite;

/// Parse the `sites.list` response into the GSCSite list. Skips entries
/// missing `siteUrl` rather than failing the whole call.
#[tracing::instrument(skip(json))]
pub(crate) fn parse_sites(json: &serde_json::Value) -> Vec<GSCSite> {
    let empty = vec![];
    json["siteEntry"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|entry| {
            let site_url = entry["siteUrl"].as_str()?.to_string();
            let permission = entry["permissionLevel"].as_str().unwrap_or("").to_string();
            Some(GSCSite {
                site_url,
                permission,
            })
        })
        .collect()
}

/// List all sites the authenticated user has access to in Search Console
#[tracing::instrument(skip(access_token))]
pub async fn list_sites(access_token: &str) -> Result<Vec<GSCSite>, String> {
    let client = crate::http_client::client();

    let resp = client
        .get("https://www.googleapis.com/webmasters/v3/sites")
        .bearer_auth(access_token)
        .timeout(crate::constants::API_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("Search Console API error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Search Console returned {} - {}", status, body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Search Console parse error: {}", e))?;

    Ok(parse_sites(&json))
}
