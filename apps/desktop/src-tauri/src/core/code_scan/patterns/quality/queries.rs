//! Database read shapes: unbounded list queries, the pagination syntax that
//! bounds them, and per-iteration lookups. Every rule here has to tell a
//! database call apart from a same-named DOM, canvas, or collection helper,
//! which is what `DB_RECEIVER` exists for.

use std::sync::LazyLock;

// Require structural pagination syntax; bare words appear in unrelated code.
pub(in crate::core::code_scan) static PAGINATION_GUARD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            // Object-literal keys with a value: `take: 50`, `limit: 100`
            regex::Regex::new(r"(?i)\btake\s*:\s*\d+").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)\blimit\s*:\s*\d+").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)\boffset\s*:\s*\d+").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)\bpageSize\s*:\s*\d+").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)\bpage_size\s*:\s*\d+").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)\bperPage\s*:\s*\d+").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)\bper_page\s*:\s*\d+").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"(?i)\b(?:take|limit|offset|pageSize|page_size|perPage|per_page)\s*:\s*[A-Za-z_$][A-Za-z0-9_$]*",
            )
            .expect("static variable pagination regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"(?i)[{,]\s*(?:take|limit|offset|pageSize|page_size|perPage|per_page|cursor)\s*[,}]",
            )
            .expect("static shorthand pagination regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)\bLIMIT\s+\d").expect("static pattern regex"), // allow-expect: compile-time literal regex
            // Explicit pagination method calls
            regex::Regex::new(r"(?i)\bpaginate\s*\(").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)\.cursor\s*\(").expect("static pattern regex"), // allow-expect: compile-time literal regex
            // Cursor as an object-literal key (Prisma-style)
            regex::Regex::new(r"(?i)\bcursor\s*:\s*\{").expect("static cursor-key regex"), // allow-expect: compile-time literal regex
        ]
    });

/// A database-shaped receiver, as either the whole identifier (`repo`, `db`)
/// or its camelCase or snake_case tail (`usersRepository`, `user_repo`,
/// `bookingModel`). Shared by every rule that has to tell a database call
/// apart from a same-named DOM, canvas, or collection helper, so the two
/// cannot drift into recognising different receivers.
const DB_RECEIVER: &str = concat!(
    r"\b(?:[A-Za-z0-9_$]*[a-z0-9_$])?",
    r"(?i:db|database|repository|repo|prisma|knex|store|table|collection|model|dao)",
);

pub(in crate::core::code_scan) static LIST_QUERY_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"\.findMany\s*\(").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\.find\s*\(\s*\{").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\.select\s*\(\s*\)").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"(?i)SELECT\s+.+\s+FROM\s+").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\.query\.").expect("static pattern regex"), // allow-expect: compile-time literal regex
            // `getAll` is also the URLSearchParams and FormData reader, so it
            // counts as a list query only on a database-shaped receiver.
            regex::Regex::new(&(DB_RECEIVER.to_string() + r"\.getAll\s*\("))
                .expect("static getAll receiver regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\.list\s*\(").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"\.scan\s*\(").expect("static pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

/// One per-iteration database lookup, as it appears after a loop opener.
///
/// Bare `.get` and `.query` are intentionally not lookup signals: Map/HashMap
/// reads and query-cache access inside loops are normal in-memory work.
/// `findOne` is shared with canvas and DOM libraries (Konva's
/// `layer.findOne('#id')` selector), so it counts only on a database-shaped
/// receiver or when its argument is an object-literal query.
fn nplus1_lookup_call() -> String {
    r"(?:\.(?:findUnique|findFirst|findById|findByPk)\s*\(".to_string()
        // `get` and `query` stay on the four literal database handles on
        // purpose: widening them to every `DB_RECEIVER` would readmit
        // `store.get(` and `collection.get(`, the in-memory reads this rule
        // deliberately excludes.
        + r"|\b(?:db|database|repository|repo)\.(?:get|query)\s*\("
        + "|"
        + DB_RECEIVER
        + r"\.findOne\s*\("
        // Any receiver, as long as the query is not a selector string: Konva's
        // `layer.findOne('#id')` is the shape that must not count, while
        // `findOne(id)` and `findOne({ where })` are database reads.
        + r"|\.findOne\s*\(\s*(?:\{|[A-Za-z_$]))"
}

pub(in crate::core::code_scan) static NPLUS1_ORM_IN_LOOP_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        let lookup_call = nplus1_lookup_call();
        vec![
            regex::Regex::new(
                &(r"(?s)for\s*\([^)]*\)\s*\{[^}]{0,200}".to_string() + &lookup_call),
            )
            .expect("static for-loop lookup regex"), // allow-expect: compile-time literal regex
            // One level of parentheses inside the callback header, so the
            // parenthesised parameter list of `forEach((item) => {` and
            // `forEach(async (item) => {` opens a loop like the bare form.
            regex::Regex::new(
                &(r"(?s)\.forEach\s*\((?:[^()]|\([^()]*\))*=>\s*\{[^}]{0,200}".to_string()
                    + &lookup_call),
            )
            .expect("static forEach lookup regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                &(r"(?s)\.map\s*\((?:[^()]|\([^()]*\))*=>\s*\{[^}]{0,200}".to_string()
                    + &lookup_call),
            )
            .expect("static map lookup regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"(?s)for\s+\w+\s+in\s+[^:]+:[^}]{0,200}(?:\b[A-Za-z_][A-Za-z0-9_]*\.objects\.(?:get|filter)|\b(?:session|db)\.query)\s*\(",
            )
            .expect("static pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

#[cfg(test)]
mod tests {
    use super::{LIST_QUERY_PATTERNS, NPLUS1_ORM_IN_LOOP_PATTERNS};

    fn any_match(patterns: &[regex::Regex], source: &str) -> bool {
        patterns.iter().any(|pattern| pattern.is_match(source))
    }

    #[test]
    fn get_all_counts_only_on_a_database_receiver() {
        assert!(!any_match(
            &LIST_QUERY_PATTERNS,
            "const tokens = searchParams.getAll(\"token\");"
        ));
        assert!(!any_match(
            &LIST_QUERY_PATTERNS,
            "const files = formData.getAll(\"file\");"
        ));
        assert!(any_match(
            &LIST_QUERY_PATTERNS,
            "const rows = await store.getAll();"
        ));
        assert!(any_match(
            &LIST_QUERY_PATTERNS,
            "const rows = await dbTable.getAll();"
        ));
        // The same receiver fragment both rules use, in its suffix form.
        assert!(any_match(
            &LIST_QUERY_PATTERNS,
            "const rows = await usersRepository.getAll();"
        ));
    }

    #[test]
    fn find_one_in_a_loop_needs_a_database_shape() {
        let konva = "for (const page of pages) {\n  const node = layer.current.findOne('#page-' + page.id);\n}";
        assert!(!any_match(&NPLUS1_ORM_IN_LOOP_PATTERNS, konva));

        let mongoose = "for (const id of ids) {\n  const row = await User.findOne({ _id: id });\n}";
        assert!(any_match(&NPLUS1_ORM_IN_LOOP_PATTERNS, mongoose));

        let repository =
            "for (const id of ids) {\n  const row = await usersRepository.findOne(id);\n}";
        assert!(any_match(&NPLUS1_ORM_IN_LOOP_PATTERNS, repository));

        let prisma = "for (const post of posts) {\n  const author = await prisma.user.findUnique({ where: { id: post.authorId } });\n}";
        assert!(any_match(&NPLUS1_ORM_IN_LOOP_PATTERNS, prisma));

        // Recall the narrowing must not cost: a bare handle, the NestJS
        // service shape, and a non-object lookup argument.
        let bare_repo = "for (const id of ids) {\n  const row = await repo.findOne(id);\n}";
        assert!(any_match(&NPLUS1_ORM_IN_LOOP_PATTERNS, bare_repo));

        let nest = "ids.forEach(async (id) => {\n  const row = await this.userRepository.findOne({ where: { id } });\n});";
        assert!(any_match(&NPLUS1_ORM_IN_LOOP_PATTERNS, nest));

        let by_id = "for (const id of ids) {\n  const row = await User.findOne(id);\n}";
        assert!(any_match(&NPLUS1_ORM_IN_LOOP_PATTERNS, by_id));
    }
}
