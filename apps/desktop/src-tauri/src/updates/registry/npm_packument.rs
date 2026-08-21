//! npm packument extraction for version, deprecation, and publish-recency facts.

/// Update-relevant facts extracted from one packument response.
pub(crate) struct PackumentFacts {
    /// `dist-tags.latest`, when present.
    pub latest_version: Option<String>,
    /// Latest-version deprecation message; legacy boolean records use an empty value.
    pub latest_deprecation: Option<String>,
    /// `Some(message)` when the installed version is deprecated.
    pub current_deprecation: Option<String>,
    /// ISO 8601 publish timestamp of the latest version, from the `time` map.
    pub last_published: Option<String>,
    /// Lifecycle scripts declared by the installed package version.
    pub install_scripts: Vec<String>,
}

pub(crate) fn extract_packument_facts(
    body: &serde_json::Value,
    current_version: &str,
) -> PackumentFacts {
    let latest_version = body
        .pointer("/dist-tags/latest")
        .and_then(|value| value.as_str())
        .map(str::to_string);

    let latest_deprecation = latest_version
        .as_deref()
        .and_then(|version| deprecation_of(body, version));
    let current_deprecation = deprecation_of(body, current_version);

    let last_published = latest_version
        .as_deref()
        .and_then(|latest| body.get("time")?.get(latest)?.as_str().map(str::to_string));

    PackumentFacts {
        latest_version,
        latest_deprecation,
        current_deprecation,
        last_published,
        install_scripts: install_scripts_of(body, current_version),
    }
}

/// Preserve absent, declared, and unknown installed-version license shapes.
pub(crate) fn license_of_installed(
    body: &serde_json::Value,
    version: &str,
) -> Option<Option<String>> {
    let version_object = body.get("versions")?.get(version)?;

    let license = match version_object.get("license") {
        Some(serde_json::Value::String(spdx)) if !spdx.trim().is_empty() => {
            Some(spdx.trim().to_string())
        }
        Some(serde_json::Value::Object(legacy)) => legacy
            .get("type")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        Some(serde_json::Value::Null) | None => version_object
            .pointer("/licenses/0/type")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        Some(other) => Some(other.to_string()),
    };

    Some(license)
}

/// npm lifecycle phases that execute arbitrary package code during install.
const INSTALL_LIFECYCLE_SCRIPTS: [&str; 3] = ["preinstall", "install", "postinstall"];

/// Return declared install scripts, using npm's `hasInstallScript` fallback.
fn install_scripts_of(body: &serde_json::Value, version: &str) -> Vec<String> {
    let Some(version_object) = body.get("versions").and_then(|v| v.get(version)) else {
        return Vec::new();
    };

    let declared: Vec<String> = INSTALL_LIFECYCLE_SCRIPTS
        .iter()
        .filter(|script| {
            version_object
                .pointer(&format!("/scripts/{script}"))
                .and_then(|value| value.as_str())
                .is_some_and(|command| !command.trim().is_empty())
        })
        .map(|script| script.to_string())
        .collect();

    if !declared.is_empty() {
        return declared;
    }

    if version_object.get("hasInstallScript") == Some(&serde_json::Value::Bool(true)) {
        return vec!["install".to_string()];
    }

    Vec::new()
}

/// Parses npm deprecation state. Empty strings mean active; legacy boolean
/// `true` means deprecated without a message.
fn deprecation_of(body: &serde_json::Value, version: &str) -> Option<String> {
    let deprecated = body.get("versions")?.get(version)?.get("deprecated")?;
    match deprecated {
        serde_json::Value::String(message) if !message.trim().is_empty() => Some(message.clone()),
        serde_json::Value::Bool(true) => Some(String::new()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_latest_version_from_dist_tags() {
        let body = serde_json::json!({ "dist-tags": { "latest": "2.0.0" } });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert_eq!(facts.latest_version.as_deref(), Some("2.0.0"));
        assert!(facts.latest_deprecation.is_none());
        assert!(facts.current_deprecation.is_none());
        assert!(facts.last_published.is_none());
    }

    #[test]
    fn captures_deprecation_message_of_latest_version() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "4.17.1" },
            "versions": {
                "4.17.1": { "deprecated": "Package no longer supported. Use lodash-es instead." }
            }
        });
        let facts = extract_packument_facts(&body, "4.16.0");
        assert_eq!(
            facts.latest_deprecation.as_deref(),
            Some("Package no longer supported. Use lodash-es instead.")
        );
        assert!(facts.current_deprecation.is_none());
    }

    #[test]
    fn captures_deprecation_of_installed_version_from_same_response() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "versions": {
                "1.0.0": { "deprecated": "Critical bug, upgrade to 2.x" },
                "2.0.0": {}
            }
        });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert!(facts.latest_deprecation.is_none());
        assert_eq!(
            facts.current_deprecation.as_deref(),
            Some("Critical bug, upgrade to 2.x")
        );
    }

    #[test]
    fn empty_string_deprecation_means_not_deprecated() {
        // `npm deprecate pkg@version ""` un-deprecates; some packuments keep
        // the empty-string field. It must not be reported as deprecated.
        let body = serde_json::json!({
            "dist-tags": { "latest": "1.0.0" },
            "versions": { "1.0.0": { "deprecated": "" } }
        });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert!(facts.latest_deprecation.is_none());
    }

    #[test]
    fn boolean_true_deprecation_is_deprecated_without_message() {
        // Legacy registry records store `deprecated: true` without a message.
        let body = serde_json::json!({
            "dist-tags": { "latest": "1.0.0" },
            "versions": { "1.0.0": { "deprecated": true } }
        });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert_eq!(facts.latest_deprecation.as_deref(), Some(""));
    }

    #[test]
    fn install_scripts_reported_in_lifecycle_order() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "versions": {
                "1.0.0": {
                    "scripts": {
                        "postinstall": "node scripts/setup.js",
                        "preinstall": "node scripts/check.js",
                        "test": "vitest"
                    }
                }
            }
        });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert_eq!(facts.install_scripts, vec!["preinstall", "postinstall"]);
    }

    #[test]
    fn non_install_scripts_are_not_flagged() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "1.0.0" },
            "versions": {
                "1.0.0": { "scripts": { "build": "tsc", "prepare": "husky" } }
            }
        });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert!(facts.install_scripts.is_empty());
    }

    #[test]
    fn empty_install_script_command_is_not_flagged() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "1.0.0" },
            "versions": {
                "1.0.0": { "scripts": { "postinstall": "  " } }
            }
        });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert!(facts.install_scripts.is_empty());
    }

    #[test]
    fn has_install_script_flag_reports_implicit_install() {
        // A bundled binding.gyp gives an implicit node-gyp install step with
        // no `scripts` entry; npm records it as `hasInstallScript: true`.
        let body = serde_json::json!({
            "dist-tags": { "latest": "1.0.0" },
            "versions": {
                "1.0.0": { "hasInstallScript": true }
            }
        });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert_eq!(facts.install_scripts, vec!["install"]);
    }

    #[test]
    fn missing_installed_version_yields_no_install_signal() {
        // The installed version object is the only trustworthy source; do
        // not guess from the latest version's scripts.
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "versions": {
                "2.0.0": { "scripts": { "postinstall": "node setup.js" } }
            }
        });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert!(facts.install_scripts.is_empty());
    }

    #[test]
    fn license_read_from_installed_version_string() {
        let body = serde_json::json!({
            "versions": { "1.0.0": { "license": " MIT " } }
        });
        assert_eq!(
            license_of_installed(&body, "1.0.0"),
            Some(Some("MIT".to_string()))
        );
    }

    #[test]
    fn license_read_from_legacy_object_and_array_forms() {
        let object_form = serde_json::json!({
            "versions": { "1.0.0": { "license": { "type": "BSD-3-Clause" } } }
        });
        assert_eq!(
            license_of_installed(&object_form, "1.0.0"),
            Some(Some("BSD-3-Clause".to_string()))
        );

        let array_form = serde_json::json!({
            "versions": { "1.0.0": { "licenses": [ { "type": "Apache-2.0" } ] } }
        });
        assert_eq!(
            license_of_installed(&array_form, "1.0.0"),
            Some(Some("Apache-2.0".to_string()))
        );
    }

    #[test]
    fn version_without_license_reports_declared_none() {
        let body = serde_json::json!({
            "versions": { "1.0.0": { "scripts": {} } }
        });
        assert_eq!(license_of_installed(&body, "1.0.0"), Some(None));
    }

    #[test]
    fn missing_version_object_makes_no_license_claim() {
        // The packument not carrying the installed version is not evidence
        // that the package is unlicensed.
        let body = serde_json::json!({
            "versions": { "2.0.0": {} }
        });
        assert_eq!(license_of_installed(&body, "1.0.0"), None);
    }

    #[test]
    fn reads_last_published_for_latest_from_time_map() {
        let body = serde_json::json!({
            "dist-tags": { "latest": "2.0.0" },
            "time": {
                "created": "2019-01-01T00:00:00.000Z",
                "1.0.0": "2019-01-01T00:00:00.000Z",
                "2.0.0": "2021-06-15T12:30:00.000Z"
            }
        });
        let facts = extract_packument_facts(&body, "1.0.0");
        assert_eq!(
            facts.last_published.as_deref(),
            Some("2021-06-15T12:30:00.000Z")
        );
    }
}
