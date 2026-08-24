//! Public RubyGems version client. Its endpoint provides no deprecation or
//! publish-recency facts.

use crate::updates::types::{classify_update, Ecosystem, InstalledPackage, PackageUpdate};
use reqwest::Client;

const CONCURRENCY_LIMIT: usize = 10;

/// Check RubyGems for latest versions. The bool is true when the census was
/// only partially observed (a fetch failed outright), so absences from
/// `updates` are unproven this sweep.
pub async fn check_updates(packages: &[InstalledPackage]) -> (Vec<PackageUpdate>, bool) {
    let fan_out = super::concurrency::check_registry_updates(packages, CONCURRENCY_LIMIT, |pkg| {
        let client = crate::http_client::client().clone();
        async move { fetch_latest(&client, &pkg.name, &pkg.version, &pkg.source, pkg.is_dev).await }
    })
    .await;
    (fan_out.results, fan_out.failed > 0)
}

/// Build a `PackageUpdate` from the parsed RubyGems `/gems/{name}.json`
/// response. Returns None when no real update should be reported. Pure;
/// tested directly.
pub(crate) fn build_update_from_response(
    name: &str,
    current: &str,
    source: &str,
    is_dev: bool,
    body: &serde_json::Value,
) -> Option<PackageUpdate> {
    let latest = body.get("version").and_then(|v| v.as_str()).unwrap_or("");
    if latest.is_empty() || latest == current {
        return None;
    }
    Some(PackageUpdate {
        name: name.to_string(),
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        ecosystem: Ecosystem::Ruby,
        update_type: classify_update(current, latest),
        is_security: false,
        advisory_severity: None,
        advisory_url: None,
        source: source.to_string(),
        is_dev,
        ..Default::default()
    })
}

async fn fetch_latest(
    client: &Client,
    name: &str,
    current: &str,
    source: &str,
    is_dev: bool,
) -> Result<Option<PackageUpdate>, String> {
    let url = format!("https://rubygems.org/api/v1/gems/{}.json", name);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if super::status_is_observed_absence(status) {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!("RubyGems returned status {} for {}", status, name));
    }
    let body: serde_json::Value = crate::http_client::read_json_limited(
        resp,
        crate::constants::RUBYGEMS_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await?;
    Ok(build_update_from_response(
        name, current, source, is_dev, &body,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updates::types::UpdateType;

    fn body_with(version: &str) -> serde_json::Value {
        serde_json::json!({"version": version})
    }

    #[test]
    fn build_update_returns_some_for_upgrade() {
        let update =
            build_update_from_response("rails", "7.0.0", "Gemfile", false, &body_with("7.1.0"))
                .expect("update");
        assert_eq!(update.name, "rails");
        assert_eq!(update.latest_version, "7.1.0");
        assert_eq!(update.ecosystem, Ecosystem::Ruby);
        assert_eq!(update.update_type, UpdateType::Minor);
    }

    #[test]
    fn build_update_returns_none_when_versions_match() {
        let result =
            build_update_from_response("rails", "7.1.0", "Gemfile", false, &body_with("7.1.0"));
        assert!(result.is_none());
    }

    #[test]
    fn build_update_returns_none_when_version_field_missing() {
        let result =
            build_update_from_response("rails", "7.0.0", "Gemfile", false, &serde_json::json!({}));
        assert!(result.is_none());
    }

    #[test]
    fn build_update_returns_none_when_version_empty_string() {
        let result = build_update_from_response("rails", "7.0.0", "Gemfile", false, &body_with(""));
        assert!(result.is_none());
    }

    #[test]
    fn build_update_propagates_is_dev_flag() {
        let update =
            build_update_from_response("rspec", "3.12.0", "Gemfile", true, &body_with("3.13.0"))
                .expect("update");
        assert!(update.is_dev);
    }
}
