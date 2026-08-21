//! HTTP-compression transport using the shared no-decompress, SSRF-guarded client.

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::performance::compression;

pub struct CompressionCheck;

#[async_trait::async_trait]
impl AsyncCheck for CompressionCheck {
    fn id(&self) -> &str {
        "performance.compression"
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        if ctx.is_localhost {
            return vec![compression::localhost_skip_result()];
        }

        let client = crate::http_client::no_decompress_client(ctx.is_strict_localhost);

        let head = send_probe(client.head(ctx.url.as_str())).await;
        match compression::evaluate_compression_head(head.as_ref()) {
            compression::CompressionStep::Done(results) => results,
            compression::CompressionStep::NeedsGet => {
                let get = send_probe(client.get(ctx.url.as_str())).await;
                compression::evaluate_compression_get(get, &ctx.page)
            }
        }
    }
}

async fn send_probe(builder: reqwest::RequestBuilder) -> Option<compression::EncodingProbe> {
    let resp = builder
        .header("Accept-Encoding", "gzip, deflate, br, zstd")
        .timeout(crate::constants::CHECK_PROBE_TIMEOUT)
        .send()
        .await
        .ok()?;
    Some(compression::EncodingProbe {
        http_status: resp.status().as_u16(),
        encoding: resp
            .headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_lowercase()),
        vary: resp
            .headers()
            .get("vary")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_lowercase())
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    #[tokio::test]
    async fn localhost_previews_skip_without_probing() {
        let ctx = crate::checks::CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse("http://localhost:4321/").expect("static test url"),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: String::new(),
                is_localhost: true,
                is_strict_localhost: true,
                http_version: Some("HTTP/1.1".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(true).clone(),
            probe_cache: Default::default(),
        };
        let results = CompressionCheck.run(&ctx).await;
        assert_eq!(results[0].check_id, CompressionCheck.id());
        assert_eq!(results[0].status, CheckStatus::Skipped);
    }
}
