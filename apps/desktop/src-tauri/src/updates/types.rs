use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which package ecosystem a dependency belongs to
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum Ecosystem {
    Npm,
    Composer,
    WordPress,
    Drupal,
    Python,
    Ruby,
    Go,
    Rust,
}

impl Ecosystem {
    /// Human-readable label
    pub fn label(&self) -> &str {
        match self {
            Ecosystem::Npm => "npm",
            Ecosystem::Composer => "Composer",
            Ecosystem::WordPress => "WordPress",
            Ecosystem::Drupal => "Drupal",
            Ecosystem::Python => "Python",
            Ecosystem::Ruby => "Ruby",
            Ecosystem::Go => "Go",
            Ecosystem::Rust => "Rust",
        }
    }
}

/// A package found in a local project directory
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub ecosystem: Ecosystem,
    pub source: String,
    pub is_dev: bool,
    /// Declaring workspace paths; `.` is the root and empty means single-package.
    #[serde(default)]
    pub workspace_members: Vec<String>,
}

/// What kind of version bump an update represents
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export_to = "ipc-bindings.ts")]
pub enum UpdateType {
    Major,
    Minor,
    Patch,
    Unknown,
}

/// An available update for an installed package
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct PackageUpdate {
    pub name: String,
    pub current_version: String,
    pub latest_version: String,
    pub ecosystem: Ecosystem,
    pub update_type: UpdateType,
    pub is_security: bool,
    pub advisory_severity: Option<String>, // "critical", "high", "medium", "low"
    pub advisory_url: Option<String>,
    /// Registry release confirmed by OSV to be free of known advisories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub advisory_fixed_version: Option<String>,
    pub source: String,
    pub is_dev: bool,
    /// Whether the registry marks the package deprecated or unsupported.
    #[serde(default)]
    pub is_deprecated: bool,
    /// Registry or client-provided deprecation notice.
    #[serde(default)]
    pub deprecation_message: Option<String>,
    /// Whether the installed version is deprecated or yanked.
    #[serde(default)]
    pub current_version_deprecated: bool,
    /// Whether the latest release is at least three years old.
    #[serde(default)]
    pub is_stale: bool,
    /// Latest publish time in the registry's native format.
    #[serde(default)]
    pub last_published: Option<String>,
    /// Workspace members that declare this package.
    #[serde(default)]
    pub workspace_members: Vec<String>,
}

impl Default for InstalledPackage {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: String::new(),
            ecosystem: Ecosystem::Npm,
            source: String::new(),
            is_dev: false,
            workspace_members: Vec::new(),
        }
    }
}

impl Default for PackageUpdate {
    fn default() -> Self {
        Self {
            name: String::new(),
            current_version: String::new(),
            latest_version: String::new(),
            ecosystem: Ecosystem::Npm,
            update_type: UpdateType::Unknown,
            is_security: false,
            advisory_severity: None,
            advisory_url: None,
            advisory_fixed_version: None,
            source: String::new(),
            is_dev: false,
            is_deprecated: false,
            deprecation_message: None,
            current_version_deprecated: false,
            is_stale: false,
            last_published: None,
            workspace_members: Vec::new(),
        }
    }
}

/// Direct npm dependency with install lifecycle scripts.
/// Stored separately from updates so current packages remain visible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallScriptPackage {
    pub name: String,
    pub version: String,
    /// Declared install lifecycle scripts in execution order.
    pub scripts: Vec<String>,
    pub is_dev: bool,
}

/// Registry-declared license for an installed direct dependency.
/// `None` means the observed version explicitly declares no license.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageLicense {
    pub name: String,
    pub version: String,
    pub license: Option<String>,
    pub is_dev: bool,
}

/// Full result from scanning a project for updates
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct UpdateReport {
    pub packages: Vec<InstalledPackage>,
    pub updates: Vec<PackageUpdate>,
    pub ecosystems_detected: Vec<Ecosystem>,
    pub scan_duration_ms: u64,
}

/// Compare two semver-like version strings, return update type
pub fn classify_update(current: &str, latest: &str) -> UpdateType {
    let cur = parse_semver(current);
    let lat = parse_semver(latest);

    match (cur, lat) {
        (Some((cmaj, cmin, _)), Some((lmaj, lmin, _))) => {
            if lmaj > cmaj {
                UpdateType::Major
            } else if lmin > cmin {
                UpdateType::Minor
            } else {
                UpdateType::Patch
            }
        }
        _ => UpdateType::Unknown,
    }
}

/// Parse a version string into (major, minor, patch), stripping common prefixes
fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v').trim_start_matches('V');
    let parts: Vec<&str> = v.split('.').collect();
    if parts.is_empty() {
        return None;
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_major() {
        assert_eq!(classify_update("1.2.3", "2.0.0"), UpdateType::Major);
    }

    #[test]
    fn test_classify_minor() {
        assert_eq!(classify_update("1.2.3", "1.3.0"), UpdateType::Minor);
    }

    #[test]
    fn test_classify_patch() {
        assert_eq!(classify_update("1.2.3", "1.2.5"), UpdateType::Patch);
    }

    #[test]
    fn test_classify_with_prefix() {
        assert_eq!(classify_update("v1.2.3", "v2.0.0"), UpdateType::Major);
    }

    #[test]
    fn test_classify_with_prerelease() {
        assert_eq!(
            classify_update("1.2.3-beta.1", "2.0.0-rc.1"),
            UpdateType::Major
        );
    }

    #[test]
    fn test_classify_unknown() {
        assert_eq!(classify_update("latest", "nightly"), UpdateType::Unknown);
    }
}
