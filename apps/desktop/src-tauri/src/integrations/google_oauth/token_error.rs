use reqwest::StatusCode;
use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
pub(super) enum TokenOperation {
    Exchange,
    Refresh,
}

#[derive(Deserialize)]
struct ProviderError {
    error: String,
    error_description: Option<String>,
}

pub(super) fn token_error_message(
    operation: TokenOperation,
    status: StatusCode,
    body: &str,
) -> String {
    let error = serde_json::from_str::<ProviderError>(body).ok();
    let code = error.as_ref().map(|error| error.error.as_str());
    let description = error
        .as_ref()
        .and_then(|error| error.error_description.as_deref());
    // Provider text can contain credentials; expose only these fixed classifications.
    let message = match (code, description, operation) {
        (Some("invalid_request"), Some("client_secret is missing."), _) => Some(
            "This SiteCMD build is missing Google's desktop client credential. Update SiteCMD, or configure GOOGLE_CLIENT_SECRET and rebuild a local development copy.",
        ),
        (Some("invalid_client" | "unauthorized_client"), _, _) => Some(
            "Google rejected this SiteCMD build's OAuth credentials. Update SiteCMD, or rebuild with the ID and secret from the same Desktop OAuth client.",
        ),
        (Some("invalid_grant"), _, TokenOperation::Exchange) => Some(
            "Google could not verify the authorization code. Start connecting Google again.",
        ),
        (Some("invalid_grant"), _, TokenOperation::Refresh) => Some(
            "Google authorization expired. Reconnect Google and try again.",
        ),
        _ => None,
    };
    tracing::warn!(?operation, %status, recognized_error = message.is_some(), "Google token request rejected");
    message.map(str::to_string).unwrap_or_else(|| {
        let operation = match operation {
            TokenOperation::Exchange => "exchange",
            TokenOperation::Refresh => "refresh",
        };
        format!("Google token {operation} returned {status}. Try connecting Google again.")
    })
}
