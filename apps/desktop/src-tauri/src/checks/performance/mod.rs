//! Performance checks: caching, compression, images, page weight, asset sampling, redirects, protocol, timing.

pub mod assets;
pub use sitecmd_engine::checks::performance::cache;
pub mod compression;
pub use sitecmd_engine::checks::performance::dom_size;
pub use sitecmd_engine::checks::performance::images;
pub mod page_weight;
pub use sitecmd_engine::checks::performance::preconnect;
pub use sitecmd_engine::checks::performance::protocol;
pub mod redirects;
pub use sitecmd_engine::checks::performance::render_blocking;
pub use sitecmd_engine::checks::performance::third_party;
pub mod timing;
pub use sitecmd_engine::checks::performance::unminified;

use super::{AsyncCheck, Check};

pub fn sync_checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(cache::CacheHeadersCheck),
        Box::new(images::ImageOptimizationCheck),
        Box::new(images::FontLoadingCheck),
        Box::new(render_blocking::RenderBlockingCheck),
        Box::new(dom_size::DomSizeCheck),
        Box::new(third_party::ThirdPartyScriptsCheck),
        Box::new(preconnect::PreconnectCheck),
        Box::new(unminified::UnminifiedCodeCheck),
        Box::new(protocol::Http2Check),
        Box::new(protocol::InlineCssSizeCheck),
    ]
}

pub fn async_checks() -> Vec<Box<dyn AsyncCheck>> {
    vec![
        Box::new(compression::CompressionCheck),
        Box::new(timing::TimingCheck),
        Box::new(redirects::RedirectChainCheck),
        Box::new(page_weight::PageWeightCheck),
        Box::new(assets::AssetSamplerCheck),
    ]
}
