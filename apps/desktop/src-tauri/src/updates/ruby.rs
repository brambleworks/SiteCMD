//! Parses Ruby dependencies and identifies direct gems from `Gemfile`.

use std::path::Path;

use super::types::{Ecosystem, InstalledPackage};

pub fn parse(dir: &Path) -> Vec<InstalledPackage> {
    let content = match super::read_dependency_file(&dir.join("Gemfile.lock")) {
        Some(content) => content,
        None => return Vec::new(),
    };

    let mut packages = Vec::new();
    let mut in_specs = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // GEM section contains the resolved gems
        if trimmed == "GEM" || trimmed == "PATH" || trimmed == "GIT" {
            in_specs = false;
            continue;
        }

        if trimmed == "specs:" {
            in_specs = true;
            continue;
        }

        // End of a section
        if !line.starts_with(' ') && !trimmed.is_empty() {
            in_specs = false;
            continue;
        }

        if !in_specs {
            continue;
        }

        // Gem lines are indented with exactly 4 spaces (top-level gems)
        // Sub-dependencies are indented with 6+ spaces
        let indent = line.len() - line.trim_start().len();
        if indent != 4 {
            continue; // Skip sub-dependencies
        }

        // Format: "    gem-name (1.2.3)"
        if let Some((name, version)) = parse_gem_line(trimmed) {
            packages.push(InstalledPackage {
                name,
                version,
                ecosystem: Ecosystem::Ruby,
                source: "Gemfile.lock".into(),
                is_dev: false,
                workspace_members: Vec::new(),
            });
        }
    }

    packages
}

fn parse_gem_line(line: &str) -> Option<(String, String)> {
    let paren_start = line.find('(')?;
    let paren_end = line.find(')')?;

    let name = line[..paren_start].trim().to_string();
    let version = line[paren_start + 1..paren_end].trim().to_string();

    if name.is_empty() || version.is_empty() {
        None
    } else {
        Some((name, version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_gemfile_lock() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();

        fs::write(
            dir.join("Gemfile.lock"),
            r#"GEM
  remote: https://rubygems.org/
  specs:
    actioncable (7.1.3)
      actionpack (= 7.1.3)
    actionpack (7.1.3)
      rack (~> 3.0)
    rails (7.1.3)
      actioncable (= 7.1.3)
    puma (6.4.2)
      nio4r (~> 2.0)

PLATFORMS
  ruby

DEPENDENCIES
  rails (~> 7.1.0)
  puma (~> 6.0)
"#,
        )
        .unwrap();

        let result = parse(dir);
        assert_eq!(result.len(), 4);

        let rails = result.iter().find(|p| p.name == "rails").unwrap();
        assert_eq!(rails.version, "7.1.3");

        let puma = result.iter().find(|p| p.name == "puma").unwrap();
        assert_eq!(puma.version, "6.4.2");
    }
}
