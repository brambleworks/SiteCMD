//! Parses Cargo dependencies and identifies direct crates from `Cargo.toml`.

use std::path::Path;

use super::types::{Ecosystem, InstalledPackage};

pub fn parse(dir: &Path) -> Vec<InstalledPackage> {
    let content = match super::read_dependency_file(&dir.join("Cargo.lock")) {
        Some(content) => content,
        None => return Vec::new(),
    };

    // Also read Cargo.toml to identify direct dependencies
    let direct_deps = read_direct_deps(dir);

    let mut packages = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_version: Option<String> = None;
    let mut current_source: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            flush_package(
                &mut current_name,
                &mut current_version,
                &mut current_source,
                &direct_deps,
                &mut packages,
            );
            continue;
        }

        if trimmed.starts_with("name = ") {
            current_name = extract_toml_value(trimmed);
        } else if trimmed.starts_with("version = ") {
            current_version = extract_toml_value(trimmed);
        } else if trimmed.starts_with("source = ") {
            current_source = extract_toml_value(trimmed);
        }
    }

    flush_package(
        &mut current_name,
        &mut current_version,
        &mut current_source,
        &direct_deps,
        &mut packages,
    );

    packages
}

fn flush_package(
    name: &mut Option<String>,
    version: &mut Option<String>,
    source: &mut Option<String>,
    direct_deps: &[String],
    packages: &mut Vec<InstalledPackage>,
) {
    if let (Some(n), Some(v)) = (name.take(), version.take()) {
        let src = source.take();

        // Skip path dependencies (local crates) - they don't have registry updates
        if src
            .as_ref()
            .map(|s| s.starts_with("path+"))
            .unwrap_or(false)
        {
            return;
        }

        // Skip the project's own crate
        let is_registry = src
            .as_ref()
            .map(|s| s.contains("crates.io") || s.contains("registry+"))
            .unwrap_or(false);

        if !is_registry {
            return; // Skip non-registry packages
        }

        let is_direct = direct_deps.iter().any(|d| d == &n);

        packages.push(InstalledPackage {
            name: n,
            version: v,
            ecosystem: Ecosystem::Rust,
            source: "Cargo.lock".into(),
            is_dev: !is_direct,
            workspace_members: Vec::new(),
        });
    } else {
        name.take();
        version.take();
        source.take();
    }
}

/// Read direct dependencies from Cargo.toml
fn read_direct_deps(dir: &Path) -> Vec<String> {
    let content = match super::read_dependency_file(&dir.join("Cargo.toml")) {
        Some(content) => content,
        None => return Vec::new(),
    };

    let mut deps = Vec::new();
    let mut in_deps = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "[dependencies]"
            || trimmed == "[dev-dependencies]"
            || trimmed == "[build-dependencies]"
        {
            in_deps = true;
            continue;
        }

        if trimmed.starts_with('[') {
            in_deps = false;
            continue;
        }

        if in_deps {
            if let Some(name) = trimmed.split('=').next() {
                let name = name.trim();
                if !name.is_empty() && !name.starts_with('#') {
                    deps.push(name.to_string());
                }
            }
        }
    }

    deps
}

fn extract_toml_value(line: &str) -> Option<String> {
    let val = line
        .split('=')
        .nth(1)?
        .trim()
        .trim_matches('"')
        .trim_matches('\'');
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_cargo_lock() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();

        fs::write(
            dir.join("Cargo.toml"),
            r#"[package]
name = "myapp"
version = "0.1.0"

[dependencies]
serde = "1"
tokio = "1"
"#,
        )
        .unwrap();

        fs::write(
            dir.join("Cargo.lock"),
            r#"[[package]]
name = "myapp"
version = "0.1.0"

[[package]]
name = "serde"
version = "1.0.195"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "tokio"
version = "1.35.1"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "pin-project-lite"
version = "0.2.13"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#,
        )
        .unwrap();

        let result = parse(dir);
        // myapp (no source) should be skipped, pin-project-lite is transitive
        assert_eq!(
            result.len(),
            3,
            "Got: {:?}",
            result
                .iter()
                .map(|p| (&p.name, p.is_dev))
                .collect::<Vec<_>>()
        );

        let serde = result.iter().find(|p| p.name == "serde").unwrap();
        assert_eq!(serde.version, "1.0.195");
        assert!(!serde.is_dev);

        let pin = result
            .iter()
            .find(|p| p.name == "pin-project-lite")
            .unwrap();
        assert!(pin.is_dev); // transitive = dev
    }
}
