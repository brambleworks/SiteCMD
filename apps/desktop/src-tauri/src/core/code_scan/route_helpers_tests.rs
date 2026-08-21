use super::{
    has_inline_secret_guard, has_wp_handler_registration, has_wp_logged_in_action,
    is_example_like_path, is_route_like, is_server_action_like, is_test_like_path,
    is_wp_gated_surface, is_write_handler, wp_rest_routes_all_have_permission_callbacks,
    SourceFile,
};
use std::path::Path;

fn source_file(relative_path: &str, content: &str) -> SourceFile {
    SourceFile {
        absolute_path: std::path::PathBuf::from(relative_path),
        relative_path: relative_path.to_string(),
        line_count: content.lines().count(),
        content: content.to_string(),
    }
}

#[test]
fn api_support_modules_are_not_routes() {
    let support = source_file(
        "apps/site/src/lib/api/auth.ts",
        "export function authorize() { return true; }",
    );
    let route = source_file(
        "apps/site/src/pages/api/auth.ts",
        "export const POST = async () => new Response();",
    );

    assert!(!is_route_like(
        &support,
        &support.content.to_ascii_lowercase()
    ));
    assert!(is_route_like(&route, &route.content.to_ascii_lowercase()));
}

#[test]
fn instructional_catalog_guides_are_scanned_source() {
    assert!(!is_example_like_path(Path::new(
        "apps/catalog/content/guides/code/security.ts"
    )));
}

#[test]
fn recognizes_rust_test_files() {
    assert!(is_test_like_path(Path::new(
        "src/commands/desktop_tests.rs"
    )));
    assert!(is_test_like_path(Path::new(
        "src/scoring/calculator_scan_tests.rs"
    )));
    assert!(is_test_like_path(Path::new("src/parser/lexer_test.rs")));
    assert!(is_test_like_path(Path::new("src/core/code_scan/tests.rs")));
    assert!(is_test_like_path(Path::new("crates/core/benches/parse.rs")));
    // `/tests/` was already covered by the shared directory convention.
    assert!(is_test_like_path(Path::new("crates/core/tests/api.rs")));
    // First-party Rust source is still analysed - including names that merely
    // end in "test.rs"/"tests.rs" without the underscore convention.
    assert!(!is_test_like_path(Path::new("src/commands/scan.rs")));
    assert!(!is_test_like_path(Path::new("src/attest.rs")));
    assert!(!is_test_like_path(Path::new("src/protests.rs")));
}

#[test]
fn quoted_route_markers_are_rule_definitions_not_routes() {
    let rule_source = source_file(
        "src/scanner/rules.ts",
        r#"const MARKERS = ["app.post(", "router.post(", "export async function post("];"#,
    );
    let lower = rule_source.content.to_lowercase();
    assert!(!is_route_like(&rule_source, &lower));
    assert!(!is_write_handler(&lower));

    // Live declarations are code, not data: still route-like.
    let express = source_file(
        "server/index.js",
        "app.post('/contact', handler);\napp.use(express.json());\n",
    );
    let lower = express.content.to_lowercase();
    assert!(is_route_like(&express, &lower));
    assert!(is_write_handler(&lower));

    // Laravel declarations survive too.
    let laravel = source_file("routes/api.php", "Route::post('/billing', 'Ctl@save');\n");
    let lower = laravel.content.to_lowercase();
    assert!(is_route_like(&laravel, &lower));
    assert!(is_write_handler(&lower));

    let fqcn = source_file(
        "modules/billing/routes.php",
        "<?php\nnamespace Modules\\Billing;\n\\Route::post('/pay', [PayController::class, 'store']);\n",
    );
    let lower = fqcn.content.to_lowercase();
    assert!(is_route_like(&fqcn, &lower));
    assert!(is_write_handler(&lower));

    // A scanner defining the "use server" directive marker is not a server
    // action; a file carrying the real directive is.
    assert!(!is_server_action_like(
        r#"let hit = lower.contains("\"use server\"") || lower.contains("'use server'");"#
    ));
    assert!(is_server_action_like(
        "\"use server\";\nexport async fn x() {}"
    ));
}

#[test]
fn recognizes_example_and_fixture_files() {
    assert!(is_example_like_path(Path::new(
        "src/lib/fix-guides/security.ts"
    )));
    assert!(is_example_like_path(Path::new("examples/cors.ts")));
    assert!(is_example_like_path(Path::new("playground/tsconfig.json")));
    assert!(is_example_like_path(Path::new(
        "src/__fixtures__/payload.ts"
    )));
    assert!(is_example_like_path(Path::new(
        "components/Button.stories.tsx"
    )));
    assert!(is_example_like_path(Path::new("config.example.ts")));
    // Real source is not example-like.
    assert!(!is_example_like_path(Path::new("src/api/contact.ts")));
    assert!(!is_example_like_path(Path::new("src/lib/auth.ts")));
}

#[test]
fn recognizes_ruby_test_files() {
    assert!(is_test_like_path(Path::new(
        "spec/vulnerabilities/sql_injection_spec.rb"
    )));
    assert!(is_test_like_path(Path::new("spec/models/user_spec.rb")));
    assert!(is_test_like_path(Path::new("test/models/user_test.rb")));
    // First-party Ruby source is still analysed.
    assert!(!is_test_like_path(Path::new("app/controllers/users.rb")));
    assert!(!is_test_like_path(Path::new("lib/parser.rb")));
}

#[test]
fn recognizes_e2e_and_cypress_test_files() {
    assert!(is_test_like_path(Path::new(
        "apps/api/v2/src/modules/webhooks/controllers/webhooks.controller.e2e-spec.ts"
    )));
    assert!(is_test_like_path(Path::new("tests/booking.e2e.ts")));
    assert!(is_test_like_path(Path::new(
        "src/platform/bookings/controllers/e2e/add-guests.helper.ts"
    )));
    assert!(is_test_like_path(Path::new("cypress/flows/login.cy.ts")));
    // Deno/Go-style underscore test convention.
    assert!(is_test_like_path(Path::new(
        "packages/fresh/src/middlewares/cors_test.ts"
    )));
    assert!(is_test_like_path(Path::new("src/util_test.tsx")));
    // First-party source is still analysed.
    assert!(!is_test_like_path(Path::new(
        "apps/api/v2/src/modules/webhooks/webhooks.controller.ts"
    )));
    assert!(!is_test_like_path(Path::new("src/latest_test_helpers.ts")));
}

#[test]
fn recognizes_node_module_test_files() {
    assert!(is_test_like_path(Path::new(
        "tools/scripts/repo-guardrail-rules-unit.test.mjs"
    )));
    assert!(is_test_like_path(Path::new("src/config.spec.cjs")));
    assert!(is_test_like_path(Path::new("src/parser_test.cjs")));
    assert!(!is_test_like_path(Path::new("src/runtime.mjs")));
}

#[test]
fn wp_hook_registrations_are_route_like() {
    assert!(has_wp_handler_registration(
        "add_action('wp_ajax_save_settings', 'my_save');"
    ));
    assert!(has_wp_handler_registration(
        "add_action(\"admin_post_export\", 'my_export');"
    ));
    assert!(has_wp_handler_registration(
        "register_rest_route('myplugin/v1', '/items', $args);"
    ));
    // Prose mentions without the quoted hook prefix do not count.
    assert!(!has_wp_handler_registration(
        "// wired up through wp_ajax_ hooks elsewhere"
    ));
    // A rule-definition source quoting the markers themselves is not a WP
    // surface (the extra delimiter/escape in front gives it away).
    assert!(!has_wp_handler_registration(
        r#"["'wp_ajax_", "\"wp_ajax_", "'admin_post_", "register_rest_route("]"#
    ));
    assert!(!has_wp_logged_in_action(
        r#"["'wp_ajax_", "\"wp_ajax_", "'admin_post_", "\"admin_post_"]"#
    ));
}

#[test]
fn gated_surface_requires_no_public_registrations() {
    // Priv-only ajax: behind the login wall.
    assert!(is_wp_gated_surface("add_action('wp_ajax_save', 'h');"));
    // A nopriv variant or an unguarded REST route makes the file public.
    assert!(!is_wp_gated_surface(
        "add_action('wp_ajax_save', 'h'); add_action('wp_ajax_nopriv_save', 'h');"
    ));
    assert!(!is_wp_gated_surface(
        "add_action('wp_ajax_save', 'h'); register_rest_route('p/v1', '/x', $a);"
    ));
    // A REST route whose registration carries a permission_callback keeps
    // the surface gated; '__return_true' declares it public.
    assert!(is_wp_gated_surface(
        "register_rest_route('p/v1', '/x', ['permission_callback' => [$this, 'check']]);"
    ));
    assert!(!is_wp_gated_surface(
        "register_rest_route('p/v1', '/x', ['permission_callback' => '__return_true']);"
    ));
    // No WP hooks at all: not a WP surface.
    assert!(!is_wp_gated_surface("router.post('/x', handler);"));
}

#[test]
fn rest_permission_callback_counting_is_per_registration() {
    // Every registration guarded -> access-control evidence.
    assert!(wp_rest_routes_all_have_permission_callbacks(
        "register_rest_route('p/v1', '/a', ['permission_callback' => [$this, 'check']]);\n\
         register_rest_route('p/v1', '/b', ['permission_callback' => 'my_guard']);"
    ));
    // Two registrations, one callback: at least one route is unguarded.
    assert!(!wp_rest_routes_all_have_permission_callbacks(
        "register_rest_route('p/v1', '/a', ['permission_callback' => [$this, 'check']]);\n\
         register_rest_route('p/v1', '/b', $args);"
    ));
    // No REST routes at all: nothing to claim.
    assert!(!wp_rest_routes_all_have_permission_callbacks(
        "add_action('wp_ajax_save', 'h');"
    ));
}

#[test]
fn nopriv_only_actions_are_not_logged_in_evidence() {
    // A logged-in action is auth evidence and a write handler.
    assert!(has_wp_logged_in_action(
        "add_action('wp_ajax_save_settings', 'my_save');"
    ));
    // Registering BOTH variants still carries the logged-in one.
    assert!(has_wp_logged_in_action(
        "add_action('wp_ajax_submit', 'h'); add_action('wp_ajax_nopriv_submit', 'h');"
    ));
    // nopriv-only handlers are public: no auth, no session for CSRF.
    assert!(!has_wp_logged_in_action(
        "add_action('wp_ajax_nopriv_track', 'my_track');"
    ));
    assert!(!has_wp_logged_in_action(
        "add_action(\"admin_post_nopriv_contact\", 'my_contact');"
    ));
}

#[test]
fn recognizes_custom_credential_and_session_auth() {
    // Hand-rolled auth the identity-library patterns miss (input is already
    // lowercased by the caller).
    assert!(has_inline_secret_guard(
        "if (!safecompare(secret, expected)) throw new error();"
    ));
    assert!(has_inline_secret_guard(
        "const ok = await bcrypt.compare(pw, hash);"
    ));
    assert!(has_inline_secret_guard(
        "const token = derivesessiontoken(expected);"
    ));
    // No credential handling -> not an auth guard.
    assert!(!has_inline_secret_guard("return json({ ok: true });"));
}
