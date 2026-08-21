//! WordPress core, plugin, and theme package discovery.

use std::path::Path;

use super::types::{Ecosystem, InstalledPackage};

/// Discover WordPress packages from standard and Bedrock layouts.
pub fn parse(dir: &Path) -> Vec<InstalledPackage> {
    let mut packages = Vec::new();

    if let Some(core_ver) = detect_core_version(dir) {
        packages.push(InstalledPackage {
            name: "wordpress".to_string(),
            version: core_ver,
            ecosystem: Ecosystem::WordPress,
            source: "wp-includes/version.php".into(),
            is_dev: false,
            workspace_members: Vec::new(),
        });
    }

    // Scan plugins
    let plugin_dirs = [
        dir.join("wp-content/plugins"),
        dir.join("web/wp-content/plugins"), // Bedrock-style
    ];
    for plugin_dir in &plugin_dirs {
        if plugin_dir.is_dir() {
            scan_plugins(plugin_dir, &mut packages);
        }
    }

    // Scan themes
    let theme_dirs = [
        dir.join("wp-content/themes"),
        dir.join("web/wp-content/themes"),
    ];
    for theme_dir in &theme_dirs {
        if theme_dir.is_dir() {
            scan_themes(theme_dir, &mut packages);
        }
    }

    packages
}

/// Read $wp_version from wp-includes/version.php
fn detect_core_version(dir: &Path) -> Option<String> {
    let paths = [
        dir.join("wp-includes/version.php"),
        dir.join("web/wp-includes/version.php"),
    ];

    for path in &paths {
        if let Some(content) = super::read_dependency_file(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                // $wp_version = '6.4.2';
                if trimmed.starts_with("$wp_version") && trimmed.contains('=') {
                    return extract_php_string(trimmed);
                }
            }
        }
    }
    None
}

/// Scan plugin directories for readme.txt with "Stable tag:" header
fn scan_plugins(plugins_dir: &Path, packages: &mut Vec<InstalledPackage>) {
    let entries = match std::fs::read_dir(plugins_dir) {
        Ok(e) => e,
        Err(_) => {
            // The directory exists (the caller checked is_dir) but cannot be
            // enumerated: installed plugins are present-but-unobservable, so
            // this pass must not read as "no plugins" and false-resolve
            // plugin vulnerability items.
            super::set_present_but_unreadable();
            return;
        }
    };

    for entry in entries {
        let Ok(entry) = entry else {
            // A mid-enumeration entry error hides an unknown slice of the
            // plugin census: same present-but-unobservable rule as the failed
            // read_dir above.
            super::set_present_but_unreadable();
            continue;
        };
        if !entry.path().is_dir() {
            continue;
        }

        let plugin_slug = entry.file_name().to_string_lossy().to_string();
        let readme = entry.path().join("readme.txt");
        let readme_md = entry.path().join("README.md");

        // Try readme.txt first (standard), then README.md
        let version = parse_readme_version(&readme)
            .or_else(|| parse_readme_version(&readme_md))
            .or_else(|| parse_plugin_php_version(&entry.path(), &plugin_slug));

        if let Some(ver) = version {
            packages.push(InstalledPackage {
                name: plugin_slug,
                version: ver,
                ecosystem: Ecosystem::WordPress,
                source: "wp-content/plugins".into(),
                is_dev: false,
                workspace_members: Vec::new(),
            });
        }
    }
}

/// Parse "Stable tag: X.Y.Z" from readme.txt
fn parse_readme_version(path: &Path) -> Option<String> {
    let content = super::read_dependency_file(path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("stable tag:") {
            let ver = trimmed
                .split(':')
                .nth(1)?
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            if ver != "trunk" && !ver.is_empty() {
                return Some(ver.to_string());
            }
        }
    }
    None
}

/// Parse "Version: X.Y.Z" from main plugin PHP file header
fn parse_plugin_php_version(plugin_dir: &Path, slug: &str) -> Option<String> {
    let php_file = plugin_dir.join(format!("{}.php", slug));
    let content = super::read_dependency_file(&php_file)?;

    // Only check the first 8KB (plugin header is at top)
    let header = &content[..content.len().min(8192)];
    for line in header.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("version:") && !lower.starts_with("version:  ") {
            let ver = trimmed.split(':').nth(1)?.trim();
            if !ver.is_empty() {
                return Some(ver.to_string());
            }
        }
    }
    None
}

/// Scan theme directories for style.css with "Version:" header
fn scan_themes(themes_dir: &Path, packages: &mut Vec<InstalledPackage>) {
    let entries = match std::fs::read_dir(themes_dir) {
        Ok(e) => e,
        Err(_) => {
            // Present-but-unenumerable themes dir: same unobservable census
            // rule as scan_plugins above.
            super::set_present_but_unreadable();
            return;
        }
    };

    for entry in entries {
        let Ok(entry) = entry else {
            // Mid-enumeration entry error: same unobservable-census rule as
            // scan_plugins above.
            super::set_present_but_unreadable();
            continue;
        };
        if !entry.path().is_dir() {
            continue;
        }

        let theme_slug = entry.file_name().to_string_lossy().to_string();
        let style = entry.path().join("style.css");

        if let Some(ver) = parse_style_css_version(&style) {
            packages.push(InstalledPackage {
                name: format!("{} (theme)", theme_slug),
                version: ver,
                ecosystem: Ecosystem::WordPress,
                source: "wp-content/themes".into(),
                is_dev: false,
                workspace_members: Vec::new(),
            });
        }
    }
}

/// Parse "Version: X.Y.Z" from theme style.css header
fn parse_style_css_version(path: &Path) -> Option<String> {
    let content = super::read_dependency_file(path)?;
    // Only check first 4KB - the header comment is at the top
    let header = &content[..content.len().min(4096)];
    for line in header.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("version:") {
            let ver = trimmed.split(':').nth(1)?.trim();
            if !ver.is_empty() {
                return Some(ver.to_string());
            }
        }
    }
    None
}

fn extract_php_string(line: &str) -> Option<String> {
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
    fn test_wordpress_scan() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();

        // Core version
        fs::create_dir_all(dir.join("wp-includes")).unwrap();
        fs::write(
            dir.join("wp-includes/version.php"),
            "<?php\n$wp_version = '6.4.2';\n",
        )
        .unwrap();

        // Plugin with readme.txt
        fs::create_dir_all(dir.join("wp-content/plugins/akismet")).unwrap();
        fs::write(
            dir.join("wp-content/plugins/akismet/readme.txt"),
            "=== Akismet Anti-spam ===\nStable tag: 5.3.1\nRequires PHP: 5.6\n",
        )
        .unwrap();

        // Plugin with PHP header only
        fs::create_dir_all(dir.join("wp-content/plugins/hello-dolly")).unwrap();
        fs::write(
            dir.join("wp-content/plugins/hello-dolly/hello-dolly.php"),
            "<?php\n/**\n * Plugin Name: Hello Dolly\n * Version: 1.7.2\n */\n",
        )
        .unwrap();

        // Theme
        fs::create_dir_all(dir.join("wp-content/themes/twentytwentyfour")).unwrap();
        fs::write(
            dir.join("wp-content/themes/twentytwentyfour/style.css"),
            "/*\nTheme Name: Twenty Twenty-Four\nVersion: 1.1\nDescription: A theme\n*/\n",
        )
        .unwrap();

        let result = parse(dir);

        // Core + 2 plugins + 1 theme = 4
        assert_eq!(
            result.len(),
            4,
            "Got: {:?}",
            result.iter().map(|p| &p.name).collect::<Vec<_>>()
        );

        let core = result.iter().find(|p| p.name == "wordpress").unwrap();
        assert_eq!(core.version, "6.4.2");

        let akismet = result.iter().find(|p| p.name == "akismet").unwrap();
        assert_eq!(akismet.version, "5.3.1");

        let hello = result.iter().find(|p| p.name == "hello-dolly").unwrap();
        assert_eq!(hello.version, "1.7.2");

        let theme = result
            .iter()
            .find(|p| p.name.contains("twentytwentyfour"))
            .unwrap();
        assert_eq!(theme.version, "1.1");
    }
}
