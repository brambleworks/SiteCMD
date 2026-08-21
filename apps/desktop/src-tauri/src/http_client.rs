//! Shared pooled HTTP clients, including a narrowly scoped localhost variant.

use reqwest::Client;
use std::fmt;
use std::sync::LazyLock;
use std::time::Duration;

const REDIRECT_LIMIT: usize = 10;

#[derive(Debug)]
pub enum BodyReadError {
    TooLarge { max_bytes: u64, received_bytes: u64 },
    TimedOut { timeout: Duration },
    Transport(reqwest::Error),
}

impl fmt::Display for BodyReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge {
                max_bytes,
                received_bytes,
            } => write!(
                formatter,
                "response body exceeded the {} byte limit after {} bytes",
                max_bytes, received_bytes
            ),
            Self::TimedOut { timeout } => {
                write!(formatter, "response body timed out after {timeout:?}")
            }
            Self::Transport(_) => write!(formatter, "failed to read response body"),
        }
    }
}

impl std::error::Error for BodyReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::TooLarge { .. } | Self::TimedOut { .. } => None,
        }
    }
}

/// Read an HTTP response body without allowing chunked or dishonest responses
/// to allocate beyond the caller's byte budget.
pub async fn read_body_limited(
    mut response: reqwest::Response,
    max_bytes: u64,
    timeout: Duration,
) -> Result<Vec<u8>, BodyReadError> {
    if let Some(content_length) = response.content_length() {
        if content_length > max_bytes {
            return Err(BodyReadError::TooLarge {
                max_bytes,
                received_bytes: content_length,
            });
        }
    }

    let read = async {
        let initial_capacity = response
            .content_length()
            .unwrap_or(0)
            .min(max_bytes)
            .try_into()
            .unwrap_or(0);
        let mut body = Vec::with_capacity(initial_capacity);
        while let Some(chunk) = response.chunk().await.map_err(BodyReadError::Transport)? {
            let received_bytes = body.len() as u64 + chunk.len() as u64;
            if received_bytes > max_bytes {
                return Err(BodyReadError::TooLarge {
                    max_bytes,
                    received_bytes,
                });
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    };

    tokio::time::timeout(timeout, read)
        .await
        .map_err(|_| BodyReadError::TimedOut { timeout })?
}

pub async fn read_text_limited(
    response: reqwest::Response,
    max_bytes: u64,
    timeout: Duration,
) -> Result<String, BodyReadError> {
    let body = read_body_limited(response, max_bytes, timeout).await?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

pub async fn read_json_limited<T>(
    response: reqwest::Response,
    max_bytes: u64,
    timeout: Duration,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let body = read_body_limited(response, max_bytes, timeout)
        .await
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&body).map_err(|error| format!("invalid JSON response: {error}"))
}

/// Shared HTTP client for production URLs.
/// Reuses TCP/TLS connections across requests.
pub fn client() -> &'static Client {
    static CLIENT: LazyLock<Client> = LazyLock::new(|| {
        Client::builder()
            .timeout(crate::constants::HTTP_CLIENT_TIMEOUT)
            .user_agent(crate::constants::USER_AGENT.as_str())
            .redirect(safe_redirect_policy(false))
            .dns_resolver(crate::dns_cache::shared())
            .build()
            .expect("Failed to build HTTP client")
    });
    &CLIENT
}

/// Shared HTTP client for localhost URLs.
/// Skips TLS certificate verification for self-signed local dev servers.
pub fn localhost_client() -> &'static Client {
    static CLIENT: LazyLock<Client> = LazyLock::new(|| {
        Client::builder()
            .timeout(crate::constants::HTTP_CLIENT_TIMEOUT)
            .user_agent(crate::constants::USER_AGENT.as_str())
            .redirect(safe_redirect_policy(true))
            .danger_accept_invalid_certs(true)
            .dns_resolver(crate::dns_cache::shared())
            .build()
            .expect("Failed to build localhost HTTP client")
    });
    &CLIENT
}

/// HTTP client that does NOT follow redirects.
/// Used by redirect chain checks, HTTPS enforcement, and open redirect detection.
pub fn no_redirect_client() -> &'static Client {
    static CLIENT: LazyLock<Client> = LazyLock::new(|| {
        Client::builder()
            .timeout(crate::constants::CHECK_PROBE_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(crate::constants::USER_AGENT.as_str())
            .dns_resolver(crate::dns_cache::shared())
            .build()
            .expect("Failed to build no-redirect HTTP client")
    });
    &CLIENT
}

/// Non-redirecting client with strict DNS policy for credentialed requests.
pub fn credentialed_service_client() -> &'static Client {
    static CLIENT: LazyLock<Client> = LazyLock::new(|| {
        Client::builder()
            .timeout(crate::constants::API_TIMEOUT_SHORT)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(crate::constants::USER_AGENT.as_str())
            .dns_resolver(crate::dns_cache::external_callback_resolver())
            .build()
            .expect("Failed to build credentialed service HTTP client")
    });
    &CLIENT
}

pub fn webhook_client() -> &'static Client {
    credentialed_service_client()
}

/// Policy-guarded client that preserves raw content-encoding responses.
pub fn no_decompress_client(is_strict_local: bool) -> &'static Client {
    static STRICT: LazyLock<Client> = LazyLock::new(|| build_no_decompress_client(true));
    static PUBLIC: LazyLock<Client> = LazyLock::new(|| build_no_decompress_client(false));
    if is_strict_local {
        &STRICT
    } else {
        &PUBLIC
    }
}

fn build_no_decompress_client(is_strict_local: bool) -> Client {
    Client::builder()
        .timeout(crate::constants::API_TIMEOUT_SHORT)
        .user_agent(crate::constants::USER_AGENT.as_str())
        .redirect(safe_redirect_policy(is_strict_local))
        .dns_resolver(crate::dns_cache::shared())
        .danger_accept_invalid_certs(is_strict_local)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .build()
        .expect("Failed to build no-decompress HTTP client")
}

/// Pick the appropriate shared client based on whether the URL is a strict
/// loopback address (localhost, 127.0.0.1,::1).
pub fn for_url(is_strict_local: bool) -> &'static Client {
    if is_strict_local {
        localhost_client()
    } else {
        client()
    }
}

fn safe_redirect_policy(allow_local_dev: bool) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= REDIRECT_LIMIT {
            return attempt.error(std::io::Error::other("too many redirects"));
        }

        // Validate IP literals here; domain redirects are checked at DNS resolution.
        if let Err(error) = crate::network_policy::validate_redirect_target_nonblocking(
            attempt.url(),
            crate::network_policy::UrlPolicy::Redirect { allow_local_dev },
        ) {
            return attempt.error(std::io::Error::other(error));
        }

        attempt.follow()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    fn ptr<T>(r: &T) -> *const T {
        r as *const T
    }

    #[test]
    fn for_url_routes_strict_local_to_localhost_client() {
        let routed = for_url(true);
        assert_eq!(
            ptr(routed),
            ptr(localhost_client()),
            "is_strict_local=true must return the localhost (cert-skipping) client"
        );
    }

    #[test]
    fn for_url_routes_non_local_to_production_client() {
        let routed = for_url(false);
        assert_eq!(
            ptr(routed),
            ptr(client()),
            "is_strict_local=false must return the TLS-validating production client"
        );
    }

    #[test]
    fn production_and_localhost_clients_are_distinct() {
        assert_ne!(
            ptr(client()),
            ptr(localhost_client()),
            "production and localhost clients must be separate instances - a regression that aliased them would silently accept invalid certs for public URLs"
        );
    }

    #[test]
    fn no_redirect_client_is_distinct_from_others() {
        assert_ne!(ptr(no_redirect_client()), ptr(client()));
        assert_ne!(ptr(no_redirect_client()), ptr(localhost_client()));
    }

    #[test]
    fn webhook_client_is_distinct_from_scan_clients() {
        assert_ne!(ptr(webhook_client()), ptr(client()));
        assert_ne!(ptr(webhook_client()), ptr(no_redirect_client()));
        assert_ne!(ptr(webhook_client()), ptr(localhost_client()));
        assert_eq!(ptr(webhook_client()), ptr(credentialed_service_client()));
    }

    #[tokio::test]
    async fn bounded_body_reader_rejects_oversized_chunked_response_before_eof() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind body-limit server");
        let addr = listener.local_addr().expect("body-limit server address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept body-limit request");
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\n\r\n",
                )
                .await
                .expect("write response head");
            for _ in 0..5 {
                stream
                    .write_all(b"8\r\n12345678\r\n")
                    .await
                    .expect("write response chunk");
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        });

        let response = localhost_client()
            .get(format!("http://{addr}/oversized"))
            .send()
            .await
            .expect("request chunked response");
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            read_body_limited(response, 32, std::time::Duration::from_secs(4)),
        )
        .await
        .expect("reader should reject before the server reaches EOF");

        assert!(matches!(
            result,
            Err(BodyReadError::TooLarge { max_bytes: 32, .. })
        ));
        server.abort();
    }
}
