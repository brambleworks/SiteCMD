use super::super::{is_example_like_path, is_test_like_path};
use super::*;
use std::path::Path;

const TSCONFIG_MAX_BYTES: u64 = 250_000;

/// True for a `tsconfig.json` or any `tsconfig.*.json` variant (base, app,
/// node, etc.), matched on the final path component only.
fn is_tsconfig_path(relative_path: &str) -> bool {
    let name = relative_path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(relative_path)
        .to_ascii_lowercase();
    name == "tsconfig.json" || (name.starts_with("tsconfig.") && name.ends_with(".json"))
}

/// Strip JSONC comments without treating comment markers inside strings as
/// syntax. Replacing comment bytes with spaces preserves line numbers.
fn strip_jsonc_comments(content: &str) -> String {
    let chars = content.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(content.len());
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < chars.len() {
        let ch = chars[index];
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            index += 1;
            continue;
        }
        if ch == '/' && chars.get(index + 1) == Some(&'/') {
            output.push(' ');
            output.push(' ');
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                output.push(' ');
                index += 1;
            }
            continue;
        }
        if ch == '/' && chars.get(index + 1) == Some(&'*') {
            output.push(' ');
            output.push(' ');
            index += 2;
            while index < chars.len() {
                if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                    output.push(' ');
                    output.push(' ');
                    index += 2;
                    break;
                }
                output.push(if chars[index] == '\n' { '\n' } else { ' ' });
                index += 1;
            }
            continue;
        }
        output.push(ch);
        index += 1;
    }
    output
}

/// Remove JSONC trailing commas outside string values so serde_json can parse
/// the effective object without adding a second JSON parser dependency.
fn strip_trailing_commas(content: &str) -> String {
    let chars = content.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(content.len());
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in chars.iter().copied().enumerate() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch == ',' {
            let next = chars[index + 1..]
                .iter()
                .copied()
                .find(|candidate| !candidate.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                output.push(' ');
                continue;
            }
        }
        output.push(ch);
    }
    output
}

fn normalized_tsconfig(content: &str) -> Option<String> {
    let without_comments = strip_jsonc_comments(content);
    let normalized = strip_trailing_commas(&without_comments);
    serde_json::from_str::<Value>(&normalized).ok()?;
    Some(normalized)
}

/// Returns the name of the explicitly-disabled type-safety setting, or `None`.
/// Only an explicit `false` fires: a missing `strict` may be inherited from an
/// `extends` base, so absence is never flagged.
fn tsconfig_weakens_type_safety(content: &str) -> Option<&'static str> {
    let normalized = normalized_tsconfig(content)?;
    let json = serde_json::from_str::<Value>(&normalized).ok()?;
    let options = json.get("compilerOptions")?.as_object()?;
    if options.get("strict").and_then(Value::as_bool) == Some(false) {
        Some("strict")
    } else if options.get("noImplicitAny").and_then(Value::as_bool) == Some(false) {
        Some("noImplicitAny")
    } else {
        None
    }
}

/// Flag TypeScript configs that explicitly turn off `strict` (or `noImplicitAny`),
/// the setting that lets untyped `any` values flow through the whole project.
pub(super) fn collect_typescript_config_issues(
    issues: &mut Vec<CodeIssue>,
    project_files: &[ProjectFile],
) {
    for file in project_files {
        if !is_tsconfig_path(&file.relative_path) {
            continue;
        }
        // Demo/playground and fixture tsconfigs relax strict mode on purpose and
        // are not the user's production config.
        let path = Path::new(&file.relative_path);
        if is_example_like_path(path) || is_test_like_path(path) {
            continue;
        }
        let Some(bytes) = read_project_file(file, TSCONFIG_MAX_BYTES) else {
            continue;
        };
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };
        let Some(setting) = tsconfig_weakens_type_safety(&content) else {
            continue;
        };
        let normalized = normalized_tsconfig(&content).unwrap_or_default();
        let line = find_line(&normalized, &format!("\"{setting}\""));
        let (severity, title, description, why_now, likely_fix, verify_hint) = if setting
            == "strict"
        {
            (
                Severity::Medium,
                "TypeScript strict mode is explicitly disabled",
                "This tsconfig explicitly sets `compilerOptions.strict` to false, disabling the strict-family checks unless individual options are re-enabled. SiteCMD does not resolve the full `extends` chain or determine which build target consumes this config, so the effective project impact needs review.",
                "Strict-family checks catch classes of nullability, implicit-any, function-parameter, property-initialization, and related type errors before runtime, but enabling them can be an intentional staged migration.",
                "Run `tsc --showConfig -p <this-config>` to inspect the effective options and confirm which build uses them. If the target should be strict, set `strict` to true and address surfaced errors with real types and narrowing; for a staged migration, enable selected flags deliberately and document the remaining exceptions.",
                "Run the exact production type-check command with this config and confirm the effective strict options match the project's policy. Exercise the changed code paths with tests rather than clearing errors through broad `any` or ignore directives.",
            )
        } else {
            (
                Severity::Low,
                "TypeScript noImplicitAny is explicitly disabled",
                "This tsconfig explicitly sets `compilerOptions.noImplicitAny` to false, allowing declarations and parameters with an inferred implicit `any`. Other strict-family checks may still be enabled. SiteCMD does not resolve the full `extends` chain or determine which build target consumes this config, so the effective project impact needs review.",
                "Implicit `any` removes compiler checks at unannotated boundaries, but this single override does not disable nullability or every other strict-family check and may be part of a staged migration.",
                "Run `tsc --showConfig -p <this-config>` to confirm the effective option and target. If implicit `any` is not intentional, remove the false override or set `noImplicitAny` to true, then add accurate annotations or narrowing at each reported boundary instead of replacing the errors with explicit `any`.",
                "Run the exact type-check and production build commands that consume this config. Confirm `noImplicitAny` resolves to the intended value and that the affected boundaries have meaningful types and behavior tests.",
            )
        };

        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("tsconfig-strict-off:{}", file.relative_path),
            category: "operations".into(),
            severity,
            title: title.into(),
            description: description.into(),
            relative_path: file.relative_path.clone(),
            absolute_path: file.absolute_path.to_string_lossy().to_string(),
            line,
            source_excerpt: excerpt_for_line(&content, line),
            evidence: Some(redact_evidence(format!(
                "`\"{setting}\": false` is set in {}, disabling strict type checking.",
                file.relative_path
            ))),
            why_now: Some(why_now.into()),
            likely_fix: Some(likely_fix.into()),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("The explicit false setting is direct evidence, but the scanner does not resolve tsconfig inheritance or prove which application target consumes this config.".into()),
            verify_hint: Some(verify_hint.into()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{is_tsconfig_path, tsconfig_weakens_type_safety};

    #[test]
    fn tsconfig_path_matching_covers_variants_only() {
        assert!(is_tsconfig_path("tsconfig.json"));
        assert!(is_tsconfig_path("apps/web/tsconfig.json"));
        assert!(is_tsconfig_path("tsconfig.app.json"));
        assert!(is_tsconfig_path("tsconfig.base.json"));

        assert!(!is_tsconfig_path("package.json"));
        assert!(!is_tsconfig_path("jsconfig.json"));
        assert!(!is_tsconfig_path("tsconfig.tsbuildinfo"));
        assert!(!is_tsconfig_path("my-tsconfig.json"));
        assert!(!is_tsconfig_path("src/tsconfig.ts"));
    }

    #[test]
    fn only_explicit_disabled_type_safety_flags() {
        // Explicit strict:false, in tight and spaced shapes.
        assert_eq!(
            tsconfig_weakens_type_safety(r#"{"compilerOptions":{"strict":false}}"#),
            Some("strict")
        );
        assert_eq!(
            tsconfig_weakens_type_safety(r#"{ "compilerOptions": { "strict" : false } }"#),
            Some("strict")
        );
        // strict on, but noImplicitAny explicitly overridden off.
        assert_eq!(
            tsconfig_weakens_type_safety(
                r#"{"compilerOptions":{"strict":true,"noImplicitAny":false}}"#
            ),
            Some("noImplicitAny")
        );

        // Strict on, or absent (possibly inherited via extends): never flagged.
        assert_eq!(
            tsconfig_weakens_type_safety(r#"{"compilerOptions":{"strict":true}}"#),
            None
        );
        assert_eq!(
            tsconfig_weakens_type_safety(r#"{"compilerOptions":{"target":"es2020"}}"#),
            None
        );

        // A commented-out disable must not fire.
        assert_eq!(
            tsconfig_weakens_type_safety(
                "{\n  \"compilerOptions\": {\n    // \"strict\": false is tempting\n    \"strict\": true\n  }\n}"
            ),
            None
        );

        // Block comments, unrelated objects, and URL-like string values are
        // not effective compiler options.
        assert_eq!(
            tsconfig_weakens_type_safety(
                "{\n  /* \"compilerOptions\": { \"strict\": false } */\n  \"compilerOptions\": { \"strict\": true }\n}"
            ),
            None
        );
        assert_eq!(
            tsconfig_weakens_type_safety(
                r#"{"tooling":{"strict":false},"compilerOptions":{"strict":true}}"#
            ),
            None
        );
        assert_eq!(
            tsconfig_weakens_type_safety(
                r#"{"extends":"https://example.test/tsconfig.json","compilerOptions":{"strict":true}}"#
            ),
            None
        );
    }
}
