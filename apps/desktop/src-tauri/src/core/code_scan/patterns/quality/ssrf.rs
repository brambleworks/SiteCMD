//! Server-side fetch destinations: the sink shapes, which of them are only a
//! bare local identifier, what binds such an identifier to a request, and the
//! local destination policies that clear the finding.

use std::sync::LazyLock;

pub(in crate::core::code_scan) static SSRF_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(
    || {
        vec![
            regex::Regex::new(
                r"fetch\s*\(\s*(?:url|body\.url|req\.body\.url|requestUrl|targetUrl)",
            )
            .expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"axios\.(get|post)\s*\(\s*(?:url|body\.url|req\.body\.url|requestUrl|targetUrl)",
            )
            .expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r#"requests\.(get|post)\s*\(\s*(?:url|data\[['"]url['"]\]|target_url)"#,
            )
            .expect("static pattern regex"), // allow-expect: compile-time literal regex
            // Direct request-accessor arguments - fetch(req.query.url) and
            // requests.get(request.args["url"]) produced no finding when
            // only fixed variable names were matched.
            regex::Regex::new(r"fetch\s*\(\s*req(?:uest)?\.(?:query|body|params)\.[A-Za-z0-9_]+")
                .expect("static SSRF pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"axios\.(?:get|post)\s*\(\s*req(?:uest)?\.(?:query|body|params)\.[A-Za-z0-9_]+",
            )
            .expect("static SSRF pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"fetch\s*\(\s*searchParams\.get\s*\(")
                .expect("static SSRF pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(
                r"requests\.(?:get|post)\s*\(\s*request\.(?:args|form|values|GET|POST)(?:\.get\s*\(|\s*\[)",
            )
            .expect("static SSRF pattern regex"), // allow-expect: compile-time literal regex
        ]
    },
);

/// The SSRF sink forms whose destination argument is a bare local identifier.
///
/// Matching one of these does not by itself show a request-derived
/// destination: the same variable names routinely hold a URL built from a
/// stored OAuth credential, a configuration value, or a provider's own
/// pagination link. `SSRF_REQUEST_SOURCE_ASSIGNMENT_PATTERNS` supplies the
/// missing half.
pub(in crate::core::code_scan) static SSRF_BARE_IDENTIFIER_FETCH_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r"fetch\s*\(\s*(?:url|requestUrl|targetUrl)\b")
            .expect("static bare-identifier fetch regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(r"axios\.(?:get|post)\s*\(\s*(?:url|requestUrl|targetUrl)\b")
            .expect("static bare-identifier axios regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(r"requests\.(?:get|post)\s*\(\s*(?:url|target_url)\b")
            .expect("static bare-identifier requests regex"), // allow-expect: compile-time literal regex
    ]
});

/// An in-file binding of one of those identifiers to a request value, which is
/// what makes a bare-identifier fetch request-derived.
///
/// The bindings cover the assignment form (`const url = req.body.url`), the
/// parsed-body form (`const url = (await req.json()).url`), and the
/// destructuring form (`const { url } = await req.json()`), which is the
/// shape most Next.js route handlers use.
pub(in crate::core::code_scan) static SSRF_REQUEST_SOURCE_ASSIGNMENT_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(
            r"\b(?:url|requestUrl|targetUrl|target_url)\s*=[^;\n]{0,200}\b(?:req|request)\.(?:body|query|params|url|args|form|values|GET|POST|json|text|formData)\b",
        )
        .expect("static request-accessor binding regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(
            r"\b(?:url|requestUrl|targetUrl|target_url)\s*=[^;\n]{0,200}\b(?:searchParams|formData|params|query|body|payload|input|values)\.get\s*\(",
        )
        .expect("static request-getter binding regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(
            r"\b(?:url|requestUrl|targetUrl|target_url)\s*=[^;\n]{0,200}\b(?:body|input|payload|params|query|data)\.[A-Za-z0-9_]+",
        )
        .expect("static request-payload binding regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(
            r"(?is)\{[^}]{0,220}\b(?:url|requestUrl|targetUrl|target_url)\b[^}]{0,220}\}\s*=\s*(?:await\s+)?(?:req|request|body|input|payload|params|query|data)\b",
        )
        .expect("static request-destructuring binding regex"), // allow-expect: compile-time literal regex
    ]
});

pub(in crate::core::code_scan) static SSRF_GUARD_PATTERNS: LazyLock<Vec<regex::Regex>> =
    LazyLock::new(|| {
        vec![
            regex::Regex::new(r"allowlist").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"hostname").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"host_whitelist").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"new URL").expect("static pattern regex"), // allow-expect: compile-time literal regex
            regex::Regex::new(r"urlparse").expect("static pattern regex"), // allow-expect: compile-time literal regex
        ]
    });

#[cfg(test)]
mod tests {
    use super::SSRF_REQUEST_SOURCE_ASSIGNMENT_PATTERNS;

    fn any_match(patterns: &[regex::Regex], source: &str) -> bool {
        patterns.iter().any(|pattern| pattern.is_match(source))
    }

    #[test]
    fn a_bare_fetch_identifier_needs_a_request_binding() {
        assert!(!any_match(
            &SSRF_REQUEST_SOURCE_ASSIGNMENT_PATTERNS,
            "const url = `${credential.account.href}/projects.json`;"
        ));
        assert!(any_match(
            &SSRF_REQUEST_SOURCE_ASSIGNMENT_PATTERNS,
            "const url = req.body.url;"
        ));
        assert!(any_match(
            &SSRF_REQUEST_SOURCE_ASSIGNMENT_PATTERNS,
            "const targetUrl = searchParams.get(\"target\");"
        ));
        assert!(any_match(
            &SSRF_REQUEST_SOURCE_ASSIGNMENT_PATTERNS,
            "const requestUrl = body.destination;"
        ));
        // The two shapes most Next.js route handlers actually use.
        assert!(any_match(
            &SSRF_REQUEST_SOURCE_ASSIGNMENT_PATTERNS,
            "const { url } = await req.json();"
        ));
        assert!(any_match(
            &SSRF_REQUEST_SOURCE_ASSIGNMENT_PATTERNS,
            "const url = (await req.json()).url;"
        ));
    }
}
