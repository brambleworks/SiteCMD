//! Public Go module proxy client for tagged versions and publish recency.

use crate::updates::types::{classify_update, Ecosystem, InstalledPackage, PackageUpdate};
use reqwest::Client;

const CONCURRENCY_LIMIT: usize = 10;

/// Check Go module proxy for latest versions. The bool is true when the
/// census was only partially observed (a fetch failed outright), so absences
/// from `updates` are unproven this sweep.
pub async fn check_updates(packages: &[InstalledPackage]) -> (Vec<PackageUpdate>, bool) {
    let fan_out = super::concurrency::check_registry_updates(packages, CONCURRENCY_LIMIT, |pkg| {
        let client = crate::http_client::client().clone();
        async move { fetch_latest(&client, &pkg.name, &pkg.version, &pkg.source, pkg.is_dev).await }
    })
    .await;
    (fan_out.results, fan_out.failed > 0)
}

/// Build a `PackageUpdate` from the parsed Go module proxy `/@latest`
/// response. Leading `v` is removed for comparison; publish time only
/// decorates an entry with staleness.
pub(crate) fn build_update_from_response(
    name: &str,
    current: &str,
    source: &str,
    is_dev: bool,
    body: &serde_json::Value,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<PackageUpdate> {
    let latest = body
        .get("Version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v');

    if latest.is_empty() || latest == current {
        return None;
    }

    let last_published = body
        .get("Time")
        .and_then(|t| t.as_str())
        .map(str::to_string);

    Some(PackageUpdate {
        name: name.to_string(),
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        ecosystem: Ecosystem::Go,
        update_type: classify_update(current, latest),
        is_security: false,
        advisory_severity: None,
        advisory_url: None,
        source: source.to_string(),
        is_dev,
        is_stale: super::is_stale_at(last_published.as_deref(), now),
        last_published,
        ..Default::default()
    })
}

/// Apply the Go proxy's uppercase-letter path encoding.
fn escape_module_path(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            out.push('!');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// The GOPROXY protocol serves "module not available" as 404 OR 410 (proxies
/// emit 410 for modules known-absent from the origin), so both are observed
/// absences, not outages that should mark the sweep partial.
fn go_status_is_observed_absence(status: reqwest::StatusCode) -> bool {
    super::status_is_observed_absence(status) || status == reqwest::StatusCode::GONE
}

async fn fetch_latest(
    client: &Client,
    name: &str,
    current: &str,
    source: &str,
    is_dev: bool,
) -> Result<Option<PackageUpdate>, String> {
    let url = format!(
        "https://proxy.golang.org/{}/@latest",
        escape_module_path(name)
    );
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if go_status_is_observed_absence(status) {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!(
            "Go module proxy returned status {} for {}",
            status, name
        ));
    }
    let body: serde_json::Value = crate::http_client::read_json_limited(
        resp,
        crate::constants::GO_PROXY_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await?;
    Ok(build_update_from_response(
        name,
        current,
        source,
        is_dev,
        &body,
        chrono::Utc::now(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updates::types::UpdateType;

    #[test]
    fn gone_and_not_found_are_observed_absences_other_failures_are_not() {
        use reqwest::StatusCode;
        assert!(go_status_is_observed_absence(StatusCode::NOT_FOUND));
        assert!(go_status_is_observed_absence(StatusCode::GONE));
        for status in [
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::FORBIDDEN,
        ] {
            assert!(
                !go_status_is_observed_absence(status),
                "{status} must count as an unobserved fetch, not an absence"
            );
        }
    }

    fn body(version: &str) -> serde_json::Value {
        serde_json::json!({"Version": version})
    }

    fn fixed_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .expect("valid fixture timestamp")
            .with_timezone(&chrono::Utc)
    }

    fn build(name: &str, current: &str, body: &serde_json::Value) -> Option<PackageUpdate> {
        build_update_from_response(name, current, "go.mod", false, body, fixed_now())
    }

    #[test]
    fn escape_module_path_bang_encodes_uppercase() {
        // Go module proxy protocol: uppercase letters must be !-escaped or the
        // proxy 404s and the module is silently treated as up to date.
        assert_eq!(
            escape_module_path("github.com/BurntSushi/toml"),
            "github.com/!burnt!sushi/toml"
        );
        assert_eq!(
            escape_module_path("github.com/Azure/azure-sdk-for-go"),
            "github.com/!azure/azure-sdk-for-go"
        );
        // All-lowercase paths are unchanged.
        assert_eq!(
            escape_module_path("github.com/gin-gonic/gin"),
            "github.com/gin-gonic/gin"
        );
    }

    #[test]
    fn build_update_returns_some_for_upgrade() {
        let update = build("github.com/spf13/cobra", "1.7.0", &body("v1.8.0")).expect("update");
        assert_eq!(update.latest_version, "1.8.0", "must strip v prefix");
        assert_eq!(update.ecosystem, Ecosystem::Go);
        assert_eq!(update.update_type, UpdateType::Minor);
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());
    }

    #[test]
    fn build_update_strips_v_prefix_in_comparison() {
        // current=1.8.0, latest="v1.8.0" → equal after strip → no update.
        let result = build("foo/bar", "1.8.0", &body("v1.8.0"));
        assert!(result.is_none(), "v1.8.0 == 1.8.0 after strip, no update");
    }

    #[test]
    fn build_update_returns_none_when_version_field_missing() {
        assert!(build("foo/bar", "1.0.0", &serde_json::json!({})).is_none());
    }

    #[test]
    fn build_update_returns_none_when_version_empty_after_strip() {
        // A body with `Version: "v"` (just the prefix) becomes empty after
        // strip and must not produce a bogus update.
        assert!(build("foo/bar", "1.0.0", &body("v")).is_none());
    }

    #[test]
    fn build_update_propagates_is_dev_flag() {
        let update = build_update_from_response(
            "github.com/stretchr/testify",
            "1.8.0",
            "go.mod",
            true,
            &body("v1.9.0"),
            fixed_now(),
        )
        .expect("update");
        assert!(update.is_dev);
    }

    #[test]
    fn build_update_uses_capital_version_field() {
        // Go proxy uses `Version` (capital V) not `version`, easy to confuse
        // with other registries that use lowercase. Regression test.
        let body = serde_json::json!({"version": "1.9.0"}); // wrong case
        let result = build("foo/bar", "1.8.0", &body);
        assert!(result.is_none(), "lowercase `version` must not match");
    }

    #[test]
    fn stale_when_latest_published_over_three_years_before_now() {
        // fixed_now is 2026-07-01; Time 2022-01-01 is > 3 years ago.
        let body = serde_json::json!({
            "Version": "v2.0.0",
            "Time": "2022-01-01T00:00:00Z"
        });
        let update = build("foo/bar", "1.0.0", &body).expect("update");
        assert!(update.is_stale);
        assert_eq!(
            update.last_published.as_deref(),
            Some("2022-01-01T00:00:00Z")
        );
        // Stale is informational only - it must not look like a defect.
        assert!(!update.is_security);
        assert!(!update.is_deprecated);
    }

    #[test]
    fn not_stale_when_latest_published_within_three_years() {
        let body = serde_json::json!({
            "Version": "v2.0.0",
            "Time": "2025-01-01T00:00:00Z"
        });
        let update = build("foo/bar", "1.0.0", &body).expect("update");
        assert!(!update.is_stale);
    }

    #[test]
    fn missing_or_unparseable_time_is_not_stale() {
        let update = build("foo/bar", "1.0.0", &body("v2.0.0")).expect("update");
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());

        let body = serde_json::json!({
            "Version": "v2.0.0",
            "Time": "not a timestamp"
        });
        let update = build("foo/bar", "1.0.0", &body).expect("update");
        assert!(!update.is_stale);
    }

    #[test]
    fn build_update_uses_capital_time_field() {
        // Like `Version`, the proxy reports `Time` with a capital T.
        let body = serde_json::json!({
            "Version": "v2.0.0",
            "time": "2020-01-01T00:00:00Z" // wrong case
        });
        let update = build("foo/bar", "1.0.0", &body).expect("update");
        assert!(!update.is_stale, "lowercase `time` must not match");
        assert!(update.last_published.is_none());
    }
}
