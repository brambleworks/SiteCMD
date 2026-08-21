use std::path::Path;

use super::DetectedUrl;

const PROJECT_METADATA_READ_CAP_BYTES: u64 = 1_000_000;

pub(super) fn read_project_text(dir: &Path, relative_path: impl AsRef<Path>) -> Option<String> {
    let path = dir.join(relative_path);
    crate::core::safe_fs::read_bounded_text_under_root(dir, &path, PROJECT_METADATA_READ_CAP_BYTES)
}

pub(super) fn is_safe_project_directory(root: &Path, path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return false;
    }
    let (Ok(canonical_root), Ok(canonical_path)) =
        (std::fs::canonicalize(root), std::fs::canonicalize(path))
    else {
        return false;
    };
    canonical_path.starts_with(canonical_root)
}

pub(super) fn read_json(dir: &Path, filename: &str) -> Option<serde_json::Value> {
    serde_json::from_str(&read_project_text(dir, filename)?).ok()
}

pub(super) fn extract_port(script: &str) -> Option<u16> {
    for (pattern, _) in &[("--port ", true), ("-p ", true), ("PORT=", true)] {
        if let Some(pos) = script.find(pattern) {
            let num: String = script[pos + pattern.len()..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(port) = num.parse() {
                return Some(port);
            }
        }
    }
    if script.contains("next") {
        return Some(3000);
    }
    if script.contains("vite") || script.contains("svelte") {
        return Some(5173);
    }
    if script.contains("astro") {
        return Some(4321);
    }
    if script.contains("nuxt") {
        return Some(3000);
    }
    if script.contains("gatsby") {
        return Some(8000);
    }
    if script.contains("react-scripts") {
        return Some(3000);
    }
    if script.contains("angular") || script.contains("ng serve") {
        return Some(4200);
    }
    None
}

pub(super) fn extract_toml_string(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let rest = &line[start..];
    let end = rest.find('"')?;
    let val = &rest[..end];
    if val.starts_with("http") {
        Some(val.to_string())
    } else {
        None
    }
}

pub(super) fn extract_php_url(line: &str) -> Option<String> {
    // Only extract from lines that look like assignments
    if !line.contains('=') && !line.contains("define") {
        return None;
    }
    for delim in ['\'', '"'] {
        for part in line.split(delim) {
            let trimmed = part.trim();
            if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
                && !trimmed.contains(' ')
                && trimmed.len() > 10
            {
                return Some(
                    trimmed
                        .trim_end_matches(';')
                        .trim_end_matches('\'')
                        .trim_end_matches('"')
                        .to_string(),
                );
            }
        }
    }
    None
}

pub(super) fn parse_yaml_urls(content: &str, source: &str, urls: &mut Vec<DetectedUrl>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // Look for uri:, url:, or URLs in values
        if trimmed.starts_with("uri:") || trimmed.starts_with("url:") {
            let val = trimmed
                .split(':')
                .skip(1)
                .collect::<Vec<_>>()
                .join(":")
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if val.starts_with("http") {
                let env = if val.contains("dev") || val.contains("local") {
                    "development"
                } else if val.contains("staging") || val.contains("stg") {
                    "staging"
                } else {
                    "production"
                };
                urls.push(DetectedUrl {
                    url: val,
                    environment: env.into(),
                    source: source.into(),
                });
            }
        }
        // Catch "https://..." as keys in route files
        if trimmed.starts_with("\"http")
            || trimmed.starts_with("'http")
            || trimmed.starts_with("http")
        {
            let url = trimmed
                .trim_matches('"')
                .trim_matches('\'')
                .trim_end_matches(':')
                .trim_end_matches('/')
                .to_string();
            if url.starts_with("http") && url.contains('.') {
                urls.push(DetectedUrl {
                    url: format!("{}/", url),
                    environment: "production".into(),
                    source: source.into(),
                });
            }
        }
    }
}
