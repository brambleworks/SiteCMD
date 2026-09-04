//! Asset-sampler transport with page-subresource policy and bounded fetching.

mod measure;

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use sitecmd_engine::checks::performance::assets;
use sitecmd_engine::checks::performance::assets::MeasuredAsset;

pub struct AssetSamplerCheck;

#[async_trait::async_trait]
impl AsyncCheck for AssetSamplerCheck {
    fn id(&self) -> &str {
        "performance.asset_weight"
    }
    fn emitted_ids(&self) -> Vec<String> {
        vec![
            "performance.asset_weight".to_string(),
            "performance.asset_caching".to_string(),
            "performance.broken_images".to_string(),
            "performance.images.heavy".to_string(),
        ]
    }
    fn category(&self) -> ScanCategory {
        ScanCategory::Performance
    }

    async fn run(&self, ctx: &CheckContext) -> Vec<CheckResult> {
        let policy = ctx.subordinate_policy();
        let collection = assets::collect_assets(
            &ctx.body,
            ctx.body_lower(),
            &ctx.url,
            |target| {
                crate::network_policy::validate_page_subresource_target(target, policy).is_ok()
            },
            crate::constants::ASSET_SAMPLE_LIMIT,
        );

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(
            crate::constants::ASSET_FETCH_CONCURRENCY,
        ));
        let mut futures = Vec::new();
        for asset in &collection.sampled {
            let client = measurement_client(ctx.is_strict_localhost).clone();
            let asset = asset.clone();
            let sem = semaphore.clone();
            futures.push(tokio::spawn(async move {
                let _permit = match sem.acquire().await {
                    Ok(permit) => permit,
                    // Semaphore is never closed; treat as unfetched if it ever is.
                    Err(_) => return MeasuredAsset::unfetched(&asset),
                };
                measure::fetch_asset(client, asset).await
            }));
        }

        let mut measured = Vec::new();
        for future in futures {
            if let Ok(asset) = future.await {
                measured.push(asset);
            }
        }

        assets::evaluate_asset_sample(ctx.body.len() as u64, &collection, &measured)
    }
}

/// The sampler reports transfer size, so it fetches through the client that
/// does not auto-decompress. The shared client negotiates gzip and brotli;
/// decoding drops `Content-Length` and leaves the decoded byte count, which
/// would be reported as the asset's weight on the wire.
fn measurement_client(is_strict_local: bool) -> &'static reqwest::Client {
    crate::http_client::no_decompress_client(is_strict_local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::CheckStatus;

    /// The one adapter path with no network: a page that references no
    /// fetchable assets. Pins the wiring of AssetSamplerCheck end to end -
    /// the engine's four rows come back under their registered ids.
    #[tokio::test]
    async fn a_page_without_assets_emits_the_four_clean_rows() {
        let ctx = crate::checks::CheckContext {
            page: crate::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url::Url::parse("https://example.com/").expect("static test url"),
                response_headers: reqwest::header::HeaderMap::new(),
                status_code: 200,
                body: "<html><body><p>No assets here.</p></body></html>".into(),
                is_localhost: false,
                is_strict_localhost: false,
                http_version: Some("HTTP/2.0".to_string()),
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: crate::http_client::for_url(false).clone(),
            probe_cache: Default::default(),
        };
        let results = AssetSamplerCheck.run(&ctx).await;
        let ids: Vec<&str> = results.iter().map(|row| row.check_id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "performance.asset_weight",
                "performance.broken_images",
                "performance.images.heavy",
                "performance.asset_caching",
            ]
        );
        assert!(results.iter().all(|row| row.status == CheckStatus::Pass));
    }

    #[test]
    fn the_policy_gate_refuses_internal_targets_from_untrusted_markup() {
        let html = r#"
            <img src="http://169.254.169.254/latest/meta-data/x.png" alt="a">
            <img src="http://10.0.0.5/internal.png" alt="b">
            <script src="http://127.0.0.1:11434/api.js"></script>
            <link rel="stylesheet" href="http://localhost:5173/app.css">
            <img src="https://cdn.example.com/ok.png" alt="c">
        "#;
        let lower = html.to_ascii_lowercase();
        let page_url = url::Url::parse("https://example.com/").expect("page url");
        let collection = assets::collect_assets(
            html,
            &lower,
            &page_url,
            |target| {
                crate::network_policy::validate_page_subresource_target(
                    target,
                    crate::network_policy::UrlPolicy::Redirect {
                        allow_local_dev: false,
                    },
                )
                .is_ok()
            },
            30,
        );
        assert_eq!(collection.sampled.len(), 1);
        assert_eq!(
            collection.sampled[0].url.as_str(),
            "https://cdn.example.com/ok.png"
        );
        assert_eq!(collection.skipped_unsupported, 4);
    }

    #[test]
    fn loopback_assets_are_allowed_only_for_strict_localhost_scans() {
        let html = r#"<img src="http://127.0.0.1:5173/hero.png" alt="a">"#;
        let lower = html.to_ascii_lowercase();
        let collect = |page: &str, allow_local_dev: bool| {
            let policy = crate::network_policy::UrlPolicy::Redirect { allow_local_dev };
            let page_url = url::Url::parse(page).expect("page url");
            assets::collect_assets(
                html,
                &lower,
                &page_url,
                |target| {
                    crate::network_policy::validate_page_subresource_target(target, policy).is_ok()
                },
                30,
            )
        };
        let local = collect("http://127.0.0.1:5173/", true);
        assert_eq!(
            local.sampled.len(),
            1,
            "local dev scans fetch their own assets"
        );
        let public = collect("https://example.com/", false);
        assert!(public.sampled.is_empty());
        assert_eq!(public.skipped_unsupported, 1);
    }

    #[test]
    fn the_sampler_never_measures_through_a_decompressing_client() {
        for is_strict_local in [false, true] {
            let sampler = measurement_client(is_strict_local) as *const reqwest::Client;
            assert_eq!(
                sampler,
                crate::http_client::no_decompress_client(is_strict_local) as *const reqwest::Client,
                "transfer size must be measured on the raw wire response"
            );
            assert_ne!(
                sampler,
                crate::http_client::for_url(is_strict_local) as *const reqwest::Client,
                "the shared client negotiates gzip and brotli; measuring through it would report decoded bytes as transfer weight"
            );
        }
    }
}
