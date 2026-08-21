//! Public npm registry client for versions, deprecation, and publish recency.

use super::npm_packument;
use crate::updates::types::{
    classify_update, Ecosystem, InstallScriptPackage, InstalledPackage, PackageLicense,
    PackageUpdate, UpdateType,
};
use reqwest::Client;

const CONCURRENCY_LIMIT: usize = 10;

const NPM_REGISTRY_BASE: &str = "https://registry.npmjs.org";

/// Everything the npm client learns from one pass over the packuments:
/// available updates plus the install-script and license posture of every
/// scanned package (including up-to-date ones, which never appear in
/// `updates`).
#[derive(Default)]
pub struct NpmScan {
    pub updates: Vec<PackageUpdate>,
    pub install_script_packages: Vec<InstallScriptPackage>,
    pub licenses: Vec<PackageLicense>,
    /// True when failed packument fetches make absences unproven. A 404 is an
    /// observed absence and does not set this flag.
    pub partial: bool,
}

/// What one packument fetch yields for one package.
struct PackageFinding {
    update: Option<PackageUpdate>,
    install_scripts: Option<InstallScriptPackage>,
    license: Option<PackageLicense>,
}

/// Check npm registry for latest versions plus install-script and license
/// posture.
pub async fn check_updates(packages: &[InstalledPackage]) -> NpmScan {
    check_updates_at(packages, NPM_REGISTRY_BASE).await
}

/// [`check_updates`] with an injectable API base so tests can drive the
/// outage / unknown-package paths against a local server instead of
/// registry.npmjs.org.
async fn check_updates_at(packages: &[InstalledPackage], api_base: &str) -> NpmScan {
    let mut scan = NpmScan::default();

    // Retain non-version findings for current packages and mark registry
    // failures partial so outages cannot read as clean scans.
    let fan_out = super::concurrency::check_registry_updates(packages, CONCURRENCY_LIMIT, |pkg| {
        let client = crate::http_client::client().clone();
        let api_base = api_base.to_string();
        async move { fetch_package(&client, &api_base, &pkg).await }
    })
    .await;
    scan.partial = fan_out.failed > 0;

    for finding in fan_out.results {
        if let Some(update) = finding.update {
            scan.updates.push(update);
        }
        if let Some(install_pkg) = finding.install_scripts {
            scan.install_script_packages.push(install_pkg);
        }
        if let Some(license) = finding.license {
            scan.licenses.push(license);
        }
    }

    // Stable order so downstream copy built from these lists does not churn
    // between polls just because task completion order changed.
    scan.install_script_packages
        .sort_by(|a, b| a.name.cmp(&b.name));
    scan.licenses.sort_by(|a, b| a.name.cmp(&b.name));
    scan
}

/// URL-encode an npm package name for the registry. Scoped packages (e.g.
/// `@vitejs/plugin-react`) need the `/` percent-encoded as `%2f`; everything
/// else is passed through unchanged.
pub(crate) fn encode_package_name(name: &str) -> String {
    if name.starts_with('@') {
        name.replace('/', "%2f")
    } else {
        name.to_string()
    }
}

/// Build an update from an npm packument.
/// Deprecated packages produce an entry even without a newer version.
pub(crate) fn build_update_from_packument(
    pkg: &InstalledPackage,
    body: &serde_json::Value,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<PackageUpdate> {
    let name = pkg.name.as_str();
    let current = pkg.version.as_str();
    let facts = npm_packument::extract_packument_facts(body, current);
    let latest = facts.latest_version.as_deref().unwrap_or("");

    let has_newer = !latest.is_empty()
        && latest != current
        && classify_update(current, latest) != UpdateType::Unknown
        && is_newer(current, latest);

    let is_deprecated = facts.latest_deprecation.is_some();
    if !has_newer && !is_deprecated {
        return None;
    }

    Some(PackageUpdate {
        name: name.to_string(),
        current_version: current.to_string(),
        // For a deprecated package with no newer version, keep the row
        // anchored to the registry's latest (or the installed version when
        // the dist-tag is missing) instead of implying an upgrade.
        latest_version: if latest.is_empty() { current } else { latest }.to_string(),
        ecosystem: Ecosystem::Npm,
        update_type: if has_newer {
            classify_update(current, latest)
        } else {
            UpdateType::Unknown
        },
        is_security: false,
        advisory_severity: None,
        advisory_url: None,
        advisory_fixed_version: None,
        source: pkg.source.clone(),
        is_dev: pkg.is_dev,
        is_deprecated,
        deprecation_message: facts
            .latest_deprecation
            .filter(|message| !message.trim().is_empty()),
        current_version_deprecated: facts.current_deprecation.is_some(),
        is_stale: super::is_stale_at(facts.last_published.as_deref(), now),
        last_published: facts.last_published,
        workspace_members: pkg.workspace_members.clone(),
    })
}

/// The installed version's install-script posture, from the same packument
/// the update check parses. `None` when the version declares no install
/// scripts. Pure; tested directly.
pub(crate) fn install_script_package_from_packument(
    name: &str,
    current: &str,
    is_dev: bool,
    body: &serde_json::Value,
) -> Option<InstallScriptPackage> {
    let scripts = npm_packument::extract_packument_facts(body, current).install_scripts;
    if scripts.is_empty() {
        return None;
    }
    Some(InstallScriptPackage {
        name: name.to_string(),
        version: current.to_string(),
        scripts,
        is_dev,
    })
}

/// The installed version's declared license, from the same packument. `None`
/// when the packument does not carry the installed version at all (no claim
/// possible). Pure; tested directly.
pub(crate) fn package_license_from_packument(
    name: &str,
    current: &str,
    is_dev: bool,
    body: &serde_json::Value,
) -> Option<PackageLicense> {
    let license = npm_packument::license_of_installed(body, current)?;
    Some(PackageLicense {
        name: name.to_string(),
        version: current.to_string(),
        license,
        is_dev,
    })
}

async fn fetch_package(
    client: &Client,
    api_base: &str,
    pkg: &InstalledPackage,
) -> Result<Option<PackageFinding>, String> {
    let name = pkg.name.as_str();
    let current = pkg.version.as_str();
    let is_dev = pkg.is_dev;
    // npm registry: GET /{package} for unscoped, GET /@scope%2Fname for
    // scoped. The full packument carries dist-tags, per-version deprecation,
    // publish timestamps, and per-version scripts in one response.
    let url = format!("{}/{}", api_base, encode_package_name(name));
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    let status = resp.status();
    if super::status_is_observed_absence(status) {
        // Unknown to the registry (private/unpublished package): an observed
        // absence with nothing to report - never a partial sweep.
        return Ok(None);
    }
    if !status.is_success() {
        // Outage-class response (5xx, 429,...): this package's registry
        // state was NOT observed; the fan-out counts it toward `partial`.
        return Err(format!(
            "npm registry returned status {} for {}",
            status, name
        ));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(Some(PackageFinding {
        update: build_update_from_packument(pkg, &body, chrono::Utc::now()),
        install_scripts: install_script_package_from_packument(name, current, is_dev, &body),
        license: package_license_from_packument(name, current, is_dev, &body),
    }))
}

pub(crate) fn is_newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Option<(u64, u64, u64)> {
        let v = v.trim_start_matches('v');
        let parts: Vec<&str> = v.split('.').collect();
        let major = parts.first()?.split('-').next()?.parse().ok()?;
        let minor = parts
            .get(1)
            .and_then(|s| s.split('-').next()?.parse().ok())
            .unwrap_or(0);
        let patch = parts
            .get(2)
            .and_then(|s| s.split('-').next()?.parse().ok())
            .unwrap_or(0);
        Some((major, minor, patch))
    };

    match (parse(current), parse(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_package_name_passes_through_unscoped() {
        assert_eq!(encode_package_name("react"), "react");
        assert_eq!(encode_package_name("lodash"), "lodash");
        assert_eq!(encode_package_name(""), "");
    }

    #[test]
    fn encode_package_name_percent_encodes_scoped_slash() {
        assert_eq!(
            encode_package_name("@vitejs/plugin-react"),
            "@vitejs%2fplugin-react"
        );
        assert_eq!(encode_package_name("@types/node"), "@types%2fnode");
    }

    #[test]
    fn encode_package_name_only_encodes_at_prefix() {
        assert_eq!(encode_package_name("foo/bar"), "foo/bar");
    }

    #[test]
    fn is_newer_recognises_major_minor_patch_bumps() {
        assert!(is_newer("1.0.0", "2.0.0"));
        assert!(is_newer("1.0.0", "1.1.0"));
        assert!(is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn is_newer_rejects_same_version() {
        assert!(!is_newer("1.2.3", "1.2.3"));
    }

    #[test]
    fn is_newer_rejects_downgrade() {
        assert!(!is_newer("2.0.0", "1.5.0"));
        assert!(!is_newer("1.0.5", "1.0.0"));
    }

    #[test]
    fn is_newer_strips_v_prefix() {
        // Some packages tag versions as `v1.2.3`. The parser tolerates
        // either form so a comparison across formats works.
        assert!(is_newer("v1.0.0", "v1.0.1"));
        assert!(is_newer("1.0.0", "v1.0.1"));
        assert!(is_newer("v1.0.0", "1.0.1"));
    }

    #[test]
    fn is_newer_handles_missing_minor_or_patch() {
        // Bare "1" or "1.0" should compare correctly against full triples.
        assert!(is_newer("1", "1.0.1"));
        assert!(is_newer("1.0", "1.0.1"));
        assert!(!is_newer("1.0.1", "1.0"));
    }

    #[test]
    fn is_newer_strips_prerelease_suffix_for_comparison() {
        // The parser treats "1.0.0-beta" the same as "1.0.0" for the
        // numeric comparison. Documents the (lossy) behaviour.
        assert!(!is_newer("1.0.0-beta", "1.0.0"));
        assert!(is_newer("1.0.0", "1.0.1-beta"));
    }

    #[test]
    fn is_newer_returns_false_for_unparseable_input() {
        assert!(!is_newer("not.a.version", "1.0.0"));
        assert!(!is_newer("1.0.0", "garbage"));
        assert!(!is_newer("", ""));
    }

    fn body_with(version: &str) -> serde_json::Value {
        serde_json::json!({ "dist-tags": { "latest": version } })
    }

    fn fixed_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .expect("valid fixture timestamp")
            .with_timezone(&chrono::Utc)
    }

    fn build(name: &str, current: &str, body: &serde_json::Value) -> Option<PackageUpdate> {
        build_update_from_packument(&installed(name, current), body, fixed_now())
    }

    #[test]
    fn build_update_returns_some_for_real_upgrade() {
        let update =
            build("react", "18.2.0", &body_with("19.0.0")).expect("should produce an update");
        assert_eq!(update.name, "react");
        assert_eq!(update.current_version, "18.2.0");
        assert_eq!(update.latest_version, "19.0.0");
        assert_eq!(update.ecosystem, Ecosystem::Npm);
        assert_eq!(update.update_type, UpdateType::Major);
        assert!(!update.is_dev);
        assert!(!update.is_security);
        assert!(update.advisory_severity.is_none());
        assert!(!update.is_deprecated);
        assert!(update.deprecation_message.is_none());
        assert!(!update.is_stale);
    }

    #[test]
    fn build_update_propagates_is_dev_flag() {
        let mut pkg = installed("vitest", "1.0.0");
        pkg.is_dev = true;
        let update =
            build_update_from_packument(&pkg, &body_with("1.0.1"), fixed_now()).expect("update");
        assert!(update.is_dev, "is_dev must be propagated to the result");
    }

    #[test]
    fn build_update_returns_none_when_versions_match() {
        // Already up to date - must not produce a noisy "update available".
        assert!(build("react", "19.0.0", &body_with("19.0.0")).is_none());
    }

    #[test]
    fn build_update_returns_none_when_dist_tags_missing() {
        // Defensive against npm registry responses without dist-tags.
        assert!(build("react", "19.0.0", &serde_json::json!({})).is_none());
    }

    #[test]
    fn build_update_returns_none_for_downgrade() {
        let result = build("react", "19.0.0", &body_with("18.2.0"));
        assert!(result.is_none(), "downgrade must not produce an update");
    }

    #[test]
    fn build_update_classifies_minor_correctly() {
        let update = build("react", "18.2.0", &body_with("18.3.0")).expect("update");
        assert_eq!(update.update_type, UpdateType::Minor);
    }

    #[test]
    fn build_update_classifies_patch_correctly() {
        let update = build("react", "18.2.0", &body_with("18.2.1")).expect("update");
        assert_eq!(update.update_type, UpdateType::Patch);
    }

    #[test]
    fn deprecated_latest_version_is_flagged_with_message() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "versions": {
                "2.0.0": { "deprecated": "Use @scope/replacement instead" }
            }
        });
        let update = build("request", "1.0.0", &body).expect("update");
        assert!(update.is_deprecated);
        assert_eq!(
            update.deprecation_message.as_deref(),
            Some("Use @scope/replacement instead")
        );
    }

    #[test]
    fn deprecated_package_surfaces_even_when_already_at_latest() {
        // Mirrors the standalone OSV entries: an entry must exist so the UI
        // can show the deprecation even though there is nothing to update to.
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.88.2" },
            "versions": {
                "2.88.2": { "deprecated": "request has been deprecated" }
            }
        });
        let update = build("request", "2.88.2", &body).expect("standalone deprecated entry");
        assert!(update.is_deprecated);
        assert_eq!(update.current_version, "2.88.2");
        assert_eq!(update.latest_version, "2.88.2");
        assert_eq!(update.update_type, UpdateType::Unknown);
        assert_eq!(
            update.deprecation_message.as_deref(),
            Some("request has been deprecated")
        );
    }

    #[test]
    fn deprecated_installed_version_is_captured_alongside_update() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "versions": {
                "1.0.0": { "deprecated": "Broken build, upgrade" },
                "2.0.0": {}
            }
        });
        let update = build("left-pad", "1.0.0", &body).expect("update");
        assert!(!update.is_deprecated);
        assert!(update.current_version_deprecated);
        assert!(update.deprecation_message.is_none());
    }

    #[test]
    fn boolean_deprecation_flags_without_a_message() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "1.0.0" },
            "versions": { "1.0.0": { "deprecated": true } }
        });
        let update = build("old-pkg", "1.0.0", &body).expect("standalone deprecated entry");
        assert!(update.is_deprecated);
        assert!(
            update.deprecation_message.is_none(),
            "boolean deprecation has no message to show"
        );
    }

    #[test]
    fn install_script_posture_read_from_installed_version() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "versions": {
                "1.4.0": { "scripts": { "postinstall": "node install.js" } },
                "2.0.0": {}
            }
        });
        let pkg = install_script_package_from_packument("sharp", "1.4.0", false, &body)
            .expect("install-script package");
        assert_eq!(pkg.name, "sharp");
        assert_eq!(pkg.version, "1.4.0");
        assert_eq!(pkg.scripts, vec!["postinstall"]);
        assert!(!pkg.is_dev);
    }

    #[test]
    fn no_install_scripts_yields_none() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "versions": { "1.0.0": { "scripts": { "build": "tsc" } } }
        });
        assert!(install_script_package_from_packument("react", "1.0.0", false, &body).is_none());
    }

    #[test]
    fn install_script_posture_covers_up_to_date_packages() {
        // The whole point of the separate channel: a package at latest has
        // no PackageUpdate entry but must still report its install scripts.
        let body = serde_json::json!({
            "dist-tags": { "latest": "1.0.0" },
            "versions": { "1.0.0": { "scripts": { "preinstall": "node check.js" } } }
        });
        assert!(build("esbuild", "1.0.0", &body).is_none());
        let pkg = install_script_package_from_packument("esbuild", "1.0.0", true, &body)
            .expect("install-script package");
        assert_eq!(pkg.scripts, vec!["preinstall"]);
        assert!(pkg.is_dev);
    }

    #[test]
    fn license_posture_read_from_installed_version() {
        let body = serde_json::json!({
            "versions": { "1.0.0": { "license": "GPL-3.0-only" } }
        });
        let license = package_license_from_packument("copyleft-lib", "1.0.0", false, &body)
            .expect("license entry");
        assert_eq!(license.license.as_deref(), Some("GPL-3.0-only"));
        assert!(!license.is_dev);
    }

    #[test]
    fn license_posture_distinguishes_undeclared_from_unknown_version() {
        // Version present without a license: an entry with license None.
        let undeclared = serde_json::json!({ "versions": { "1.0.0": {} } });
        let entry = package_license_from_packument("mystery", "1.0.0", false, &undeclared)
            .expect("entry for version without license");
        assert!(entry.license.is_none());

        // Version absent from the packument: no entry, no claim.
        let unknown = serde_json::json!({ "versions": { "2.0.0": {} } });
        assert!(package_license_from_packument("mystery", "1.0.0", false, &unknown).is_none());
    }

    #[test]
    fn stale_when_latest_published_over_three_years_before_now() {
        // fixed_now is 2026-07-01; latest published 2022-01-01 is > 3 years.
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "time": { "2.0.0": "2022-01-01T00:00:00.000Z" }
        });
        let update = build("moment", "1.0.0", &body).expect("update");
        assert!(update.is_stale);
        assert_eq!(
            update.last_published.as_deref(),
            Some("2022-01-01T00:00:00.000Z")
        );
        // Stale is informational only - it must not look like a defect.
        assert!(!update.is_security);
        assert!(!update.is_deprecated);
    }

    #[test]
    fn not_stale_when_latest_published_within_three_years() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "time": { "2.0.0": "2025-01-01T00:00:00.000Z" }
        });
        let update = build("react", "1.0.0", &body).expect("update");
        assert!(!update.is_stale);
        assert_eq!(
            update.last_published.as_deref(),
            Some("2025-01-01T00:00:00.000Z")
        );
    }

    #[test]
    fn stale_alone_does_not_create_a_standalone_entry() {
        // Unlike deprecation, staleness on an up-to-date package produces no
        // entry: there is no action to take and no update row to attach to.
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "time": { "2.0.0": "2020-01-01T00:00:00.000Z" }
        });
        assert!(build("moment", "2.0.0", &body).is_none());
    }

    fn installed(name: &str, version: &str) -> InstalledPackage {
        InstalledPackage {
            name: name.to_string(),
            version: version.to_string(),
            ecosystem: Ecosystem::Npm,
            source: "package-lock.json".to_string(),
            is_dev: false,
            workspace_members: Vec::new(),
        }
    }

    /// One-shot local HTTP server: answers every accepted connection with
    /// `status_line` + `body` (mirrors the OSV stub), so the npm client can
    /// be driven offline.
    async fn spawn_registry_stub(status_line: &'static str, body: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind registry stub");
        let address = listener.local_addr().expect("registry stub address");
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut request = [0u8; 8192];
                let _ = stream.read(&mut request).await;
                let response = format!(
                    "{}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    status_line,
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        format!("http://{}", address)
    }

    #[tokio::test]
    async fn http_500_marks_the_scan_partial() {
        let base = spawn_registry_stub("HTTP/1.1 500 Internal Server Error", "").await;

        let scan = check_updates_at(&[installed("left-pad", "1.0.0")], &base).await;

        assert!(scan.updates.is_empty());
        assert!(
            scan.partial,
            "a failed packument fetch must mark the scan partial so dependency items survive"
        );
    }

    #[tokio::test]
    async fn http_404_is_an_observed_absence_not_a_partial_scan() {
        let base = spawn_registry_stub("HTTP/1.1 404 Not Found", "{}").await;

        let scan = check_updates_at(&[installed("internal-private-pkg", "1.0.0")], &base).await;

        assert!(scan.updates.is_empty());
        assert!(!scan.partial, "a 404 must not degrade the scan");
    }

    #[tokio::test]
    async fn transport_error_marks_the_scan_partial() {
        // Connection refused (listener bound then dropped): the transport
        // error path must count as unobserved exactly like an HTTP outage.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        drop(listener);

        let scan = check_updates_at(&[installed("left-pad", "1.0.0")], &base).await;

        assert!(scan.updates.is_empty());
        assert!(scan.partial);
    }

    #[tokio::test]
    async fn successful_sweep_stays_complete() {
        let base =
            spawn_registry_stub("HTTP/1.1 200 OK", r#"{"dist-tags": {"latest": "2.0.0"}}"#).await;

        let scan = check_updates_at(&[installed("left-pad", "1.0.0")], &base).await;

        assert_eq!(scan.updates.len(), 1);
        assert!(!scan.partial);
    }
}
