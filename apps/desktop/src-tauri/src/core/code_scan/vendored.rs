//! Detection of vendored third-party library bundles committed into a project.

/// Conservatively identifies vendored bundles so first-party source is never
/// skipped on a license banner or a single embedded data line alone.
pub(super) fn looks_like_vendored_library(content: &str) -> bool {
    let long_line_bytes: usize = content
        .lines()
        .filter(|line| line.len() > 2000)
        .map(|line| line.len())
        .sum();
    if long_line_bytes > 0 && long_line_bytes * 2 >= content.len() {
        return true;
    }
    if content.lines().count() < 1500 {
        return false;
    }
    let head: String = content.chars().take(2048).collect();
    head.trim_start().starts_with("/*!") || head.contains("@license") || head.contains("@preserve")
}

#[cfg(test)]
mod tests {
    use super::looks_like_vendored_library;

    #[test]
    fn detects_vendored_library_banners_and_minification() {
        // Large distribution bundles that ship a banner.
        let big_jquery = format!(
            "/*!\n * jQuery JavaScript Library v3.5.1\n */\n{}",
            "var a = 1;\n".repeat(2000)
        );
        assert!(looks_like_vendored_library(&big_jquery));
        let big_angular = format!(
            "/**\n * @license AngularJS v1.0.8\n */\n{}",
            "var b = 2;\n".repeat(2000)
        );
        assert!(looks_like_vendored_library(&big_angular));
        // Minified/bundled output: long lines dominate the file.
        let minified = format!("var a={};\n", "x".repeat(2500));
        assert!(looks_like_vendored_library(&minified));

        let with_blob = format!(
            "export const ICON = \"{}\";\n{}",
            "A".repeat(2500),
            "export function realCode() { return 1 }\n".repeat(200)
        );
        assert!(!looks_like_vendored_library(&with_blob));

        // First-party source with a SPDX/copyright `/*!` header (as OWASP Juice
        // Shop puts on every file) must NOT be mistaken for a vendored library:
        // a license header on a normal-sized file is first-party code.
        assert!(!looks_like_vendored_library(
            "/*!\n * Copyright (c) 2014-2026 The Authors.\n * SPDX-License-Identifier: MIT\n */\nexport const x = 1\n"
        ));
        // A large first-party file WITHOUT a banner stays analysable (god-module
        // and friends should still get a chance to flag it).
        let big_unbannered = format!("export const x = 1\n{}", "const y = 2\n".repeat(2000));
        assert!(!looks_like_vendored_library(&big_unbannered));
        // Plain first-party source.
        assert!(!looks_like_vendored_library(
            "import express from 'express'\n\nexport function handler(req, res) {\n  res.send('ok')\n}\n"
        ));
    }
}
