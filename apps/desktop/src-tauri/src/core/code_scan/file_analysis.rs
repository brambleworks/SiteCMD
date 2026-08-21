use super::*;
mod ai_checks;
mod architecture_checks;
mod js_sinks;
mod php_sinks;
mod python_sinks;
mod route_security;
mod service_security;
mod signals;

use ai_checks::collect_ai_issues;
use architecture_checks::collect_architecture_issues;
use js_sinks::collect_js_sink_issues;
use php_sinks::collect_php_sink_issues;
use python_sinks::collect_python_sink_issues;
use route_security::collect_route_security_issues;
use service_security::collect_service_security_issues;
use signals::FileAnalysisSignals;

/// Return a same-length executable-code view with comments and literals blanked.
pub(super) fn blank_non_code_for_env(file: &SourceFile) -> String {
    let lower_path = file.relative_path.to_ascii_lowercase();
    if lower_path.ends_with(".py") {
        blank_python(&file.content, true)
    } else if lower_path.ends_with(".php") {
        super::laravel_routes::blank_php(&file.content, true)
    } else if lower_path.ends_with(".rs") {
        blank_rust_non_code(&file.content)
    } else {
        js_sinks::blank_js(&file.content, true)
    }
}

/// Blank Rust comments and string literals while preserving byte offsets.
fn blank_rust_non_code(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = bytes.to_vec();
    let mut index = 0;
    let blank = |out: &mut Vec<u8>, start: usize, end: usize| {
        for slot in out.iter_mut().take(end).skip(start) {
            if !slot.is_ascii_whitespace() {
                *slot = b' ';
            }
        }
    };

    while index < bytes.len() {
        if let Some(end) = rust_raw_string_end(bytes, index) {
            blank(&mut out, index, end);
            index = end;
            continue;
        }
        match bytes[index] {
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let end = content[index..]
                    .find('\n')
                    .map(|offset| index + offset)
                    .unwrap_or(bytes.len());
                blank(&mut out, index, end);
                index = end;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let end = content[index + 2..]
                    .find("*/")
                    .map(|offset| index + offset + 4)
                    .unwrap_or(bytes.len());
                blank(&mut out, index, end);
                index = end;
            }
            b'"' => {
                let mut cursor = index + 1;
                while cursor < bytes.len() {
                    if bytes[cursor] == b'\\' {
                        cursor += 2;
                        continue;
                    }
                    if bytes[cursor] == b'"' {
                        cursor += 1;
                        break;
                    }
                    cursor += 1;
                }
                let end = cursor.min(bytes.len());
                blank(&mut out, index, end);
                index = end;
            }
            _ => index += 1,
        }
    }

    String::from_utf8(out).unwrap_or_else(|_| content.to_string())
}

fn rust_raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let raw_prefix = if bytes.get(start) == Some(&b'r') {
        start
    } else if bytes.get(start) == Some(&b'b') && bytes.get(start + 1) == Some(&b'r') {
        start + 1
    } else {
        return None;
    };
    if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        return None;
    }

    let mut quote = raw_prefix + 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hash_count = quote - raw_prefix - 1;
    let mut cursor = quote + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"'
            && (0..hash_count).all(|offset| bytes.get(cursor + 1 + offset) == Some(&b'#'))
        {
            return Some((cursor + 1 + hash_count).min(bytes.len()));
        }
        cursor += 1;
    }
    Some(bytes.len())
}

/// Blank Python comments and optionally strings while preserving UTF-8 byte
/// offsets and newlines. Sink checks use separate keyword and taint views.
pub(in crate::core::code_scan) fn blank_python(content: &str, blank_strings: bool) -> String {
    let bytes = content.as_bytes();
    let mut out = bytes.to_vec();
    let mut index = 0;
    // Preserve whitespace so byte offsets and line numbers remain stable.
    let blank = |out: &mut Vec<u8>, start: usize, end: usize| {
        for slot in out.iter_mut().take(end).skip(start) {
            if !slot.is_ascii_whitespace() {
                *slot = b' ';
            }
        }
    };
    while index < bytes.len() {
        match bytes[index] {
            b'#' => {
                let end = content[index..]
                    .find('\n')
                    .map(|offset| index + offset)
                    .unwrap_or(bytes.len());
                blank(&mut out, index, end);
                index = end;
            }
            quote @ (b'\'' | b'"') => {
                // Triple-quoted string when the same quote repeats twice more.
                let is_triple =
                    bytes.get(index + 1) == Some(&quote) && bytes.get(index + 2) == Some(&quote);
                let body_start = if is_triple { index + 3 } else { index + 1 };
                let mut cursor = body_start;
                let mut close = bytes.len();
                while cursor < bytes.len() {
                    if bytes[cursor] == b'\\' {
                        cursor += 2;
                        continue;
                    }
                    if is_triple {
                        if bytes[cursor] == quote
                            && bytes.get(cursor + 1) == Some(&quote)
                            && bytes.get(cursor + 2) == Some(&quote)
                        {
                            close = cursor + 3;
                            break;
                        }
                    } else if bytes[cursor] == quote || bytes[cursor] == b'\n' {
                        // A bare newline ends an unterminated single-line string.
                        close = if bytes[cursor] == quote {
                            cursor + 1
                        } else {
                            cursor
                        };
                        break;
                    }
                    cursor += 1;
                }
                let close = close.min(bytes.len());
                // Triple-quoted strings are overwhelmingly docstrings, so always
                // blank them; single-line strings only when blank_strings is set
                // (taint inside an f-string must survive the comment-only view).
                if blank_strings || is_triple {
                    blank(&mut out, index, close);
                }
                index = close;
            }
            _ => index += 1,
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| content.to_string())
}

pub(super) struct FileAnalysisContext<'a> {
    file: &'a SourceFile,
    content: &'a str,
    signals: FileAnalysisSignals,
    oauth_callback_like: bool,
    one_time_token_handler: bool,
    session_cookie_handler: bool,
    upload_handler: bool,
    multi_tenant_handler: bool,
    needs_outbound_guards: bool,
    responsibility_labels: Vec<&'static str>,
}

/// Per-file predicates collected during parallel analysis for the serial
/// operations phase. Raw pattern fields remain distinct from combined signals.
pub(super) struct FileSignalSummary {
    pub(super) pattern_registry: bool,
    pub(super) scanner_rule_impl: bool,
    pub(super) route_like: bool,
    pub(super) server_action_like: bool,
    pub(super) uses_env: bool,
    pub(super) uses_llm: bool,
    pub(super) touches_db: bool,
    pub(super) background_jobs: bool,
    pub(super) frontend_supabase: bool,
    pub(super) client_auth: bool,
    pub(super) healthcheck: bool,
    pub(super) error_reporting: bool,
    pub(super) structured_logging: bool,
    pub(super) ai_observability: bool,
    pub(super) feature_flags: bool,
    pub(super) error_boundary: bool,
    pub(super) job_visibility: bool,
    pub(super) job_marker_words: bool,
    pub(super) auth_enforcement: bool,
    pub(super) ai_heavy_marker: bool,
    pub(super) shared_data_layer: bool,
    pub(super) sensitive_handler: bool,
    pub(super) write_handler: bool,
    pub(super) inline_rust_tests: bool,
}

pub(super) fn analyze_file(
    file: &SourceFile,
    laravel_protection: &LaravelRouteProtection,
) -> (Vec<CodeIssue>, FileSignalSummary) {
    let mut issues = Vec::new();
    let content = &file.content;
    let (signals, summary) = FileAnalysisSignals::from_file(file, laravel_protection);
    let oauth_callback_like = signals.route_like
        && signals.has_oauth_code
        && signals.has_oauth_token_exchange
        && (file.relative_path.to_ascii_lowercase().contains("callback")
            || signals.lower.contains("oauth")
            || signals.lower.contains("openid"));
    let one_time_token_handler = signals.route_like
        && signals.has_request_token
        && signals.has_one_time_token_flow
        && (signals.touches_db || signals.sensitive_handler);
    let session_cookie_handler = signals.route_like
        && signals.has_cookie_write
        && (signals.has_session_cookie_name
            || signals.has_cookie_session
            || file.relative_path.to_ascii_lowercase().contains("auth")
            || file.relative_path.to_ascii_lowercase().contains("login")
            || file.relative_path.to_ascii_lowercase().contains("session"));
    let upload_handler = signals.route_like
        && signals.has_upload_flow
        && (signals.parses_body || signals.has_upload_input || signals.has_storage_write);
    let multi_tenant_handler = signals.route_like
        && signals.touches_db
        && signals.has_auth
        && signals.has_db_query
        && (signals.has_multi_tenant_context || is_multi_tenant_route_like(file));
    let needs_outbound_guards = signals.route_like
        && !signals.uses_llm
        && signals.uses_outbound_http
        && !signals.skips_internal_http;
    let path_lower = file.relative_path.to_ascii_lowercase();
    // Generic File and checkout tokens help route detection but are too weak
    // to assign architecture responsibility in non-route modules.
    let architecture_upload_flow = signals.has_upload_flow
        && (signals.route_like
            || signals.has_upload_input
            || signals.has_storage_write
            || path_lower.contains("upload"));
    let architecture_payment_flow = signals.has_payment_flow
        && (signals.route_like
            || signals.uses_stripe_checkout
            || path_lower.contains("billing")
            || path_lower.contains("payment")
            || path_lower.contains("subscription")
            || path_lower.contains("commerce")
            || signals.lower.contains("new stripe(")
            || signals.lower.contains("stripe."));
    let responsibility_labels = collect_code_responsibilities(ResponsibilitySignals {
        has_auth: signals.has_auth,
        has_authz: signals.has_authz,
        has_validation: signals.has_validation,
        touches_db: signals.touches_db,
        uses_llm: signals.uses_llm,
        uses_outbound_http: needs_outbound_guards,
        is_webhook: signals.is_webhook,
        has_upload_flow: architecture_upload_flow,
        has_payment_flow: architecture_payment_flow,
        has_email_flow: signals.has_email_flow,
        has_background_jobs: has_any(content, &BACKGROUND_JOB_PATTERNS),
        dangerous_html: signals.dangerous_html,
    });
    let ctx = FileAnalysisContext {
        file,
        content,
        signals,
        oauth_callback_like,
        one_time_token_handler,
        session_cookie_handler,
        upload_handler,
        multi_tenant_handler,
        needs_outbound_guards,
        responsibility_labels,
    };

    collect_ai_issues(&mut issues, &ctx);
    collect_route_security_issues(&mut issues, &ctx);
    collect_service_security_issues(&mut issues, &ctx);
    // Run precise sink analysis first so broader architecture heuristics can
    // avoid duplicate findings.
    collect_js_sink_issues(&mut issues, &ctx);
    collect_architecture_issues(&mut issues, &ctx);
    collect_php_sink_issues(&mut issues, &ctx);
    collect_python_sink_issues(&mut issues, &ctx);

    (issues, summary)
}

fn is_js_or_ts_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".mjs")
        || lower.ends_with(".cjs")
}

fn is_next_config_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    matches!(
        base,
        "next.config.js" | "next.config.mjs" | "next.config.cjs" | "next.config.ts"
    )
}

fn is_typescript_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".ts") || lower.ends_with(".tsx")
}

fn is_jsx_or_tsx_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".tsx") || lower.ends_with(".jsx")
}

pub(super) fn is_config_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("config")
        || lower.contains(".env")
        || lower.ends_with(".json")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
}

fn looks_like_pattern_registry(content: &str) -> bool {
    if content.contains("LazyLock<Vec<regex::Regex>>")
        || content.matches("regex::Regex::new").count() >= 6
        // Scanner signal aggregators consume many pattern catalogs without
        // compiling the expressions themselves. Treating the marker names as
        // application behavior otherwise invents auth, billing, AI, and DB
        // responsibilities in the scanner implementation.
        || (content.matches("_PATTERNS").count() >= 12
            && content.matches("has_any(").count() >= 6)
    {
        return true;
    }

    // Several distinct placeholder literals identify a detection-rule registry,
    // not an application containing real secrets.
    const DETECTION_MARKERS: &[&str] = &[
        "changeme",
        "change-me",
        "placeholder",
        "redacted",
        "dummy",
        "supersecretkey",
        "password123",
        "your-api-key",
        "your_api_key",
        "your_value_here",
        "example-key",
        "fake-key",
        "test-secret",
        "replace_me",
        "replace-me",
        "not-set",
    ];
    let lower = content.to_ascii_lowercase();
    let distinct_markers = DETECTION_MARKERS
        .iter()
        .filter(|marker| lower.contains(*marker))
        .count();
    if distinct_markers >= 4 {
        return true;
    }

    // Several quoted route markers identify rule data rather than live routes.
    // Exclude markers that real PHP routinely quotes or interpolates.
    const QUOTED_ROUTE_MARKERS: &[&str] = &[
        "route::get(",
        "route::post(",
        "route::put(",
        "route::patch(",
        "route::delete(",
        "route::any(",
        "route::resource(",
        "router.post(",
        "router.put(",
        "router.patch(",
        "router.delete(",
        "app.post(",
        "app.put(",
        "app.patch(",
        "app.delete(",
        "export async function get(",
        "export async function post(",
        "register_rest_route(",
        "dangerouslysetinnerhtml",
    ];
    let quoted_route_markers = QUOTED_ROUTE_MARKERS
        .iter()
        .filter(|marker| has_delimiter_prefixed_occurrence(&lower, marker))
        .count();
    quoted_route_markers >= 4
}

/// Whether `marker` appears as quoted data rather than executable code.
///
/// Backslashes escape quote-leading markers but qualify bare PHP names, matching
/// `route_helpers::contains_unquoted`.
fn has_delimiter_prefixed_occurrence(lower: &str, marker: &str) -> bool {
    let escape_marks_data = marker.starts_with('"') || marker.starts_with('\'');
    let mut search = 0;
    while let Some(pos) = lower[search..].find(marker) {
        let at = search + pos;
        let preceding = lower[..at].chars().next_back();
        if matches!(preceding, Some('"' | '\'' | '`'))
            || (escape_marks_data && preceding == Some('\\'))
        {
            return true;
        }
        search = at + marker.len();
    }
    false
}

#[cfg(test)]
mod pattern_registry_tests {
    use super::looks_like_pattern_registry;

    #[test]
    fn recognizes_marker_definition_files() {
        let markers =
            r#"const PLACEHOLDER_MARKERS = ["changeme", "placeholder", "redacted", "dummy"];"#;
        assert!(looks_like_pattern_registry(markers));
        // A normal source file is not a pattern registry.
        assert!(!looks_like_pattern_registry(
            "export function add(a, b) { return a + b; }"
        ));
    }

    #[test]
    fn recognizes_quoted_route_marker_definitions() {
        // A scanner's route-helper source quotes many distinct declaration
        // markers; that shape must classify as rule-definition code.
        assert!(looks_like_pattern_registry(
            r#"const ROUTE_MARKERS = ["route::get(", "route::post(", "app.post(", "router.post(", "export async function post("];"#
        ));
        // Real declarations are code, not data: an actual Express app with
        // several live routes stays analysable.
        assert!(!looks_like_pattern_registry(
            "app.post('/a', a);\napp.put('/b', b);\napp.patch('/c', c);\napp.delete('/d', d);\nrouter.post('/e', e);\n"
        ));
    }

    #[test]
    fn recognizes_pattern_signal_aggregators() {
        let mut source = String::from("fn signals(content: &str) {\n");
        for index in 0..12 {
            source.push_str(&format!(
                "let signal_{index} = has_any(content, &RULE_{index}_PATTERNS);\n"
            ));
        }
        source.push('}');
        assert!(looks_like_pattern_registry(&source));

        assert!(!looks_like_pattern_registry(
            "let auth = has_any(content, &AUTH_PATTERNS);"
        ));
    }

    #[test]
    fn fqcn_laravel_routes_are_not_a_pattern_registry() {
        assert!(!looks_like_pattern_registry(
            "<?php\n\\Route::get('/a', A::class);\n\\Route::post('/b', B::class);\n\\Route::put('/c', C::class);\n\\Route::delete('/d', D::class);\n"
        ));
    }
}

#[cfg(test)]
mod blank_python_tests {
    use super::blank_python;

    #[test]
    fn preserves_fstrings_but_removes_comments_and_docstrings() {
        let src = "def f():\n    \"\"\"os.system(x) here\"\"\"\n    y = f\"{request.args['q']}\"  # note request.form\n";

        let scan = blank_python(src, false);
        // Byte offsets and newlines are preserved so line numbers stay correct.
        assert_eq!(scan.len(), src.len());
        assert_eq!(scan.matches('\n').count(), src.matches('\n').count());
        // Docstring body and the `#` comment are blanked in both views.
        assert!(
            !scan.contains("os.system"),
            "docstring not blanked: {scan:?}"
        );
        assert!(
            !scan.contains("request.form"),
            "comment not blanked: {scan:?}"
        );
        // The f-string argument survives the comment-only view so taint is seen.
        assert!(
            scan.contains("request.args['q']"),
            "f-string was wrongly blanked: {scan:?}"
        );

        // The structure view additionally blanks all string literals.
        let structure = blank_python(src, true);
        assert_eq!(structure.len(), src.len());
        assert!(!structure.contains("request.args['q']"));
    }
}
