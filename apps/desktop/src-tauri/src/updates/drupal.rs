//! Drupal core, module, and theme package discovery.

use std::path::Path;

use super::types::{Ecosystem, InstalledPackage};

/// Discover Drupal core, modules, and themes from standard project layouts.
pub fn parse(dir: &Path) -> Vec<InstalledPackage> {
    let mut packages = Vec::new();

    let webroot = if dir.join("web/core/lib/Drupal.php").exists() {
        "web"
    } else if dir.join("docroot/core/lib/Drupal.php").exists() {
        "docroot"
    } else if dir.join("core/lib/Drupal.php").exists() {
        "."
    } else {
        return packages; // Not a Drupal project
    };

    // Core version
    if let Some(ver) = detect_core_version(dir, webroot) {
        packages.push(InstalledPackage {
            name: "drupal/core".to_string(),
            version: ver,
            ecosystem: Ecosystem::Drupal,
            source: format!("{}/core/lib/Drupal.php", webroot),
            is_dev: false,
            workspace_members: Vec::new(),
        });
    }

    // Scan contributed modules
    let module_dirs = [
        dir.join(format!("{}/modules/contrib", webroot)),
        dir.join(format!("{}/modules/custom", webroot)),
        // Some projects put modules at project root
        dir.join("modules/contrib"),
    ];
    for module_dir in &module_dirs {
        if module_dir.is_dir() {
            let is_custom = module_dir.to_string_lossy().contains("custom");
            scan_info_yml(module_dir, "module", is_custom, &mut packages);
        }
    }

    // Scan contributed themes
    let theme_dirs = [
        dir.join(format!("{}/themes/contrib", webroot)),
        dir.join(format!("{}/themes/custom", webroot)),
        dir.join("themes/contrib"),
    ];
    for theme_dir in &theme_dirs {
        if theme_dir.is_dir() {
            let is_custom = theme_dir.to_string_lossy().contains("custom");
            scan_info_yml(theme_dir, "theme", is_custom, &mut packages);
        }
    }

    packages
}

/// Read Drupal::VERSION from core/lib/Drupal.php
fn detect_core_version(dir: &Path, webroot: &str) -> Option<String> {
    let path = if webroot == "." {
        dir.join("core/lib/Drupal.php")
    } else {
        dir.join(format!("{}/core/lib/Drupal.php", webroot))
    };

    let content = super::read_dependency_file(&path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        // const VERSION = '10.2.3';
        if trimmed.contains("VERSION")
            && trimmed.contains('=')
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("*")
        {
            return extract_quoted_value(trimmed);
        }
    }
    None
}

/// Scan a directory for *.info.yml files and extract module/theme versions
fn scan_info_yml(
    parent_dir: &Path,
    component_type: &str,
    is_custom: bool,
    packages: &mut Vec<InstalledPackage>,
) {
    let entries = match std::fs::read_dir(parent_dir) {
        Ok(e) => e,
        Err(_) => {
            // The directory exists (the caller checked is_dir) but cannot be
            // enumerated: installed modules/themes are present-but-
            // unobservable, so this pass must not read as "nothing installed"
            // and false-resolve their vulnerability/update items.
            super::set_present_but_unreadable();
            return;
        }
    };

    for entry in entries {
        let Ok(entry) = entry else {
            // A mid-enumeration entry error hides an unknown slice of the
            // module/theme census: same present-but-unobservable rule as the
            // failed read_dir above.
            super::set_present_but_unreadable();
            continue;
        };
        if !entry.path().is_dir() {
            continue;
        }

        let module_name = entry.file_name().to_string_lossy().to_string();

        // Look for {module_name}.info.yml
        let info_path = entry.path().join(format!("{}.info.yml", module_name));
        if !info_path.exists() {
            continue;
        }

        if let Some(content) = super::read_dependency_file(&info_path) {
            let version = parse_info_yml_version(&content);
            let name_display = parse_info_yml_name(&content).unwrap_or_else(|| module_name.clone());

            if let Some(ver) = version {
                packages.push(InstalledPackage {
                    name: module_name,
                    version: ver,
                    ecosystem: Ecosystem::Drupal,
                    source: format!(
                        "{}/{} ({})",
                        if is_custom { "custom" } else { "contrib" },
                        name_display,
                        component_type
                    ),
                    is_dev: false,
                    workspace_members: Vec::new(),
                });
            } else if !is_custom {
                // Contrib without version - might be from Composer (version in composer.lock)
                // Still record it so we can cross-reference
                packages.push(InstalledPackage {
                    name: module_name,
                    version: "unknown".to_string(),
                    ecosystem: Ecosystem::Drupal,
                    source: format!("contrib/{} ({})", name_display, component_type),
                    is_dev: false,
                    workspace_members: Vec::new(),
                });
            }
        }
    }
}

/// Parse `version:` from a `.info.yml` file.
fn parse_info_yml_version(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version:") {
            let ver = trimmed
                .trim_start_matches("version:")
                .trim()
                .trim_matches('\'')
                .trim_matches('"');
            if !ver.is_empty() && ver != "VERSION" {
                return Some(ver.to_string());
            }
        }
    }
    None
}

/// Parse `name:` from a `.info.yml` file.
fn parse_info_yml_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name:") {
            let name = trimmed
                .trim_start_matches("name:")
                .trim()
                .trim_matches('\'')
                .trim_matches('"');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn extract_quoted_value(line: &str) -> Option<String> {
    for delim in ['\'', '"'] {
        let parts: Vec<&str> = line.split(delim).collect();
        if parts.len() >= 2 {
            let val = parts[1].trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_drupal_scan() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();

        // Core
        fs::create_dir_all(dir.join("web/core/lib")).unwrap();
        fs::write(
            dir.join("web/core/lib/Drupal.php"),
            "<?php\nnamespace Drupal;\nclass Drupal {\n  const VERSION = '10.2.3';\n}\n",
        )
        .unwrap();

        // Contrib module
        fs::create_dir_all(dir.join("web/modules/contrib/admin_toolbar")).unwrap();
        fs::write(
            dir.join("web/modules/contrib/admin_toolbar/admin_toolbar.info.yml"),
            "name: 'Admin Toolbar'\ntype: module\nversion: '3.4.2'\ncore_version_requirement: ^10\n",
        ).unwrap();

        // Contrib theme
        fs::create_dir_all(dir.join("web/themes/contrib/gin")).unwrap();
        fs::write(
            dir.join("web/themes/contrib/gin/gin.info.yml"),
            "name: Gin\ntype: theme\nversion: '3.0.9'\ncore_version_requirement: ^10\n",
        )
        .unwrap();

        let result = parse(dir);

        assert_eq!(
            result.len(),
            3,
            "Got: {:?}",
            result
                .iter()
                .map(|p| (&p.name, &p.version))
                .collect::<Vec<_>>()
        );

        let core = result.iter().find(|p| p.name == "drupal/core").unwrap();
        assert_eq!(core.version, "10.2.3");

        let toolbar = result.iter().find(|p| p.name == "admin_toolbar").unwrap();
        assert_eq!(toolbar.version, "3.4.2");

        let gin = result.iter().find(|p| p.name == "gin").unwrap();
        assert_eq!(gin.version, "3.0.9");
    }
}
