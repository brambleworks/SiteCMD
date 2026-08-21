use super::super::*;
use crate::checks::IssueConfidence;

fn issue_ids(report: &CodeScanReport) -> Vec<String> {
    report.issues.iter().map(|issue| issue.id.clone()).collect()
}

fn has_issue(report: &CodeScanReport, prefix: &str) -> bool {
    report
        .issues
        .iter()
        .any(|issue| issue.id.starts_with(prefix))
}

#[test]
fn wp_ajax_handler_without_nonce_or_capability_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-content/plugins/myplugin/includes/settings.php",
        r#"<?php
add_action('wp_ajax_myplugin_save', 'myplugin_save_settings');

function myplugin_save_settings() {
    global $wpdb;
    $title = $_POST['title'];
    $wpdb->update($wpdb->prefix . 'myplugin_settings', ['title' => $title], ['id' => 1]);
    wp_send_json_success();
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    // Cookie-backed WP auth context + state change + no nonce -> CSRF.
    assert!(
        has_issue(&report, "csrf-missing:"),
        "expected csrf-missing, got: {:?}",
        issue_ids(&report)
    );
    // $_POST parsed with no sanitize_*/filter_var/validate in sight.
    assert!(
        has_issue(&report, "request-validation:"),
        "expected request-validation, got: {:?}",
        issue_ids(&report)
    );
    // Logged-in ajax registration IS identity auth (core enforces login),
    // so the finding to raise is missing authorization, not missing auth.
    assert!(!has_issue(&report, "sensitive-auth:"));
    // Direct-to-$wpdb hook callbacks are normal plugin architecture; the
    // layering checks must stay quiet on WP handlers.
    assert!(!has_issue(&report, "db-in-route:"));
    assert!(!has_issue(&report, "multi-write-no-transaction:"));
}

#[test]
fn wp_ajax_handler_with_nonce_and_capability_is_clean() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-content/plugins/myplugin/includes/settings.php",
        r#"<?php
add_action('wp_ajax_myplugin_save', 'myplugin_save_settings');

function myplugin_save_settings() {
    check_ajax_referer('myplugin_save', 'nonce');
    if (!current_user_can('manage_options')) {
        wp_send_json_error('forbidden', 403);
    }
    global $wpdb;
    $title = sanitize_text_field($_POST['title']);
    $wpdb->update($wpdb->prefix . 'myplugin_settings', ['title' => $title], ['id' => 1]);
    wp_send_json_success();
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "csrf-missing:"),
        "nonce check should satisfy CSRF, got: {:?}",
        issue_ids(&report)
    );
    assert!(!has_issue(&report, "request-validation:"));
    assert!(!has_issue(&report, "sensitive-auth:"));
    assert!(!has_issue(&report, "sensitive-authz:"));
}

#[test]
fn wp_admin_page_registration_counts_as_the_auth_gate() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-content/plugins/myplugin/admin/menu.php",
        r#"<?php
add_action('admin_menu', 'myplugin_register_menu');

function myplugin_register_menu() {
    add_menu_page('MyPlugin', 'MyPlugin', 'manage_options', 'myplugin', 'myplugin_render_admin');
}

function myplugin_render_admin() {
    echo '<div class="wrap"><h1>Settings</h1></div>';
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "sensitive-auth:"),
        "capability-gated menu page misread as unauthenticated, got: {:?}",
        issue_ids(&report)
    );
    assert!(!has_issue(&report, "sensitive-authz:"));
}

#[test]
fn laravel_route_closure_without_validation_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "routes/web.php",
        r#"<?php
use Illuminate\Support\Facades\Route;

Route::post('/feedback', function (Request $request) {
    $message = $request->input('message');
    Feedback::create(['message' => $message]);
    return response()->json(['ok' => true]);
});
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "request-validation:"),
        "expected request-validation, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn laravel_route_closure_with_validate_is_clean() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "routes/web.php",
        r#"<?php
use Illuminate\Support\Facades\Route;

Route::post('/feedback', function (Request $request) {
    $data = $request->validate(['message' => 'required|string|max:2000']);
    Feedback::create($data);
    return response()->json(['ok' => true]);
});
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "request-validation:"),
        "validate() should count as validation, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn laravel_auth_middleware_suppresses_sensitive_auth() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "routes/admin.php",
        r#"<?php
use Illuminate\Support\Facades\Route;

Route::middleware(['auth', 'can:manage-billing'])->group(function () {
    Route::post('/admin/billing', function (Request $request) {
        $data = $request->validate(['plan' => 'required|string']);
        Billing::update($data);
        return back();
    });
});
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "sensitive-auth:"),
        "middleware('auth') should be auth evidence, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn php_location_header_from_superglobal_is_open_redirect() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/go.php",
        r#"<?php
$target = $_GET['next'];
header('Location: ' . $target . '?src=' . $_GET['src']);
exit;
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "open-redirect:"),
        "expected open-redirect, got: {:?}",
        issue_ids(&report)
    );

    let unguarded = TempDir::new().unwrap();
    write_file(
        unguarded.path(),
        "public/api/go.php",
        r#"<?php
wp_redirect($_GET['next']);
exit;
"#,
    );

    let report = audit_project(unguarded.path()).unwrap();
    assert!(
        has_issue(&report, "open-redirect:"),
        "unvalidated wp_redirect is an open redirect, got: {:?}",
        issue_ids(&report)
    );

    let guarded = TempDir::new().unwrap();
    write_file(
        guarded.path(),
        "public/api/go.php",
        r#"<?php
wp_redirect(wp_validate_redirect($_GET['next'], home_url()));
exit;
"#,
    );

    let report = audit_project(guarded.path()).unwrap();
    assert!(
        !has_issue(&report, "open-redirect:"),
        "wp_validate_redirect should count as the allowlist guard, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn wpdb_superglobal_interpolation_is_raw_sql_unsafe() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-content/plugins/myplugin/includes/lookup.php",
        r#"<?php
add_action('wp_ajax_myplugin_lookup', 'myplugin_lookup');

function myplugin_lookup() {
    global $wpdb;
    $rows = $wpdb->get_results("SELECT * FROM {$wpdb->prefix}items WHERE owner = '$_GET[owner]'");
    wp_send_json($rows);
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "raw-sql-unsafe:"),
        "expected raw-sql-unsafe, got: {:?}",
        issue_ids(&report)
    );

    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "wp-content/plugins/myplugin/includes/lookup.php",
        r#"<?php
add_action('wp_ajax_myplugin_lookup', 'myplugin_lookup');

function myplugin_lookup() {
    check_ajax_referer('myplugin_lookup', 'nonce');
    global $wpdb;
    $rows = $wpdb->get_results(
        $wpdb->prepare("SELECT * FROM {$wpdb->prefix}items WHERE owner = %s", $_GET['owner'])
    );
    wp_send_json($rows);
}
"#,
    );

    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "raw-sql-unsafe:"),
        "prepare() must not flag, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn php_echo_of_superglobal_is_unsafe_html() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/search.php",
        r#"<?php
echo '<h1>Results for ' . $_GET['q'] . '</h1>';
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "unsafe-html:"),
        "expected unsafe-html, got: {:?}",
        issue_ids(&report)
    );

    let escaped = TempDir::new().unwrap();
    write_file(
        escaped.path(),
        "public/api/search.php",
        r#"<?php
echo '<h1>Results for ' . htmlspecialchars($_GET['q'], ENT_QUOTES) . '</h1>';
"#,
    );

    let report = audit_project(escaped.path()).unwrap();
    assert!(
        !has_issue(&report, "unsafe-html:"),
        "htmlspecialchars should count as sanitization, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn php_setcookie_flag_detection() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/login.php",
        r#"<?php
$user = authenticate($_POST['email'], $_POST['password']);
$token = create_session_token($user);
setcookie('session_id', $token, time() + 86400, '/');
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "session-cookie-flags:"),
        "expected session-cookie-flags, got: {:?}",
        issue_ids(&report)
    );

    let hardened = TempDir::new().unwrap();
    write_file(
        hardened.path(),
        "public/api/login.php",
        r#"<?php
$user = authenticate($_POST['email'], $_POST['password']);
$token = create_session_token($user);
setcookie('session_id', $token, [
    'expires' => time() + 86400,
    'path' => '/',
    'secure' => true,
    'httponly' => true,
    'samesite' => 'Lax',
]);
"#,
    );

    let report = audit_project(hardened.path()).unwrap();
    assert!(
        !has_issue(&report, "session-cookie-flags:"),
        "options-array flags should satisfy the check, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn wp_presence_check_only_handler_is_clean() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-content/plugins/myplugin/admin/welcome-panel.php",
        r#"<?php
add_action('wp_ajax_myplugin_update_panel', 'myplugin_update_panel');

function myplugin_update_panel() {
    check_ajax_referer('myplugin-panel-nonce', 'panelnonce');
    $vote = 1;
    if (empty($_POST['visible'])) {
        $vote = 0;
    }
    update_user_meta(get_current_user_id(), 'myplugin_panel', $vote);
    wp_die();
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "request-validation:"),
        "presence check misread as body parsing, got: {:?}",
        issue_ids(&report)
    );
    assert!(
        !has_issue(&report, "public-endpoint-rate-limit:"),
        "priv-only ajax surface misread as public, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn wp_rest_capability_callbacks_count_as_auth() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-content/plugins/myplugin/includes/rest-api.php",
        r#"<?php
add_action('rest_api_init', function () {
    register_rest_route('myplugin/v1', '/accounts', [
        [
            'methods' => WP_REST_Server::EDITABLE,
            'callback' => 'myplugin_update_account',
            'permission_callback' => static function () {
                return current_user_can('manage_options');
            },
        ],
    ]);
});

function myplugin_update_account(WP_REST_Request $request) {
    $name = sanitize_text_field($request['name']);
    update_option('myplugin_account_name', $name);
    return rest_ensure_response(['ok' => true]);
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "sensitive-auth:"),
        "capability-gated REST controller misread as auth-less, got: {:?}",
        issue_ids(&report)
    );
    assert!(!has_issue(&report, "sensitive-authz:"));
}

#[test]
fn laravel_form_request_hint_counts_as_validation() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "app/Api/V1/Controllers/AttachmentController.php",
        r#"<?php

namespace App\Api\V1\Controllers;

use App\Api\V1\Requests\StoreRequest;

class AttachmentController extends Controller
{
    public function store(StoreRequest $request)
    {
        $path = $request->file('attachment')->store('attachments');
        return response()->json(['path' => $path]);
    }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "upload-validation:"),
        "FormRequest-delegated upload rules misread as missing, got: {:?}",
        issue_ids(&report)
    );
    assert!(!has_issue(&report, "request-validation:"));
}

#[test]
fn php_object_serialize_is_not_a_cookie_write() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-content/plugins/myplugin/includes/cache.php",
        r#"<?php
add_action('wp_ajax_myplugin_warm_cache', 'myplugin_warm_cache');

function myplugin_warm_cache() {
    check_ajax_referer('myplugin_warm', 'nonce');
    $payload = ['access_token' => get_option('myplugin_token'), 'session' => wp_get_session_token()];
    update_option('myplugin_cache', serialize($payload));
    wp_send_json_success();
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "session-cookie-flags:"),
        "serialize(\\$data) misread as a cookie write, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn laravel_controller_behind_auth_middleware_is_quiet() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "routes/api.php",
        r#"<?php
Route::group(
    ['namespace' => 'App\Api\Controllers', 'prefix' => 'v1/accounts',
     'middleware' => ['auth:api,sanctum', 'throttle:60,1']],
    static function (): void {
        Route::get('accounts/{account}', ['uses' => 'AccountController@show', 'as' => 'show']);
        Route::post('accounts', ['uses' => 'AccountController@store', 'as' => 'store']);
    }
);
"#,
    );
    write_file(
        temp.path(),
        "app/Api/Controllers/AccountController.php",
        r#"<?php

namespace App\Api\Controllers;

class AccountController extends Controller
{
    public function show($account)
    {
        return response()->json($this->repository->find($account));
    }

    public function store()
    {
        return response()->json($this->repository->store(request()->all()));
    }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "sensitive-auth:"),
        "middleware-protected controller misread as auth-less, got: {:?}",
        issue_ids(&report)
    );
    assert!(
        !has_issue(&report, "sensitive-authz:"),
        "routes-layer middleware must also settle the authz question, got: {:?}",
        issue_ids(&report)
    );
    assert!(
        !has_issue(&report, "public-endpoint-rate-limit:"),
        "throttle middleware misread as missing rate limiting, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn laravel_controller_routed_publicly_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "routes/web.php",
        r#"<?php
Route::post('/billing', [BillingController::class, 'store']);
"#,
    );
    write_file(
        temp.path(),
        "app/Http/Controllers/BillingController.php",
        r#"<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;

class BillingController extends Controller
{
    public function store(Request $request)
    {
        $plan = $request->input('plan');
        return response()->json(['plan' => $plan]);
    }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "sensitive-auth:"),
        "publicly routed billing controller should read as auth-less, got: {:?}",
        issue_ids(&report)
    );
    assert!(
        has_issue(&report, "request-validation:"),
        "raw ->input() read without validation should flag, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn laravel_route_level_middleware_chain_is_the_auth_gate() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "routes/web.php",
        r#"<?php
Route::post('/settings', [SettingsController::class, 'update'])->middleware('auth');
"#,
    );
    write_file(
        temp.path(),
        "app/Http/Controllers/SettingsController.php",
        r#"<?php

namespace App\Http\Controllers;

class SettingsController extends Controller
{
    public function update()
    {
        return response()->json(['ok' => true]);
    }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "sensitive-auth:"),
        "route-level ->middleware('auth') misread as missing, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn laravel_guest_middleware_is_not_auth() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "routes/web.php",
        r#"<?php
Route::group(['middleware' => ['guest']], function () {
    Route::post('/admin/preview', [AdminPreviewController::class, 'store']);
});
"#,
    );
    write_file(
        temp.path(),
        "app/Http/Controllers/AdminPreviewController.php",
        r#"<?php

namespace App\Http\Controllers;

class AdminPreviewController extends Controller
{
    public function store()
    {
        return response()->json(['ok' => true]);
    }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "sensitive-auth:"),
        "'guest' middleware must not count as protection, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn wp_rest_return_true_routes_stay_publicly_abusable() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-content/plugins/myplugin/includes/rest-api.php",
        r#"<?php
add_action('rest_api_init', function () {
    register_rest_route('myplugin/v1', '/contact', [
        'methods' => WP_REST_Server::CREATABLE,
        'callback' => 'myplugin_submit_contact',
        'permission_callback' => '__return_true',
    ]);
});

function myplugin_submit_contact(WP_REST_Request $request) {
    $message = sanitize_textarea_field($request['message']);
    wp_mail(get_option('admin_email'), 'Contact form', $message);
    return rest_ensure_response(['ok' => true]);
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "public-endpoint-rate-limit:"),
        "deliberately public REST route lost its rate-limit finding, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn wp_rest_delegated_permission_callbacks_gate_the_surface() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "wp-content/plugins/myplugin/includes/rest-api.php",
        r#"<?php
class Myplugin_Accounts_Controller extends Myplugin_REST_Controller {
    public function register_routes() {
        register_rest_route('myplugin/v1', '/accounts', [
            'methods' => WP_REST_Server::CREATABLE,
            'callback' => [$this, 'create_item'],
            'permission_callback' => [$this, 'create_item_permissions_check'],
        ]);
    }

    public function create_item($request) {
        $name = sanitize_text_field($request['name']);
        update_option('myplugin_account_name', $name);
        return rest_ensure_response(['ok' => true]);
    }
}
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "sensitive-auth:"),
        "delegated permission_callback misread as auth-less, got: {:?}",
        issue_ids(&report)
    );
    assert!(
        !has_issue(&report, "sensitive-authz:"),
        "permission_callback IS the WP REST authorization hook, got: {:?}",
        issue_ids(&report)
    );
    assert!(
        !has_issue(&report, "public-endpoint-rate-limit:"),
        "fully guarded REST surface misread as publicly abusable, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn php_dynamic_include_from_superglobal_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/page.php",
        r#"<?php
$page = $_GET['page'];
include __DIR__ . '/pages/' . $_GET['page'] . '.php';
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("php-file-inclusion:"))
        .expect("expected php-file-inclusion");
    assert_scoped_static_sink_copy(issue);

    // Safe forms: a request value used only as the index into a server-owned
    // allowlist, and the constant-path require_once that every plugin uses.
    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "public/api/page.php",
        r#"<?php
$pages = ['home' => 'home.php', 'about' => 'about.php'];
require_once __DIR__ . '/bootstrap.php';
include __DIR__ . '/pages/' . ($pages[$_GET['page']] ?? '404.php');
"#,
    );

    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "php-file-inclusion:"),
        "allowlist-indexed include and constant require must stay quiet, got: {:?}",
        issue_ids(&report)
    );

    // A quoted array key named 'include' mapped to a superglobal (the Contact
    // Form 7 admin dispatch shape) is not an include statement at all.
    let array_key = TempDir::new().unwrap();
    write_file(
        array_key.path(),
        "wp-content/plugins/myplugin/admin/admin.php",
        r#"<?php
$args = ['include' => $_REQUEST['service'], 'action' => 'view'];
do_action('myplugin_admin', $args);
"#,
    );

    let report = audit_project(array_key.path()).unwrap();
    assert!(
        !has_issue(&report, "php-file-inclusion:"),
        "'include' => \\$_REQUEST array key misread as an include statement, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn php_sink_keywords_in_comments_and_strings_do_not_flag() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/page.php",
        r#"<?php
// First include the page template, then read $_GET['page'] safely below.
$page = htmlspecialchars($_GET['page'] ?? 'home');
# We deliberately never system() or exec() on $_REQUEST here.
$notice = "You can include $_GET[ref] in the query string, but we ignore it.";
echo $page;
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        !has_issue(&report, "php-file-inclusion:"),
        "an 'include' keyword inside a comment or string must not flag, got: {:?}",
        issue_ids(&report)
    );
    assert!(
        !has_issue(&report, "php-dynamic-command:"),
        "'system'/'exec' inside a comment must not flag, got: {:?}",
        issue_ids(&report)
    );

    // Guard against over-blanking: a real interpolated include still flags.
    let real = TempDir::new().unwrap();
    write_file(
        real.path(),
        "public/api/load.php",
        r#"<?php
include "pages/$_GET[page].php";
"#,
    );
    let report = audit_project(real.path()).unwrap();
    assert!(
        has_issue(&report, "php-file-inclusion:"),
        "an interpolated superglobal in a real include path must still flag, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn php_unserialize_of_request_input_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/session.php",
        r#"<?php
$state = unserialize(base64_decode($_COOKIE['state']));
render($state);
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("php-object-injection:"))
        .expect("expected php-object-injection");
    assert_scoped_static_sink_copy(issue);

    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "public/api/session.php",
        r#"<?php
// allowed_classes => false disables object instantiation.
$state = unserialize($_COOKIE['state'], ['allowed_classes' => false]);
// A DB-sourced value is not request input.
$prefs = unserialize($user->preferences);
// A class method named unserialize is not the builtin sink.
$theme = ThemeProperties::unserialize(json_decode($value));
render($state, $prefs, $theme);
"#,
    );

    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "php-object-injection:"),
        "allowed_classes, DB value, and ::unserialize must stay quiet, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn php_eval_from_superglobal_is_owned_by_php_code_execution_only() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/eval.php",
        r#"<?php
$body = $_POST['payload'];
eval($_POST['expression']);
echo json_encode(['ok' => true, 'body' => strlen($body)]);
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "php-code-execution:"),
        "expected php-code-execution, got: {:?}",
        issue_ids(&report)
    );
    assert!(
        !has_issue(&report, "eval-exec-injection:"),
        "PHP eval must not double-flag via generic eval-exec-injection, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn php_command_from_superglobal_is_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/run.php",
        r#"<?php
system('ping -c 1 ' . $_GET['host']);
"#,
    );

    let report = audit_project(temp.path()).unwrap();
    assert!(
        has_issue(&report, "php-dynamic-command:"),
        "expected php-dynamic-command, got: {:?}",
        issue_ids(&report)
    );
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("php-dynamic-command:"))
        .expect("php-dynamic-command issue");
    assert_scoped_static_sink_copy(issue);
    let fix = issue.likely_fix.as_deref().unwrap_or_default();
    assert!(fix.contains("fixed executable") && fix.contains("argument array"));
    assert!(fix.contains("leading-option"));
    assert!(!fix.contains("pass arguments through escapeshellarg"));
    let verify = issue.verify_hint.as_deref().unwrap_or_default();
    assert!(verify.contains("mock") || verify.contains("test harness"));
    assert!(!verify.contains("marker command"));
    // The precise PHP sink owns this: the fuzzy generic shell-injection must
    // not also fire on the same call.
    assert!(
        !has_issue(&report, "shell-injection:"),
        "PHP shell sinks should not double-flag via generic shell-injection, got: {:?}",
        issue_ids(&report)
    );

    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "public/api/run.php",
        r#"<?php
// Escaped argument.
system('ping -c 1 ' . escapeshellarg($_GET['host']));
// Constant command.
$version = shell_exec('which ffmpeg');
// PDO ->exec runs SQL, not a shell.
$pdo->exec($_POST['statement']);
"#,
    );

    let report = audit_project(safe.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("php-dynamic-command:"))
        .expect("escapeshellarg command argument should produce a scoped review finding");
    assert_eq!(issue.severity, Severity::Medium);
    assert!(issue.title.contains("Shell-escaped request argument"));
    assert!(issue.description.contains("does not by itself constrain"));
}

#[test]
fn php_eval_and_preg_replace_e_from_superglobal_are_flagged() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/calc.php",
        r#"<?php
eval('return ' . $_GET['expr'] . ';');
"#,
    );
    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("php-code-execution:"))
        .expect("expected php-code-execution for eval of a superglobal");
    assert_scoped_static_sink_copy(issue);

    let preg = TempDir::new().unwrap();
    write_file(
        preg.path(),
        "public/api/rewrite.php",
        r#"<?php
$out = preg_replace('/(\w+)/e', $_POST['code'], $subject);
"#,
    );
    let report = audit_project(preg.path()).unwrap();
    assert!(
        has_issue(&report, "php-code-execution:"),
        "expected php-code-execution for preg_replace /e with a superglobal, got: {:?}",
        issue_ids(&report)
    );

    // Safe forms: a constant eval body, a preg_replace WITHOUT the /e modifier
    // (the replacement is data, not code), and a method call named ->assert.
    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "public/api/rewrite.php",
        r#"<?php
eval('$total = 1 + 2;');
$clean = preg_replace('/[^a-z]/i', '', $_POST['slug']);
$this->assertEquals($_GET['expected'], $actual);
"#,
    );
    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "php-code-execution:"),
        "constant eval, /i preg_replace, and ->assertEquals must stay quiet, got: {:?}",
        issue_ids(&report)
    );
}

#[test]
fn php_file_sink_from_superglobal_is_path_traversal() {
    let temp = TempDir::new().unwrap();
    write_file(
        temp.path(),
        "public/api/download.php",
        r#"<?php
echo file_get_contents(__DIR__ . '/uploads/' . $_GET['file']);
"#,
    );
    let report = audit_project(temp.path()).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.id.starts_with("php-path-traversal:"))
        .expect("expected php-path-traversal for file_get_contents");
    assert_scoped_static_sink_copy(issue);
    assert!(!issue
        .verify_hint
        .as_deref()
        .unwrap_or_default()
        .contains("/etc/passwd"));

    let unlink = TempDir::new().unwrap();
    write_file(
        unlink.path(),
        "public/api/delete.php",
        r#"<?php
unlink('/var/data/' . $_POST['name']);
"#,
    );
    let report = audit_project(unlink.path()).unwrap();
    assert!(
        has_issue(&report, "php-path-traversal:"),
        "expected php-path-traversal for unlink of a superglobal, got: {:?}",
        issue_ids(&report)
    );

    // Safe forms: basename confines the value to one path segment, an
    // allowlist maps the value to a known file, and a constant path is fixed.
    let safe = TempDir::new().unwrap();
    write_file(
        safe.path(),
        "public/api/download.php",
        r#"<?php
$name = basename($_GET['file']);
echo file_get_contents(__DIR__ . '/uploads/' . basename($_GET['file']));
$reports = ['q1' => 'q1.pdf', 'q2' => 'q2.pdf'];
readfile(__DIR__ . '/reports/' . ($reports[$_GET['report']] ?? '404.pdf'));
$config = file_get_contents(__DIR__ . '/config.json');
"#,
    );
    let report = audit_project(safe.path()).unwrap();
    assert!(
        !has_issue(&report, "php-path-traversal:"),
        "basename, allowlist index, and constant path must stay quiet, got: {:?}",
        issue_ids(&report)
    );
}

fn assert_scoped_static_sink_copy(issue: &CodeIssue) {
    assert_eq!(issue.confidence, IssueConfidence::NeedsReview);
    assert!(issue.title.contains("Request accessor"), "{}", issue.title);
    assert!(
        issue.description.contains("Static analysis matched"),
        "{}",
        issue.description
    );
    assert!(
        issue.description.contains("does not establish"),
        "{}",
        issue.description
    );
}
