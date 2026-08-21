//! Desktop reqwest adapter for portable engine probe outcomes.

use sitecmd_engine::probe::{
    decode_probe_body, BodyPolicy, ProbeFailure, ProbeFailureClass, ProbeMethod, ProbeOutcome,
    ProbeRequest, ProbeResponse, RedirectPolicy,
};

/// GET one probe URL under the standard probe timeout and body cap, with
/// the common policies (follow redirects, 2xx body).
pub(crate) async fn probe_get(client: &reqwest::Client, url: &str) -> ProbeOutcome {
    probe(client, ProbeRequest::get(url)).await
}

/// Execute one probe request. `client` serves redirect-following requests;
/// `RedirectPolicy::None` swaps in the shared no-redirect client so a 3xx
/// classifies as the answer instead of being followed.
pub(crate) async fn probe(client: &reqwest::Client, request: ProbeRequest) -> ProbeOutcome {
    probe_with_timeout(client, request, None).await
}

/// Execute one probe request under an explicit timeout. Link probing uses a
/// longer budget for different-host destinations.
pub(crate) async fn probe_with_timeout(
    client: &reqwest::Client,
    request: ProbeRequest,
    timeout: Option<std::time::Duration>,
) -> ProbeOutcome {
    let client = match request.redirects {
        RedirectPolicy::Follow => client,
        RedirectPolicy::None => crate::http_client::no_redirect_client(),
    };
    let mut builder = match request.method {
        ProbeMethod::Get => client.get(&request.url),
        ProbeMethod::Head => client.head(&request.url),
    }
    .timeout(timeout.unwrap_or(crate::constants::CHECK_PROBE_TIMEOUT));
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }

    let response = match builder.send().await {
        Ok(response) => response,
        Err(error) => {
            return ProbeOutcome::Failure(ProbeFailure {
                class: if error.is_timeout() {
                    ProbeFailureClass::Timeout
                } else {
                    ProbeFailureClass::Transport
                },
                detail: error.to_string(),
            })
        }
    };

    let status = response.status().as_u16();
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let content_length = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect();

    let read_body = match (request.method, request.body) {
        (ProbeMethod::Head, _) | (_, BodyPolicy::None) => false,
        (_, BodyPolicy::SuccessOnly) => response.status().is_success(),
        (_, BodyPolicy::Always) => true,
    };
    if !read_body {
        return ProbeOutcome::Response(ProbeResponse {
            status,
            final_url,
            content_type,
            content_length,
            headers,
            body: None,
        });
    }

    match crate::http_client::read_body_limited(
        response,
        crate::constants::MAX_PROBE_BODY_SIZE,
        crate::constants::CHECK_PROBE_TIMEOUT,
    )
    .await
    {
        Ok(bytes) => ProbeOutcome::Response(ProbeResponse {
            status,
            final_url,
            content_type,
            content_length,
            headers,
            body: Some(decode_probe_body(bytes)),
        }),
        // SuccessOnly consumers need the body as evidence; Always consumers
        // treat the status line as primary and degrade to an absent body.
        Err(error) => match request.body {
            BodyPolicy::Always => ProbeOutcome::Response(ProbeResponse {
                status,
                final_url,
                content_type,
                content_length,
                headers,
                body: None,
            }),
            _ => ProbeOutcome::Failure(failure_from_body_error(error)),
        },
    }
}

fn failure_from_body_error(error: crate::http_client::BodyReadError) -> ProbeFailure {
    use crate::http_client::BodyReadError;
    ProbeFailure {
        class: match &error {
            BodyReadError::TooLarge { .. } => ProbeFailureClass::BodyCapExceeded,
            BodyReadError::TimedOut { .. } => ProbeFailureClass::Timeout,
            BodyReadError::Transport(_) => ProbeFailureClass::Transport,
        },
        detail: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::BodyReadError;

    #[test]
    fn body_read_errors_classify_by_kind() {
        let too_large = failure_from_body_error(BodyReadError::TooLarge {
            max_bytes: 10,
            received_bytes: 11,
        });
        assert_eq!(too_large.class, ProbeFailureClass::BodyCapExceeded);
        let timed_out = failure_from_body_error(BodyReadError::TimedOut {
            timeout: std::time::Duration::from_secs(5),
        });
        assert_eq!(timed_out.class, ProbeFailureClass::Timeout);
        assert!(!too_large.detail.is_empty() && !timed_out.detail.is_empty());
    }

    #[tokio::test]
    async fn refused_connection_classifies_as_transport_failure() {
        let outcome = probe_get(crate::http_client::for_url(false), "http://127.0.0.1:1/").await;
        match outcome {
            ProbeOutcome::Failure(failure) => {
                assert_eq!(failure.class, ProbeFailureClass::Transport);
            }
            ProbeOutcome::Response(_) => panic!("closed port cannot respond"),
        }
    }
}
