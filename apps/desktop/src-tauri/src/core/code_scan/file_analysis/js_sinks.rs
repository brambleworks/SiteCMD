use super::*;
use std::sync::LazyLock;

// Match request accessors only when they flow directly into a child_process
// command argument. A precise match suppresses the broader file-level rule.

/// Inbound request accessors: Express/Koa-style req.query/body/params and
/// the web-standard searchParams.get(...). The Python/PHP modules call the
/// same concept TAINT.
const TAINT: &str = r"(?:\breq(?:uest)?\.(?:query|body|params)\b|\bsearchParams\.get\s*\()";

/// Compile a compile-time-literal pattern. The single `.expect` line stays
/// under the guardrail's length limit so rustfmt never splits its
/// `allow-expect` marker onto a second line (the marker rule is line-based).
fn static_regex(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).expect("static regex") // allow-expect: compile-time literal regex
}

/// Evidence that the file actually uses Node's child_process module (require,
/// import, or the fully qualified `child_process.exec` form). Without this
/// gate, a bare `exec(` could be anything - a local helper, a DB driver.
static CHILD_PROCESS_IMPORT: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(
        r#"(?:require\s*\(\s*["'](?:node:)?child_process["']|from\s+["'](?:node:)?child_process["'])"#,
    )
});

/// Bare or `child_process` exec sinks, excluding method calls and longer identifiers.
static JS_EXEC_CALL: LazyLock<regex::Regex> = LazyLock::new(|| {
    static_regex(
        r"(?:^|[^A-Za-z0-9_$.])(?:child_process\s*\.\s*)?(?:exec|execSync|execFile|execFileSync|spawn|spawnSync)\s*\(",
    )
});

static JS_TAINT: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(TAINT));

/// Blank comments and optionally string interiors while preserving offsets.
/// Separate views distinguish quoted sink names from taint inside templates.
pub(super) fn blank_js(content: &str, blank_strings: bool) -> String {
    enum State {
        Code,
        Line,
        Block,
        Str(u8),
    }
    let bytes = content.as_bytes();
    let mut out = bytes.to_vec();
    let mut state = State::Code;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        match state {
            State::Code => match byte {
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    state = State::Line;
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    i += 2;
                    continue;
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    state = State::Block;
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    i += 2;
                    continue;
                }
                b'"' | b'\'' | b'`' => state = State::Str(byte),
                _ => {}
            },
            State::Line => {
                if byte == b'\n' {
                    state = State::Code;
                } else {
                    out[i] = b' ';
                }
            }
            State::Block => {
                if byte == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    state = State::Code;
                    i += 2;
                    continue;
                }
                if byte != b'\n' {
                    out[i] = b' ';
                }
            }
            State::Str(quote) => {
                if byte == b'\\' {
                    if blank_strings {
                        out[i] = b' ';
                        if i + 1 < out.len() && !bytes[i + 1].is_ascii_whitespace() {
                            out[i + 1] = b' ';
                        }
                    }
                    i += 2;
                    continue;
                }
                if byte == quote || (quote != b'`' && byte == b'\n') {
                    state = State::Code;
                } else if blank_strings && !byte.is_ascii_whitespace() {
                    out[i] = b' ';
                }
            }
        }
        i += 1;
    }
    // Only ASCII bytes are overwritten with spaces (multibyte chars are
    // blanked byte-by-byte, whole), so this stays valid UTF-8.
    String::from_utf8(out).unwrap_or_else(|_| content.to_string())
}

/// Bounded argument span ending at the matching closing parenthesis.
pub(super) fn call_arg_window(content: &str, after: usize, cap: usize) -> &str {
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

/// Return the first top-level argument, ignoring commas in strings and nested values.
fn first_arg(window: &str) -> &str {
    let bytes = window.as_bytes();
    let mut depth: i32 = 0;
    let mut quote: Option<u8> = None;
    let mut end = window.len();
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if let Some(open_quote) = quote {
            if byte == b'\\' {
                i += 2;
                continue;
            }
            if byte == open_quote {
                quote = None;
            }
        } else {
            match byte {
                b'"' | b'\'' | b'`' => quote = Some(byte),
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

/// Return the keyword's 1-based line, excluding any consumed leading separator.
fn keyword_line(content: &str, matched: &regex::Match<'_>) -> u32 {
    let keyword_start = content[matched.start()..matched.end()]
        .find(|ch: char| ch.is_ascii_alphabetic())
        .map(|offset| matched.start() + offset)
        .unwrap_or(matched.start());
    let newlines = content[..keyword_start]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count();
    newlines as u32 + 1
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

pub(super) fn collect_js_sink_issues(issues: &mut Vec<CodeIssue>, ctx: &FileAnalysisContext<'_>) {
    let file = ctx.file;
    if !is_js_or_ts_file(&file.relative_path) {
        return;
    }
    if !CHILD_PROCESS_IMPORT.is_match(ctx.content) {
        return;
    }
    let structure = blank_js(ctx.content, true);
    let scan = blank_js(ctx.content, false);

    for m in JS_EXEC_CALL.find_iter(&structure) {
        let arg = first_arg(call_arg_window(&scan, m.end(), 500));
        if JS_TAINT.is_match(arg) {
            issues.push(build_issue(
                "js-command-injection",
                "security",
                Severity::Critical,
                "Request accessor appears in process command input",
                "Static analysis matched a request accessor in the first command or executable argument of a Node child_process exec-family call. It does not establish runtime reachability, preceding validation, authorization, the effective options, or deployed exposure. If an attacker can reach the call and control that value, exec and execSync may interpret shell syntax, while spawn and execFile forms may allow executable selection depending on the matched API and options.",
                file,
                Some(keyword_line(&structure, &m)),
                Some("The first command/executable argument of an exec, execSync, spawn, or execFile-family call contains a request accessor directly.".into()),
                Some("Trace the matched value, API overload, and options first. Select a fixed executable in server code, keep shell mode disabled, and pass validated request-derived data only through an argument array. Apply a command-specific allowlist, reject leading-option values when the target could reinterpret them, and do not let input select the executable.".into()),
                Some("In a unit test or process mock, capture the executable, argument array, and options for valid input plus metacharacter and leading-option payloads. Confirm the executable and argument boundaries remain fixed, the shell is disabled, and invalid values are rejected without launching an unintended process.".into()),
            ));
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{blank_js, first_arg};

    #[test]
    fn string_blanking_keeps_offsets_and_template_taint_stays_in_scan_view() {
        let src = "const msg = `run exec(${cmd})`;\nexec(`ls ${req.query.dir}`);\n";
        let structure = blank_js(src, true);
        let scan = blank_js(src, false);
        assert_eq!(structure.len(), src.len());
        // The quoted `exec(` disappears from the structure view; the real
        // call at line 2 survives in both views.
        assert!(!structure[..structure.find('\n').unwrap()].contains("exec("));
        assert!(structure.contains("\nexec("));
        // Taint inside the template literal survives the scan view.
        assert!(scan.contains("req.query.dir"));
        assert!(!blank_js(src, true).contains("req.query.dir"));
    }

    #[test]
    fn first_arg_stops_at_the_argument_comma_not_inside_literals() {
        assert_eq!(
            first_arg("`git clone ${req.body.url}`, (err) => {})"),
            "`git clone ${req.body.url}`"
        );
        // Commas inside an array argument do not split early.
        assert_eq!(first_arg("['a', 'b'], opts)"), "['a', 'b']");
    }
}
