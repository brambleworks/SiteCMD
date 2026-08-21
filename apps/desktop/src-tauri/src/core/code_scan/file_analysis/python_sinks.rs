use super::*;
use std::sync::LazyLock;

// Match request accessors only when they appear directly in a dangerous sink.
// Local-variable flow requires data-flow analysis and is intentionally excluded.
// `request.META` is also excluded because it is primarily server-populated.

/// Supported Python web-framework request accessors.
const TAINT: &str = r"request\.(?:GET|POST|args|form|values|json|get_json|get_data|data|query_params|COOKIES|cookies|body|files|FILES)\b";

/// Consumed leading context for bare `eval` and `exec` builtins.
/// Rejects longer names, attribute calls, and `ast.literal_eval` without lookbehind.
const CODE_LEAD: &str = r"(?:^|[^A-Za-z0-9_.])";

/// Compile a literal regex while keeping the line-based allow marker attached.
fn static_regex(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).expect("static regex") // allow-expect: compile-time literal regex
}

/// Shell and process APIs that always invoke a shell.
static PY_SHELL_CMD_CALL: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(r"\b(?:os\.system|os\.popen[234]?|commands\.get(?:status)?output)\s*\(")
});

/// The subprocess family. These take a shell only when shell=True, so a match
/// counts as command injection only when shell=True is present in the call.
static PY_SUBPROCESS_CALL: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(r"\bsubprocess\.(?:run|call|check_call|check_output|Popen)\s*\(")
});

/// The shell=True keyword argument that turns a subprocess call into a shell.
static PY_SHELL_TRUE: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(r"shell\s*=\s*True"));

/// A request accessor already wrapped in shlex.quote is neutralized; these
/// spans are blanked before asking whether any raw taint reaches the command
/// sink (the analog of the PHP escapeshellarg exclusion).
static PY_SHELL_QUOTE: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"shlex\.quote\s*\([^)]{0,200}\)"));

/// Unsafe deserialization sinks: pickle/marshal/dill load arbitrary objects,
/// and yaml.load (without a safe loader) does the same. yaml.safe_load never
/// matches - after `yaml.` it starts with `safe_load`, not `load`.
static PY_DESERIALIZE_CALL: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(
        r"\b(?:cPickle\.loads?|_pickle\.loads?|pickle\.loads?|marshal\.loads|dill\.loads?|yaml\.(?:unsafe_load_all|load_all|unsafe_load|load))\s*\(",
    )
});

/// The built-in eval / exec code-execution sinks.
static PY_CODE_EXEC_CALL: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(&format!(r"{CODE_LEAD}(?:eval|exec)\s*\(")));

/// Raw SQL escape hatches whose safety depends on whether request taint reaches
/// the query string instead of a bound-parameter argument.
static PY_SQL_CALL: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(r"(?:\.(?:execute|executemany|executescript|query|raw)|\bRawSQL)\s*\(")
});

/// SQLAlchemy expression constructors that bind request values instead of
/// interpolating them into SQL.
static PY_SQLALCHEMY_EXPR: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"^\s*(?:\w+\s*\.\s*)*(?:select|insert|update|delete)\s*\("));

/// Template-compilation sinks whose first argument is evaluated.
/// File-based `render_template` remains excluded.
static PY_TEMPLATE_CALL: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"(?:render_template_string|\.from_string)\s*\("));

/// Redirect helpers whose first argument can receive request input directly.
static PY_REDIRECT_CALL: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(
        r"\b(?:redirect|HttpResponseRedirect|HttpResponsePermanentRedirect|RedirectResponse)\s*\(",
    )
});

/// Filesystem path sinks, excluding attribute `.open` calls and Flask's
/// confined `send_from_directory` helper.
static PY_FILE_CALL: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(&format!(
        r"(?:{CODE_LEAD}open|\b(?:os\.remove|os\.unlink|send_file))\s*\("
    ))
});

/// A request accessor confined by os.path.basename or Werkzeug's
/// secure_filename is the standard traversal mitigation; these spans are
/// blanked before asking whether any raw taint reaches a path sink.
static PY_PATH_GUARD: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"(?:os\.path\.basename|secure_filename)\s*\([^)]{0,200}\)"));

static PY_TAINT: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(TAINT));

fn is_python_file(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".py")
}

/// Return a bounded call-argument span using byte-safe parenthesis matching.
/// Parentheses inside strings may shorten the span but cannot create a false hit.
fn call_arg_window(content: &str, after: usize, cap: usize) -> &str {
    let bytes = content.as_bytes();
    let start = after.min(content.len());
    let end_cap = (start + cap).min(content.len());
    let mut depth = 1usize;
    let mut end = start;
    while end < end_cap {
        match bytes[end] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    end += 1;
                    break;
                }
            }
            _ => {}
        }
        end += 1;
    }
    while end < content.len() && !content.is_char_boundary(end) {
        end += 1;
    }
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

#[derive(Clone, Copy)]
enum CommandTaint {
    Raw,
    ShellQuoted,
}

/// Classify raw and shell-quoted request input separately.
fn command_window_taint(window: &str) -> Option<CommandTaint> {
    if !PY_TAINT.is_match(window) {
        return None;
    }
    let residual = PY_SHELL_QUOTE.replace_all(window, " ");
    Some(if PY_TAINT.is_match(&residual) {
        CommandTaint::Raw
    } else {
        CommandTaint::ShellQuoted
    })
}

/// Return the first positional argument, ignoring commas inside strings and
/// nested delimiters. Later arguments are treated as bound parameters.
fn first_arg(window: &str) -> &str {
    let bytes = window.as_bytes();
    let mut depth: i32 = 0;
    let mut quote: Option<u8> = None;
    let mut end = window.len();
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if let Some(open_quote) = quote {
            // Backslash escapes the next byte inside a string.
            if byte == b'\\' {
                i += 2;
                continue;
            }
            if byte == open_quote {
                quote = None;
            }
        } else {
            match byte {
                b'"' | b'\'' => quote = Some(byte),
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b',' if depth <= 0 => {
                    end = i;
                    break;
                }
                _ => {}
            }
        }
        i += 1;
    }
    while end < window.len() && !window.is_char_boundary(end) {
        end += 1;
    }
    &window[..end]
}

pub(super) fn collect_python_sink_issues(
    issues: &mut Vec<CodeIssue>,
    ctx: &FileAnalysisContext<'_>,
) {
    let file = ctx.file;
    if !is_python_file(&file.relative_path) {
        return;
    }
    // Preserve byte offsets across structural and taint views: the former
    // removes strings, while the latter retains f-string request accessors.
    let structure = super::blank_python(ctx.content, true);
    let scan = super::blank_python(ctx.content, false);

    // Always-shell sinks qualify on taint; subprocess calls also require
    // `shell=True`. Remediation is specific to the matched sink family.
    let mut command_match: Option<(u32, bool, CommandTaint)> = None;
    for m in PY_SHELL_CMD_CALL.find_iter(&structure) {
        let window = call_arg_window(&scan, m.end(), 500);
        if let Some(taint) = command_window_taint(window) {
            command_match = Some((keyword_line(&structure, &m), true, taint));
            break;
        }
    }
    if command_match.is_none() {
        for m in PY_SUBPROCESS_CALL.find_iter(&structure) {
            let window = call_arg_window(&scan, m.end(), 500);
            if PY_SHELL_TRUE.is_match(window) {
                if let Some(taint) = command_window_taint(window) {
                    command_match = Some((keyword_line(&structure, &m), false, taint));
                }
            }
            if command_match.is_some() {
                break;
            }
        }
    }
    if let Some((line, is_os_system_family, taint)) = command_match {
        let (evidence, likely_fix) = if is_os_system_family {
            (
                "An os.system or os.popen shell string contains a request accessor (request.GET, request.POST, request.args, request.form, ...) directly.",
                "os.system and os.popen always run a shell string. Replace the call with subprocess.run using a fixed executable and an argument list with shell=False. Validate values against a command-specific allowlist and reject leading-option input when the target program could reinterpret it. If shell syntax is unavoidable, keep the script server-owned and pass data as positional arguments or environment values to a fixed script rather than concatenating shell source.",
            )
        } else {
            (
                "A subprocess call with shell=True contains a request accessor (request.GET, request.POST, request.args, request.form, ...) directly in the command input.",
                "Drop shell=True, select a fixed executable in server code, and pass validated values in an argument list. Apply a command-specific allowlist and reject leading-option input when the target program could reinterpret it. If shell syntax is unavoidable, keep the script server-owned and pass untrusted data as positional arguments or environment values to that fixed script.",
            )
        };
        let (severity, title, description) = match taint {
            CommandTaint::Raw => (
                Severity::Critical,
                "Request accessor appears in shell command input",
                "Static analysis matched a request accessor inside a shell-backed process call. It does not establish runtime reachability, preceding validation, authorization, or deployed exposure. If an attacker can reach the call and control the matched value, shell grammar or command-specific option parsing may change the intended operation.",
            ),
            CommandTaint::ShellQuoted => (
                Severity::Medium,
                "Shell-quoted request argument needs command-policy review",
                "Static analysis matched a request accessor inside shlex.quote() in a shell-backed process call. Quoting protects one shell argument from shell metacharacter interpretation, but it does not by itself constrain the target program's leading-option, path, URL, or resource semantics. Review whether the executable is fixed and the resulting argument is allowed for that command.",
            ),
        };
        issues.push(build_issue(
            "python-command-injection",
            "security",
            severity,
            title,
            description,
            file,
            Some(line),
            Some(evidence.into()),
            Some(likely_fix.into()),
            Some("Use a unit test or subprocess mock to capture the executable, argument list, and shell option for valid input plus metacharacter and leading-option payloads. Confirm the executable and boundaries stay fixed, shell=False, and invalid values are rejected without starting an unintended process.".into()),
        ));
    }

    // Unsafe deserialization: pickle/marshal/yaml.load on request input.
    for m in PY_DESERIALIZE_CALL.find_iter(&structure) {
        let window = call_arg_window(&scan, m.end(), 400);
        // yaml.load(data, Loader=yaml.SafeLoader) is the documented safe form.
        if window.contains("SafeLoader") {
            continue;
        }
        if PY_TAINT.is_match(window) {
            issues.push(build_issue(
                "python-unsafe-deserialization",
                "security",
                Severity::Critical,
                "Request accessor passed to unsafe deserializer",
                "Static analysis matched a request accessor inside pickle.loads, marshal.loads, or yaml.load without a recognized safe loader. It does not establish runtime reachability, upstream authentication or validation, or the effective loader configuration. If an attacker can supply the decoded bytes, pickle and unsafe YAML object construction can invoke attacker-influenced object behavior; marshal is not designed for untrusted data and can crash or behave unpredictably on malformed input.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("pickle.loads/marshal.loads/yaml.load (or a similar unsafe loader) receives a request accessor directly, with no safe loader such as yaml.SafeLoader.".into()),
                Some("Trace the matched value and loader first. For untrusted structured data, use a schema-validated format such as JSON; for YAML, use yaml.safe_load or an explicitly safe loader. Do not use pickle or marshal as an untrusted interchange format. A server signature can establish origin only when verification occurs before deserialization and key handling and replay policy are sound.".into()),
                Some("In an isolated unit test, use an inert canary type or loader hook that records attempted construction without executing operating-system commands. Confirm untrusted payloads are rejected before an unsafe loader runs, while the documented safe format still parses valid input.".into()),
            ));
            break;
        }
    }

    // Code execution: request input reaching the eval / exec builtins.
    for m in PY_CODE_EXEC_CALL.find_iter(&structure) {
        let window = call_arg_window(&scan, m.end(), 400);
        if PY_TAINT.is_match(window) {
            issues.push(build_issue(
                "python-code-execution",
                "security",
                Severity::Critical,
                "Request accessor appears in dynamic code evaluation",
                "Static analysis matched a request accessor inside the built-in eval() or exec() call. It does not establish runtime reachability, upstream validation, authorization, or deployed exposure. If an attacker can control the evaluated value, Python code runs with the application's process privileges.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("A built-in eval() or exec() call contains a request accessor directly.".into()),
                Some("Trace the matched value into the built-in first. Replace dynamic Python evaluation with schema parsing, ast.literal_eval for literal-only syntax, or a server-owned dispatch table. If the feature genuinely requires expressions, use a purpose-built constrained language with an explicit grammar, operation allowlist, and CPU/memory/time limits; a restricted globals dictionary is not a general sandbox.".into()),
                Some("Use an isolated unit test with an inert expression such as 6 * 7 and an evaluator spy. Confirm request input is rejected or treated as data and that neither eval nor exec is invoked; do not verify by executing operating-system commands.".into()),
            ));
            break;
        }
    }

    // Inspect only SQL text construction, not separately bound values.
    for m in PY_SQL_CALL.find_iter(&structure) {
        let arg = first_arg(call_arg_window(&scan, m.end(), 600));
        // psycopg2's sql.SQL.format(sql.Identifier(...), sql.Literal(...))
        // composition safely quotes dynamic identifiers and values.
        if arg.contains("sql.Identifier") || arg.contains("sql.Literal") {
            continue;
        }
        // SQLAlchemy expression-language first argument (select/update/...):
        // the accessor inside is bound by the builder, not interpolated.
        if PY_SQLALCHEMY_EXPR.is_match(arg) {
            continue;
        }
        if PY_TAINT.is_match(arg) {
            issues.push(build_issue(
                "python-sql-injection",
                "data",
                Severity::Critical,
                "Request accessor appears in raw SQL text",
                "Static analysis matched a request accessor in the first query-text argument of a raw SQL call instead of a separate bound-value argument. It does not establish runtime reachability, upstream allowlisting, database permissions, or whether a wrapper rewrites the call. If attacker-controlled text reaches SQL syntax, it can change the query within the connected database role's privileges.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("A request accessor is interpolated, concatenated, or formatted into the query string of an execute/executemany/raw/RawSQL call, rather than passed as a separate bound parameter.".into()),
                Some("Trace the effective database call, then keep query structure constant and pass values through the driver's parameter API. For dynamic identifiers, map request choices to server-owned identifiers or use the driver's supported identifier-composition API; value parameters cannot safely bind table or column names.".into()),
                Some("Use a disposable test database and a query-capture spy. Submit inert quote and tautology-shaped strings, then confirm the captured SQL text stays constant, the value travels only in the bound-parameter collection, and the database role remains least privilege.".into()),
            ));
            break;
        }
    }

    // Server-side template injection: request input compiled as a Jinja
    // template. Passing request data as a context value (after the first
    // argument) is safe; only request input in the template argument flags.
    for m in PY_TEMPLATE_CALL.find_iter(&structure) {
        let arg = first_arg(call_arg_window(&scan, m.end(), 500));
        if PY_TAINT.is_match(arg) {
            issues.push(build_issue(
                "python-template-injection",
                "security",
                Severity::Critical,
                "Request accessor appears in server-side template source",
                "Static analysis matched a request accessor in the template-source argument of render_template_string or from_string rather than in a context value. It does not establish runtime reachability, upstream allowlisting, the Jinja environment, or deployed exposure. If an attacker controls template source, Jinja expressions can expose template context and may reach dangerous application objects depending on the environment and available globals.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("A request accessor is part of the template string passed to render_template_string or from_string, not a context value passed alongside it.".into()),
                Some("Trace the matched value into the effective Jinja environment. Keep template source server-owned and pass request data only as context values, with output-context-appropriate escaping. If user-authored templates are a product requirement, isolate them in a deliberately constrained template environment with a narrow context and resource limits; ordinary autoescaping does not make attacker-controlled template source safe.".into()),
                Some("In an isolated test, submit the inert marker {{7*7}} and confirm it is rendered as data rather than evaluated as template source. Also verify context-appropriate escaping and that untrusted users cannot select or modify server templates.".into()),
            ));
            break;
        }
    }

    // Open redirect: request input as the redirect target. Only the first
    // argument is inspected, so redirect(url, code=302) with a validated url
    // variable stays quiet; only a raw accessor in the target flags.
    for m in PY_REDIRECT_CALL.find_iter(&structure) {
        let arg = first_arg(call_arg_window(&scan, m.end(), 400));
        if PY_TAINT.is_match(arg) {
            issues.push(build_issue(
                "python-open-redirect",
                "security",
                Severity::High,
                "Request accessor used as redirect target",
                "Static analysis matched a request accessor as the first redirect target and found no recognized local allowlist or same-origin helper in that expression. It does not establish runtime reachability, upstream validation, or the framework's effective URL normalization. If absolute or scheme-relative external targets are accepted, a trusted application URL can redirect users to an attacker-chosen site and support phishing; sensitive authorization flows can have additional impact.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("A redirect / HttpResponseRedirect / RedirectResponse target is a request accessor (request.args, request.GET, request.form, ...) directly, with no allowlist or same-origin guard.".into()),
                Some("Trace any validation before the redirect. Prefer server-owned route names or relative destinations. If return targets are required, parse and normalize with the framework's URL utilities and allowlist exact origins or application routes; account for scheme-relative URLs, backslashes, encoded separators, user-info, mixed case, and proxy origin handling.".into()),
                Some("In a route test, try an approved relative path plus absolute, scheme-relative, encoded, backslash, and user-info variants targeting example.invalid. Confirm only the documented destinations are accepted after the same normalization used in production.".into()),
            ));
            break;
        }
    }

    // Path traversal: request input as a filesystem path. Only the first
    // argument is inspected, and basename/secure_filename-confined spans
    // are blanked first, so the guarded forms stay quiet.
    for m in PY_FILE_CALL.find_iter(&structure) {
        let arg = first_arg(call_arg_window(&scan, m.end(), 400));
        let residual = PY_PATH_GUARD.replace_all(arg, " ");
        if PY_TAINT.is_match(&residual) {
            issues.push(build_issue(
                "python-path-traversal",
                "security",
                Severity::High,
                "Request accessor used as filesystem path",
                "Static analysis matched a request accessor in a filesystem path argument without a recognized basename or secure_filename wrapper. It does not establish runtime reachability, upstream allowlisting, the process working directory, or a later canonical-root check. If traversal survives normalization, the operation may read, serve, or delete files outside the intended root within the process account's permissions.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("An open()/os.remove()/os.unlink()/send_file() call receives a request accessor directly as its path, with no os.path.basename() or secure_filename() confining it.".into()),
                Some("Trace the effective path policy. Prefer an opaque server-side object id or an allowlist. Otherwise treat input as one filename segment, join it beneath a fixed root, and compare a safely resolved existing target or parent directory against that root before opening; handle symlinks and write/create operations explicitly. For Flask downloads, prefer send_from_directory with a fixed trusted directory and current framework behavior.".into()),
                Some("Create an isolated temporary root with one allowed canary and a second canary just outside it. Test plain, dot-segment, absolute, encoded-separator, backslash, symlink, and create/write variants, and confirm only the inside canary can be reached.".into()),
            ));
            break;
        }
    }
}
