//! PyPI package updates, release recency, and installed-version yank state.
//!
//! PyPI omits yanked releases from `info.version`, so yanks mark only the
//! installed version as deprecated.

use crate::updates::types::{classify_update, Ecosystem, InstalledPackage, PackageUpdate};
use reqwest::Client;

const CONCURRENCY_LIMIT: usize = 10;

/// Check PyPI for latest versions. The bool is true when the census was only
/// partially observed (a fetch failed outright), so absences from `updates`
/// are unproven this sweep.
pub async fn check_updates(packages: &[InstalledPackage]) -> (Vec<PackageUpdate>, bool) {
    let fan_out = super::concurrency::check_registry_updates(packages, CONCURRENCY_LIMIT, |pkg| {
        let client = crate::http_client::client().clone();
        async move { fetch_latest(&client, &pkg.name, &pkg.version, &pkg.source, pkg.is_dev).await }
    })
    .await;
    (fan_out.results, fan_out.failed > 0)
}

/// Publish timestamp of the latest release: `urls[]` lists the files of the
/// release `info.version` points at, each carrying `upload_time_iso_8601`.
/// Takes the first file that has one.
fn latest_upload_time(body: &serde_json::Value) -> Option<String> {
    body.get("urls")?.as_array()?.iter().find_map(|file| {
        file.get("upload_time_iso_8601")?
            .as_str()
            .map(str::to_string)
    })
}

/// True only when the installed release has files and all are yanked.
fn release_is_yanked(body: &serde_json::Value, version: &str) -> bool {
    let Some(files) = body
        .get("releases")
        .and_then(|r| r.get(version))
        .and_then(|f| f.as_array())
    else {
        return false;
    };
    if files.is_empty() {
        return false;
    }
    files.iter().all(|file| {
        file.get("yanked")
            .and_then(|y| y.as_bool())
            .unwrap_or(false)
    })
}

/// Build an update from PyPI's declared latest version and optional metadata.
/// `now` is injected for deterministic staleness checks.
pub(crate) fn build_update_from_response(
    name: &str,
    current: &str,
    source: &str,
    is_dev: bool,
    body: &serde_json::Value,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<PackageUpdate> {
    let latest = body
        .get("info")
        .and_then(|i| i.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if latest.is_empty() || latest == current {
        return None;
    }

    let last_published = latest_upload_time(body);

    Some(PackageUpdate {
        name: name.to_string(),
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        ecosystem: Ecosystem::Python,
        update_type: classify_update(current, latest),
        is_security: false,
        advisory_severity: None,
        advisory_url: None,
        source: source.to_string(),
        is_dev,
        current_version_deprecated: release_is_yanked(body, current),
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
    // PyPI JSON API: GET https://pypi.org/pypi/{package}/json
    let url = format!("https://pypi.org/pypi/{}/json", name);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    let status = resp.status();
    if super::status_is_observed_absence(status) {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!("PyPI returned status {} for {}", status, name));
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

    fn body_with_version(version: &str) -> serde_json::Value {
        serde_json::json!({"info": {"version": version}})
    }

    fn fixed_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .expect("valid fixture timestamp")
            .with_timezone(&chrono::Utc)
    }

    fn build(name: &str, current: &str, body: &serde_json::Value) -> Option<PackageUpdate> {
        build_update_from_response(name, current, "requirements.txt", false, body, fixed_now())
    }

    #[test]
    fn build_update_returns_some_for_real_upgrade() {
        let update = build("django", "4.2.0", &body_with_version("5.0.0")).expect("update");
        assert_eq!(update.name, "django");
        assert_eq!(update.current_version, "4.2.0");
        assert_eq!(update.latest_version, "5.0.0");
        assert_eq!(update.ecosystem, Ecosystem::Python);
        assert_eq!(update.update_type, UpdateType::Major);
        assert!(!update.is_dev);
        assert!(!update.current_version_deprecated);
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());
    }

    #[test]
    fn build_update_propagates_is_dev_flag() {
        let update = build_update_from_response(
            "pytest",
            "8.0.0",
            "requirements-dev.txt",
            true,
            &body_with_version("8.0.1"),
            fixed_now(),
        )
        .expect("update");
        assert!(update.is_dev);
    }

    #[test]
    fn build_update_returns_none_when_versions_match() {
        // Already up to date - no spurious "update available" notification.
        assert!(build("django", "5.0.0", &body_with_version("5.0.0")).is_none());
    }

    #[test]
    fn build_update_returns_none_when_info_object_missing() {
        // Defensive against PyPI responses that lack `info`.
        assert!(build("django", "5.0.0", &serde_json::json!({})).is_none());
    }

    #[test]
    fn build_update_returns_none_when_version_field_missing() {
        assert!(build("django", "5.0.0", &serde_json::json!({"info": {}})).is_none());
    }

    #[test]
    fn build_update_classifies_minor_correctly() {
        let update = build("django", "5.0.0", &body_with_version("5.1.0")).expect("update");
        assert_eq!(update.update_type, UpdateType::Minor);
    }

    #[test]
    fn build_update_classifies_patch_correctly() {
        let update = build("django", "5.0.0", &body_with_version("5.0.1")).expect("update");
        assert_eq!(update.update_type, UpdateType::Patch);
    }

    #[test]
    fn build_update_reports_downgrade_as_update_intentionally() {
        let result = build("django", "5.0.0", &body_with_version("4.2.0"));
        assert!(
            result.is_some(),
            "current PyPI implementation forwards downgrade-looking versions through; see docstring",
        );
    }

    #[test]
    fn fully_yanked_current_release_sets_current_version_deprecated() {
        let body = serde_json::json!({
            "info": {"version": "5.0.0"},
            "releases": {
                "4.2.0": [
                    {"yanked": true},
                    {"yanked": true},
                ]
            }
        });
        let update = build("django", "4.2.0", &body).expect("update");
        assert!(update.current_version_deprecated);
        assert!(
            !update.is_deprecated,
            "PyPI's info.version skips yanked releases; a yank is not a package-level deprecation"
        );
    }

    #[test]
    fn partially_yanked_release_is_not_flagged() {
        // Only a release whose EVERY file entry is yanked counts as yanked.
        let body = serde_json::json!({
            "info": {"version": "5.0.0"},
            "releases": {
                "4.2.0": [
                    {"yanked": true},
                    {"yanked": false},
                ]
            }
        });
        let update = build("django", "4.2.0", &body).expect("update");
        assert!(!update.current_version_deprecated);
    }

    #[test]
    fn empty_or_missing_file_list_means_unknown_not_yanked() {
        let body = serde_json::json!({
            "info": {"version": "5.0.0"},
            "releases": {"4.2.0": []}
        });
        let update = build("django", "4.2.0", &body).expect("update");
        assert!(!update.current_version_deprecated);

        let body = serde_json::json!({"info": {"version": "5.0.0"}});
        let update = build("django", "4.2.0", &body).expect("update");
        assert!(!update.current_version_deprecated);
    }

    #[test]
    fn stale_when_latest_uploaded_over_three_years_before_now() {
        // fixed_now is 2026-07-01; uploaded 2022-01-01 is > 3 years ago.
        let body = serde_json::json!({
            "info": {"version": "5.0.0"},
            "urls": [{"upload_time_iso_8601": "2022-01-01T00:00:00.000000Z"}]
        });
        let update = build("django", "4.2.0", &body).expect("update");
        assert!(update.is_stale);
        assert_eq!(
            update.last_published.as_deref(),
            Some("2022-01-01T00:00:00.000000Z")
        );
        // Stale is informational only - it must not look like a defect.
        assert!(!update.is_security);
    }

    #[test]
    fn not_stale_when_latest_uploaded_within_three_years() {
        let body = serde_json::json!({
            "info": {"version": "5.0.0"},
            "urls": [{"upload_time_iso_8601": "2025-01-01T00:00:00.000000Z"}]
        });
        let update = build("django", "4.2.0", &body).expect("update");
        assert!(!update.is_stale);
    }

    #[test]
    fn takes_first_available_upload_time_across_files() {
        // The first file entry may lack the field (defensive); the next one
        // that has it supplies the timestamp.
        let body = serde_json::json!({
            "info": {"version": "5.0.0"},
            "urls": [
                {"filename": "pkg.tar.gz"},
                {"upload_time_iso_8601": "2021-06-15T00:00:00.000000Z"},
            ]
        });
        let update = build("django", "4.2.0", &body).expect("update");
        assert_eq!(
            update.last_published.as_deref(),
            Some("2021-06-15T00:00:00.000000Z")
        );
        assert!(update.is_stale);
    }

    #[test]
    fn missing_or_unparseable_upload_time_is_not_stale() {
        let body = serde_json::json!({"info": {"version": "5.0.0"}, "urls": []});
        let update = build("django", "4.2.0", &body).expect("update");
        assert!(!update.is_stale);
        assert!(update.last_published.is_none());

        let body = serde_json::json!({
            "info": {"version": "5.0.0"},
            "urls": [{"upload_time_iso_8601": "not a timestamp"}]
        });
        let update = build("django", "4.2.0", &body).expect("update");
        assert!(!update.is_stale);
    }
}
