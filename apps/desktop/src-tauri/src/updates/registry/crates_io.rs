//! Public crates.io client for versions, publish recency, and installed-version
//! yank state.

use crate::updates::types::{classify_update, Ecosystem, InstalledPackage, PackageUpdate};
use reqwest::Client;

const CONCURRENCY_LIMIT: usize = 10;

/// Check crates.io for latest versions. The bool is true when the census was
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

/// The `versions[]` entry whose `num` matches `version`, when present.
fn version_entry<'a>(body: &'a serde_json::Value, version: &str) -> Option<&'a serde_json::Value> {
    body.get("versions")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("num").and_then(|n| n.as_str()) == Some(version))
}

/// Build a `PackageUpdate` from the parsed crates.io response. Prefers
/// `max_stable_version`, falling back to `max_version` when necessary.
/// Recency and yank state decorate entries but do not create them.
pub(crate) fn build_update_from_response(
    name: &str,
    current: &str,
    source: &str,
    is_dev: bool,
    body: &serde_json::Value,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<PackageUpdate> {
    let latest = body
        .get("crate")
        .and_then(|c| c.get("max_stable_version"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            body.get("crate")
                .and_then(|c| c.get("max_version"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("");

    if latest.is_empty() || latest == current {
        return None;
    }

    // `created_at` of the chosen latest version feeds staleness. A yanked
    // installed version maps to `current_version_deprecated` only:
    // `max_stable_version` already excludes yanked releases, and crates.io
    // has no crate-level deprecation, so `is_deprecated` stays false.
    let last_published = version_entry(body, latest)
        .and_then(|entry| entry.get("created_at"))
        .and_then(|t| t.as_str())
        .map(str::to_string);
    let current_yanked = version_entry(body, current)
        .and_then(|entry| entry.get("yanked"))
        .and_then(|y| y.as_bool())
        .unwrap_or(false);

    Some(PackageUpdate {
        name: name.to_string(),
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        ecosystem: Ecosystem::Rust,
        update_type: classify_update(current, latest),
        is_security: false,
        advisory_severity: None,
        advisory_url: None,
        source: source.to_string(),
        is_dev,
        current_version_deprecated: current_yanked,
        is_stale: super::is_stale_at(last_published.as_deref(), now),
        last_published,
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
    let url = format!("https://crates.io/api/v1/crates/{}", name);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if super::status_is_observed_absence(status) {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!("crates.io returned status {} for {}", status, name));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
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

    fn body_with(stable: Option<&str>, max: Option<&str>) -> serde_json::Value {
        let mut crate_obj = serde_json::Map::new();
        if let Some(s) = stable {
            crate_obj.insert(
                "max_stable_version".into(),
                serde_json::Value::String(s.into()),
            );
        }
        if let Some(m) = max {
            crate_obj.insert("max_version".into(), serde_json::Value::String(m.into()));
        }
        serde_json::json!({"crate": serde_json::Value::Object(crate_obj)})
    }

    fn fixed_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .expect("valid fixture timestamp")
            .with_timezone(&chrono::Utc)
    }

    fn build(name: &str, current: &str, body: &serde_json::Value) -> Option<PackageUpdate> {
        build_update_from_response(name, current, "Cargo.toml", false, body, fixed_now())
    }

    #[test]
    fn build_update_returns_some_for_upgrade() {
        let update = build(
            "tokio",
            "1.40.0",
            &body_with(Some("1.45.0"), Some("1.46.0-rc.1")),
        )
        .expect("update");
        assert_eq!(update.latest_version, "1.45.0");
        assert_eq!(update.ecosystem, Ecosystem::Rust);
        assert_eq!(update.update_type, UpdateType::Minor);
        assert!(!update.is_deprecated);
        assert!(!update.current_version_deprecated);
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());
    }

    #[test]
    fn build_update_prefers_max_stable_over_max_version() {
        let update = build(
            "serde",
            "1.0.150",
            &body_with(Some("1.0.200"), Some("2.0.0-alpha.1")),
        )
        .expect("update");
        assert_eq!(
            update.latest_version, "1.0.200",
            "must pick max_stable_version when both fields exist",
        );
    }

    #[test]
    fn build_update_falls_back_to_max_version_when_no_stable() {
        // For brand-new crates that have only published pre-releases.
        let update =
            build("newcrate", "0.0.1", &body_with(None, Some("0.1.0-alpha.1"))).expect("update");
        assert_eq!(update.latest_version, "0.1.0-alpha.1");
    }

    #[test]
    fn build_update_returns_none_when_versions_match() {
        assert!(build("tokio", "1.45.0", &body_with(Some("1.45.0"), None)).is_none());
    }

    #[test]
    fn build_update_returns_none_when_crate_object_missing() {
        assert!(build("tokio", "1.0.0", &serde_json::json!({})).is_none());
    }

    #[test]
    fn build_update_returns_none_when_neither_version_field_present() {
        assert!(build("tokio", "1.0.0", &serde_json::json!({"crate": {}})).is_none());
    }

    #[test]
    fn build_update_returns_none_when_stable_field_empty_string() {
        assert!(build("tokio", "1.0.0", &body_with(Some(""), None)).is_none());
    }

    #[test]
    fn build_update_propagates_is_dev_flag() {
        let update = build_update_from_response(
            "criterion",
            "0.5.0",
            "Cargo.toml",
            true,
            &body_with(Some("0.6.0"), None),
            fixed_now(),
        )
        .expect("update");
        assert!(update.is_dev);
    }

    #[test]
    fn yanked_current_version_sets_current_version_deprecated() {
        let body = serde_json::json!({
            "crate": {"max_stable_version": "1.2.0"},
            "versions": [
                {"num": "1.2.0", "yanked": false},
                {"num": "1.1.0", "yanked": true},
            ]
        });
        let update = build("mycrate", "1.1.0", &body).expect("update");
        assert!(update.current_version_deprecated);
        assert!(
            !update.is_deprecated,
            "a yank is version-scoped: crates.io has no crate-level deprecation"
        );
        assert!(update.deprecation_message.is_none());
    }

    #[test]
    fn unyanked_current_version_is_not_flagged() {
        let body = serde_json::json!({
            "crate": {"max_stable_version": "1.2.0"},
            "versions": [
                {"num": "1.2.0", "yanked": false},
                {"num": "1.1.0", "yanked": false},
            ]
        });
        let update = build("mycrate", "1.1.0", &body).expect("update");
        assert!(!update.current_version_deprecated);
    }

    #[test]
    fn missing_versions_array_means_no_yank_and_no_staleness() {
        let update = build("tokio", "1.40.0", &body_with(Some("1.45.0"), None)).expect("update");
        assert!(!update.current_version_deprecated);
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());
    }

    #[test]
    fn stale_when_latest_created_over_three_years_before_now() {
        // fixed_now is 2026-07-01; created 2022-01-01 is > 3 years ago.
        let body = serde_json::json!({
            "crate": {"max_stable_version": "1.2.0"},
            "versions": [
                {"num": "1.2.0", "yanked": false, "created_at": "2022-01-01T00:00:00.000000+00:00"},
            ]
        });
        let update = build("mycrate", "1.0.0", &body).expect("update");
        assert!(update.is_stale);
        assert_eq!(
            update.last_published.as_deref(),
            Some("2022-01-01T00:00:00.000000+00:00")
        );
        // Stale is informational only - it must not look like a defect.
        assert!(!update.is_security);
    }

    #[test]
    fn not_stale_when_latest_created_within_three_years() {
        let body = serde_json::json!({
            "crate": {"max_stable_version": "1.2.0"},
            "versions": [
                {"num": "1.2.0", "created_at": "2025-01-01T00:00:00.000000+00:00"},
            ]
        });
        let update = build("mycrate", "1.0.0", &body).expect("update");
        assert!(!update.is_stale);
    }

    #[test]
    fn unparseable_created_at_is_not_stale() {
        let body = serde_json::json!({
            "crate": {"max_stable_version": "1.2.0"},
            "versions": [
                {"num": "1.2.0", "created_at": "not a timestamp"},
            ]
        });
        let update = build("mycrate", "1.0.0", &body).expect("update");
        assert!(!update.is_stale);
    }

    #[test]
    fn staleness_reads_created_at_of_the_chosen_latest_version() {
        // The versions array lists newest first, but the chosen latest is
        // matched by `num`, not position.
        let body = serde_json::json!({
            "crate": {"max_stable_version": "1.2.0"},
            "versions": [
                {"num": "2.0.0-rc.1", "created_at": "2026-06-01T00:00:00.000000+00:00"},
                {"num": "1.2.0", "created_at": "2021-01-01T00:00:00.000000+00:00"},
            ]
        });
        let update = build("mycrate", "1.0.0", &body).expect("update");
        assert_eq!(
            update.last_published.as_deref(),
            Some("2021-01-01T00:00:00.000000+00:00")
        );
        assert!(update.is_stale);
    }
}
