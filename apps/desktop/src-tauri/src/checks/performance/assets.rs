//! Asset-sampler transport with page-subresource policy and bounded fetching.

mod measure;

use std::future::Future;

use crate::checks::{AsyncCheck, CheckContext, CheckResult, ScanCategory};
use futures_util::{stream, StreamExt};
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

        let client = measurement_client(ctx.is_strict_localhost);
        let measured = collect_measurements(
            collection
                .sampled
                .clone()
                .into_iter()
                .map(move |asset| measure::fetch_asset(client.clone(), asset)),
        )
        .await;

        assets::evaluate_asset_sample(ctx.body.len() as u64, &collection, &measured)
    }
}

async fn collect_measurements<F>(fetches: impl IntoIterator<Item = F>) -> Vec<MeasuredAsset>
where
    F: Future<Output = MeasuredAsset>,
{
    // Owning each future ties running and queued requests to the check lifetime.
    let mut measured: Vec<_> = stream::iter(fetches.into_iter().enumerate())
        .map(|(index, fetch)| async move { (index, fetch.await) })
        .buffer_unordered(crate::constants::ASSET_FETCH_CONCURRENCY)
        .collect()
        .await;
    measured.sort_unstable_by_key(|(index, _)| *index);
    measured.into_iter().map(|(_, asset)| asset).collect()
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

    #[test]
    fn dropping_the_sampler_cancels_running_fetches_without_starting_queued_assets() {
        use std::cell::Cell;
        use std::task::{Context, Poll};

        struct RunningFetch<'a>(&'a Cell<usize>);

        impl Drop for RunningFetch<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() - 1);
            }
        }

        let started = Cell::new(0);
        let running = Cell::new(0);
        let fetches = (0..crate::constants::ASSET_SAMPLE_LIMIT).map(|_| async {
            started.set(started.get() + 1);
            running.set(running.get() + 1);
            let _running = RunningFetch(&running);
            std::future::pending::<MeasuredAsset>().await
        });
        let mut sampler = Box::pin(collect_measurements(fetches));
        let mut context = Context::from_waker(futures_util::task::noop_waker_ref());

        assert!(matches!(sampler.as_mut().poll(&mut context), Poll::Pending));
        assert_eq!(started.get(), crate::constants::ASSET_FETCH_CONCURRENCY);
        assert_eq!(running.get(), crate::constants::ASSET_FETCH_CONCURRENCY);

        drop(sampler);

        assert_eq!(running.get(), 0, "in-flight fetch futures must be dropped");
        assert_eq!(
            started.get(),
            crate::constants::ASSET_FETCH_CONCURRENCY,
            "queued assets must never start after the parent is dropped"
        );
    }

    #[test]
    fn sampler_refills_when_later_fetches_finish_and_preserves_sample_order() {
        use std::cell::Cell;
        use std::task::{Context, Poll};

        let measured_asset = |index| {
            MeasuredAsset::unfetched(&assets::CollectedAsset {
                url: url::Url::parse(&format!("https://example.com/{index}.png"))
                    .expect("asset URL"),
                kind: assets::AssetKind::Image,
                has_srcset: false,
                group: 0,
            })
        };
        let (mut senders, receivers): (Vec<_>, Vec<_>) = (0..crate::constants::ASSET_SAMPLE_LIMIT)
            .map(|_| {
                let (sender, receiver) = tokio::sync::oneshot::channel();
                (Some(sender), receiver)
            })
            .unzip();
        let started = Cell::new(0);
        let fetches = receivers.into_iter().map(|receiver| async {
            started.set(started.get() + 1);
            receiver.await.expect("measurement")
        });
        let mut sampler = Box::pin(collect_measurements(fetches));
        let mut context = Context::from_waker(futures_util::task::noop_waker_ref());
        assert!(matches!(sampler.as_mut().poll(&mut context), Poll::Pending));

        for (index, sender) in senders
            .iter_mut()
            .enumerate()
            .take(crate::constants::ASSET_FETCH_CONCURRENCY)
            .skip(1)
        {
            sender
                .take()
                .expect("sender")
                .send(measured_asset(index))
                .expect("finish later request");
        }
        assert!(matches!(sampler.as_mut().poll(&mut context), Poll::Pending));
        assert!(
            started.get() > crate::constants::ASSET_FETCH_CONCURRENCY,
            "a slow first asset must not stall the queue"
        );

        for (index, sender) in senders.into_iter().enumerate() {
            if let Some(sender) = sender {
                sender
                    .send(measured_asset(index))
                    .expect("finish remaining request");
            }
        }
        let Poll::Ready(measured) = sampler.as_mut().poll(&mut context) else {
            panic!("all asset requests completed");
        };
        let expected: Vec<_> = (0..crate::constants::ASSET_SAMPLE_LIMIT)
            .map(|index| measured_asset(index).url)
            .collect();
        assert_eq!(
            measured
                .into_iter()
                .map(|asset| asset.url)
                .collect::<Vec<_>>(),
            expected
        );
    }

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
