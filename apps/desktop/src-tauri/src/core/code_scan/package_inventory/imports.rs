use super::*;

pub(in crate::core::code_scan) fn collect_js_package_refs(
    files: &[SourceFile],
) -> Vec<PackageReference> {
    let mut refs = Vec::new();

    for file in files {
        let Some(ext) = file
            .absolute_path
            .extension()
            .and_then(|value| value.to_str())
        else {
            continue;
        };
        if !JS_SOURCE_EXTENSIONS
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(ext))
        {
            continue;
        }

        // Commented-out imports are not dependencies.
        // Blanking preserves byte positions, so line numbers computed
        // against the original content stay correct.
        let scannable = blank_js_comments(&file.content);
        for pattern in IMPORT_CAPTURE_PATTERNS.iter() {
            for capture in pattern.captures_iter(&scannable) {
                let Some(specifier_match) = capture.get(1) else {
                    continue;
                };
                let Some(package_name) = normalize_package_spec(specifier_match.as_str()) else {
                    continue;
                };

                refs.push(PackageReference {
                    package_name,
                    relative_path: file.relative_path.clone(),
                    absolute_path: file.absolute_path.to_string_lossy().to_string(),
                    line: Some(line_number(&file.content, specifier_match.start())),
                });
            }
        }
    }

    refs
}

/// Blank `//` and `/* */` comment interiors with spaces (newlines kept,
/// byte positions preserved) so commented-out imports are never parsed
/// as package references. String and template literals are tracked so a
/// `/*` inside a string does not start a comment.
pub(in crate::core::code_scan) fn blank_js_comments(content: &str) -> String {
    enum State {
        Code,
        Line,
        Block,
        Str(u8),
    }
    let bytes = content.as_bytes();
    let mut out = bytes.to_vec();
    let mut state = State::Code;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        match state {
            State::Code => match byte {
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    state = State::Line;
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    i += 2;
                    continue;
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    state = State::Block;
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    i += 2;
                    continue;
                }
                b'"' | b'\'' | b'`' => state = State::Str(byte),
                _ => {}
            },
            State::Line => {
                if byte == b'\n' {
                    state = State::Code;
                } else {
                    out[i] = b' ';
                }
            }
            State::Block => {
                if byte == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    state = State::Code;
                    i += 2;
                    continue;
                }
                if byte != b'\n' {
                    out[i] = b' ';
                }
            }
            State::Str(quote) => {
                if byte == b'\\' {
                    i += 2;
                    continue;
                }
                if byte == quote || (quote != b'`' && byte == b'\n') {
                    state = State::Code;
                }
            }
        }
        i += 1;
    }
    // Only ASCII bytes are overwritten with spaces (multibyte chars inside
    // comments are blanked byte-by-byte, whole), so this stays valid UTF-8.
    String::from_utf8(out).unwrap_or_else(|_| content.to_string())
}

/// Package names referenced by imports in content outside the normal source walk.
pub(in crate::core::code_scan) fn collect_package_names_in_content(
    content: &str,
) -> std::collections::HashSet<String> {
    let content = blank_js_comments(content);
    let mut names = std::collections::HashSet::new();
    for pattern in IMPORT_CAPTURE_PATTERNS.iter() {
        for capture in pattern.captures_iter(&content) {
            if let Some(name) = capture
                .get(1)
                .and_then(|m| normalize_package_spec(m.as_str()))
            {
                names.insert(name);
            }
        }
    }
    names
}

pub(in crate::core::code_scan) fn normalize_package_spec(specifier: &str) -> Option<String> {
    let specifier = specifier.trim();
    if specifier.is_empty()
        || specifier.starts_with('.')
        || specifier.starts_with('/')
        || specifier.starts_with("@/")
        || specifier.starts_with("~/")
        || specifier.starts_with('#')
        || specifier.starts_with('$')
        || specifier.starts_with("http://")
        || specifier.starts_with("https://")
        // Node builtins are never npm dependencies, regardless of whether
        // NODE_BUILTINS lists the exact name (e.g. `node:module`).
        || specifier.starts_with("node:")
    {
        return None;
    }

    // Scheme-prefixed virtual modules are not npm packages.
    if specifier
        .split('/')
        .next()
        .is_some_and(|head| head.contains(':'))
    {
        return None;
    }

    let package_name = if specifier.starts_with('@') {
        let mut segments = specifier.split('/');
        let scope = segments.next()?;
        let name = segments.next()?;
        format!("{}/{}", scope, name)
    } else {
        specifier.split('/').next()?.to_string()
    };

    if NODE_BUILTINS.iter().any(|builtin| *builtin == package_name) {
        return None;
    }

    if COMMON_INTERNAL_IMPORT_ROOTS
        .iter()
        .any(|candidate| *candidate == package_name)
        && specifier.contains('/')
    {
        return None;
    }

    Some(package_name.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_framework_virtual_modules_and_node_builtins() {
        assert_eq!(normalize_package_spec("astro:content"), None);
        assert_eq!(normalize_package_spec("astro:middleware"), None);
        assert_eq!(normalize_package_spec("cloudflare:workers"), None);
        assert_eq!(normalize_package_spec("virtual:my-plugin"), None);
        assert_eq!(normalize_package_spec("node:module"), None);
        assert_eq!(normalize_package_spec("node:fs"), None);
        // Bare builtins (no `node:` prefix) must also be recognised -- these were
        // reported as undeclared on twenty/n8n.
        assert_eq!(normalize_package_spec("async_hooks"), None);
        assert_eq!(normalize_package_spec("worker_threads"), None);
        assert_eq!(normalize_package_spec("vm"), None);
        assert_eq!(normalize_package_spec("tls"), None);
        assert_eq!(normalize_package_spec("querystring"), None);
        assert_eq!(normalize_package_spec("perf_hooks"), None);
        // Subpath builtins reduce to their first segment.
        assert_eq!(normalize_package_spec("fs/promises"), None);
    }

    #[test]
    fn keeps_real_npm_packages() {
        assert_eq!(normalize_package_spec("preact"), Some("preact".to_string()));
        assert_eq!(
            normalize_package_spec("@astrojs/preact"),
            Some("@astrojs/preact".to_string())
        );
        assert_eq!(
            normalize_package_spec("react/jsx-runtime"),
            Some("react".to_string())
        );
    }

    fn source_file(content: &str) -> SourceFile {
        SourceFile {
            absolute_path: std::path::PathBuf::from("/tmp/sample.ts"),
            relative_path: "sample.ts".to_string(),
            content: content.to_string(),
            line_count: content.lines().count(),
        }
    }

    #[test]
    fn ignores_import_keyword_inside_string_literal() {
        let file = source_file(
            "export function detectLanguage (code: string): string {\n  if (code.includes('import ') || code.includes('export ') || code.includes(': ')) return 'typescript'\n}\n",
        );
        let refs = collect_js_package_refs(std::slice::from_ref(&file));
        assert!(
            refs.is_empty(),
            "expected no package refs, got: {:?}",
            refs.iter().map(|r| &r.package_name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn commented_out_imports_are_not_package_references() {
        let file = source_file(
            "/*\nimport ghost from \"ghost-package\";\nconst old = require(\"legacy-pkg\");\n*/\n// const gone = require(\"line-comment-pkg\");\nconst s = \"/* not a comment\";\nimport real from \"real-pkg\";\n",
        );
        let names = collect_js_package_refs(std::slice::from_ref(&file))
            .into_iter()
            .map(|r| r.package_name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["real-pkg".to_string()],
            "only the uncommented import counts (a string containing /* must not hide it)"
        );
    }

    #[test]
    fn still_detects_real_import_statements() {
        let file = source_file(
            "import { EditorView } from '@codemirror/view'\nimport express from 'express'\nconst pg = require('pg')\nconst mod = await import('lodash')\n",
        );
        let names = collect_js_package_refs(std::slice::from_ref(&file))
            .into_iter()
            .map(|r| r.package_name)
            .collect::<Vec<_>>();
        assert!(names.contains(&"@codemirror/view".to_string()), "{names:?}");
        assert!(names.contains(&"express".to_string()), "{names:?}");
        assert!(names.contains(&"pg".to_string()), "{names:?}");
        assert!(names.contains(&"lodash".to_string()), "{names:?}");
    }

    #[test]
    fn detects_multiline_imports_and_package_re_exports() {
        let file = source_file(
            "import {\n  createTransport,\n  type Envelope,\n} from '@sentry/core';\nexport { captureException }\n  from \"@sentry/browser\";\nexport * from 'zod';\n",
        );
        let names = collect_js_package_refs(std::slice::from_ref(&file))
            .into_iter()
            .map(|reference| reference.package_name)
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(
            names,
            std::collections::HashSet::from([
                "@sentry/core".to_string(),
                "@sentry/browser".to_string(),
                "zod".to_string(),
            ])
        );
    }
}
