use super::*;
use std::collections::HashSet;
use std::sync::LazyLock;

/// Resolves Laravel route middleware across route, bootstrap, and kernel files.
/// Unknown middleware never suppresses a finding.
fn static_regex(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).expect("static regex") // allow-expect: compile-time literal regex
}

static CONTROLLER_CLASS_REF: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"\b([A-Za-z_][A-Za-z0-9_]*Controller)::class"));

static CONTROLLER_USES_REF: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(r#"['"]([A-Za-z_][A-Za-z0-9_\\]*Controller)@[A-Za-z_][A-Za-z0-9_]*['"]"#)
});

static GROUP_CALL: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"(?i)(?:Route\s*::|->)\s*group\s*\("));

/// `$middleware->group('api', [` / `appendToGroup` / `prependToGroup` in the
/// Laravel 11+ bootstrap fluent style. The match ends at the array's opening
/// bracket so the caller can bracket-match the definition span.
static BOOTSTRAP_GROUP_DEF: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(
        r#"->\s*(?:group|appendToGroup|prependToGroup)\s*\(\s*['"]([A-Za-z0-9_.-]+)['"]\s*,\s*\["#,
    )
});

/// `'api' => [` entries in the legacy Kernel `$middlewareGroups` array.
static KERNEL_GROUP_DEF: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r#"['"]([A-Za-z0-9_.-]+)['"]\s*=>\s*\["#));

/// Class references inside a group definition span (IsAdmin::class).
static MIDDLEWARE_CLASS_REF: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"\b([A-Za-z_][A-Za-z0-9_]*)::class"));

/// Middleware alias entries: `'audio.auth' => AuthenticateAudioRequests::class`
/// in `$middleware->alias([...])` or the legacy Kernel `$routeMiddleware`.
static ALIAS_ENTRY: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(r#"['"]([A-Za-z0-9_.-]+)['"]\s*=>\s*\\?([A-Za-z_][A-Za-z0-9_\\]*)::class"#)
});

/// Class-form auth middleware used directly in a route chain:
/// `->middleware([AuthenticateSubsonicRequests::class])`. The `\b` cannot sit
/// inside an identifier, so RedirectIfAuthenticated never matches.
static AUTH_CLASS_MIDDLEWARE: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"\bauthenticate[a-z0-9_]*::class"));

/// Laravel route-registration verbs used for whole-file protection checks.
static ROUTE_VERB: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(
        r"(?i)\bRoute\s*::\s*(?:get|post|put|patch|delete|options|any|match|resource|apiResource|singleton|fallback|redirect|view)\s*\(",
    )
});

#[derive(Debug, Clone, Copy)]
pub(super) struct LaravelControllerStatus {
    /// Every routes-file reference to this controller sits behind an
    /// auth-granting middleware (quoted `auth*`, `can:`, an Authenticate*
    /// middleware class, or a group/alias whose definition grants auth).
    pub(super) auth_protected: bool,
    /// No reference is reachable both unauthenticated and unthrottled - the
    /// controller has no publicly hammerable surface.
    pub(super) throttled: bool,
}

#[derive(Debug, Default)]
pub(super) struct LaravelRouteProtection {
    /// Keyed by lowercased controller class basename (e.g. "accountcontroller").
    controllers: HashMap<String, LaravelControllerStatus>,
    /// Routes files whose framework-level base group grants auth (e.g.
    /// routes/api.php when the project's api group contains 'auth:api').
    /// The file itself is then a gated surface, not a public handler.
    auth_protected_routes_files: HashSet<String>,
    /// Every routes file the resolver parsed (lowercased relative paths).
    parsed_routes_files: HashSet<String>,
}

impl LaravelRouteProtection {
    /// Match controller basenames using a pessimistically merged protection state.
    pub(super) fn status_for(&self, relative_path: &str) -> Option<LaravelControllerStatus> {
        let normalized = relative_path.replace('\\', "/").to_ascii_lowercase();
        let basename = normalized.rsplit('/').next().unwrap_or(&normalized);
        let class_name = basename.strip_suffix(".php")?;
        self.controllers.get(class_name).copied()
    }

    pub(super) fn routes_file_is_auth_protected(&self, relative_path: &str) -> bool {
        let normalized = relative_path.replace('\\', "/").to_ascii_lowercase();
        self.auth_protected_routes_files.contains(&normalized)
    }

    /// Whether the resolver parsed this Laravel routes manifest.
    pub(super) fn is_routes_manifest(&self, relative_path: &str) -> bool {
        let normalized = relative_path.replace('\\', "/").to_ascii_lowercase();
        self.parsed_routes_files.contains(&normalized)
    }
}

fn is_laravel_routes_file(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with(".php")
        && (normalized.starts_with("routes/") || normalized.contains("/routes/"))
}

/// Where middleware groups are defined: the Laravel 11+ bootstrap file or
/// the legacy HTTP kernel.
fn is_middleware_definition_file(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with("bootstrap/app.php") || normalized.ends_with("http/kernel.php")
}

#[derive(Debug, Default)]
struct MiddlewareGroupGrants {
    auth: HashSet<String>,
    throttle: HashSet<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct BaseGrants {
    auth: bool,
    throttle: bool,
}

pub(super) fn collect_laravel_route_protection(files: &[SourceFile]) -> LaravelRouteProtection {
    let grants = collect_middleware_group_grants(files);
    let auth_markers = build_markers(&["'auth", "\"auth", "'can:", "\"can:"], &grants.auth);
    let throttle_markers = build_markers(&["'throttle", "\"throttle"], &grants.throttle);

    let mut protection = LaravelRouteProtection::default();
    for file in files {
        if !is_laravel_routes_file(&file.relative_path) {
            continue;
        }
        let lower = file.content.to_ascii_lowercase();
        if !lower.contains("route::") {
            continue;
        }
        let base = file_base_grants(&file.relative_path, &grants);
        protection
            .parsed_routes_files
            .insert(file.relative_path.replace('\\', "/").to_ascii_lowercase());
        let declares_auth = collect_from_routes_file(
            &file.content,
            base,
            &auth_markers,
            &throttle_markers,
            &mut protection.controllers,
        );
        // Routes manifests with framework or explicit auth middleware delegate
        // per-handler posture to their controllers.
        if base.auth || declares_auth {
            protection
                .auth_protected_routes_files
                .insert(file.relative_path.replace('\\', "/").to_ascii_lowercase());
        }
    }

    protection
}

/// Quote-anchored middleware markers: the literal auth/throttle prefixes plus
/// every group name whose definition grants the capability, wrapped in both
/// quote styles so `'middleware' => ['user-full-auth']` resolves.
fn build_markers(base: &[&str], group_names: &HashSet<String>) -> Vec<String> {
    let mut markers: Vec<String> = base.iter().map(|marker| marker.to_string()).collect();
    for name in group_names {
        markers.push(format!("'{name}'"));
        markers.push(format!("\"{name}\""));
    }
    markers
}

/// Resolve protection inherited from a conventional route file's middleware group.
fn file_base_grants(relative_path: &str, grants: &MiddlewareGroupGrants) -> BaseGrants {
    let normalized = relative_path.replace('\\', "/").to_ascii_lowercase();
    let basename = normalized.rsplit('/').next().unwrap_or(&normalized);
    let group = match basename {
        "api.php" => "api",
        "web.php" => "web",
        _ => return BaseGrants::default(),
    };
    BaseGrants {
        auth: grants.auth.contains(group),
        throttle: grants.throttle.contains(group),
    }
}

fn collect_middleware_group_grants(files: &[SourceFile]) -> MiddlewareGroupGrants {
    let mut grants = MiddlewareGroupGrants::default();
    // Custom middleware classes listed in group definitions (IsAdmin::class)
    // grant nothing by name; they are resolved through their own files below.
    let mut pending_classes: Vec<(String, String)> = Vec::new();

    for file in files {
        if !is_middleware_definition_file(&file.relative_path) {
            continue;
        }
        // Structure copy (strings + comments blanked) drives bracket
        // matching; scan copy (comments blanked, strings kept) is what the
        // markers are read from, so a commented-out middleware line cannot
        // grant anything. Both copies preserve byte offsets.
        let structure = blank_php(&file.content, true);
        let scan = blank_php(&file.content, false);

        let mut record_definition = |name: &str, bracket_open: usize| {
            let Some(close) = matching_close(&structure, bracket_open, b'[', b']') else {
                return;
            };
            let span = &scan[bracket_open..close];
            let span_lower = span.to_ascii_lowercase();
            // Middleware definitions may use string aliases or class names for
            // authentication and throttling.
            if span_lower.contains("'auth")
                || span_lower.contains("\"auth")
                || span_lower.contains("authenticate::class")
            {
                grants.auth.insert(name.to_ascii_lowercase());
            }
            if span_lower.contains("'throttle")
                || span_lower.contains("\"throttle")
                || span_lower.contains("throttlerequests")
            {
                grants.throttle.insert(name.to_ascii_lowercase());
            }
            for capture in MIDDLEWARE_CLASS_REF.captures_iter(span) {
                if let Some(class) = capture.get(1) {
                    pending_classes.push((
                        name.to_ascii_lowercase(),
                        class.as_str().to_ascii_lowercase(),
                    ));
                }
            }
        };

        for capture in BOOTSTRAP_GROUP_DEF.captures_iter(&scan) {
            if let (Some(name), Some(whole)) = (capture.get(1), capture.get(0)) {
                record_definition(name.as_str(), whole.end() - 1);
            }
        }
        if scan.to_ascii_lowercase().contains("middlewaregroups") {
            for capture in KERNEL_GROUP_DEF.captures_iter(&scan) {
                if let (Some(name), Some(whole)) = (capture.get(1), capture.get(0)) {
                    record_definition(name.as_str(), whole.end() - 1);
                }
            }
        }

        // Resolve middleware aliases through the same transitive class check as groups.
        for capture in ALIAS_ENTRY.captures_iter(&scan) {
            let (Some(name), Some(class)) = (capture.get(1), capture.get(2)) else {
                continue;
            };
            let alias = name.as_str().to_ascii_lowercase();
            let class_basename = class
                .as_str()
                .rsplit('\\')
                .next()
                .unwrap_or(class.as_str())
                .to_ascii_lowercase();
            if class_basename.starts_with("authenticate") {
                grants.auth.insert(alias);
            } else {
                pending_classes.push((alias, class_basename));
            }
        }
    }

    // Treat custom middleware as an auth gate only when its class rejects guests;
    // a positive identity check may instead guard guest-only routes.
    for (group, class) in pending_classes {
        if grants.auth.contains(&group) {
            continue;
        }
        let target = format!("/{class}.php");
        let rejects_guests = files.iter().any(|file| {
            let normalized = file.relative_path.replace('\\', "/").to_ascii_lowercase();
            (normalized.ends_with(&target) || normalized == target[1..])
                && middleware_class_rejects_guests(&file.content)
        });
        if rejects_guests {
            grants.auth.insert(group);
        }
    }

    grants
}

fn middleware_class_rejects_guests(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    lower.contains("->guest()")
        || lower.contains("auth::guest(")
        || lower.contains("auth()->guest(")
        || lower.contains("!auth()->check")
        || lower.contains("abort_unless(auth()")
        || lower.contains("extends authenticate")
}

/// Returns whether the file itself declares any auth-granting middleware
/// (a group or per-route chain), which marks it as an auth-managing manifest.
fn collect_from_routes_file(
    content: &str,
    base: BaseGrants,
    auth_markers: &[String],
    throttle_markers: &[String],
    controllers: &mut HashMap<String, LaravelControllerStatus>,
) -> bool {
    // Use a string-blanked copy for structure and a comments-only-blanked copy
    // for middleware names, which are string literals.
    let structure = blank_php(content, true);
    let scan = blank_php(content, false);
    let groups = collect_group_scopes(&scan, &structure, auth_markers, throttle_markers);
    let mut declares_auth = groups.iter().any(|group| group.auth);

    // Combine enclosing, statement, and framework protection. Any
    // withoutMiddleware opt-out is conservatively treated as unprotected.
    let protection_at = |position: usize| -> (bool, bool) {
        let in_group = |grant: fn(&GroupScope) -> bool| {
            groups
                .iter()
                .any(|group| group.body.contains(&position) && grant(group))
        };
        let statement = statement_span(&scan, &structure, position);
        let opted_out = statement.to_ascii_lowercase().contains("withoutmiddleware");
        let auth = !opted_out
            && (base.auth
                || in_group(|g| g.auth)
                || middleware_span_grants(statement, auth_markers));
        let throttled = !opted_out
            && (base.throttle
                || in_group(|g| g.throttle)
                || middleware_span_grants(statement, throttle_markers));
        (auth, throttled)
    };

    // Controllers in endpoint maps inherit protection only when every route
    // registration in the file is protected.
    let mut registration_count = 0usize;
    let mut every_registration_auth = true;
    let mut every_registration_throttled = true;
    for verb_match in ROUTE_VERB.find_iter(&structure) {
        let (auth, throttled) = protection_at(verb_match.start());
        registration_count += 1;
        every_registration_auth &= auth;
        every_registration_throttled &= throttled;
    }
    let file_fully_auth = registration_count > 0 && every_registration_auth;
    let file_fully_throttled = registration_count > 0 && every_registration_throttled;

    let mut record = |name: &str, position: usize| {
        let class_name = name
            .rsplit('\\')
            .next()
            .unwrap_or(name)
            .to_ascii_lowercase();
        let (position_auth, position_throttled) = protection_at(position);
        declares_auth |= position_auth && !base.auth;
        let auth = position_auth || file_fully_auth;
        let throttled = position_throttled || file_fully_throttled;

        let entry = controllers
            .entry(class_name)
            .or_insert(LaravelControllerStatus {
                auth_protected: true,
                throttled: true,
            });
        // Merge pessimistically: any unprotected reference removes auth, while
        // throttled means no reference is both public and unthrottled.
        entry.auth_protected &= auth;
        entry.throttled &= auth || throttled;
    };

    for capture in CONTROLLER_CLASS_REF.captures_iter(&scan) {
        if let (Some(name), Some(whole)) = (capture.get(1), capture.get(0)) {
            record(name.as_str(), whole.start());
        }
    }
    for capture in CONTROLLER_USES_REF.captures_iter(&scan) {
        if let (Some(name), Some(whole)) = (capture.get(1), capture.get(0)) {
            record(name.as_str(), whole.start());
        }
    }

    declares_auth
}

#[derive(Debug)]
struct GroupScope {
    body: std::ops::Range<usize>,
    auth: bool,
    throttle: bool,
}

/// Collect middleware grants for nested route-group body ranges.
fn collect_group_scopes(
    scan: &str,
    structure: &str,
    auth_markers: &[String],
    throttle_markers: &[String],
) -> Vec<GroupScope> {
    let mut scopes = Vec::new();

    for group_match in GROUP_CALL.find_iter(structure) {
        // Search both chained middleware before `group` and middleware inside
        // the group config array; names must come from the string-preserving copy.
        let statement_start = structure[..group_match.start()]
            .rfind([';', '{', '}'])
            .map(|index| index + 1)
            .unwrap_or(0);
        let lookback = &scan[statement_start..group_match.start()];

        let after_call = &structure[group_match.end()..];
        let config_len = ["function", "fn(", "fn ("]
            .iter()
            .filter_map(|marker| after_call.find(marker))
            .min()
            .unwrap_or(0);
        let lookahead = &scan[group_match.end()..group_match.end() + config_len];

        let auth = middleware_span_grants(lookback, auth_markers)
            || middleware_span_grants(lookahead, auth_markers);
        let throttle = middleware_span_grants(lookback, throttle_markers)
            || middleware_span_grants(lookahead, throttle_markers);

        // The first `{` after the call opens the group closure (config arrays
        // use `[`, and string braces are blanked), and its matching `}` ends
        // the scope.
        let Some(body_open) = structure[group_match.end()..]
            .find('{')
            .map(|offset| group_match.end() + offset)
        else {
            continue;
        };
        let Some(body_close) = matching_close(structure, body_open, b'{', b'}') else {
            continue;
        };

        scopes.push(GroupScope {
            body: body_open..body_close,
            auth,
            throttle,
        });
    }

    scopes
}

/// Check middleware markers only inside the declaration's argument list.
fn middleware_span_grants(span: &str, markers: &[String]) -> bool {
    // Auth marker lists start with `'auth`; only those also accept the
    // class form (`->middleware([AuthenticateSubsonicRequests::class])`).
    let accepts_class_form = markers.first().is_some_and(|marker| marker == "'auth");
    let lower = span.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(found) = lower[search_from..].find("middleware") {
        let start = search_from + found;
        let keyword_end = start + "middleware".len();
        if let Some(window) = middleware_argument_window(&lower, keyword_end) {
            if markers
                .iter()
                .any(|marker| window.contains(marker.as_str()))
            {
                return true;
            }
            if accepts_class_form && AUTH_CLASS_MIDDLEWARE.is_match(window) {
                return true;
            }
        }
        search_from = keyword_end;
    }
    false
}

/// Return the call, array, or scalar value following a middleware declaration.
fn middleware_argument_window(lower: &str, keyword_end: usize) -> Option<&str> {
    const MAX_WINDOW: usize = 600;
    let rest = &lower[keyword_end..(keyword_end + MAX_WINDOW).min(lower.len())];
    let bytes = rest.as_bytes();
    let mut i = 0;
    // Skip the optional array-key quote, whitespace, and arrow.
    if matches!(bytes.first(), Some(b'\'' | b'"')) {
        i += 1;
    }
    while matches!(bytes.get(i), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        i += 1;
    }
    if bytes.get(i) == Some(&b'=') && bytes.get(i + 1) == Some(&b'>') {
        i += 2;
        while matches!(bytes.get(i), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            i += 1;
        }
    }
    let (open, close) = match bytes.get(i)? {
        b'(' => (b'(', b')'),
        b'[' => (b'[', b']'),
        quote @ (b'\'' | b'"') => {
            let end = rest[i + 1..].find(*quote as char)? + i + 1;
            return Some(&rest[i..=end]);
        }
        _ => return None,
    };
    let mut depth = 0usize;
    for (offset, byte) in bytes.iter().enumerate().skip(i) {
        if *byte == open {
            depth += 1;
        } else if *byte == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(&rest[i..=offset]);
            }
        }
    }
    // Unclosed within the cap: use what is available.
    Some(&rest[i..])
}

/// Return the enclosing statement using structural boundaries and scan-text content.
fn statement_span<'a>(scan: &'a str, structure: &str, position: usize) -> &'a str {
    let start = structure[..position]
        .rfind([';', '{', '}'])
        .map(|index| index + 1)
        .unwrap_or(0);
    let end = structure[position..]
        .find(';')
        .map(|offset| position + offset)
        .unwrap_or(structure.len());
    &scan[start..end]
}

fn matching_close(structure: &str, open: usize, open_byte: u8, close_byte: u8) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in structure[open..].bytes().enumerate() {
        if byte == open_byte {
            depth += 1;
        } else if byte == close_byte {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(open + offset);
            }
        }
    }
    None
}

/// Blank PHP comments and optionally strings while preserving byte offsets.
/// PHP sink checks use separate keyword and taint views of the result.
pub(in crate::core::code_scan) fn blank_php(content: &str, blank_strings: bool) -> String {
    let bytes = content.as_bytes();
    let mut out = bytes.to_vec();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            quote @ (b'\'' | b'"') => {
                let mut cursor = index + 1;
                while cursor < bytes.len() {
                    if bytes[cursor] == b'\\' {
                        cursor += 2;
                        continue;
                    }
                    if bytes[cursor] == quote {
                        break;
                    }
                    cursor += 1;
                }
                if blank_strings {
                    let end = cursor.min(bytes.len().saturating_sub(1));
                    for slot in out.iter_mut().take(end + 1).skip(index) {
                        if !slot.is_ascii_whitespace() {
                            *slot = b' ';
                        }
                    }
                }
                index = cursor + 1;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let end = content[index..]
                    .find('\n')
                    .map(|offset| index + offset)
                    .unwrap_or(bytes.len());
                out[index..end].fill(b' ');
                index = end;
            }
            b'#' => {
                let end = content[index..]
                    .find('\n')
                    .map(|offset| index + offset)
                    .unwrap_or(bytes.len());
                out[index..end].fill(b' ');
                index = end;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let end = content[index + 2..]
                    .find("*/")
                    .map(|offset| index + offset + 4)
                    .unwrap_or(bytes.len());
                for slot in out.iter_mut().take(end).skip(index) {
                    if !slot.is_ascii_whitespace() {
                        *slot = b' ';
                    }
                }
                index = end;
            }
            _ => index += 1,
        }
    }
    // Only ASCII bytes were rewritten (quotes, slashes -> spaces), so the
    // result is valid UTF-8 whenever the input was.
    String::from_utf8(out).unwrap_or_else(|_| content.to_string())
}
