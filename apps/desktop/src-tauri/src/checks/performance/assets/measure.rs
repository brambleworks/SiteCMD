//! Desktop transfer-size measurement for sampled assets.
//!
//! Prefer `Content-Range`, then `Content-Length`, then a bounded body count.

use crate::http_client::{read_body_limited, BodyReadError};
use sitecmd_engine::checks::performance::assets::{
    parse_content_range_total, CollectedAsset, MeasuredAsset,
};

/// Fetch one asset and measure its transfer size. Never panics; failures come
/// back as `status_code: 0` / `bytes: None` so the caller can report honestly.
pub async fn fetch_asset(client: reqwest::Client, asset: CollectedAsset) -> MeasuredAsset {
    let mut measured = MeasuredAsset::unfetched(&asset);
    let response = match client
        .get(asset.url.clone())
        .header(reqwest::header::RANGE, "bytes=0-0")
        .timeout(crate::constants::CHECK_PROBE_TIMEOUT)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return measured,
    };

    measured.status_code = response.status().as_u16();
    measured.content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or(value)
                .trim()
                .to_ascii_lowercase()
        });
    measured.cache_control = response
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    if measured.status_code >= 400 {
        return measured;
    }

    // 206 Partial Content: Content-Range names the full size for one byte
    // of transfer.
    if measured.status_code == 206 {
        measured.bytes = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_content_range_total);
        return measured;
    }

    // The server ignored the Range request; Content-Length is the next best
    // signal.
    if let Some(length) = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
    {
        measured.bytes = Some(length);
        return measured;
    }

    // No size headers at all: count the bytes ourselves, bounded so a huge or
    // dishonest response cannot blow the probe budget.
    match read_body_limited(
        response,
        crate::constants::MAX_PROBE_BODY_SIZE,
        crate::constants::CHECK_PROBE_TIMEOUT,
    )
    .await
    {
        Ok(body) => measured.bytes = Some(body.len() as u64),
        Err(BodyReadError::TooLarge { received_bytes, .. }) => {
            measured.bytes = Some(received_bytes);
            measured.measured_floor = true;
        }
        Err(_) => {}
    }
    measured
}
