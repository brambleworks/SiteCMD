//! Telling the OAuth authorization server's own token endpoint apart from a
//! client-side redirect callback. The client-callback checks do not apply to
//! the server side of the flow.

use std::sync::LazyLock;

/// The authorization server's own token endpoint, which reads a client secret
/// out of the incoming request. That side of OAuth has no browser state to
/// bind and no code verifier of its own, so the client-callback checks do not
/// apply to it.
pub(in crate::core::code_scan) static OAUTH_SERVER_REQUEST_SECRET_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r"(?i)\b(?:req|request)\s*(?:\.\s*body\s*)?\.\s*client_?secret\b")
            .expect("static request client-secret accessor regex"), // allow-expect: compile-time literal regex
        regex::Regex::new(
            r"(?is)\{[^}]{0,220}\bclient_?secret\b[^}]{0,220}\}\s*=\s*(?:await\s+)?(?:req|request)\b",
        )
        .expect("static request client-secret destructuring regex"), // allow-expect: compile-time literal regex
    ]
});

/// A client secret read off an already-parsed request body. On its own this is
/// also how a client builds its OUTGOING token request, so the caller pairs it
/// with the file actually parsing a request body: an OAuth redirect callback
/// reads query parameters, not a body.
pub(in crate::core::code_scan) static OAUTH_SERVER_PARSED_BODY_SECRET_PATTERNS: LazyLock<
    Vec<regex::Regex>,
> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r"(?i)\b(?:body|input|payload|dto|data)\s*\.\s*client_?secret\b")
            .expect("static parsed-body client-secret accessor regex"), // allow-expect: compile-time literal regex
    ]
});

#[cfg(test)]
mod tests {
    use super::{OAUTH_SERVER_PARSED_BODY_SECRET_PATTERNS, OAUTH_SERVER_REQUEST_SECRET_PATTERNS};

    fn any_match(patterns: &[regex::Regex], source: &str) -> bool {
        patterns.iter().any(|pattern| pattern.is_match(source))
    }

    #[test]
    fn a_client_secret_read_off_the_request_marks_the_authorization_server() {
        assert!(any_match(
            &OAUTH_SERVER_REQUEST_SECRET_PATTERNS,
            "exchangeCodeForTokens(clientId, req.body.code, req.body.client_secret)"
        ));
        assert!(any_match(
            &OAUTH_SERVER_REQUEST_SECRET_PATTERNS,
            "const { code, client_secret } = await request.json()"
        ));
        // A client callback sending its own configured secret is not the
        // server, and neither is one that names a local outgoing payload:
        // only the request receivers count here.
        assert!(!any_match(
            &OAUTH_SERVER_REQUEST_SECRET_PATTERNS,
            "client_secret: process.env.GITHUB_CLIENT_SECRET"
        ));
        assert!(!any_match(
            &OAUTH_SERVER_REQUEST_SECRET_PATTERNS,
            "exchangeCodeForTokens(clientId, payload.code, payload.client_secret)"
        ));
    }

    #[test]
    fn a_parsed_body_secret_is_only_a_shape_the_caller_pairs_with_body_parsing() {
        assert!(any_match(
            &OAUTH_SERVER_PARSED_BODY_SECRET_PATTERNS,
            "exchangeCodeForTokens(clientId, body.code, body.client_secret)"
        ));
        assert!(any_match(
            &OAUTH_SERVER_PARSED_BODY_SECRET_PATTERNS,
            "return this.exchange(clientId, dto.client_secret)"
        ));
        assert!(!any_match(
            &OAUTH_SERVER_PARSED_BODY_SECRET_PATTERNS,
            "client_secret: process.env.GITHUB_CLIENT_SECRET"
        ));
    }
}
