use super::{
    build_token_exchange_form, build_token_refresh_form, parse_callback_request,
    start_callback_server, token_error_message, TokenOperation,
};
use reqwest::StatusCode;

#[test]
fn token_exchange_keeps_pkce_and_the_desktop_client_credential() {
    let form = build_token_exchange_form(
        "client-id",
        Some(" desktop-credential "),
        "verifier",
        "code",
        "http://localhost:1234/callback",
    );

    assert_eq!(
        form,
        vec![
            ("code", "code"),
            ("client_id", "client-id"),
            ("code_verifier", "verifier"),
            ("redirect_uri", "http://localhost:1234/callback"),
            ("grant_type", "authorization_code"),
            ("client_secret", "desktop-credential"),
        ]
    );
}

#[test]
fn token_refresh_includes_the_desktop_client_credential() {
    let form = build_token_refresh_form("client-id", Some(" desktop-credential "), "refresh-token");

    assert_eq!(
        form,
        vec![
            ("refresh_token", "refresh-token"),
            ("client_id", "client-id"),
            ("grant_type", "refresh_token"),
            ("client_secret", "desktop-credential"),
        ]
    );
}

#[test]
fn token_forms_omit_unconfigured_credentials() {
    for secret in [None, Some(""), Some(" \t\n")] {
        let forms = [
            build_token_exchange_form(
                "client",
                secret,
                "verifier",
                "code",
                "http://localhost:1/callback",
            ),
            build_token_refresh_form("client", secret, "refresh"),
        ];
        for form in forms {
            assert!(!form.iter().any(|(key, _)| *key == "client_secret"));
            assert!(form.contains(&("client_id", "client")));
        }
    }
}

#[test]
fn missing_google_credential_explains_how_to_repair_the_build() {
    for operation in [TokenOperation::Exchange, TokenOperation::Refresh] {
        let message = token_error_message(
            operation,
            StatusCode::BAD_REQUEST,
            r#"{"error":"invalid_request","error_description":"client_secret is missing."}"#,
        );
        assert!(message.contains("GOOGLE_CLIENT_SECRET"));
        assert!(message.contains("rebuild"));
    }
}

#[test]
fn rejected_client_credentials_require_a_matching_desktop_client() {
    for code in ["invalid_client", "unauthorized_client"] {
        let message = token_error_message(
            TokenOperation::Exchange,
            StatusCode::UNAUTHORIZED,
            &format!(r#"{{"error":"{code}"}}"#),
        );
        assert!(message.contains("same Desktop OAuth client"));
    }
}

#[test]
fn invalid_grants_distinguish_reauthorization_from_expired_refresh_tokens() {
    let body = r#"{"error":"invalid_grant"}"#;
    let exchange = token_error_message(TokenOperation::Exchange, StatusCode::BAD_REQUEST, body);
    let refresh = token_error_message(TokenOperation::Refresh, StatusCode::BAD_REQUEST, body);
    assert!(exchange.contains("authorization code"));
    assert_eq!(
        refresh,
        "Google authorization expired. Reconnect Google and try again."
    );
}

#[test]
fn provider_diagnostics_are_never_echoed_or_classified_by_substring() {
    for body in [
        "credential-sentinel invalid_grant",
        r#"{"error":"credential-sentinel","error_description":"invalid_grant"}"#,
        r#"{"error":"invalid_request","error_description":"credential-sentinel invalid_grant"}"#,
    ] {
        let message = token_error_message(TokenOperation::Refresh, StatusCode::BAD_REQUEST, body);
        assert!(message.contains("400 Bad Request"));
        assert!(!message.contains("credential-sentinel"));
        assert!(!message.contains("authorization expired"));
    }
    let message = token_error_message(
        TokenOperation::Refresh,
        StatusCode::BAD_REQUEST,
        r#"{"error":"invalid_grant","error_description":"credential-sentinel"}"#,
    );
    assert!(!message.contains("credential-sentinel"));
}

#[test]
fn callback_parser_requires_get_callback_code_and_state() {
    assert_eq!(
        parse_callback_request("GET /callback?code=abc&state=xyz HTTP/1.1\r\n\r\n"),
        Some(("abc".to_string(), "xyz".to_string()))
    );
    assert!(parse_callback_request("POST /callback?code=abc&state=xyz HTTP/1.1\r\n\r\n").is_none());
    assert!(parse_callback_request("GET /other?code=abc&state=xyz HTTP/1.1\r\n\r\n").is_none());
    assert!(parse_callback_request("GET /callback?code=abc HTTP/1.1\r\n\r\n").is_none());
}

#[tokio::test]
async fn invalid_local_request_does_not_consume_callback_listener() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let (port, receiver) = start_callback_server("expected-state".to_string())
        .await
        .expect("start callback server");

    let mut invalid = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect invalid callback");
    invalid
        .write_all(b"GET /callback?code=bad&state=wrong HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("write invalid callback");
    let mut invalid_response = Vec::new();
    invalid
        .read_to_end(&mut invalid_response)
        .await
        .expect("read invalid response");
    assert!(String::from_utf8_lossy(&invalid_response).contains("400 Bad Request"));

    let mut valid = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect valid callback");
    valid
        .write_all(b"GET /callback?code=good-code&state=expected-state HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("write valid callback");
    let mut valid_response = Vec::new();
    valid
        .read_to_end(&mut valid_response)
        .await
        .expect("read valid response");
    assert!(String::from_utf8_lossy(&valid_response).contains("200 OK"));

    let code = tokio::time::timeout(std::time::Duration::from_secs(2), receiver)
        .await
        .expect("callback should complete")
        .expect("callback channel");
    assert_eq!(code, "good-code");
}
