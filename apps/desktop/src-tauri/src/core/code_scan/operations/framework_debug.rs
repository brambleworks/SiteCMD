//! Detects framework debug modes in production-facing configuration.
//! Development, local, and test configuration is excluded.

use super::*;
use std::sync::LazyLock;

#[rustfmt::skip] // keeps the allow-expect justification on the .expect line
static DJANGO_DEBUG_TRUE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?m)^[ \t]*DEBUG\s*=\s*True\b")
        .expect("static Django DEBUG regex") // allow-expect: compile-time literal regex
});

static DJANGO_ALLOWED_HOSTS_WILDCARD: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?m)^[ \t]*ALLOWED_HOSTS\s*=\s*\[\s*["']\*["']\s*\]"#)
        .expect("static ALLOWED_HOSTS regex") // allow-expect: compile-time literal regex
});

static WP_DEBUG_TRUE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(?i)define\s*\(\s*["']WP_DEBUG["']\s*,\s*true\s*\)"#)
        .expect("static WP_DEBUG regex") // allow-expect: compile-time literal regex
});

/// Production-like Django settings; split layouts count only prod modules.
fn is_django_prodish_settings(relative_path: &str) -> bool {
    let lower = relative_path.to_ascii_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    if base == "settings.py" {
        return true;
    }
    lower.contains("settings") && matches!(base, "prod.py" | "production.py")
}

fn is_prodish_env_path(relative_path: &str) -> bool {
    let lower = relative_path.to_ascii_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    base == ".env.prod"
        || base == ".env.production"
        || base.starts_with(".env.prod.")
        || base.starts_with(".env.production.")
}

pub(super) fn collect_framework_debug_issues(
    issues: &mut Vec<CodeIssue>,
    files: &[SourceFile],
    env_files: &[EnvFileSnapshot],
) {
    for file in files {
        let lower_base = file
            .relative_path
            .to_ascii_lowercase()
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_string();

        if is_django_prodish_settings(&file.relative_path) {
            let debug_on = DJANGO_DEBUG_TRUE.is_match(&file.content);
            let hosts_wildcard = DJANGO_ALLOWED_HOSTS_WILDCARD.is_match(&file.content);
            if debug_on || hosts_wildcard {
                let flags = match (debug_on, hosts_wildcard) {
                    (true, true) => "`DEBUG = True` and `ALLOWED_HOSTS = ['*']` are",
                    (true, false) => "`DEBUG = True` is",
                    _ => "`ALLOWED_HOSTS = ['*']` is",
                };
                let title = match (debug_on, hosts_wildcard) {
                    (true, true) => "Django settings enable debug and accept every Host",
                    (true, false) => "Django settings enable debug mode",
                    _ => "Django settings accept every Host header",
                };
                let description = match (debug_on, hosts_wildcard) {
                    (true, true) => "This Django settings module enables `DEBUG` and disables Django's Host allowlist. If this module is selected in production, an unhandled exception can expose sensitive request, settings, template, query, and source context when Django renders its technical 500 response. Accepting every Host also removes Django's host-header validation; actual cache, absolute-URL, and password-reset impact depends on proxy and application behavior.",
                    (true, false) => "This Django settings module enables `DEBUG`. If it is selected in production, an unhandled exception can expose sensitive request, settings, template, query, and source context when Django renders its technical 500 response. The scan cannot determine which settings module the deployment selects.",
                    _ => "This Django settings module sets `ALLOWED_HOSTS = ['*']`, disabling Django's host-header allowlist if the deployment selects it. That widens exposure to Host-header-dependent bugs, but it does not by itself prove cache poisoning or password-reset hijacking; proxy validation and how the application constructs absolute URLs determine exploitability.",
                };
                issues.push(build_issue(
                    "framework-debug-enabled",
                    "security",
                    // Debug pages leak settings and stack traces: High. A
                    // wildcard host list alone enables Host-header attacks
                    // but leaks nothing by itself: Medium.
                    if debug_on { Severity::High } else { Severity::Medium },
                    title,
                    description,
                    file,
                    first_match_line_single(&file.content, &DJANGO_DEBUG_TRUE)
                        .or_else(|| first_match_line_single(&file.content, &DJANGO_ALLOWED_HOSTS_WILDCARD)),
                    Some(format!("{} set in {}.", flags, file.relative_path)),
                    Some("Make the selected production settings explicit. Parse and validate `DEBUG` as a boolean that defaults/fails closed to false, and parse `ALLOWED_HOSTS` into the exact public hostnames the service accepts. Add a production startup assertion so debug or an empty/wildcard host policy cannot be enabled accidentally; preserve detailed errors only in access-controlled logs and monitoring.".into()),
                    Some("Run the selected production settings in staging, inspect the resolved `DEBUG` and host policy without logging secrets, and trigger a controlled unhandled exception. Confirm the response is generic, details reach only approved telemetry, valid hosts work, and an unapproved Host receives Django's rejection before password-reset or absolute-URL generation.".into()),
                ));
            }
        }

        if lower_base == "wp-config.php" && WP_DEBUG_TRUE.is_match(&file.content) {
            issues.push(build_issue(
                "framework-debug-enabled",
                "security",
                Severity::High,
                "WordPress config enables WP_DEBUG",
                "This wp-config.php defines WP_DEBUG as true. If this file serves production, WordPress can enable additional diagnostics; whether visitors see notices or paths also depends on WP_DEBUG_DISPLAY, PHP display_errors, plugins, and the error path. Static source cannot determine the deployed file or effective runtime settings.",
                file,
                first_match_line_single(&file.content, &WP_DEBUG_TRUE),
                Some(format!("`define('WP_DEBUG', true)` is set in {}.", file.relative_path)),
                Some("Set `WP_DEBUG` false in production and enable it only through an explicit development configuration. Use PHP/server logging or an access-controlled error-monitoring integration for production diagnostics; `WP_DEBUG_LOG` depends on `WP_DEBUG` and is not a standalone production logger. Keep `WP_DEBUG_DISPLAY`/`display_errors` off for public responses.".into()),
                Some("Inspect effective production constants and PHP settings without exposing secrets, then trigger a controlled notice/error in staging. Confirm no stack, path, query, request, or credential detail appears in the response and approved server-side telemetry still records a useful sanitized event.".into()),
            ));
        }
    }

    for env_file in env_files {
        if !is_prodish_env_path(&env_file.relative_path) {
            continue;
        }
        let debug_value = env_file.entries.get("APP_DEBUG").map(|value| {
            value
                .trim_matches(|c| c == '"' || c == '\'')
                .trim()
                .to_ascii_lowercase()
        });
        if debug_value.as_deref() != Some("true") {
            continue;
        }
        let line = env_file
            .content
            .lines()
            .position(|line| line.trim_start().starts_with("APP_DEBUG"))
            .map(|index| index as u32 + 1);
        issues.push(CodeIssue {
            check_id: String::new(),
            id: format!("framework-debug-enabled:{}", env_file.relative_path),
            category: "security".into(),
            severity: Severity::High,
            title: "Production env file enables framework debug mode".into(),
            description: "This production-named environment file sets APP_DEBUG=true. If the deployment loads it in a framework that honors this flag, an unhandled error may render detailed stack, request, configuration, query, or source context. The scan cannot establish whether the file is tracked or selected at runtime, and exact disclosure depends on the framework and error handler.".into(),
            relative_path: env_file.relative_path.clone(),
            absolute_path: env_file.absolute_path.to_string_lossy().to_string(),
            line,
            source_excerpt: None,
            evidence: Some(redact_evidence(format!(
                "APP_DEBUG=true is set in {}.",
                env_file.relative_path
            ))),
            why_now: Some("Public error paths are routinely probed. When a production runtime honors a verbose debug flag, an otherwise ordinary exception can disclose context that helps an attacker or exposes sensitive values not filtered by the framework.".into()),
            likely_fix: Some("Set `APP_DEBUG=false` in production configuration, parse and validate the effective value at startup, and fail deployment when a production mode resolves to debug. Send sanitized details to access-controlled logs or monitoring instead of the response, and keep secret redaction enabled there.".into()),
            confidence: crate::checks::IssueConfidence::NeedsReview,
            confidence_reason: Some("The literal is present in a production-named env file, but the scan cannot establish whether that file is tracked, deployed, selected at runtime, or honored by the active framework.".into()),
            verify_hint: Some("Run the selected production configuration in staging and trigger a controlled unhandled error. Confirm the public response is generic; no stack, request data, paths, query text, configuration, or credentials appear; and approved telemetry receives a useful sanitized event.".into()),
        });
    }
}
