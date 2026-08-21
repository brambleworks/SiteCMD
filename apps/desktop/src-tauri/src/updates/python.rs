//! Parses Python dependencies from Poetry, Pipenv, or requirements files.

use std::path::Path;

use super::types::{Ecosystem, InstalledPackage};

pub fn parse(dir: &Path) -> Vec<InstalledPackage> {
    // Try in order: most specific lockfile first
    if let Some(pkgs) = parse_poetry_lock(dir) {
        if !pkgs.is_empty() {
            return pkgs;
        }
    }
    if let Some(pkgs) = parse_pipfile_lock(dir) {
        if !pkgs.is_empty() {
            return pkgs;
        }
    }
    // Fall back to requirements.txt
    parse_requirements_txt(dir)
}

fn parse_requirements_txt(dir: &Path) -> Vec<InstalledPackage> {
    let filenames = [
        "requirements.txt",
        "requirements/base.txt",
        "requirements/production.txt",
    ];
    let mut packages = Vec::new();

    for filename in &filenames {
        let path = dir.join(filename);
        let content = match super::read_dependency_file(&path) {
            Some(content) => content,
            None => continue,
        };

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                continue;
            }

            // Handle: package==1.2.3, package>=1.2.3, package~=1.2.3
            // Also handle extras: package[extra]==1.2.3
            let (name, version) = if let Some(pos) = trimmed.find("==") {
                let name = &trimmed[..pos];
                let ver = &trimmed[pos + 2..];
                (name, Some(ver))
            } else if let Some(pos) = trimmed.find(">=") {
                let name = &trimmed[..pos];
                let ver = &trimmed[pos + 2..];
                // >= is a minimum, not exact, but still useful
                (name, Some(ver))
            } else if let Some(pos) = trimmed.find("~=") {
                let name = &trimmed[..pos];
                let ver = &trimmed[pos + 2..];
                (name, Some(ver))
            } else {
                continue; // No version pinned
            };

            // Strip extras: "requests[security]" → "requests"
            let clean_name = name.split('[').next().unwrap_or(name).trim();
            // Strip trailing version constraints like ",<2.0"
            let clean_ver = version.map(|v| v.split(',').next().unwrap_or(v).trim());

            if let Some(ver) = clean_ver {
                if !clean_name.is_empty() && !ver.is_empty() {
                    packages.push(InstalledPackage {
                        name: clean_name.to_lowercase(),
                        version: ver.to_string(),
                        ecosystem: Ecosystem::Python,
                        source: filename.to_string(),
                        is_dev: false,
                        workspace_members: Vec::new(),
                    });
                }
            }
        }

        if !packages.is_empty() {
            break; // Got packages from first available file
        }
    }

    packages
}

fn parse_pipfile_lock(dir: &Path) -> Option<Vec<InstalledPackage>> {
    let content = super::read_dependency_file(&dir.join("Pipfile.lock"))?;
    let lock: serde_json::Value = serde_json::from_str(&content).ok()?;
    let mut packages = Vec::new();

    for (section, is_dev) in [("default", false), ("develop", true)] {
        if let Some(deps) = lock.get(section).and_then(|v| v.as_object()) {
            for (name, info) in deps {
                if let Some(ver) = info.get("version").and_then(|v| v.as_str()) {
                    let clean_ver = ver.trim_start_matches("==").trim();
                    packages.push(InstalledPackage {
                        name: name.to_lowercase(),
                        version: clean_ver.to_string(),
                        ecosystem: Ecosystem::Python,
                        source: "Pipfile.lock".into(),
                        is_dev,
                        workspace_members: Vec::new(),
                    });
                }
            }
        }
    }

    Some(packages)
}

fn parse_poetry_lock(dir: &Path) -> Option<Vec<InstalledPackage>> {
    let content = super::read_dependency_file(&dir.join("poetry.lock"))?;
    let mut packages = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_version: Option<String> = None;
    let mut current_optional = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                packages.push(InstalledPackage {
                    name: name.to_lowercase(),
                    version,
                    ecosystem: Ecosystem::Python,
                    source: "poetry.lock".into(),
                    is_dev: current_optional,
                    workspace_members: Vec::new(),
                });
            }
            current_optional = false;
            continue;
        }

        if trimmed.starts_with("name = ") {
            current_name = extract_toml_value(trimmed);
        } else if trimmed.starts_with("version = ") {
            current_version = extract_toml_value(trimmed);
        } else if trimmed.starts_with("optional = true") {
            current_optional = true;
        }
    }

    if let (Some(name), Some(version)) = (current_name, current_version) {
        packages.push(InstalledPackage {
            name: name.to_lowercase(),
            version,
            ecosystem: Ecosystem::Python,
            source: "poetry.lock".into(),
            is_dev: current_optional,
            workspace_members: Vec::new(),
        });
    }

    Some(packages)
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
    fn test_requirements_txt() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(
            dir.join("requirements.txt"),
            "# Web framework\ndjango==4.2.9\nrequests[security]==2.31.0\ncelery>=5.3.0,<6.0\n# Dev tools\n-r requirements/dev.txt\n",
        ).unwrap();

        let result = parse(dir);
        assert_eq!(result.len(), 3);

        let django = result.iter().find(|p| p.name == "django").unwrap();
        assert_eq!(django.version, "4.2.9");

        let requests = result.iter().find(|p| p.name == "requests").unwrap();
        assert_eq!(requests.version, "2.31.0");
    }

    #[test]
    fn test_pipfile_lock() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(
            dir.join("Pipfile.lock"),
            r#"{"default":{"flask":{"version":"==3.0.0"},"jinja2":{"version":"==3.1.3"}},"develop":{"pytest":{"version":"==7.4.4"}}}"#,
        ).unwrap();

        let result = parse(dir);
        assert_eq!(result.len(), 3);

        let flask = result.iter().find(|p| p.name == "flask").unwrap();
        assert_eq!(flask.version, "3.0.0");
        assert!(!flask.is_dev);

        let pytest = result.iter().find(|p| p.name == "pytest").unwrap();
        assert!(pytest.is_dev);
    }

    #[test]
    fn test_poetry_lock() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(
            dir.join("poetry.lock"),
            r#"[[package]]
name = "fastapi"
version = "0.109.0"

[[package]]
name = "uvicorn"
version = "0.27.0"
"#,
        )
        .unwrap();

        let result = parse(dir);
        assert_eq!(result.len(), 2);

        let fastapi = result.iter().find(|p| p.name == "fastapi").unwrap();
        assert_eq!(fastapi.version, "0.109.0");
    }
}
