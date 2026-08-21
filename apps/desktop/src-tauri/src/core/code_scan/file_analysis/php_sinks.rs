use super::*;
use std::sync::LazyLock;

// Match request superglobals only when passed directly to dangerous sinks.
// Following local variables without data-flow analysis creates too many false
// positives, and `$_SERVER` commonly supplies safe filesystem base paths.

/// User-controlled request superglobals. $_SERVER is intentionally absent.
const TAINT: &str = r"\$_(?:GET|POST|REQUEST|COOKIE)";

/// Consume safe leading context because Rust regexes do not support lookbehind.
const LEAD: &str = r"(?:^|[^A-Za-z0-9_>$:'\x22-])";

/// Compile a static pattern while keeping the line-based allow marker attached.
fn static_regex(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).expect("static regex") // allow-expect: compile-time literal regex
}

/// PHP include and require calls with a keyword boundary.
static PHP_INCLUDE_CALL: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(&format!(r"{LEAD}(?:include|require)(?:_once)?(?:\s|\()")));

/// Request superglobals used only to index a server-defined allowlist.
static PHP_ALLOWLIST_INDEX: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(&format!(r"\$[A-Za-z_]\w*\s*\[\s*{TAINT}")));

/// unserialize(...) - the PHP object-injection sink.
static PHP_UNSERIALIZE_CALL: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(&format!(r"{LEAD}unserialize\s*\(")));

/// Shell / process execution sinks. `->exec` (PDO executes SQL, not a shell)
/// and `::` static calls are filtered by the leading guard.
static PHP_COMMAND_CALL: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(&format!(
        r"{LEAD}(?:system|exec|shell_exec|passthru|proc_open|popen|pcntl_exec)\s*\("
    ))
});

/// Escaped request data remains a scoped review because escaping is not an allowlist.
static PHP_ESCAPED_TAINT: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(&format!(
        r"escapeshell(?:arg|cmd)\s*\([^)]{{0,200}}{TAINT}[^)]{{0,200}}\)"
    ))
});

/// PHP dynamic-execution sinks, including legacy `assert` and `create_function`.
/// Method calls such as PHPUnit assertions are excluded by the leading guard.
static PHP_CODE_EXEC_CALL: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(&format!(r"{LEAD}(?:eval|assert|create_function)\s*\(")));

/// preg_replace: only dangerous when the pattern literal carries the /e
/// modifier (removed in PHP 7, still present in legacy plugin code), which
/// evals the replacement.
static PHP_PREG_REPLACE_CALL: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(&format!(r"{LEAD}preg_replace\s*\(")));

/// Filesystem sinks that read, write, open, or delete the path handed to
/// them.
static PHP_FILE_CALL: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(&format!(
        r"{LEAD}(?:readfile|file_get_contents|file_put_contents|fopen|unlink)\s*\("
    ))
});

/// A request superglobal confined to a single path segment by basename is
/// the standard traversal mitigation; these spans are blanked before asking
/// whether any raw taint reaches a filesystem sink.
static PHP_BASENAME_TAINT: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(&format!(
        r"basename\s*\([^)]{{0,200}}{TAINT}[^)]{{0,200}}\)"
    ))
});

static PHP_TAINT: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(TAINT));

/// Detect the legacy `e` modifier in a quoted `preg_replace` pattern.
fn preg_replace_pattern_has_e_modifier(window: &str) -> bool {
    let trimmed = window.trim_start();
    let Some(quote) = trimmed.chars().next().filter(|c| *c == '\'' || *c == '"') else {
        return false;
    };
    let body = &trimmed[1..];
    let Some(end) = body.find(quote) else {
        return false;
    };
    let pattern = &body[..end];
    let Some(delimiter) = pattern.chars().next() else {
        return false;
    };
    // A regex delimiter is a non-alphanumeric, non-backslash, non-space char.
    if delimiter.is_ascii_alphanumeric() || delimiter == '\\' || delimiter.is_whitespace() {
        return false;
    }
    let Some(close) = pattern.rfind(delimiter).filter(|index| *index > 0) else {
        return false;
    };
    pattern[close + delimiter.len_utf8()..].contains('e')
}

fn is_php_file(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".php")
}

/// The statement window after a sink call: from `start` to the next `;` (a PHP
/// statement terminator) or `cap` bytes, whichever comes first, clamped to a
/// UTF-8 boundary so slicing never panics on multibyte source.
fn statement_window(content: &str, start: usize, cap: usize) -> &str {
    let mut end = content[start..]
        .find(';')
        .map(|offset| start + offset)
        .unwrap_or(content.len())
        .min(start + cap);
    while end < content.len() && !content.is_char_boundary(end) {
        end += 1;
    }
    let start = start.min(content.len());
    &content[start..end]
}

/// Return the keyword's 1-based line, excluding any consumed leading separator.
fn keyword_line(content: &str, matched: &regex::Match<'_>) -> u32 {
    let keyword_start = content[matched.start()..matched.end()]
        .find(|ch: char| ch.is_ascii_alphabetic())
        .map(|offset| matched.start() + offset)
        .unwrap_or(matched.start());
    // A column-zero keyword belongs after its preceding newline.
    let newlines = content[..keyword_start]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count();
    newlines as u32 + 1
}

pub(super) fn collect_php_sink_issues(issues: &mut Vec<CodeIssue>, ctx: &FileAnalysisContext<'_>) {
    let file = ctx.file;
    if !is_php_file(&file.relative_path) {
        return;
    }
    // Preserve offsets across a structure view that blanks strings and a taint
    // view that retains interpolated request data while blanking comments.
    let structure = crate::core::code_scan::laravel_routes::blank_php(ctx.content, true);
    let scan = crate::core::code_scan::laravel_routes::blank_php(ctx.content, false);

    // File inclusion (LFI / RFI): request input in the include target.
    for m in PHP_INCLUDE_CALL.find_iter(&structure) {
        let window = statement_window(&scan, m.end(), 300);
        let residual = PHP_ALLOWLIST_INDEX.replace_all(window, " ");
        if PHP_TAINT.is_match(&residual) {
            issues.push(build_issue(
                "php-file-inclusion",
                "security",
                Severity::Critical,
                "Request accessor appears in dynamic include target",
                "Static analysis matched a request superglobal in an include or require target without a recognized server-owned allowlist expression. It does not establish runtime reachability, upstream validation, PHP configuration, or deployed exposure. If an attacker controls the effective path, local inclusion can expose or execute an unintended PHP-readable file; remote inclusion additionally depends on the relevant PHP URL-include settings.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("An include/require target contains a $_GET, $_POST, $_REQUEST, or $_COOKIE value directly, with no allowlist indexing it.".into()),
                Some("Trace the matched expression and any prior validation. Replace request-derived include paths with a server-owned map from a small route or template key to fixed files, and reject unknown keys before include or require runs. Do not rely on extension suffixes or path stripping as an include allowlist.".into()),
                Some("In an isolated fixture directory, map one approved key to an inert PHP canary and place a second canary outside the allowed root. Test unknown keys, dot segments, absolute paths, encoded separators, null bytes as handled by the supported PHP version, and remote URLs; confirm only the mapped canary can be included.".into()),
            ));
            break;
        }
    }

    // Object injection: unserialize on attacker-controlled bytes.
    for m in PHP_UNSERIALIZE_CALL.find_iter(&structure) {
        let window = statement_window(&scan, m.end(), 240);
        // `unserialize($x, ['allowed_classes' => false])` disables object
        // instantiation, which is the documented mitigation.
        if window.contains("allowed_classes") {
            continue;
        }
        if PHP_TAINT.is_match(window) {
            issues.push(build_issue(
                "php-object-injection",
                "security",
                Severity::Critical,
                "Request accessor passed to PHP unserialize()",
                "Static analysis matched a request superglobal inside unserialize() without a recognized allowed_classes => false option. It does not establish runtime reachability, prior integrity verification, or which classes and gadget methods are available. If an attacker controls the serialized bytes and object instantiation is allowed, magic methods can produce application-specific side effects and may form a code-execution gadget chain.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("unserialize() receives a $_GET, $_POST, $_REQUEST, or $_COOKIE value and does not pass allowed_classes => false.".into()),
                Some("Trace the effective bytes and any integrity check. Prefer schema-validated JSON or another non-object interchange format for untrusted input. If legacy scalar or array serialization is unavoidable, verify integrity before unserialize and pass ['allowed_classes' => false]; do not treat a class allowlist as proof that its magic methods are safe.".into()),
                Some("In an isolated unit test, register an inert canary class whose magic method only records an attempted instantiation. Confirm untrusted payloads are rejected before unserialize or decoded with object creation disabled, without invoking operating-system commands or real side effects.".into()),
            ));
            break;
        }
    }

    // Command execution: distinguish raw request input from input wrapped in a
    // shell-escaping helper. Escaping lowers shell-grammar risk but does not
    // establish a fixed executable or command-specific argument policy.
    for m in PHP_COMMAND_CALL.find_iter(&structure) {
        let window = statement_window(&scan, m.end(), 240);
        if !PHP_TAINT.is_match(window) {
            continue;
        }
        let residual = PHP_ESCAPED_TAINT.replace_all(window, " ");
        let raw = PHP_TAINT.is_match(&residual);
        let (severity, title, description, evidence) = if raw {
            (
                Severity::Critical,
                "Request accessor appears in process command input",
                "Static analysis matched a request superglobal inside a PHP shell or process execution call. It does not establish runtime reachability, preceding validation, authorization, or deployed exposure. If an attacker can reach the call and control the matched value, shell grammar, executable selection, or command-specific option parsing may change the intended operation.",
                "A system, exec, shell_exec, passthru, proc_open, popen, or pcntl_exec command statement contains a request superglobal outside a recognized shell-escaping wrapper.",
            )
        } else {
            (
                Severity::Medium,
                "Shell-escaped request argument needs command-policy review",
                "Static analysis matched a request superglobal inside escapeshellarg() or escapeshellcmd() at a PHP command boundary. Escaping can reduce shell-metacharacter interpretation, but it does not by itself constrain the executable, leading options, paths, URLs, or resources the target program accepts. Review the exact command and supported PHP/platform escaping behavior.",
                "A PHP process-execution call contains a request superglobal only inside a recognized escapeshellarg or escapeshellcmd span; command-specific argument policy was not resolved.",
            )
        };
        issues.push(build_issue(
            "php-dynamic-command",
            "security",
            severity,
            title,
            description,
            file,
            Some(keyword_line(&structure, &m)),
            Some(evidence.into()),
            Some("Trace the final executable, arguments, and shell mode. Prefer a process API and invocation form that bypasses shell parsing, select a fixed executable in server code, and pass values through an argument array where the supported PHP/runtime API permits it. Allowlist command-specific values, insert an end-of-options delimiter when the target supports it, and reject leading-option input or dangerous resource schemes. Keep shell source entirely server-owned if a shell is unavoidable.".into()),
            Some("In a unit test or process mock, capture the executable, argument array, and shell mode for approved values plus metacharacter, whitespace, leading-option, path, and URL payloads. Confirm the intended boundaries and resource policy remain fixed and invalid input starts no process.".into()),
        ));
        break;
    }

    // Dynamic code execution: request input reaching eval/assert/
    // create_function, or a preg_replace whose pattern carries /e.
    let mut code_exec_line: Option<u32> = None;
    for m in PHP_CODE_EXEC_CALL.find_iter(&structure) {
        let window = statement_window(&scan, m.end(), 300);
        let residual = PHP_ALLOWLIST_INDEX.replace_all(window, " ");
        if PHP_TAINT.is_match(&residual) {
            code_exec_line = Some(keyword_line(&structure, &m));
            break;
        }
    }
    if code_exec_line.is_none() {
        for m in PHP_PREG_REPLACE_CALL.find_iter(&structure) {
            let window = statement_window(&scan, m.end(), 300);
            if preg_replace_pattern_has_e_modifier(window) && PHP_TAINT.is_match(window) {
                code_exec_line = Some(keyword_line(&structure, &m));
                break;
            }
        }
    }
    if let Some(line) = code_exec_line {
        issues.push(build_issue(
            "php-code-execution",
            "security",
            Severity::Critical,
            "Request accessor appears in dynamic PHP evaluation",
            "Static analysis matched a request superglobal inside eval, assert, create_function, or the evaluated replacement of preg_replace /e. It does not establish runtime reachability, upstream validation, the selected PHP version, or deployed exposure. eval evaluates PHP source on supported versions; the other matched legacy constructs evaluate source only on PHP versions where those behaviors still exist. If attacker-controlled text reaches an active evaluator, it runs with the application's process privileges.",
            file,
            Some(line),
            Some("An eval/assert/create_function call, or a preg_replace call whose pattern literal ends in an /e modifier, contains a $_GET, $_POST, $_REQUEST, or $_COOKIE value directly.".into()),
            Some("Trace the matched value and supported PHP runtime first. Replace dynamic source evaluation with schema parsing or a server-owned callable map, remove create_function in favor of a normal closure, and replace preg_replace /e with preg_replace_callback whose callback is fixed server code. Do not treat a character blacklist as a PHP sandbox.".into()),
            Some("Use an isolated unit test with an inert arithmetic expression and an evaluator/callback spy. Confirm request input is rejected or treated as data and no dynamic evaluator receives it; do not verify by executing operating-system commands.".into()),
        ));
    }

    // Path traversal: request input reaching a filesystem read/write/open/
    // delete sink with no basename confinement.
    for m in PHP_FILE_CALL.find_iter(&structure) {
        let window = statement_window(&scan, m.end(), 300);
        let residual = PHP_ALLOWLIST_INDEX.replace_all(window, " ");
        let residual = PHP_BASENAME_TAINT.replace_all(&residual, " ");
        if PHP_TAINT.is_match(&residual) {
            issues.push(build_issue(
                "php-path-traversal",
                "security",
                Severity::High,
                "Request accessor used as filesystem path",
                "Static analysis matched a request superglobal in a filesystem path argument without a recognized basename or server-owned allowlist expression. It does not establish runtime reachability, upstream validation, the process working directory, or a later canonical-root check. If traversal survives normalization, the operation may read, write, or delete files outside the intended root within the process account's permissions.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("A readfile/file_get_contents/file_put_contents/fopen/unlink call contains a request superglobal directly, with no basename() confining it to a single path segment and no allowlist indexing it.".into()),
                Some("Trace the effective path policy. Prefer an opaque server-side object id or a server-owned allowlist. Otherwise treat input as one filename segment, join it beneath a fixed root, and compare a safely canonicalized existing target or parent directory against that root before access; handle symlinks and create/write operations explicitly.".into()),
                Some("Create an isolated temporary root with one allowed canary and another canary just outside it. Test dot segments, absolute paths, encoded separators, backslashes, symlinks, and create/write variants, and confirm only the inside canary can be reached.".into()),
            ));
            break;
        }
    }
}
