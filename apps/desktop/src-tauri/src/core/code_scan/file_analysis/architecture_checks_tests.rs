use super::*;

/// Build a route body with `vars` request-derived assignments followed by
/// `evals` dynamic-evaluation calls, only the last of which reads one of them.
/// Every earlier call still forces the request-variable matchers to be
/// consulted, so compiling them per call makes the work quadratic.
fn synthetic_route(vars: usize, evals: usize) -> String {
    let mut content = String::from("export async function POST(req: Request) {\n");
    for index in 0..vars {
        content.push_str(&format!(
            "  const requestValue{index} = await req.json();\n"
        ));
    }
    for index in 0..evals - 1 {
        content.push_str(&format!("  const constant{index} = eval(\"1 + 1\");\n"));
    }
    content.push_str(&format!(
        "  const result = eval(requestValue{});\n",
        vars - 1
    ));
    content.push_str("  return Response.json({ result });\n}\n");
    content
}

/// Compiling one pattern per request variable per evaluation call took ~40s in
/// a debug build on this ~48KB input. The bound is far above the ~30ms the
/// compiled-once path needs, so only a return of the quadratic compile can
/// trip it.
#[test]
fn request_variable_matchers_are_not_recompiled_for_every_evaluation_call() {
    let vars = 600;
    let evals = 600;
    let content = synthetic_route(vars, evals);

    let start = std::time::Instant::now();
    let line = request_derived_dynamic_eval_line(&content);
    let elapsed = start.elapsed();

    assert_eq!(
        line,
        Some((vars + evals + 1) as u32),
        "the only request-derived evaluation is the last call in the file"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "analysing a {}-byte route took {:?}; request-variable matchers are being recompiled per evaluation call",
        content.len(),
        elapsed
    );
}

/// The matchers are grouped into alternations for speed. Grouping must not
/// change which arguments count as request-derived, including for names the
/// old per-name patterns handled unusually (a leading or embedded `$` is not
/// a regex word character, so `\b` behaves differently around it).
#[test]
fn chunked_request_variable_matchers_accept_exactly_the_per_name_patterns() {
    let mut names = (0..REQUEST_VAR_MATCHER_CHUNK * 2 + 7)
        .map(|index| format!("requestValue{index}"))
        .collect::<std::collections::BTreeSet<String>>();
    names.insert("$payload".to_string());
    names.insert("body$raw".to_string());
    let matchers = request_var_matchers(&names);
    assert!(
        matchers.len() > 2,
        "the sample must span more than one chunk to exercise grouping"
    );

    let samples = [
        "(requestValue0)",
        "(requestValue1)",
        "(requestValue10)",
        "(requestValue5000)",
        "(prefixrequestValue3)",
        "(requestValue3suffix)",
        "($payload)",
        "(x.$payload)",
        "(body$raw)",
        "(JSON.parse(\"{}\"))",
        "()",
    ];
    for arguments in samples {
        let one_pattern_per_name = names.iter().any(|name| {
            regex::Regex::new(&format!(r"\b{}\b", regex::escape(name)))
                .map(|pattern| pattern.is_match(arguments))
                .unwrap_or(false)
        });
        assert_eq!(
            has_any(arguments, &matchers),
            one_pattern_per_name,
            "grouped matchers disagreed with per-name patterns on {:?}",
            arguments
        );
    }
}

#[test]
fn a_credential_literal_being_hashed_is_recognised_as_a_hash_argument() {
    let source = r#"await bcrypt.hash("password123", 10);"#;
    let start = source.find(r#""password123""#).unwrap();
    assert!(is_hash_call_argument(source, start));

    let argon = r#"const digest = await argon2.hash('changeme');"#;
    let argon_start = argon.find("'changeme'").unwrap();
    assert!(is_hash_call_argument(argon, argon_start));

    let bare = r#"const digest = hash("supersecretkey");"#;
    let bare_start = bare.find(r#""supersecretkey""#).unwrap();
    assert!(is_hash_call_argument(bare, bare_start));
}

#[test]
fn a_shipped_default_credential_is_not_a_hash_argument() {
    let fallback = r#"const secret = process.env.ADMIN_PASSWORD || "changeme";"#;
    let fallback_start = fallback.find(r#""changeme""#).unwrap();
    assert!(!is_hash_call_argument(fallback, fallback_start));

    // A same-named helper that is not a hash call must not excuse the literal.
    let lookalike = r#"const key = geoHash("password123");"#;
    let lookalike_start = lookalike.find(r#""password123""#).unwrap();
    assert!(!is_hash_call_argument(lookalike, lookalike_start));
}
