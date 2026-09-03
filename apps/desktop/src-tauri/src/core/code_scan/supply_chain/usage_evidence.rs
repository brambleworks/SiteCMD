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
        // Tests, mocks, fixtures, and examples are withheld from the
        // issue-emitting source walk, but a dependency they import is used.
        let relative = std::path::Path::new(&file.relative_path);
        let is_withheld_source = JS_SOURCE_EXTENSIONS
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&ext))
            && (is_test_like_path(relative) || is_example_like_path(relative));
        // Tool configuration selects plugins, presets, parsers, reporters, and
        // transforms by quoted package name rather than by import.
        let is_tool_config = is_tool_config_file_name(&file_name);
        if !is_component && !is_style && !is_withheld_source && !is_tsconfig && !is_tool_config {
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
            names.extend(quoted_package_names(&content));
        } else {
            names.extend(collect_package_names_in_content(&content));
            if is_tool_config {
                names.extend(quoted_package_names(&content));
            }
        }
    }
    names
}

/// Tool configuration whose file name follows no `*.config.*` convention.
static TOOL_CONFIG_FILE_NAMES: &[&str] = &["nest-cli.json"];

/// Whether a file name is tool configuration whose quoted strings select
/// packages (`vitest.config.ts`, `.eslintrc.cjs`, `jest.config.js`,
/// `babel.config.json`, `tsconfig.build.json`, `nest-cli.json`, ...).
fn is_tool_config_file_name(file_name: &str) -> bool {
    let name = file_name.strip_prefix('.').unwrap_or(file_name);
    TOOL_CONFIG_FILE_NAMES.contains(&name)
        || name.contains(".config.")
        || ["jest", "eslint", "vitest", "babel", "tsconfig"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// Package names appearing as quoted strings in tool configuration.
fn quoted_package_names(content: &str) -> HashSet<String> {
    const MAX_QUOTED_VALUES: usize = 2_000;
    let mut names = HashSet::new();
    let mut rest = content;
    let mut seen = 0usize;
    while let Some(open) = rest.find(['"', '\'']) {
        let quote = rest.as_bytes()[open];
        let after = &rest[open + 1..];
        let Some(close) = after.find(quote as char) else {
            break;
        };
        let value = &after[..close];
        rest = &after[close + 1..];
        seen += 1;
        if seen >= MAX_QUOTED_VALUES {
            break;
        }
        // A configuration value that names a package is the bare specifier
        // itself, not a path, a glob, or a sentence.
        if value.is_empty()
            || value.len() > 214
            || value.contains(char::is_whitespace)
            || value.contains(['*', '\\', '(', ')', '<', '>', '$', '^', '|', '=', ':'])
        {
            continue;
        }
        if let Some(name) = normalize_package_spec(value) {
            names.insert(name);
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
    use super::{
        is_tool_config_file_name, quoted_package_names, style_import_package,
        tsconfig_jsx_import_source_package,
    };

    #[test]
    fn tool_configuration_files_are_recognized() {
        for name in [
            "jest.config.ts",
            "vitest.config.mts",
            ".eslintrc.cjs",
            "eslint.config.js",
            "babel.config.json",
            "jest-e2e.ts",
            "tsconfig.build.json",
            "tailwind.config.cjs",
            // Named by convention rather than by a `*.config.*` pattern.
            "nest-cli.json",
        ] {
            assert!(is_tool_config_file_name(name), "{name}");
        }
        for name in ["package.json", "index.ts", "server.js", "readme.md"] {
            assert!(!is_tool_config_file_name(name), "{name}");
        }
    }

    /// A NestJS project names its schematics collection and CLI plugins in
    /// `nest-cli.json` and nowhere else, so a scan blind to that file reports
    /// those packages unused.
    #[test]
    fn nest_cli_configuration_names_count_as_usage() {
        let names = quoted_package_names(
            "{\n  \"language\": \"ts\",\n  \"collection\": \"@nestjs/schematics\",\n  \"sourceRoot\": \"src\",\n  \"compilerOptions\": {\n    \"plugins\": [\"@nestjs/swagger\"]\n  }\n}\n",
        );
        assert!(names.contains("@nestjs/schematics"), "{names:?}");
        assert!(names.contains("@nestjs/swagger"), "{names:?}");
    }

    #[test]
    fn quoted_configuration_values_resolve_to_package_names() {
        let names = quoted_package_names(
            "export default {\n  preset: \"ts-jest\",\n  parser: '@typescript-eslint/parser',\n  reporters: [\"default\", \"jest-junit\"],\n  setupFiles: [\"<rootDir>/test/env.ts\"],\n  testRegex: \".*\\\\.spec\\\\.ts$\",\n}\n",
        );
        assert!(names.contains("ts-jest"), "{names:?}");
        assert!(names.contains("@typescript-eslint/parser"), "{names:?}");
        assert!(names.contains("jest-junit"), "{names:?}");
        // Paths, globs, and regex sources are not package names.
        assert!(
            !names.iter().any(|name| name.contains("rootdir")),
            "{names:?}"
        );
        assert!(!names.iter().any(|name| name.contains("spec")), "{names:?}");
    }

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
