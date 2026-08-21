//! Public Packagist client for version, abandonment, and publish-recency facts.
//! Package-level facts come from the first complete p2 entry.

use crate::updates::types::{
    classify_update, Ecosystem, InstalledPackage, PackageUpdate, UpdateType,
};
use reqwest::Client;

const CONCURRENCY_LIMIT: usize = 10;

/// Check Packagist (Composer) registry for latest versions. The bool is true
/// when the census was only partially observed (a fetch failed outright), so
/// absences from `updates` are unproven this sweep.
pub async fn check_updates(packages: &[InstalledPackage]) -> (Vec<PackageUpdate>, bool) {
    let fan_out = super::concurrency::check_registry_updates(packages, CONCURRENCY_LIMIT, |pkg| {
        let client = crate::http_client::client().clone();
        async move { fetch_latest(&client, &pkg.name, &pkg.version, &pkg.source, pkg.is_dev).await }
    })
    .await;
    (fan_out.results, fan_out.failed > 0)
}

/// Return the newest stable Packagist version and optional publish time.
pub(crate) fn find_latest_stable(
    versions: &[serde_json::Value],
) -> Option<(String, Option<String>)> {
    versions.iter().find_map(|entry| {
        let ver = entry.get("version").and_then(|v| v.as_str())?;
        if ver.contains("dev")
            || ver.contains("alpha")
            || ver.contains("beta")
            || ver.contains("RC")
        {
            return None;
        }
        let time = entry
            .get("time")
            .and_then(|t| t.as_str())
            .map(str::to_string);
        Some((ver.trim_start_matches('v').to_string(), time))
    })
}

/// Reads Packagist's abandonment marker from its first complete p2 entry.
pub(crate) fn abandonment_message(versions: &[serde_json::Value]) -> Option<String> {
    let abandoned = versions.first()?.get("abandoned")?;
    match abandoned {
        serde_json::Value::Bool(true) => {
            Some("The maintainer marked this package abandoned.".to_string())
        }
        serde_json::Value::String(replacement) if !replacement.trim().is_empty() => Some(format!(
            "Abandoned by the maintainer. Suggested replacement: {}.",
            replacement.trim()
        )),
        // `abandoned: ""` still means abandoned, just with no replacement named.
        serde_json::Value::String(_) => {
            Some("The maintainer marked this package abandoned.".to_string())
        }
        _ => None,
    }
}

/// Publish `time` of the first (newest) version entry, which is always
/// complete in a minified p2 file.
fn first_publish_time(versions: &[serde_json::Value]) -> Option<String> {
    versions.first()?.get("time")?.as_str().map(str::to_string)
}

/// Build a `PackageUpdate` from the parsed Packagist `/p2/{name}.json` body.
/// Abandoned packages produce an entry without an update; staleness only
/// decorates entries produced for another reason.
pub(crate) fn build_update_from_response(
    name: &str,
    current: &str,
    source: &str,
    is_dev: bool,
    body: &serde_json::Value,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<PackageUpdate> {
    let versions = body
        .get("packages")
        .and_then(|p| p.get(name))
        .and_then(|v| v.as_array())?;

    let deprecation_message = abandonment_message(versions);
    let is_deprecated = deprecation_message.is_some();

    let (latest, stable_time) = match find_latest_stable(versions) {
        Some((version, time)) => (version, time),
        None => (String::new(), None),
    };
    let has_newer = !latest.is_empty() && latest != current;
    if !has_newer && !is_deprecated {
        return None;
    }

    // Publish time of the chosen stable entry; when minification stripped it
    // from that entry, fall back to the first (newest) element's time.
    let last_published = stable_time.or_else(|| first_publish_time(versions));

    Some(PackageUpdate {
        name: name.to_string(),
        current_version: current.to_string(),
        // For an abandoned package with no stable version, anchor the row to
        // the installed version instead of implying an upgrade.
        latest_version: if latest.is_empty() {
            current.to_string()
        } else {
            latest.clone()
        },
        ecosystem: Ecosystem::Composer,
        update_type: if has_newer {
            classify_update(current, &latest)
        } else {
            UpdateType::Unknown
        },
        is_security: false,
        advisory_severity: None,
        advisory_url: None,
        source: source.to_string(),
        is_dev,
        is_deprecated,
        deprecation_message,
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
    // Packagist API: GET https://repo.packagist.org/p2/{vendor}/{package}.json
    let url = format!("https://repo.packagist.org/p2/{}.json", name);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    let status = resp.status();
    if super::status_is_observed_absence(status) {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!("Packagist returned status {} for {}", status, name));
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

    fn body_with_versions(name: &str, versions: Vec<&str>) -> serde_json::Value {
        let entries: Vec<serde_json::Value> = versions
            .into_iter()
            .map(|v| serde_json::json!({"version": v}))
            .collect();
        serde_json::json!({"packages": {name: entries}})
    }

    fn latest_stable_version(versions: &[serde_json::Value]) -> Option<String> {
        find_latest_stable(versions).map(|(version, _)| version)
    }

    #[test]
    fn find_latest_stable_picks_newest_stable() {
        // Packagist returns newest first - pick the first stable.
        let versions = vec![
            serde_json::json!({"version": "5.1.0"}),
            serde_json::json!({"version": "5.0.5"}),
            serde_json::json!({"version": "4.9.0"}),
        ];
        assert_eq!(latest_stable_version(&versions).as_deref(), Some("5.1.0"));
    }

    #[test]
    fn find_latest_stable_strips_v_prefix() {
        let versions = vec![serde_json::json!({"version": "v5.1.0"})];
        assert_eq!(latest_stable_version(&versions).as_deref(), Some("5.1.0"));
    }

    #[test]
    fn find_latest_stable_skips_pre_releases() {
        let versions = vec![
            serde_json::json!({"version": "6.0.0-dev"}),
            serde_json::json!({"version": "6.0.0-alpha1"}),
            serde_json::json!({"version": "6.0.0-beta3"}),
            serde_json::json!({"version": "6.0.0-RC1"}),
            serde_json::json!({"version": "5.1.0"}), // first stable
            serde_json::json!({"version": "5.0.0"}),
        ];
        assert_eq!(latest_stable_version(&versions).as_deref(), Some("5.1.0"));
    }

    #[test]
    fn find_latest_stable_returns_none_when_only_pre_releases() {
        let versions = vec![
            serde_json::json!({"version": "1.0.0-dev"}),
            serde_json::json!({"version": "1.0.0-alpha"}),
        ];
        assert!(find_latest_stable(&versions).is_none());
    }

    #[test]
    fn find_latest_stable_returns_none_for_empty_input() {
        let versions: Vec<serde_json::Value> = Vec::new();
        assert!(find_latest_stable(&versions).is_none());
    }

    #[test]
    fn find_latest_stable_skips_entries_without_version_field() {
        let versions = vec![
            serde_json::json!({"name": "no version field"}),
            serde_json::json!({"version": "5.1.0"}),
        ];
        assert_eq!(latest_stable_version(&versions).as_deref(), Some("5.1.0"));
    }

    #[test]
    fn find_latest_stable_carries_the_chosen_entry_time() {
        let versions = vec![
            serde_json::json!({"version": "6.0.0-beta1", "time": "2026-06-01T00:00:00+00:00"}),
            serde_json::json!({"version": "5.1.0", "time": "2024-03-01T00:00:00+00:00"}),
        ];
        let (version, time) = find_latest_stable(&versions).expect("stable entry");
        assert_eq!(version, "5.1.0");
        assert_eq!(time.as_deref(), Some("2024-03-01T00:00:00+00:00"));
    }

    fn fixed_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .expect("valid fixture timestamp")
            .with_timezone(&chrono::Utc)
    }

    fn build(
        name: &str,
        current: &str,
        is_dev: bool,
        body: &serde_json::Value,
    ) -> Option<PackageUpdate> {
        build_update_from_response(name, current, "composer.json", is_dev, body, fixed_now())
    }

    #[test]
    fn build_update_returns_some_for_real_upgrade() {
        let body = body_with_versions("symfony/console", vec!["7.0.0", "6.4.0"]);
        let update = build("symfony/console", "6.4.0", false, &body).expect("update");
        assert_eq!(update.name, "symfony/console");
        assert_eq!(update.current_version, "6.4.0");
        assert_eq!(update.latest_version, "7.0.0");
        assert_eq!(update.ecosystem, Ecosystem::Composer);
        assert_eq!(update.update_type, UpdateType::Major);
        assert!(!update.is_dev);
        assert!(!update.is_deprecated);
        assert!(update.deprecation_message.is_none());
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());
    }

    #[test]
    fn build_update_returns_none_when_already_latest() {
        let body = body_with_versions("symfony/console", vec!["7.0.0"]);
        assert!(build("symfony/console", "7.0.0", false, &body).is_none());
    }

    #[test]
    fn build_update_returns_none_when_packages_object_missing() {
        assert!(build("symfony/console", "6.0.0", false, &serde_json::json!({})).is_none());
    }

    #[test]
    fn build_update_returns_none_when_package_name_not_in_response() {
        // Packagist sometimes returns a packages object that's missing the
        // requested name (e.g. unpublished / private package).
        let body = serde_json::json!({"packages": {"other/pkg": [{"version": "1.0.0"}]}});
        assert!(build("symfony/console", "6.0.0", false, &body).is_none());
    }

    #[test]
    fn build_update_returns_none_when_only_pre_releases_available() {
        let body = body_with_versions("symfony/console", vec!["8.0.0-beta1", "8.0.0-RC1"]);
        let result = build("symfony/console", "7.0.0", false, &body);
        assert!(result.is_none(), "must not push beta/RC as an update");
    }

    #[test]
    fn build_update_propagates_is_dev_flag() {
        let body = body_with_versions("phpunit/phpunit", vec!["10.5.0"]);
        let update = build("phpunit/phpunit", "10.4.0", true, &body).expect("update");
        assert!(update.is_dev);
    }

    #[test]
    fn build_update_strips_v_prefix_in_comparison() {
        // composer.lock often stores versions as "v1.2.3"; the parser strips
        // the prefix so equal-after-strip is correctly treated as no-op.
        let body = body_with_versions("foo/bar", vec!["v5.0.0"]);
        let result = build("foo/bar", "5.0.0", false, &body);
        assert!(result.is_none(), "v5.0.0 == 5.0.0 after strip: no update");
    }

    #[test]
    fn abandoned_true_flags_deprecated_with_generic_message() {
        let body = serde_json::json!({"packages": {"swiftmailer/swiftmailer": [
            {"version": "6.3.0", "abandoned": true},
            {"version": "6.2.0"},
        ]}});
        let update = build("swiftmailer/swiftmailer", "6.2.0", false, &body).expect("update");
        assert!(update.is_deprecated);
        assert_eq!(
            update.deprecation_message.as_deref(),
            Some("The maintainer marked this package abandoned.")
        );
    }

    #[test]
    fn abandoned_string_names_the_replacement_package() {
        let body = serde_json::json!({"packages": {"swiftmailer/swiftmailer": [
            {"version": "6.3.0", "abandoned": "symfony/mailer"},
        ]}});
        let update = build("swiftmailer/swiftmailer", "6.2.0", false, &body).expect("update");
        assert!(update.is_deprecated);
        assert_eq!(
            update.deprecation_message.as_deref(),
            Some("Abandoned by the maintainer. Suggested replacement: symfony/mailer.")
        );
    }

    #[test]
    fn abandoned_package_surfaces_even_when_already_at_latest() {
        // Mirrors the npm rule: an entry must exist so the UI can show the
        // abandonment even though there is nothing to update to.
        let body = serde_json::json!({"packages": {"foo/bar": [
            {"version": "2.0.0", "abandoned": true},
        ]}});
        let update = build("foo/bar", "2.0.0", false, &body).expect("standalone abandoned entry");
        assert!(update.is_deprecated);
        assert_eq!(update.current_version, "2.0.0");
        assert_eq!(update.latest_version, "2.0.0");
        assert_eq!(update.update_type, UpdateType::Unknown);
    }

    #[test]
    fn abandoned_false_is_not_deprecated() {
        let body = serde_json::json!({"packages": {"foo/bar": [
            {"version": "2.0.0", "abandoned": false},
        ]}});
        let update = build("foo/bar", "1.0.0", false, &body).expect("update");
        assert!(!update.is_deprecated);
    }

    #[test]
    fn abandoned_is_read_from_first_element_only() {
        // p2 minification: only the first (newest) element is guaranteed
        // complete. An `abandoned` on a later element must not be trusted.
        let body = serde_json::json!({"packages": {"foo/bar": [
            {"version": "2.0.0"},
            {"version": "1.0.0", "abandoned": true},
        ]}});
        let update = build("foo/bar", "1.0.0", false, &body).expect("update");
        assert!(!update.is_deprecated);
    }

    #[test]
    fn stale_when_latest_published_over_three_years_before_now() {
        // fixed_now is 2026-07-01; published 2022-01-01 is > 3 years ago.
        let body = serde_json::json!({"packages": {"foo/bar": [
            {"version": "2.0.0", "time": "2022-01-01T00:00:00+00:00"},
        ]}});
        let update = build("foo/bar", "1.0.0", false, &body).expect("update");
        assert!(update.is_stale);
        assert_eq!(
            update.last_published.as_deref(),
            Some("2022-01-01T00:00:00+00:00")
        );
        // Stale is informational only - it must not look like a defect.
        assert!(!update.is_security);
        assert!(!update.is_deprecated);
    }

    #[test]
    fn not_stale_when_latest_published_within_three_years() {
        let body = serde_json::json!({"packages": {"foo/bar": [
            {"version": "2.0.0", "time": "2025-01-01T00:00:00+00:00"},
        ]}});
        let update = build("foo/bar", "1.0.0", false, &body).expect("update");
        assert!(!update.is_stale);
    }

    #[test]
    fn stale_reads_time_from_the_chosen_stable_entry() {
        // The newest entry is a pre-release; the chosen stable is older and
        // carries its own time, which must win over the first element's.
        let body = serde_json::json!({"packages": {"foo/bar": [
            {"version": "3.0.0-beta1", "time": "2026-06-01T00:00:00+00:00"},
            {"version": "2.0.0", "time": "2021-01-01T00:00:00+00:00"},
        ]}});
        let update = build("foo/bar", "1.0.0", false, &body).expect("update");
        assert!(update.is_stale);
        assert_eq!(
            update.last_published.as_deref(),
            Some("2021-01-01T00:00:00+00:00")
        );
    }

    #[test]
    fn stale_falls_back_to_first_element_time_when_minified() {
        // p2 minification can strip `time` off later entries; the first
        // (newest) element is always complete and supplies the fallback.
        let body = serde_json::json!({"packages": {"foo/bar": [
            {"version": "2.0.0", "time": "2021-01-01T00:00:00+00:00"},
            {"version": "1.5.0"},
        ]}});
        let update = build("foo/bar", "1.0.0", false, &body).expect("update");
        assert_eq!(
            update.last_published.as_deref(),
            Some("2021-01-01T00:00:00+00:00")
        );
        assert!(update.is_stale);
    }

    #[test]
    fn missing_or_unparseable_time_is_not_stale() {
        let body = body_with_versions("foo/bar", vec!["2.0.0"]);
        let update = build("foo/bar", "1.0.0", false, &body).expect("update");
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());

        let body = serde_json::json!({"packages": {"foo/bar": [
            {"version": "2.0.0", "time": "not a timestamp"},
        ]}});
        let update = build("foo/bar", "1.0.0", false, &body).expect("update");
        assert!(!update.is_stale);
    }

    #[test]
    fn stale_alone_does_not_create_a_standalone_entry() {
        // Staleness on an up-to-date, non-abandoned package produces no
        // entry: there is no action to take and no update row to attach to.
        let body = serde_json::json!({"packages": {"foo/bar": [
            {"version": "2.0.0", "time": "2020-01-01T00:00:00+00:00"},
        ]}});
        assert!(build("foo/bar", "2.0.0", false, &body).is_none());
    }
}
