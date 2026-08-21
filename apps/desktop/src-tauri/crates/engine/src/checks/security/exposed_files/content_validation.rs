//! Reject catch-all 200 responses unless the body matches the requested file.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EnvBodyVerdict {
    /// Env-shaped content with at least one non-placeholder value assigned to
    /// a secret-bearing key name.
    SecretAssignments,
    /// Env-shaped content without a substantive secret-looking assignment.
    EnvFormatOnly,
    /// Not env-shaped content (catch-all route, JSON, prose, and so on).
    NotEnvContent,
}

fn substantive_secret_value(value: &str) -> bool {
    let value = value.trim().trim_matches(['\'', '"']);
    let lower = value.to_ascii_lowercase();
    if value.len() < 6
        || lower.starts_with("${")
        || lower.starts_with("<")
        || [
            "changeme",
            "change-me",
            "example",
            "password",
            "secret",
            "replace_me",
            "replace-me",
            "your_value_here",
            "todo",
            "undefined",
            "not-set",
        ]
        .contains(&lower.as_str())
        || lower.chars().all(|c| matches!(c, 'x' | '*' | '-'))
    {
        return false;
    }
    true
}

pub(super) fn classify_env_body(body: &str) -> EnvBodyVerdict {
    let mut env_lines = 0usize;
    let mut other_lines = 0usize;
    let mut has_secret_assignment = false;

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            other_lines += 1;
            continue;
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            || value.trim().is_empty()
        {
            other_lines += 1;
            continue;
        }

        env_lines += 1;
        let key_upper = key.to_ascii_uppercase();
        const SECRET_KEY_HINTS: &[&str] = &[
            "SECRET",
            "PASSWORD",
            "TOKEN",
            "KEY",
            "DATABASE",
            "DB_",
            "AUTH",
            "PRIVATE",
            "SMTP",
            "MAIL",
            "API",
            "CREDENTIAL",
        ];
        if SECRET_KEY_HINTS.iter().any(|hint| key_upper.contains(hint))
            && substantive_secret_value(value)
        {
            has_secret_assignment = true;
        }
    }

    if env_lines == 0 || other_lines > env_lines {
        EnvBodyVerdict::NotEnvContent
    } else if has_secret_assignment {
        EnvBodyVerdict::SecretAssignments
    } else {
        EnvBodyVerdict::EnvFormatOnly
    }
}

fn first_line_is_git_oid(body: &str) -> bool {
    let line = body.lines().next().unwrap_or("").trim();
    matches!(line.len(), 40 | 64) && line.chars().all(|c| c.is_ascii_hexdigit())
}

pub(super) fn expected_file_signature(
    path: &str,
    content_type: &str,
    body: &str,
) -> Option<&'static str> {
    if body.trim().is_empty() {
        return None;
    }
    let lower = body.to_ascii_lowercase();
    let html_response = content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");

    match path {
        "/.env" => match classify_env_body(body) {
            EnvBodyVerdict::SecretAssignments => Some(".env assignments with secret-like values"),
            EnvBodyVerdict::EnvFormatOnly => Some(".env-format assignments"),
            EnvBodyVerdict::NotEnvContent => None,
        },
        "/.git/HEAD"
            if body.trim_start().starts_with("ref: refs/") || first_line_is_git_oid(body) =>
        {
            Some("Git HEAD reference")
        }
        "/.git/config" if lower.contains("[core]") && lower.contains("repositoryformatversion") => {
            Some("Git configuration")
        }
        "/wp-config.php"
            if lower.contains("db_name")
                && lower.contains("db_password")
                && (lower.contains("<?php") || lower.contains("define(")) =>
        {
            Some("WordPress configuration source")
        }
        // Real .DS_Store data starts with 00 00 00 01 followed by Bud1, and the
        // full eight-byte prefix survives lossy UTF-8 conversion.
        "/.DS_Store" if body.as_bytes().starts_with(b"\x00\x00\x00\x01Bud1") => {
            Some("DS_Store binary header")
        }
        "/.htaccess"
            if !html_response
                && [
                    "rewriteengine ",
                    "rewriterule ",
                    "<ifmodule",
                    "authtype ",
                    "redirect ",
                ]
                .iter()
                .any(|marker| lower.contains(marker)) =>
        {
            Some("Apache directives")
        }
        "/web.config"
            if lower.contains("<configuration")
                && (lower.contains("<system.web") || lower.contains("<connectionstrings")) =>
        {
            Some("IIS configuration XML")
        }
        "/phpinfo.php" if lower.contains("phpinfo()") && lower.contains("php version") => {
            Some("phpinfo output")
        }
        "/debug.log" | "/error.log"
            if !html_response
                && [
                    "[error]",
                    "php warning:",
                    "php fatal error:",
                    "uncaught exception",
                    "stack trace:",
                    " trace]",
                ]
                .iter()
                .any(|marker| lower.contains(marker)) =>
        {
            Some("log output")
        }
        "/backup.sql" | "/database.sql"
            if !html_response
                && [
                    "-- mysql dump",
                    "-- postgresql database dump",
                    "create table ",
                    "insert into ",
                    "pg_dump",
                ]
                .iter()
                .any(|marker| lower.contains(marker)) =>
        {
            Some("SQL dump content")
        }
        "/backup.zip" if body.starts_with("PK\u{3}\u{4}") => Some("ZIP archive header"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WP_CONFIG_SAMPLE: &str = "<?php\n\
/**\n\
 * The base configuration for WordPress\n\
 *\n\
 * The wp-config.php creation script uses this file during the installation.\n\
 * You don't have to use the web site, you can copy this file to \"wp-config.php\"\n\
 * and fill in the values.\n\
 *\n\
 * This file contains the following configurations:\n\
 *\n\
 * * Database settings\n\
 * * Secret keys\n\
 * * Database table prefix\n\
 * * ABSPATH\n\
 *\n\
 * @link https://wordpress.org/documentation/article/editing-wp-config-php/\n\
 *\n\
 * @package WordPress\n\
 */\n\
\n\
// ** Database settings - You can get this info from your web host ** //\n\
/** The name of the database for WordPress */\n\
define( 'DB_NAME', 'database_name_here' );\n\
\n\
/** Database username */\n\
define( 'DB_USER', 'username_here' );\n\
\n\
/** Database password */\n\
define( 'DB_PASSWORD', 'password_here' );\n";

    #[test]
    fn generic_plain_text_does_not_verify_sensitive_files() {
        let catch_all = "Welcome to the API. See /docs for available routes.";
        for path in [
            "/.env",
            "/.git/HEAD",
            "/.git/config",
            "/wp-config.php",
            "/.DS_Store",
            "/.htaccess",
            "/web.config",
            "/phpinfo.php",
            "/debug.log",
            "/error.log",
            "/backup.sql",
            "/backup.zip",
            "/database.sql",
        ] {
            assert!(
                expected_file_signature(path, "text/plain", catch_all).is_none(),
                "generic text must not verify {path}"
            );
        }
    }

    #[test]
    fn representative_sensitive_content_matches_its_path() {
        let fixtures = [
            ("/.git/HEAD", "ref: refs/heads/main", "Git HEAD reference"),
            (
                "/.git/config",
                "[core]\nrepositoryformatversion = 0\n[remote \"origin\"]",
                "Git configuration",
            ),
            (
                "/wp-config.php",
                WP_CONFIG_SAMPLE,
                "WordPress configuration source",
            ),
            (
                "/.DS_Store",
                "\u{0}\u{0}\u{0}\u{1}Bud1\u{0}\u{0}",
                "DS_Store binary header",
            ),
            (
                "/.htaccess",
                "RewriteEngine On\nRewriteRule ^ index.php [L]",
                "Apache directives",
            ),
            (
                "/web.config",
                "<configuration><system.webServer></system.webServer></configuration>",
                "IIS configuration XML",
            ),
            (
                "/phpinfo.php",
                "<html><title>PHP 8.3 - phpinfo()</title><h1>PHP Version 8.3</h1></html>",
                "phpinfo output",
            ),
            (
                "/debug.log",
                "[14-Jul-2026 10:11:12 UTC] PHP Warning: example",
                "log output",
            ),
            (
                "/backup.sql",
                "-- MySQL dump\nCREATE TABLE users (id int);",
                "SQL dump content",
            ),
            ("/backup.zip", "PK\u{3}\u{4}binary", "ZIP archive header"),
        ];

        for (path, body, expected_label) in fixtures {
            assert_eq!(
                expected_file_signature(path, "application/octet-stream", body),
                Some(expected_label),
                "signature mismatch for {path}"
            );
        }
    }

    #[test]
    fn placeholders_do_not_make_env_content_critical() {
        assert_eq!(
            classify_env_body("API_KEY=changeme\nDB_PASSWORD=xxxxxx\nAPP_NAME=demo"),
            EnvBodyVerdict::EnvFormatOnly
        );
        assert_eq!(
            classify_env_body("API_KEY=a8d92f0c12\nAPP_NAME=demo"),
            EnvBodyVerdict::SecretAssignments
        );
    }

    #[test]
    fn arbitrary_phpinfo_html_does_not_match() {
        let shell = "<!doctype html><html><body><h1>Welcome</h1></body></html>";
        assert!(expected_file_signature("/phpinfo.php", "text/html", shell).is_none());
    }

    #[test]
    fn stock_wp_config_defines_land_past_the_old_sample_window() {
        let sample_500: String = WP_CONFIG_SAMPLE.chars().take(500).collect();
        assert!(
            expected_file_signature("/wp-config.php", "text/x-php", &sample_500).is_none(),
            "500-char sample must not reach the DB_* defines (fixture would not reproduce the bug)"
        );

        let sample_current: String = WP_CONFIG_SAMPLE
            .chars()
            .take(super::super::PROBE_SIGNATURE_SAMPLE_CHARS)
            .collect();
        assert_eq!(
            expected_file_signature("/wp-config.php", "text/x-php", &sample_current),
            Some("WordPress configuration source"),
            "current sample window must reach the DB_* defines"
        );
    }

    #[test]
    fn ds_store_magic_is_matched_at_offset_four_not_zero() {
        assert_eq!(
            expected_file_signature(
                "/.DS_Store",
                "application/octet-stream",
                "\u{0}\u{0}\u{0}\u{1}Bud1\u{0}\u{0}\u{0}\u{0}"
            ),
            Some("DS_Store binary header"),
            "genuine .DS_Store prefix must match"
        );
        assert!(
            expected_file_signature("/.DS_Store", "application/octet-stream", "Bud1\0\0\0\0")
                .is_none(),
            "a bare Bud1-at-offset-0 body is not a real .DS_Store and must not match"
        );
    }
}
