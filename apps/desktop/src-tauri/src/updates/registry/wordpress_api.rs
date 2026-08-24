//! Public WordPress.org client for core, plugin, and theme versions. Plugin
//! responses also provide closure and publish-recency facts.

use crate::updates::types::{
    classify_update, Ecosystem, InstalledPackage, PackageUpdate, UpdateType,
};
use reqwest::Client;

const CONCURRENCY_LIMIT: usize = 5; // WordPress.org API is slower

/// Message shown for plugins closed on WordPress.org.
const CLOSED_PLUGIN_MESSAGE: &str =
    "This plugin was closed on WordPress.org and no longer receives updates.";

/// Return WordPress updates plus whether any package fetch was unobserved.
/// Partial results must not resolve existing update or vulnerability findings.
pub async fn check_updates(packages: &[InstalledPackage]) -> (Vec<PackageUpdate>, bool) {
    // Core uses its own endpoint outside the plugin concurrency pool.
    let mut core_handles = Vec::new();
    for pkg in packages {
        if pkg.name == "wordpress" {
            let client = crate::http_client::client().clone();
            let version = pkg.version.clone();
            let source = pkg.source.clone();
            core_handles.push(tokio::spawn(async move {
                check_core(&client, &version, &source).await
            }));
        }
    }

    let plugins: Vec<InstalledPackage> = packages
        .iter()
        .filter(|pkg| pkg.name != "wordpress" && !pkg.source.contains("themes"))
        .cloned()
        .collect();

    let fan_out = super::concurrency::check_registry_updates(&plugins, CONCURRENCY_LIMIT, |pkg| {
        let client = crate::http_client::client().clone();
        async move { fetch_plugin_latest(&client, &pkg.name, &pkg.version, &pkg.source).await }
    })
    .await;
    let mut updates = fan_out.results;
    let mut partial = fan_out.failed > 0;

    for handle in core_handles {
        match handle.await {
            Ok(Ok(Some(update))) => updates.push(update),
            Ok(Ok(None)) => {}
            // A failed core version-check leaves WordPress core unobserved:
            // the ecosystem's census is partial, mirroring the plugin fan-out.
            Ok(Err(e)) => {
                tracing::warn!("updates: WordPress core check failed: {}", e);
                partial = true;
            }
            Err(e) => {
                tracing::warn!("updates: WordPress core check task died: {}", e);
                partial = true;
            }
        }
    }
    (updates, partial)
}

/// Whether the WordPress.org `last_updated` display string (e.g.
/// "2025-06-10 6:26pm GMT") is stale. The leading `YYYY-MM-DD` date is
/// parsed defensively - anything unparseable is not stale - and
/// compared at midnight UTC.
fn last_updated_is_stale(last_updated: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> bool {
    let Some(raw) = last_updated else {
        return false;
    };
    let Some(date_part) = raw.get(..10) else {
        return false;
    };
    let Ok(date) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") else {
        return false;
    };
    let Some(midnight) = date.and_hms_opt(0, 0, 0) else {
        return false;
    };
    super::published_is_stale(midnight.and_utc(), now)
}

/// Build an update from WordPress plugin metadata.
///
/// Returns `None` when the version is current and the plugin is not explicitly
/// closed. `now` is injected for deterministic staleness checks.
pub(crate) fn build_plugin_update_from_response(
    slug: &str,
    current: &str,
    source: &str,
    body: &serde_json::Value,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<PackageUpdate> {
    let closed = body.get("closed").and_then(|c| c.as_bool()) == Some(true);

    let latest = body.get("version").and_then(|v| v.as_str()).unwrap_or("");
    let has_newer = !latest.is_empty() && latest != current;
    if !has_newer && !closed {
        return None;
    }

    let last_updated = body.get("last_updated").and_then(|v| v.as_str());

    Some(PackageUpdate {
        name: slug.to_string(),
        current_version: current.to_string(),
        // A closed plugin's error payload has no version info; anchor the
        // row to the installed version instead of implying an upgrade.
        latest_version: if latest.is_empty() { current } else { latest }.to_string(),
        ecosystem: Ecosystem::WordPress,
        update_type: if has_newer {
            classify_update(current, latest)
        } else {
            UpdateType::Unknown
        },
        is_security: false,
        advisory_severity: None,
        advisory_url: None,
        source: source.to_string(),
        is_dev: false,
        is_deprecated: closed,
        deprecation_message: closed.then(|| CLOSED_PLUGIN_MESSAGE.to_string()),
        is_stale: last_updated_is_stale(last_updated, now),
        // Stored raw: WordPress.org reports a display string, not ISO 8601.
        last_published: last_updated.map(str::to_string),
        ..Default::default()
    })
}

/// Build a WordPress core update from the first recommended offer.
/// Core releases do not receive package-staleness findings.
pub(crate) fn build_core_update_from_response(
    current: &str,
    source: &str,
    body: &serde_json::Value,
) -> Option<PackageUpdate> {
    let latest = body
        .get("offers")
        .and_then(|o| o.as_array())
        .and_then(|a| a.first())
        .and_then(|o| o.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if latest.is_empty() || latest == current {
        return None;
    }
    Some(PackageUpdate {
        name: "wordpress".to_string(),
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        ecosystem: Ecosystem::WordPress,
        update_type: classify_update(current, latest),
        is_security: false,
        advisory_severity: None,
        advisory_url: None,
        source: source.to_string(),
        is_dev: false,
        ..Default::default()
    })
}

async fn fetch_plugin_latest(
    client: &Client,
    slug: &str,
    current: &str,
    source: &str,
) -> Result<Option<PackageUpdate>, String> {
    let url = "https://api.wordpress.org/plugins/info/1.2/";
    let resp = client
        .get(url)
        .query(&[("action", "plugin_information"), ("slug", slug)])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if super::status_is_observed_absence(status) {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!(
            "WordPress.org plugin API returned status {} for {}",
            status, slug
        ));
    }
    let body: serde_json::Value = crate::http_client::read_json_limited(
        resp,
        crate::constants::WORDPRESS_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await?;
    Ok(build_plugin_update_from_response(
        slug,
        current,
        source,
        &body,
        chrono::Utc::now(),
    ))
}

async fn check_core(
    client: &Client,
    current: &str,
    source: &str,
) -> Result<Option<PackageUpdate>, String> {
    let url = "https://api.wordpress.org/core/version-check/1.7/";
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if super::status_is_observed_absence(status) {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!(
            "WordPress.org version-check returned status {}",
            status
        ));
    }
    let body: serde_json::Value = crate::http_client::read_json_limited(
        resp,
        crate::constants::WORDPRESS_API_RESPONSE_MAX_BYTES,
        crate::constants::BODY_READ_TIMEOUT,
    )
    .await?;
    Ok(build_core_update_from_response(current, source, &body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updates::types::UpdateType;

    fn fixed_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .expect("valid fixture timestamp")
            .with_timezone(&chrono::Utc)
    }

    fn build_plugin(slug: &str, current: &str, body: &serde_json::Value) -> Option<PackageUpdate> {
        build_plugin_update_from_response(
            slug,
            current,
            "wp-content/plugins/wordpress-seo",
            body,
            fixed_now(),
        )
    }

    #[test]
    fn plugin_update_returns_some_for_upgrade() {
        let body = serde_json::json!({"version": "3.0.0"});
        let update = build_plugin("wordpress-seo", "2.5.0", &body).expect("update");
        assert_eq!(update.name, "wordpress-seo");
        assert_eq!(update.latest_version, "3.0.0");
        assert_eq!(update.ecosystem, Ecosystem::WordPress);
        assert_eq!(update.update_type, UpdateType::Major);
        assert!(!update.is_dev);
        assert!(!update.is_deprecated);
        assert!(update.deprecation_message.is_none());
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());
    }

    #[test]
    fn plugin_update_returns_none_when_versions_match() {
        let body = serde_json::json!({"version": "3.0.0"});
        assert!(build_plugin("wordpress-seo", "3.0.0", &body).is_none());
    }

    #[test]
    fn plugin_update_returns_none_when_version_field_missing() {
        assert!(build_plugin("wordpress-seo", "3.0.0", &serde_json::json!({})).is_none());
    }

    #[test]
    fn plugin_update_classifies_minor_correctly() {
        let body = serde_json::json!({"version": "2.6.0"});
        let update = build_plugin("wordpress-seo", "2.5.0", &body).expect("update");
        assert_eq!(update.update_type, UpdateType::Minor);
    }

    #[test]
    fn closed_plugin_is_flagged_deprecated_with_message() {
        // A closed plugin answers with an error payload instead of info.
        let body = serde_json::json!({
            "error": "closed",
            "closed": true,
            "closed_date": "2024-03-15",
            "description": "This plugin has been closed as of March 15, 2024."
        });
        let update = build_plugin("old-plugin", "1.2.0", &body).expect("closed entry");
        assert!(update.is_deprecated);
        assert_eq!(
            update.deprecation_message.as_deref(),
            Some("This plugin was closed on WordPress.org and no longer receives updates.")
        );
    }

    #[test]
    fn closed_plugin_surfaces_even_with_no_version_info() {
        // Mirrors the npm no-newer rule: the closed payload carries no
        // `version`, so the entry anchors to the installed version.
        let body = serde_json::json!({"error": "closed", "closed": true});
        let update = build_plugin("old-plugin", "1.2.0", &body).expect("closed entry");
        assert_eq!(update.current_version, "1.2.0");
        assert_eq!(update.latest_version, "1.2.0");
        assert_eq!(update.update_type, UpdateType::Unknown);
    }

    #[test]
    fn error_response_without_closed_true_produces_no_entry() {
        // Bad slug / API errors must not masquerade as closures.
        let body = serde_json::json!({"error": "Plugin not found."});
        assert!(build_plugin("no-such-plugin", "1.0.0", &body).is_none());

        let body = serde_json::json!({"error": "closed", "closed": false});
        assert!(build_plugin("still-open", "1.0.0", &body).is_none());

        // `closed` as a non-boolean must not count either.
        let body = serde_json::json!({"error": "closed", "closed": "true"});
        assert!(build_plugin("stringly-closed", "1.0.0", &body).is_none());
    }

    /// WordPress.org `last_updated` fixture dated `days_ago` before
    /// fixed_now, in the API's display format.
    fn wp_last_updated(days_ago: i64) -> String {
        let date = (fixed_now() - chrono::Duration::days(days_ago)).format("%Y-%m-%d");
        format!("{} 6:26pm GMT", date)
    }

    #[test]
    fn stale_when_last_updated_over_three_years_before_now() {
        let raw = wp_last_updated(crate::updates::registry::STALE_AFTER_DAYS + 1);
        let body = serde_json::json!({"version": "3.0.0", "last_updated": raw});
        let update = build_plugin("wordpress-seo", "2.5.0", &body).expect("update");
        assert!(update.is_stale);
        assert_eq!(
            update.last_published.as_deref(),
            Some(raw.as_str()),
            "last_published stores the raw display string"
        );
        // Stale is informational only - it must not look like a defect.
        assert!(!update.is_security);
        assert!(!update.is_deprecated);
    }

    #[test]
    fn not_stale_when_last_updated_within_three_years() {
        // fixed_now is midnight UTC, so a date exactly STALE_AFTER_DAYS ago
        // sits exactly at the threshold: not stale (strict boundary).
        let body = serde_json::json!({
            "version": "3.0.0",
            "last_updated": wp_last_updated(crate::updates::registry::STALE_AFTER_DAYS)
        });
        let update = build_plugin("wordpress-seo", "2.5.0", &body).expect("update");
        assert!(!update.is_stale);
    }

    #[test]
    fn missing_or_unparseable_last_updated_is_not_stale() {
        let body = serde_json::json!({"version": "3.0.0"});
        let update = build_plugin("wordpress-seo", "2.5.0", &body).expect("update");
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());

        // Not a leading YYYY-MM-DD date: parsed defensively, not stale.
        let body = serde_json::json!({"version": "3.0.0", "last_updated": "a while ago"});
        let update = build_plugin("wordpress-seo", "2.5.0", &body).expect("update");
        assert!(!update.is_stale);
        assert_eq!(update.last_published.as_deref(), Some("a while ago"));

        // Shorter than 10 chars must not panic or flag.
        let body = serde_json::json!({"version": "3.0.0", "last_updated": "2020"});
        let update = build_plugin("wordpress-seo", "2.5.0", &body).expect("update");
        assert!(!update.is_stale);
    }

    #[test]
    fn stale_alone_does_not_create_a_standalone_entry() {
        // Staleness only decorates an update entry; it does not create one.
        let body = serde_json::json!({
            "version": "3.0.0",
            "last_updated": "2019-01-01 6:26pm GMT"
        });
        assert!(build_plugin("wordpress-seo", "3.0.0", &body).is_none());
    }

    #[test]
    fn core_update_returns_some_for_upgrade() {
        // WordPress core uses `offers[0].version` (not a top-level field).
        let body = serde_json::json!({
            "offers": [{"version": "6.5.0"}]
        });
        let update =
            build_core_update_from_response("6.4.0", "wp-config.php", &body).expect("update");
        assert_eq!(
            update.name, "wordpress",
            "core update name MUST be 'wordpress'"
        );
        assert_eq!(update.latest_version, "6.5.0");
        assert_eq!(update.ecosystem, Ecosystem::WordPress);
    }

    #[test]
    fn core_update_uses_first_offer_when_multiple() {
        // The version-check API returns multiple offers (e.g. minor + major
        // upgrades). The first is the recommended target.
        let body = serde_json::json!({
            "offers": [
                {"version": "6.5.5"}, // recommended (latest patch in current major)
                {"version": "7.0.0"}, // available major upgrade
            ]
        });
        let update =
            build_core_update_from_response("6.5.0", "wp-config.php", &body).expect("update");
        assert_eq!(update.latest_version, "6.5.5");
    }

    #[test]
    fn core_update_returns_none_when_versions_match() {
        let body = serde_json::json!({"offers": [{"version": "6.5.0"}]});
        let result = build_core_update_from_response("6.5.0", "wp-config.php", &body);
        assert!(result.is_none());
    }

    #[test]
    fn core_update_returns_none_when_offers_array_empty() {
        let body = serde_json::json!({"offers": []});
        let result = build_core_update_from_response("6.5.0", "wp-config.php", &body);
        assert!(result.is_none());
    }

    #[test]
    fn core_update_returns_none_when_offers_missing() {
        let body = serde_json::json!({});
        let result = build_core_update_from_response("6.5.0", "wp-config.php", &body);
        assert!(result.is_none());
    }

    #[test]
    fn core_update_returns_none_when_offer_missing_version_field() {
        let body = serde_json::json!({"offers": [{"download": "https://..."}]});
        let result = build_core_update_from_response("6.5.0", "wp-config.php", &body);
        assert!(result.is_none());
    }
}
