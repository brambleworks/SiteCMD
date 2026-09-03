use super::*;

/// A data-access module whose methods each own one write. Two writes in such a
/// file belong to two callers, never to one unatomic operation.
pub(in crate::core::code_scan) fn is_repository_module(relative_path: &str) -> bool {
    let lower = relative_path.replace('\\', "/").to_ascii_lowercase();
    lower.contains("/repositories/")
        || lower.starts_with("repositories/")
        || lower.contains("/repository/")
        || lower.starts_with("repository/")
        || lower
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains(".repository."))
}

pub(in crate::core::code_scan) fn is_js_source_path(relative_path: &str) -> bool {
    Path::new(relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            JS_SOURCE_EXTENSIONS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
}

/// Awaited write-like database calls inside the busiest single function or
/// method body, so sibling handlers and sibling repository methods in one
/// module are never counted as one multi-step write. Non-JavaScript files, and
/// JavaScript files with no brace-delimited body, keep the whole-file count.
///
/// The result is never zero for a file that holds two or more write-like calls.
/// Callers read this count as "does this file write to a database" as well as
/// "how many writes share one handler": `has_db_query` and the server-action
/// work test both depend on it, so a zero would withdraw the file's database
/// classification rather than merely lower a count.
///
/// Brace balancing is textual: a brace inside a string or template literal can
/// shift a body boundary, which moves the per-handler figure but never the
/// non-zero floor above.
pub(in crate::core::code_scan) fn max_db_writes_per_handler(
    file: &SourceFile,
    content: &str,
) -> usize {
    let whole_file = count_matches(content, &DB_WRITE_PATTERNS);
    if whole_file < 2 || !is_js_source_path(&file.relative_path) {
        return whole_file;
    }
    let awaited = awaited_db_write_positions(content);
    if awaited.len() < 2 {
        // The file holds at least two write-like calls to have reached here, so
        // never report zero: `db_write_count` also feeds `has_db_query` and the
        // server-action work test, where zero would erase a handler's database
        // and public classification entirely.
        return awaited.len().max(1);
    }
    let bodies = js_function_body_ranges(content);
    if bodies.is_empty() {
        return max_awaited_per_top_level_function(content, &awaited);
    }
    // The same floor as the early return above: the file holds at least two
    // write-like calls, so a body map that places none of them inside a
    // function must not report that the file touches no database.
    bodies
        .iter()
        .map(|(open, close)| {
            awaited
                .iter()
                .filter(|position| *position > open && *position < close)
                .count()
        })
        .max()
        .unwrap_or(0)
        .max(1)
}

/// Fallback for a file whose braces never resolve into a body, such as a
/// minified bundle: split on column-zero function starts so sibling module
/// functions still count separately.
fn max_awaited_per_top_level_function(content: &str, awaited: &[usize]) -> usize {
    let starts = JS_TOP_LEVEL_FUNCTION_START_PATTERN
        .find_iter(content)
        .map(|found| found.start())
        .collect::<Vec<_>>();
    if starts.is_empty() {
        return awaited.len();
    }
    let mut busiest = awaited.iter().filter(|at| **at < starts[0]).count();
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(content.len());
        busiest = busiest.max(
            awaited
                .iter()
                .filter(|at| **at >= *start && **at < end)
                .count(),
        );
    }
    busiest
}

/// Byte offsets of write-like database calls that the surrounding statement
/// awaits. A returned or fire-and-forget call is not part of a sequence the
/// handler can wrap in one transaction.
fn awaited_db_write_positions(content: &str) -> Vec<usize> {
    let mut positions = DB_WRITE_PATTERNS
        .iter()
        .flat_map(|pattern| pattern.find_iter(content).map(|found| found.start()))
        .filter(|start| statement_awaits(content, *start))
        .collect::<Vec<_>>();
    positions.sort_unstable();
    positions.dedup();
    positions
}

/// Whether the expression that contains the call at `at` is awaited.
///
/// The scan walks back to the start of the statement: past balanced braces, and
/// past the unclosed `(` or `[` of an enclosing call or array, so both writes in
/// `await Promise.all([a.create(...), b.create(...)])` count. It stops at the
/// `;` that ends the previous statement or at the `{` that opens the block.
fn statement_awaits(content: &str, at: usize) -> bool {
    const STATEMENT_LOOK_BACK: usize = 400;
    let mut window_start = at.saturating_sub(STATEMENT_LOOK_BACK);
    while !content.is_char_boundary(window_start) {
        window_start += 1;
    }
    let head = &content.as_bytes()[window_start..at];
    let mut brace_depth = 0usize;
    let mut statement_start = 0;
    for (offset, byte) in head.iter().enumerate().rev() {
        match byte {
            b'}' => brace_depth += 1,
            b'{' => {
                if brace_depth == 0 {
                    statement_start = offset + 1;
                    break;
                }
                brace_depth -= 1;
            }
            b';' if brace_depth == 0 => {
                statement_start = offset + 1;
                break;
            }
            _ => {}
        }
    }
    content[window_start + statement_start..at]
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|word| word == "await")
}

/// Open and close offsets of every brace-delimited function, method, or arrow
/// body in the file, found by balancing braces in one pass.
fn js_function_body_ranges(content: &str) -> Vec<(usize, usize)> {
    let bytes = content.as_bytes();
    let mut open_braces: Vec<(usize, bool)> = Vec::new();
    let mut bodies = Vec::new();
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'{' => {
                // Bounded look-back: indentation and line breaks are short, and
                // an unbounded scan would be quadratic on large files.
                let window_start = index.saturating_sub(200);
                let preceding = bytes[window_start..index]
                    .iter()
                    .rposition(|candidate| !candidate.is_ascii_whitespace())
                    .map(|position| bytes[window_start + position]);
                // A body opens after a parameter list or an arrow.
                let is_body = matches!(preceding, Some(b')') | Some(b'>'));
                open_braces.push((index, is_body));
            }
            b'}' => {
                if let Some((open, is_body)) = open_braces.pop() {
                    if is_body {
                        bodies.push((open, index));
                    }
                }
            }
            _ => {}
        }
    }
    bodies
}
