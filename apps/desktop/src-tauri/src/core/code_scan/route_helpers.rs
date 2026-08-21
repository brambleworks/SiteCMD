use super::*;

#[derive(Debug, Default, Clone)]

pub(super) struct NextMiddlewareProtection {
    pub(super) global: bool,
    pub(super) prefixes: Vec<String>,
}

impl NextMiddlewareProtection {
    fn covers(&self, route_path: &str) -> bool {
        self.global
            || self
                .prefixes
                .iter()
                .any(|prefix| route_path.starts_with(prefix))
    }
}

pub(super) fn apply_framework_auth_overrides(
    framework: Option<&str>,
    files: &[SourceFile],
    issues: &mut Vec<CodeIssue>,
) {
    if framework != Some("Next.js") {
        return;
    }

    let middleware_protection = collect_next_middleware_protection(files);
    if !middleware_protection.global && middleware_protection.prefixes.is_empty() {
        return;
    }

    issues.retain(|issue| {
        if !issue.id.starts_with("sensitive-auth:") {
            return true;
        }
        let Some(route_path) = route_path_from_relative_path(&issue.relative_path) else {
            return true;
        };
        !middleware_protection.covers(&route_path)
    });
}

pub(super) fn has_inline_secret_guard(lower: &str) -> bool {
    let reads_request_credential = lower.contains("authorization")
        || lower.contains("bearer ")
        || lower.contains("x-api-key")
        || lower.contains("api-key")
        || lower.contains("admin_key")
        || lower.contains("api_key")
        || lower.contains(".query(\"key\")")
        || lower.contains(".query('key')")
        || lower.contains(".headers.get(\"authorization\")")
        || lower.contains(".headers.get('authorization')")
        || lower.contains(".headers.get(\"x-api-key\")")
        || lower.contains(".headers.get('x-api-key')")
        || lower.contains(".header(\"authorization\")")
        || lower.contains(".header('authorization')")
        || lower.contains(".header(\"x-api-key\")")
        || lower.contains(".header('x-api-key')");
    let reads_secret_source = lower.contains("process.env")
        || lower.contains("c.env.")
        || lower.contains("env.")
        || lower.contains("std::env")
        || lower.contains("os.getenv");
    let compares_values = lower.contains("===")
        || lower.contains("!==")
        || lower.contains("==")
        || lower.contains("!=")
        || lower.contains("timingsafeequal");

    // Recognize custom constant-time checks, password verification, and session
    // token derivation that identity-library patterns cannot name.
    let custom_credential_auth = lower.contains("safecompare")
        || lower.contains("timingsafeequal")
        || lower.contains("bcrypt.compare")
        || lower.contains("argon2.verify")
        || lower.contains("derivesessiontoken")
        || lower.contains("createsessiontoken")
        || lower.contains("issuesessiontoken");

    (reads_request_credential && reads_secret_source && compares_values) || custom_credential_auth
}

/// WordPress registers HTTP-reachable handlers through hook names
/// (`wp_ajax_{action}`, `admin_post_{action}`) and `register_rest_route`.
/// The quoted-prefix matching keeps prose mentions from counting. Takes the
/// lowercased content the route helpers already work on.
pub(super) fn has_wp_handler_registration(lower: &str) -> bool {
    // Require unquoted hook markers so scanner rule definitions do not count
    // as WordPress registrations.
    contains_unquoted(lower, "'wp_ajax_")
        || contains_unquoted(lower, "\"wp_ajax_")
        || contains_unquoted(lower, "'admin_post_")
        || contains_unquoted(lower, "\"admin_post_")
        || contains_unquoted(lower, "register_rest_route(")
}

/// Treats WordPress Ajax and admin-post actions without `nopriv_` as
/// authenticated write handlers. Public-only actions are excluded because
/// they have no user session for CSRF to exploit.
pub(super) fn has_wp_logged_in_action(lower: &str) -> bool {
    // Positional matching avoids full-content copies and excludes only the
    // immediate nopriv_ variant.
    ["'wp_ajax_", "\"wp_ajax_", "'admin_post_", "\"admin_post_"]
        .iter()
        .any(|prefix| has_quoted_prefix_without_nopriv(lower, prefix))
}

fn has_quoted_prefix_without_nopriv(lower: &str, quoted_prefix: &str) -> bool {
    let mut search = 0;
    while let Some(pos) = lower[search..].find(quoted_prefix) {
        let at = search + pos;
        let after = at + quoted_prefix.len();
        // An occurrence preceded by another delimiter or an escape is the
        // prefix being defined as data in a rule source, not a registration.
        let marker_as_data = matches!(
            lower[..at].chars().next_back(),
            Some('"' | '\'' | '`' | '\\')
        );
        if !marker_as_data && !lower[after..].starts_with("nopriv_") {
            return true;
        }
        search = after;
    }
    false
}

/// Return whether every WordPress REST registration has a non-public
/// permission callback. Callback bodies may live in parent classes.
pub(super) fn wp_rest_routes_all_have_permission_callbacks(lower: &str) -> bool {
    let registration_count = lower.matches("register_rest_route(").count();
    if registration_count == 0 {
        return false;
    }
    let callback_count = lower.matches("'permission_callback'").count()
        + lower.matches("\"permission_callback\"").count();
    callback_count >= registration_count && !lower.contains("__return_true")
}

/// Return whether every WordPress HTTP entry point is access-controlled.
pub(super) fn is_wp_gated_surface(lower: &str) -> bool {
    if lower.contains("wp_ajax_nopriv_") || lower.contains("admin_post_nopriv_") {
        return false;
    }
    let has_rest = lower.contains("register_rest_route(");
    if has_rest && !wp_rest_routes_all_have_permission_callbacks(lower) {
        return false;
    }
    has_wp_logged_in_action(lower) || has_rest
}

/// Route markers matched only outside quoted rule definitions and generated data.
static ROUTE_DECLARATION_MARKERS: &[&str] = &[
    "route::get(",
    "route::post(",
    "route::put(",
    "route::patch(",
    "route::delete(",
    "route::any(",
    "route::match(",
    "route::resource(",
    "export async function get(",
    "export async function post(",
    "router.post(",
    "router.put(",
    "router.patch(",
    "router.delete(",
    "app.post(",
    "app.put(",
    "app.patch(",
    "app.delete(",
    "@app.post",
    "@router.post",
    "@app.put",
    "@router.put",
];

/// Write-capable route markers that avoid loose string-form method matches.
/// WordPress hook markers are handled separately.
static WRITE_HANDLER_MARKERS: &[&str] = &[
    "export async function post(",
    "export async function put(",
    "export async function patch(",
    "export async function delete(",
    "router.post(",
    "router.put(",
    "router.patch(",
    "router.delete(",
    "app.post(",
    "app.put(",
    "app.patch(",
    "app.delete(",
    "@app.post",
    "@router.post",
    "@app.put",
    "@router.put",
    "route::post",
    "route::put",
    "route::patch",
    "route::delete",
    "wp_rest_server::creatable",
    "wp_rest_server::editable",
    "wp_rest_server::deletable",
];

/// Whether `marker` occurs outside a quoted data definition.
///
/// Backslashes escape quote-leading markers, but qualify bare PHP names such as
/// `\Route::post`, which remain live declarations.
fn contains_unquoted(lower: &str, marker: &str) -> bool {
    let escape_marks_data = marker.starts_with('"') || marker.starts_with('\'');
    let mut search = 0;
    while let Some(pos) = lower[search..].find(marker) {
        let at = search + pos;
        let preceding = lower[..at].chars().next_back();
        let marker_as_data = matches!(preceding, Some('"' | '\'' | '`'))
            || (escape_marks_data && preceding == Some('\\'));
        if !marker_as_data {
            return true;
        }
        search = at + marker.len();
    }
    false
}

fn has_unquoted_marker(lower: &str, markers: &[&str]) -> bool {
    markers
        .iter()
        .any(|marker| contains_unquoted(lower, marker))
}

pub(super) fn is_route_like(file: &SourceFile, lower: &str) -> bool {
    let rel = file.relative_path.to_lowercase();
    // Laravel's resources/ tree is frontend assets: a file under an /api/
    // path there is an HTTP *client* wrapper, not a server route.
    let path_rule_applies = !rel.starts_with("resources/") && !rel.contains("/resources/");
    // PHP files under API or route paths are support material unless they read
    // raw requests or register framework routes.
    let php_direct_endpoint = !rel.ends_with(".php")
        || lower.contains("$_get")
        || lower.contains("$_post")
        || lower.contains("$_request")
        || lower.contains("php://input");
    let conventional_route_path = rel.starts_with("api/")
        || (rel.contains("/api/") && !rel.contains("/lib/api/"))
        || rel.contains("/src/api/")
        || rel.contains("/server/api/")
        || rel.contains("/pages/api/")
        || rel.starts_with("pages/api/")
        || rel.contains("/routes/")
        || rel.starts_with("routes/");
    (path_rule_applies && php_direct_endpoint && conventional_route_path)
        || rel.ends_with("/route.ts")
        || rel.ends_with("/route.js")
        || rel.ends_with("/route.tsx")
        || rel.ends_with("/route.jsx")
        || rel == "routes/web.php"
        || rel == "routes/api.php"
        || has_wp_handler_registration(lower)
        || has_unquoted_marker(lower, ROUTE_DECLARATION_MARKERS)
}

pub(super) fn is_write_handler(lower: &str) -> bool {
    has_unquoted_marker(lower, WRITE_HANDLER_MARKERS) || has_wp_logged_in_action(lower)
}

pub(super) fn is_server_action_like(lower: &str) -> bool {
    // The directive markers carry their own quotes; a rule source that quotes
    // the MARKER shows an extra delimiter or escape in front, and a real
    // `"use server"` directive never does.
    contains_unquoted(lower, "\"use server\"") || contains_unquoted(lower, "'use server'")
}

pub(super) fn is_frontend_surface(file: &SourceFile) -> bool {
    let normalized = file.relative_path.replace('\\', "/").to_ascii_lowercase();
    (normalized.starts_with("app/") && !normalized.starts_with("app/api/"))
        || (normalized.starts_with("src/app/") && !normalized.starts_with("src/app/api/"))
        || (normalized.starts_with("pages/") && !normalized.starts_with("pages/api/"))
        || (normalized.starts_with("src/pages/") && !normalized.starts_with("src/pages/api/"))
        || normalized.contains("/components/")
}

/// Excludes test fixtures from issue-emitting analysis while retaining them in
/// the project inventory used for test-coverage evidence.
pub(super) fn is_test_like_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    lower.contains("/__tests__/")
        || lower.contains("/tests/")
        || lower.contains("/test/")
        // Recognize NestJS, Playwright, and Cypress end-to-end naming forms that
        // do not match the generic .spec. suffix.
        || lower.contains("/e2e/")
        || lower.contains(".e2e-spec.")
        || lower.contains(".e2e.")
        || lower.contains(".cy.")
        || lower.ends_with("test.php")
        || lower.ends_with("testcase.php")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".test.tsx")
        || lower.ends_with(".test.js")
        || lower.ends_with(".test.jsx")
        || lower.ends_with(".test.mjs")
        || lower.ends_with(".test.cjs")
        || lower.ends_with(".spec.ts")
        || lower.ends_with(".spec.tsx")
        || lower.ends_with(".spec.js")
        || lower.ends_with(".spec.jsx")
        || lower.ends_with(".spec.mjs")
        || lower.ends_with(".spec.cjs")
        || lower.ends_with("_test.py")
        || lower.ends_with("_test.go")
        // Deno (and some JS projects) use the underscore `*_test.ts` convention
        // (like Go), which the dotted `.test.ts` checks above miss.
        || lower.ends_with("_test.ts")
        || lower.ends_with("_test.tsx")
        || lower.ends_with("_test.js")
        || lower.ends_with("_test.jsx")
        || lower.ends_with("_test.mjs")
        || lower.ends_with("_test.cjs")
        // Ruby RSpec and Minitest conventions.
        || lower.contains("/spec/")
        || lower.ends_with("_spec.rb")
        || lower.ends_with("_test.rb")
        // Cargo integration, benchmark, and sibling-test conventions.
        || lower.contains("/benches/")
        || file_name.eq_ignore_ascii_case("tests.rs")
        || lower.ends_with("_test.rs")
        || lower.ends_with("_tests.rs")
}

/// Exclude examples, fixtures, mocks, playgrounds, and teaching snippets.
pub(super) fn is_example_like_path(path: &Path) -> bool {
    // Prefix a separator so leading directory segments also match.
    let lower = format!("/{}", path.to_string_lossy().to_ascii_lowercase());
    lower.contains("/__mocks__/")
        || lower.contains("/__fixtures__/")
        || lower.contains("/fixtures/")
        || lower.contains("/mocks/")
        || lower.contains("/examples/")
        || lower.contains("/example/")
        || lower.contains("/playground/")
        || lower.contains("/playgrounds/")
        || lower.contains("/snippets/")
        || lower.contains("/fix-guides/")
        || lower.contains("/code-fix-guides/")
        || lower.contains(".example.")
        || lower.contains(".sample.")
        || lower.contains(".fixture.")
        || lower.contains(".mock.")
        || lower.contains(".stories.")
}

pub(super) fn is_sensitive_handler(file: &SourceFile, lower: &str) -> bool {
    let rel = file.relative_path.to_lowercase();
    let sensitive_path = [
        "admin",
        "billing",
        "settings",
        "account",
        "internal",
        "backoffice",
    ]
    .iter()
    .any(|needle| rel.contains(needle));
    let sensitive_content = [
        "admin",
        "billing",
        "subscription",
        "payment",
        "role",
        "permission",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    sensitive_path
        || (sensitive_content && (is_write_handler(lower) || is_server_action_like(lower)))
}

pub(super) fn is_multi_tenant_route_like(file: &SourceFile) -> bool {
    let rel = file.relative_path.to_ascii_lowercase();
    [
        "/workspaces/",
        "workspace/",
        "/organizations/",
        "/organization/",
        "/orgs/",
        "/teams/",
        "/tenants/",
        "/accounts/",
        "/memberships/",
    ]
    .iter()
    .any(|needle| rel.contains(needle))
}

/// Whether a lowercased path can satisfy nearby-test evidence.
pub(super) fn is_test_artifact_path(path: &str) -> bool {
    path.ends_with(".test.ts")
        || path.ends_with(".test.tsx")
        || path.ends_with(".test.js")
        || path.ends_with(".test.jsx")
        || path.ends_with(".spec.ts")
        || path.ends_with(".spec.tsx")
        || path.ends_with(".spec.js")
        || path.ends_with(".spec.jsx")
        || path.contains("/__tests__/")
}

/// `test_paths_lower` must be the project inventory pre-filtered through
/// `is_test_artifact_path`.
pub(super) fn has_nearby_test(relative_path: &str, test_paths_lower: &[&str]) -> bool {
    let normalized = relative_path.to_ascii_lowercase();
    let (dir, file_name) = normalized
        .rsplit_once('/')
        .unwrap_or(("", normalized.as_str()));
    let stem = file_name.split('.').next().unwrap_or(file_name);

    test_paths_lower.iter().any(|path| {
        if dir.is_empty() {
            path.contains(stem)
        } else {
            path.starts_with(dir) && path.contains(stem)
        }
    })
}

pub(super) fn has_inline_rust_tests(content_lower: &str) -> bool {
    content_lower.contains("#[cfg(test)]") || content_lower.contains("#[test]")
}

pub(super) fn collect_next_middleware_protection(files: &[SourceFile]) -> NextMiddlewareProtection {
    let mut protection = NextMiddlewareProtection::default();

    for file in files {
        if !is_next_middleware_file(&file.relative_path) {
            continue;
        }
        let lower = file.content.to_ascii_lowercase();
        if !(has_any(&file.content, &AUTH_PATTERNS)
            || has_inline_secret_guard(&lower)
            || lower.contains("withauth")
            || lower.contains("clerkmiddleware"))
        {
            continue;
        }

        let matcher_section = lower
            .find("matcher")
            .map(|index| &file.content[index..])
            .unwrap_or(&file.content);
        let mut found_prefix = false;
        for pattern in NEXT_MIDDLEWARE_MATCHER_PATTERNS.iter() {
            for capture in pattern.captures_iter(matcher_section) {
                let Some(raw_matcher) = capture.get(1).map(|value| value.as_str()) else {
                    continue;
                };
                let Some(prefix) = normalize_middleware_matcher(raw_matcher) else {
                    continue;
                };
                found_prefix = true;
                if !protection.prefixes.contains(&prefix) {
                    protection.prefixes.push(prefix);
                }
            }
        }

        if !found_prefix {
            protection.global = true;
        }
    }

    protection
}

pub(super) fn is_next_middleware_file(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/").to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "middleware.ts" | "middleware.js" | "src/middleware.ts" | "src/middleware.js"
    )
}

pub(super) fn normalize_middleware_matcher(raw_matcher: &str) -> Option<String> {
    if !raw_matcher.starts_with('/') {
        return None;
    }

    let mut normalized = raw_matcher.to_string();
    if let Some(prefix) = normalized.split(':').next() {
        normalized = prefix.to_string();
    }
    normalized = normalized.replace("(.*)", "");
    normalized = normalized.replace('*', "");
    normalized = normalized.trim_end_matches('/').to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub(super) fn route_path_from_relative_path(relative_path: &str) -> Option<String> {
    let normalized = relative_path.replace('\\', "/");
    let trimmed = normalized.strip_prefix("src/").unwrap_or(&normalized);

    if let Some(rest) = trimmed.strip_prefix("app/") {
        for suffix in ["/route.ts", "/route.js", "/route.tsx", "/route.jsx"] {
            if let Some(route) = rest.strip_suffix(suffix) {
                return Some(normalize_route_path(route));
            }
        }
    }

    if let Some(rest) = trimmed.strip_prefix("pages/") {
        for suffix in [".ts", ".js", ".tsx", ".jsx"] {
            if let Some(route) = rest.strip_suffix(suffix) {
                return Some(normalize_route_path(route));
            }
        }
    }

    None
}

pub(super) fn normalize_route_path(route: &str) -> String {
    let mut parts = Vec::new();
    for segment in route.split('/') {
        if segment.is_empty() || (segment.starts_with('(') && segment.ends_with(')')) {
            continue;
        }
        parts.push(segment);
    }
    format!("/{}", parts.join("/"))
}

#[derive(Debug, Clone, Copy)]

pub(super) struct ResponsibilitySignals {
    pub(super) has_auth: bool,
    pub(super) has_authz: bool,
    pub(super) has_validation: bool,
    pub(super) touches_db: bool,
    pub(super) uses_llm: bool,
    pub(super) uses_outbound_http: bool,
    pub(super) is_webhook: bool,
    pub(super) has_upload_flow: bool,
    pub(super) has_payment_flow: bool,
    pub(super) has_email_flow: bool,
    pub(super) has_background_jobs: bool,
    pub(super) dangerous_html: bool,
}

pub(super) fn collect_code_responsibilities(signals: ResponsibilitySignals) -> Vec<&'static str> {
    let mut labels = Vec::new();
    if signals.has_auth {
        labels.push("auth");
    }
    if signals.has_authz {
        labels.push("authorization");
    }
    if signals.has_validation {
        labels.push("validation");
    }
    if signals.touches_db {
        labels.push("database writes");
    }
    if signals.uses_llm {
        labels.push("AI calls");
    }
    if signals.uses_outbound_http {
        labels.push("external APIs");
    }
    if signals.is_webhook {
        labels.push("webhooks");
    }
    if signals.has_upload_flow {
        labels.push("uploads");
    }
    if signals.has_payment_flow {
        labels.push("billing");
    }
    if signals.has_email_flow {
        labels.push("email delivery");
    }
    if signals.has_background_jobs {
        labels.push("background jobs");
    }
    if signals.dangerous_html {
        labels.push("HTML rendering");
    }
    labels
}

pub(super) fn is_publicly_abusable_route(file: &SourceFile, lower: &str) -> bool {
    let normalized = file.relative_path.replace('\\', "/").to_ascii_lowercase();
    PUBLIC_RISK_ENDPOINT_PATTERNS
        .iter()
        .any(|pattern| pattern.is_match(&normalized) || pattern.is_match(lower))
}

#[cfg(test)]
#[path = "route_helpers_tests.rs"]
mod tests;
