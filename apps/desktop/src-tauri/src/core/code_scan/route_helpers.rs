use super::*;

mod db_writes;
mod framework_auth;
mod html_sinks;

pub(in crate::core::code_scan) use db_writes::*;
pub(in crate::core::code_scan) use framework_auth::*;
pub(in crate::core::code_scan) use html_sinks::*;

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

/// Whether `marker` occurs outside a quoted data definition, and outside a
/// longer identifier or member chain.
///
/// Backslashes escape quote-leading markers, but qualify bare PHP names such as
/// `\Route::post`, which remain live declarations. A marker that starts with an
/// identifier character must not be a member of a longer chain, so the ORM call
/// `prisma.app.delete(` does not satisfy the Express marker `app.delete(`. A
/// compound name is still a live declaration: `usersRouter.post(` keeps
/// matching `router.post(`, because only the member dot marks a receiver the
/// marker does not own.
fn contains_unquoted(lower: &str, marker: &str) -> bool {
    let escape_marks_data = marker.starts_with('"') || marker.starts_with('\'');
    let marker_starts_identifier = marker
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric() || first == '_');
    let mut search = 0;
    while let Some(pos) = lower[search..].find(marker) {
        let at = search + pos;
        let preceding = lower[..at].chars().next_back();
        let marker_as_data = matches!(preceding, Some('"' | '\'' | '`'))
            || (escape_marks_data && preceding == Some('\\'))
            || (marker_starts_identifier && preceding == Some('.'));
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

/// Whether the file is a client-side data module or component. React Query
/// hooks and `"use client"` components live under `api/` and `routes/`
/// directories in common project layouts but never serve a request.
pub(super) fn is_client_module(lower: &str) -> bool {
    has_any(lower, &CLIENT_MODULE_PATTERNS)
}

/// The request-handler marker in the file, named for evidence text. The
/// pattern list reads the original content because the HTTP verb exports are
/// case-sensitive; the substring checks below stay on the lowercased copy.
pub(super) fn route_handler_marker(rel: &str, content: &str, lower: &str) -> Option<&'static str> {
    if let Some((label, _)) = ROUTE_HANDLER_MARKER_PATTERNS
        .iter()
        .find(|(_, pattern)| pattern.is_match(content))
    {
        return Some(label);
    }
    // Every module under `pages/api/` exports its handler as the default.
    if (rel.starts_with("pages/api/") || rel.contains("/pages/api/"))
        && lower.contains("export default")
    {
        return Some("a default export in a Pages Router API route");
    }
    // PHP entry points read the request superglobals directly.
    if lower.contains("$_get")
        || lower.contains("$_post")
        || lower.contains("$_request")
        || lower.contains("php://input")
    {
        return Some("a direct PHP request read");
    }
    if has_wp_handler_registration(lower) {
        return Some("a WordPress handler registration");
    }
    if has_unquoted_marker(lower, ROUTE_DECLARATION_MARKERS) {
        return Some("a framework route declaration");
    }
    None
}

/// The line of the first request-handler marker, so a finding whose risk word
/// came from the path can point at the handler instead of an unrelated line.
pub(super) fn route_handler_marker_line(content: &str) -> Option<u32> {
    ROUTE_HANDLER_MARKER_PATTERNS
        .iter()
        .find_map(|(_, pattern)| {
            pattern
                .find(content)
                .map(|found| line_number(content, found.start()))
        })
}

/// Why the file counts as a route, phrased for evidence text, or `None` when
/// it is not route-like.
///
/// For JavaScript and TypeScript a route-shaped path is a hint, never a
/// verdict: config, type, service, and repository modules all live under `api/`
/// and `routes/` directories, so there the path rule and the `route.ts`
/// basename both require a request handler in the content. Other languages keep
/// the path rule as it stands, with the PHP request read they already required.
pub(super) fn route_like_evidence(file: &SourceFile, lower: &str) -> Option<String> {
    let rel = file.relative_path.replace('\\', "/").to_lowercase();
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
    let route_basename = rel.ends_with("/route.ts")
        || rel.ends_with("/route.js")
        || rel.ends_with("/route.tsx")
        || rel.ends_with("/route.jsx");

    if rel == "routes/web.php" || rel == "routes/api.php" {
        return Some("a Laravel route manifest".into());
    }
    // A React Query module has no server handler of its own. A framework route
    // that co-locates its loader with the page component does, and that marker
    // outranks the client hooks beside it.
    let marker = route_handler_marker(&rel, &file.content, lower);
    if marker.is_none() && is_client_module(lower) {
        return None;
    }
    let on_route_path = path_rule_applies && conventional_route_path && php_direct_endpoint;
    if !is_js_source_path(&rel) {
        if on_route_path {
            return Some(match marker {
                Some(marker) => format!("a route path and {marker}"),
                None => "a route path".to_string(),
            });
        }
        return marker
            .filter(|_| {
                has_wp_handler_registration(lower)
                    || has_unquoted_marker(lower, ROUTE_DECLARATION_MARKERS)
            })
            .map(str::to_string);
    }
    let marker = marker?;
    if on_route_path {
        return Some(format!("a route path and {marker}"));
    }
    if route_basename {
        return Some(format!("a `route` module basename and {marker}"));
    }
    // A framework route declaration is route evidence wherever it sits.
    if has_wp_handler_registration(lower) || has_unquoted_marker(lower, ROUTE_DECLARATION_MARKERS) {
        return Some(marker.to_string());
    }
    None
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
    // Prefix a separator so a leading directory segment also matches; callers
    // pass both absolute walk paths and project-relative paths, and a
    // root-level `e2e/` or `tests/` directory has no separator in front of it.
    let lower = format!("/{}", path.to_string_lossy().to_ascii_lowercase());
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    lower.contains("/__tests__/")
        || lower.contains("/tests/")
        || lower.contains("/test/")
        // Test-support trees that hold fake databases, render helpers, and
        // seed users.
        || lower.contains("/testing/")
        || lower.contains(".integration-test.")
        || lower.ends_with("-test.ts")
        || lower.ends_with("-test.tsx")
        || lower.ends_with("-test.js")
        || lower.ends_with("-test.jsx")
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
        // Framework repositories ship runnable teaching apps under sample/.
        || lower.contains("/sample/")
        || lower.contains("/samples/")
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

/// Whether a query predicate or a write payload is carried by a local object
/// that was built from the caller's own session in the same file, as in
/// `const installForObject = teamId ? { teamId } : { userId: req.session.user.id };`
/// followed by `where: { type, ...installForObject }`, or
/// `const data = { type, userId: session.user?.id };` followed by
/// `prisma.credential.create({ data })`. Either way the binding carries the
/// ownership predicate the literal scope patterns look for.
pub(super) fn has_session_scoped_binding(content: &str) -> bool {
    let carried = WHERE_SPREAD_PATTERN
        .captures_iter(content)
        .chain(WRITE_PAYLOAD_BINDING_PATTERN.captures_iter(content))
        .filter_map(|capture| capture.get(1).map(|name| name.as_str().to_string()))
        .collect::<Vec<_>>();
    if carried.is_empty() {
        return false;
    }
    SESSION_SCOPED_BINDING_PATTERN
        .captures_iter(content)
        .filter_map(|capture| capture.get(1).map(|name| name.as_str()))
        .any(|binding| carried.iter().any(|name| name == binding))
}

/// Whether a lowercased path names `word` as a path segment rather than as a
/// substring inside one. Router decoration (`_authenticated+`, `(group)`,
/// `[param]`) is stripped and Remix dot-delimited segments are split, so
/// `admin+/stats.tsx` and `settings.billing.tsx` match while the transformer
/// directory `api-to-internal` does not match `internal`.
pub(super) fn path_has_word(path: &str, word: &str) -> bool {
    path.split('/').any(|segment| {
        segment
            .trim_start_matches(['_', '(', '['])
            .trim_end_matches(['+', ')', ']'])
            .split('.')
            .any(|part| part.starts_with(word))
    })
}

pub(super) fn is_sensitive_handler(file: &SourceFile, lower: &str) -> bool {
    let rel = file.relative_path.replace('\\', "/").to_lowercase();
    // Path segments only, matched at their start: `accounts` and
    // `settings-layout` are still the account and settings surfaces, while the
    // transformer directory `api-to-internal` is not an internal endpoint
    // because `internal` does not begin any segment of it.
    let sensitive_path = [
        "admin",
        "billing",
        "settings",
        "account",
        "internal",
        "backoffice",
    ]
    .iter()
    .any(|needle| path_has_word(&rel, needle));
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

/// Strips the path and quote delimiters a route-name pattern captures around
/// its word.
fn trim_word(matched: &str) -> &str {
    matched.trim_matches(|character: char| !character.is_alphanumeric())
}

/// The abuse-sensitive word that made the file look public, and where it was
/// found, so evidence can name it instead of asserting the whole category.
#[derive(Debug, Clone)]
pub(super) struct PublicRiskMatch {
    word: String,
    in_route_path: bool,
}

impl PublicRiskMatch {
    /// The matched word and where it came from, for evidence text.
    pub(super) fn describe(&self) -> String {
        let location = if self.in_route_path {
            "the route path"
        } else {
            "the file content"
        };
        format!("`{}` in {location}", self.word)
    }

    /// Whether the word came from the path, in which case no line in the file
    /// carries it and the finding should point at the handler instead.
    pub(super) fn in_route_path(&self) -> bool {
        self.in_route_path
    }
}

pub(super) fn public_risk_match(file: &SourceFile, lower: &str) -> Option<PublicRiskMatch> {
    let normalized = file.relative_path.replace('\\', "/").to_ascii_lowercase();
    for pattern in PUBLIC_RISK_ENDPOINT_PATTERNS.iter() {
        if let Some(found) = pattern.find(&normalized) {
            return Some(PublicRiskMatch {
                word: trim_word(found.as_str()).to_string(),
                in_route_path: true,
            });
        }
        if let Some(found) = pattern.find(lower) {
            return Some(PublicRiskMatch {
                word: trim_word(found.as_str()).to_string(),
                in_route_path: false,
            });
        }
    }
    None
}

#[cfg(test)]
#[path = "route_helpers_tests.rs"]
mod tests;
