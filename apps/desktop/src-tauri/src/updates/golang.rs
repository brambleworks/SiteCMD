//! Parses Go module dependencies from `go.mod` and `go.sum`.

use std::path::Path;

use super::types::{Ecosystem, InstalledPackage};

pub fn parse(dir: &Path) -> Vec<InstalledPackage> {
    let content = match super::read_dependency_file(&dir.join("go.mod")) {
        Some(content) => content,
        None => return Vec::new(),
    };

    let mut packages = Vec::new();
    let mut in_require = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("require (") || trimmed == "require (" {
            in_require = true;
            continue;
        }

        if trimmed == ")" {
            in_require = false;
            continue;
        }

        // Single-line require: "require module/path v1.2.3"
        if trimmed.starts_with("require ") && !trimmed.contains('(') {
            if let Some(pkg) = parse_require_line(&trimmed[8..]) {
                packages.push(pkg);
            }
            continue;
        }

        // Inside require block
        if in_require {
            if let Some(pkg) = parse_require_line(trimmed) {
                packages.push(pkg);
            }
        }
    }

    packages
}

fn parse_require_line(line: &str) -> Option<InstalledPackage> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }

    // Skip "// indirect" marked dependencies
    let is_indirect = trimmed.contains("// indirect");
    let clean = trimmed.split("//").next()?.trim();

    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let module_path = parts[0];
    let version = parts[1].trim_start_matches('v');

    // Skip replace directives and incompatible versions
    if version.contains("+incompatible") || module_path.is_empty() {
        let clean_ver = version.trim_end_matches("+incompatible");
        if !clean_ver.is_empty() {
            return Some(InstalledPackage {
                name: module_path.to_string(),
                version: clean_ver.to_string(),
                ecosystem: Ecosystem::Go,
                source: "go.mod".into(),
                is_dev: is_indirect,
                workspace_members: Vec::new(),
            });
        }
        return None;
    }

    Some(InstalledPackage {
        name: module_path.to_string(),
        version: version.to_string(),
        ecosystem: Ecosystem::Go,
        source: "go.mod".into(),
        is_dev: is_indirect,
        workspace_members: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_go_mod() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();

        fs::write(
            dir.join("go.mod"),
            r#"module github.com/myorg/myapp

go 1.21

require (
	github.com/gin-gonic/gin v1.9.1
	github.com/go-sql-driver/mysql v1.7.1
	golang.org/x/text v0.14.0 // indirect
)

require github.com/stretchr/testify v1.8.4
"#,
        )
        .unwrap();

        let result = parse(dir);
        assert_eq!(result.len(), 4);

        let gin = result
            .iter()
            .find(|p| p.name == "github.com/gin-gonic/gin")
            .unwrap();
        assert_eq!(gin.version, "1.9.1");
        assert!(!gin.is_dev);

        let text = result
            .iter()
            .find(|p| p.name == "golang.org/x/text")
            .unwrap();
        assert_eq!(text.version, "0.14.0");
        assert!(text.is_dev); // indirect = dev
    }
}
