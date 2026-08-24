//! Verdicts over a bounded sample of page-referenced assets.
//!
//! Runtime adapters supply bytes, status, content type, and cache facts. This is
//! a sampler, not a browser transfer trace.

mod collect;
mod results;

pub use collect::{collect_assets, AssetCollection, AssetKind, CollectedAsset};

use crate::checks::CheckResult;
use serde::{Deserialize, Serialize};

/// One sampled asset after the runtime's measurement pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasuredAsset {
    pub url: String,
    pub kind: AssetKind,
    /// 0 means the request never produced a response (DNS/connect/timeout).
    pub status_code: u16,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub content_type: Option<String>,
    /// Raw Cache-Control response header, if any - a reliable signal from
    /// the same fetch that measures the bytes, no extra request.
    #[serde(default)]
    pub cache_control: Option<String>,
    /// True when the byte count is a truncated lower bound, not a total.
    #[serde(default)]
    pub measured_floor: bool,
    #[serde(default)]
    pub has_srcset: bool,
    /// Responsive-image group id carried from the collected asset; the weight
    /// check counts one representative per group. See `CollectedAsset::group`.
    pub group: u32,
}

impl MeasuredAsset {
    /// An asset we never got a response for.
    pub fn unfetched(asset: &CollectedAsset) -> Self {
        Self {
            url: asset.url.to_string(),
            kind: asset.kind,
            status_code: 0,
            bytes: None,
            content_type: None,
            cache_control: None,
            measured_floor: false,
            has_srcset: asset.has_srcset,
            group: asset.group,
        }
    }
}

/// Parse the total size out of a Content-Range header value such as
/// `bytes 0-0/12345`. Returns None when the total is unknown (`*`). Shared
/// so every runtime's Range-probe adapter classifies the header identically.
pub fn parse_content_range_total(value: &str) -> Option<u64> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (_, total) = rest.rsplit_once('/')?;
    let total = total.trim();
    if total == "*" {
        return None;
    }
    total.parse().ok()
}

/// Grade one collection plus its measurements into the sampler's four rows.
pub fn evaluate_asset_sample(
    html_bytes: u64,
    collection: &AssetCollection,
    measured: &[MeasuredAsset],
) -> Vec<CheckResult> {
    let page_origin = collection.page_origin.as_deref();
    vec![
        results::asset_weight_result(html_bytes, collection, measured),
        results::broken_images_result(measured, page_origin),
        results::heavy_images_result(measured, page_origin),
        results::asset_caching_result(measured, page_origin),
    ]
}

/// Format bytes with decimal KB/MB units, matching Chrome DevTools.
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1000.0;
    const MB: f64 = 1_000_000.0;
    let value = bytes as f64;
    if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod content_range_tests {
    use super::parse_content_range_total;

    #[test]
    fn parses_standard_content_range_totals() {
        assert_eq!(parse_content_range_total("bytes 0-0/12345"), Some(12345));
        assert_eq!(parse_content_range_total(" bytes 0-0/1 "), Some(1));
        assert_eq!(
            parse_content_range_total("bytes 0-1023/4194304"),
            Some(4_194_304)
        );
    }

    #[test]
    fn parses_unsatisfied_range_with_known_total() {
        assert_eq!(parse_content_range_total("bytes */999"), Some(999));
    }

    #[test]
    fn rejects_unknown_totals_and_garbage() {
        assert_eq!(parse_content_range_total("bytes 0-0/*"), None);
        assert_eq!(parse_content_range_total("bytes 0-0"), None);
        assert_eq!(parse_content_range_total("items 0-0/50"), None);
        assert_eq!(parse_content_range_total(""), None);
        assert_eq!(parse_content_range_total("bytes 0-0/abc"), None);
    }
}
