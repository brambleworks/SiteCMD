//! Parses production and development dependencies from `composer.lock`.

use std::path::Path;

use super::types::{Ecosystem, InstalledPackage};

pub fn parse(dir: &Path) -> Vec<InstalledPackage> {
    let content = match super::read_dependency_file(&dir.join("composer.lock")) {
        Some(content) => content,
        None => return Vec::new(),
    };

    let lock: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut packages = Vec::new();

    // "packages" array - production dependencies
    if let Some(pkgs) = lock.get("packages").and_then(|v| v.as_array()) {
        for pkg in pkgs {
            if let Some(installed) = parse_composer_package(pkg, false) {
                packages.push(installed);
            }
        }
    }

    // "packages-dev" array - dev dependencies
    if let Some(pkgs) = lock.get("packages-dev").and_then(|v| v.as_array()) {
        for pkg in pkgs {
            if let Some(installed) = parse_composer_package(pkg, true) {
                packages.push(installed);
            }
        }
    }

    packages
}

fn parse_composer_package(pkg: &serde_json::Value, is_dev: bool) -> Option<InstalledPackage> {
    let name = pkg.get("name")?.as_str()?;
    let version = pkg.get("version")?.as_str()?;

    // Clean version: remove "v" prefix, handle "dev-" branches
    let clean_version = version.trim_start_matches('v').to_string();

    // Skip dev branches like "dev-main", "dev-master"
    if clean_version.starts_with("dev-") {
        return None;
    }

    Some(InstalledPackage {
        name: name.to_string(),
        version: clean_version,
        ecosystem: Ecosystem::Composer,
        source: "composer.lock".into(),
        is_dev,
        workspace_members: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_composer_lock() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();

        fs::write(
            dir.join("composer.lock"),
            r#"{
                "packages": [
                    {"name": "drupal/core", "version": "10.2.3"},
                    {"name": "drupal/admin_toolbar", "version": "3.4.2"},
                    {"name": "guzzlehttp/guzzle", "version": "v7.8.1"},
                    {"name": "some/dev-branch", "version": "dev-main"}
                ],
                "packages-dev": [
                    {"name": "phpunit/phpunit", "version": "10.5.5"},
                    {"name": "drupal/devel", "version": "5.1.2"}
                ]
            }"#,
        )
        .unwrap();

        let result = parse(dir);

        // dev-main should be skipped
        assert_eq!(result.len(), 5);

        let core = result.iter().find(|p| p.name == "drupal/core").unwrap();
        assert_eq!(core.version, "10.2.3");
        assert!(!core.is_dev);

        let guzzle = result
            .iter()
            .find(|p| p.name == "guzzlehttp/guzzle")
            .unwrap();
        assert_eq!(guzzle.version, "7.8.1"); // v prefix stripped

        let phpunit = result.iter().find(|p| p.name == "phpunit/phpunit").unwrap();
        assert!(phpunit.is_dev);

        // dev-main should not be present
        assert!(result
            .iter()
            .find(|p| p.name == "some/dev-branch")
            .is_none());
    }

    #[test]
    fn test_no_composer_lock() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();

        let result = parse(dir);
        assert!(result.is_empty());
    }
}
