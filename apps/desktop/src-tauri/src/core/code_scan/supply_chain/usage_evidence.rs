use super::*;

/// Collect dependency use outside the primary source walk.
pub(super) fn collect_extra_usage_evidence(project_files: &[ProjectFile]) -> HashSet<String> {
    const MAX_EVIDENCE_FILES: usize = 2_000;
    const MAX_EVIDENCE_BYTES: u64 = 1_000_000;
    let mut names = HashSet::new();
    let mut read_count = 0usize;
    for file in project_files {
        let Some(ext) = std::path::Path::new(&file.relative_path)
            .extension()
            .and_then(|value| value.to_str())
        else {
            continue;
        };
        let ext = ext.to_ascii_lowercase();
        let is_component = matches!(ext.as_str(), "vue" | "svelte" | "astro");
        let is_style = matches!(ext.as_str(), "css" | "scss" | "sass" | "less");
        let file_name = std::path::Path::new(&file.relative_path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let is_tsconfig = (file_name == "tsconfig.json"
            || (file_name.starts_with("tsconfig.") && file_name.ends_with(".json")))
            && ext == "json";
        let is_test_source = JS_SOURCE_EXTENSIONS
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&ext))
            && is_test_like_path(std::path::Path::new(&file.relative_path));
        if !is_component && !is_style && !is_test_source && !is_tsconfig {
            continue;
        }
        if read_count >= MAX_EVIDENCE_FILES {
            break;
        }
        read_count += 1;
        let Some(bytes) = read_project_file(file, MAX_EVIDENCE_BYTES) else {
            continue;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };
        if is_style {
            for line in content.lines() {
                if let Some(name) = style_import_package(line) {
                    names.insert(name);
                }
            }
        } else if is_tsconfig {
            for line in content.lines() {
                if let Some(name) = tsconfig_jsx_import_source_package(line) {
                    names.insert(name);
                }
            }
        } else {
            names.extend(collect_package_names_in_content(&content));
        }
    }
    names
}

/// Package selected by TypeScript's `compilerOptions.jsxImportSource`.
fn tsconfig_jsx_import_source_package(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line.strip_prefix(r#""jsxImportSource""#)?;
    let value = rest.strip_prefix(':')?.trim().trim_end_matches(',').trim();
    let specifier = serde_json::from_str::<String>(value).ok()?;
    normalize_package_spec(&specifier)
}

/// Package name referenced by a stylesheet `@import "pkg"` / `@use "pkg"`
/// line (bundler bare-specifier form, optional legacy `~` prefix).
/// Relative paths, URLs, and `sass:` builtins return None.
fn style_import_package(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("@import")
        .or_else(|| trimmed.strip_prefix("@use"))?;
    let rest = rest.trim_start();
    let rest = rest
        .strip_prefix("url(")
        .map(str::trim_start)
        .unwrap_or(rest);
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value = &rest[1..];
    let end = value.find(quote)?;
    let spec = value[..end].trim_start_matches('~');
    normalize_package_spec(spec)
}

#[cfg(test)]
mod tests {
    use super::{style_import_package, tsconfig_jsx_import_source_package};

    #[test]
    fn jsx_import_source_resolves_to_package_name() {
        assert_eq!(
            tsconfig_jsx_import_source_package(r#""jsxImportSource": "preact","#).as_deref(),
            Some("preact")
        );
        assert_eq!(
            tsconfig_jsx_import_source_package(
                r#""jsxImportSource": "@emotion/react/jsx-runtime""#
            )
            .as_deref(),
            Some("@emotion/react")
        );
        assert_eq!(
            tsconfig_jsx_import_source_package(r#""jsx": "react-jsx""#),
            None
        );
    }

    #[test]
    fn style_imports_resolve_to_package_names() {
        assert_eq!(
            style_import_package(r#"@import "normalize.css";"#).as_deref(),
            Some("normalize.css")
        );
        assert_eq!(
            style_import_package(r#"@import "~bootstrap/dist/css/bootstrap.css";"#).as_deref(),
            Some("bootstrap")
        );
        assert_eq!(
            style_import_package(r#"@use "@fontsource/inter/index.css";"#).as_deref(),
            Some("@fontsource/inter")
        );
        // Relative paths, URLs, and sass builtins are not packages.
        assert_eq!(style_import_package(r#"@import "./local.css";"#), None);
        assert_eq!(
            style_import_package(r#"@import url("https://cdn.example.com/f.css");"#),
            None
        );
        assert_eq!(style_import_package(r#"@use "sass:math";"#), None);
        assert_eq!(style_import_package("body { margin: 0; }"), None);
    }
}
