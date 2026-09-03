//! Builds verdicts from collected asset samples.

use super::{format_bytes, AssetCollection, AssetKind, MeasuredAsset};
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

fn evidence_url(raw_url: &str, page_origin: Option<&str>) -> String {
    crate::log_sanitizer::evidence_safe_page_url_for_site(raw_url, page_origin)
}

/// A sampled image larger than this is called out individually.
/// Decimal bytes so the "300 KB" copy matches the math exactly.
/// Sampler-local tuning value; timeouts and sample caps stay with the
/// runtime's transport.
const HEAVY_IMAGE_BYTES: u64 = 300_000;

/// Measured page weight above this warns (2.5 MB, decimal).
const ASSET_WEIGHT_WARN_BYTES: u64 = 2_500_000;

/// Measured page weight above this fails (5 MB, decimal).
const ASSET_WEIGHT_FAIL_BYTES: u64 = 5_000_000;

/// How many offending URLs to list inline in a description.
const MAX_LISTED_URLS: usize = 5;

/// Minimum useful cache lifetime for immutable assets.
const IMMUTABLE_CACHE_MIN_SECS: u64 = 604_800;

/// Filename heuristic for content-hashed immutable assets.
fn looks_fingerprinted(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let filename = path.rsplit('/').next().unwrap_or(path);
    filename.split(['.', '-', '_']).any(is_hash_token)
}

/// Match build hashes while excluding common version words and names.
fn is_hash_token(token: &str) -> bool {
    if token.len() < 8 || !token.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let digits = token.chars().filter(|c| c.is_ascii_digit()).count();
    let letters = token.chars().filter(|c| c.is_ascii_alphabetic()).count();
    digits >= 2 && letters >= 2
}

/// Parse the largest `max-age`/`s-maxage` value out of a Cache-Control header.
fn cache_max_age(cache_control: &str) -> Option<u64> {
    let lower = cache_control.to_ascii_lowercase();
    let mut best: Option<u64> = None;
    for directive in lower.split(',') {
        if let Some((name, value)) = directive.split_once('=') {
            let name = name.trim();
            if name == "max-age" || name == "s-maxage" {
                if let Ok(secs) = value.trim().trim_matches('"').parse::<u64>() {
                    best = Some(best.map_or(secs, |current| current.max(secs)));
                }
            }
        }
    }
    best
}

/// Whether a fingerprinted asset's cache header is durable enough. `no-store`
/// and `no-cache` defeat caching regardless of max-age, so they never qualify.
fn cache_is_durable(cache_control: Option<&str>) -> bool {
    let Some(cache_control) = cache_control else {
        return false;
    };
    let lower = cache_control.to_ascii_lowercase();
    if lower.contains("no-store") || lower.contains("no-cache") {
        return false;
    }
    cache_max_age(&lower).is_some_and(|secs| secs >= IMMUTABLE_CACHE_MIN_SECS)
}

/// Same registrable domain (PSL eTLD+1) as the scanned page, so a site's own
/// CDN subdomain is graded while another organization's origin is not. Hosts
/// without a public suffix (localhost, IP literals) must match exactly.
fn is_same_site(asset_url: &str, page_origin: Option<&str>) -> bool {
    fn site(url: &str) -> Option<String> {
        let parsed = url::Url::parse(url).ok()?;
        let host = parsed
            .host_str()?
            .trim_end_matches('.')
            .to_ascii_lowercase();
        Some(psl::domain_str(&host).map(str::to_string).unwrap_or(host))
    }
    match (page_origin.and_then(site), site(asset_url)) {
        (Some(page), Some(asset)) => page == asset,
        _ => false,
    }
}

/// No page origin means no asset can be placed on the site or off it, so the
/// check reports what it could not establish instead of grading nothing.
fn unattributable_asset_caching_result(fingerprinted_count: usize) -> CheckResult {
    CheckResult {
        check_id: "performance.asset_caching".into(),
        category: ScanCategory::Performance,
        title: "Static asset caching not assessed".into(),
        description: format!(
            "{} sampled asset{} a fingerprint-like filename, but this run carried no page origin, so none of them could be attributed to the site or to a third party. Only the site's own assets are graded for cache freshness, so no verdict was reached.",
            fingerprinted_count,
            if fingerprinted_count == 1 {
                " has"
            } else {
                "s have"
            }
        ),
        status: CheckStatus::Skipped,
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: None,
        raw_data: Some(serde_json::json!({
            "reason": "no_page_origin",
            "fingerprinted_sampled": fingerprinted_count,
            "min_durable_secs": IMMUTABLE_CACHE_MIN_SECS,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: None,
    }
}

pub(super) fn asset_caching_result(
    measured: &[MeasuredAsset],
    page_origin: Option<&str>,
) -> CheckResult {
    // Only successfully fetched assets whose filename has a hash-like token
    // are in scope. That token is a build-pattern heuristic, not proof that
    // the URL changes whenever the bytes change.
    let fingerprinted: Vec<&MeasuredAsset> = measured
        .iter()
        .filter(|asset| (200..300).contains(&asset.status_code) && looks_fingerprinted(&asset.url))
        .collect();
    // Without a page origin no asset can be attributed to the site or to a
    // third party, so same-site membership is unknown rather than empty and
    // there is no verdict to report. With nothing fingerprinted the answer is
    // the same either way, so that case falls through to the normal wording.
    if page_origin.is_none() && !fingerprinted.is_empty() {
        return unattributable_asset_caching_result(fingerprinted.len());
    }
    // Only the site's own assets are graded: another origin sets its own
    // cache headers, and the site owner cannot change them.
    let (same_site, third_party): (Vec<&MeasuredAsset>, Vec<&MeasuredAsset>) = fingerprinted
        .iter()
        .copied()
        .partition(|asset| is_same_site(&asset.url, page_origin));
    let weak: Vec<&MeasuredAsset> = same_site
        .iter()
        .copied()
        .filter(|asset| !cache_is_durable(asset.cache_control.as_deref()))
        .collect();
    let third_party_note = if third_party.is_empty() {
        String::new()
    } else {
        format!(
            " {} sampled third-party asset{} with a fingerprint-like filename {} not graded because another origin sets {} cache headers.",
            third_party.len(),
            if third_party.len() == 1 { "" } else { "s" },
            if third_party.len() == 1 { "was" } else { "were" },
            if third_party.len() == 1 { "its" } else { "their" },
        )
    };

    let status = if weak.is_empty() {
        CheckStatus::Pass
    } else {
        CheckStatus::Warn
    };
    // A caching miss is a pure optimization, never a correctness or launch
    // blocker, so it stays Low regardless of how many assets are affected.
    let severity = Severity::Low;

    let listed: Vec<String> = weak
        .iter()
        .take(MAX_LISTED_URLS)
        .map(|asset| {
            format!(
                "{} ({})",
                evidence_url(&asset.url, page_origin),
                asset
                    .cache_control
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("no Cache-Control")
            )
        })
        .collect();

    let description = if fingerprinted.is_empty() {
        "No sampled static-asset filename matched this check's fingerprint-like token pattern."
            .to_string()
    } else if same_site.is_empty() {
        format!(
            "No sampled same-site asset filename matched this check's fingerprint-like token pattern.{}",
            third_party_note
        )
    } else if weak.is_empty() {
        format!(
            "All {} sampled same-site assets with fingerprint-like filenames are served with at least one week of browser or shared-cache freshness. Filename shape does not prove the URLs are content-addressed.{}",
            same_site.len(),
            third_party_note
        )
    } else {
        format!(
            "{} of {} sampled same-site assets with fingerprint-like filenames {} served without the check's one-week freshness threshold: {}. The names look content-hashed, but source markup alone does not prove that each URL changes whenever its bytes change.{}",
            weak.len(),
            same_site.len(),
            if weak.len() == 1 { "is" } else { "are" },
            listed.join("; "),
            third_party_note
        )
    };

    CheckResult {
        check_id: "performance.asset_caching".into(),
        category: ScanCategory::Performance,
        title: if weak.is_empty() {
            "Static asset caching".into()
        } else {
            "Fingerprint-like asset filenames with short caching".into()
        },
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix: if weak.is_empty() {
            None
        } else {
            Some(
                "First verify that the build produces a new URL whenever an asset's bytes change and that the response is public and identical across users. Only then apply a long-lived policy such as `Cache-Control: public, max-age=31536000, immutable` at the authoritative CDN/static-file layer. Keep revalidation or shorter freshness for mutable, private, personalized, or operationally unversioned URLs."
                    .into(),
            )
        },
        raw_data: Some(serde_json::json!({
            "fingerprinted_sampled": fingerprinted.len(),
            "third_party_sampled": third_party.len(),
            "weak_count": weak.len(),
            "min_durable_secs": IMMUTABLE_CACHE_MIN_SECS,
            "weak": weak
                .iter()
                .map(|asset| serde_json::json!({
                    "url": evidence_url(&asset.url, page_origin),
                    "cache_control": asset.cache_control,
                }))
                .collect::<Vec<_>>(),
        })),
        confidence: if weak.is_empty() {
            IssueConfidence::High
        } else {
            IssueConfidence::NeedsReview
        },
        confidence_reason: (!weak.is_empty()).then(|| "The cache headers and filename tokens are directly observed, but the scanner cannot prove that the token is a content hash or that the URL is immutable, public, and identical across users.".into()),
        why_it_matters: if weak.is_empty() {
            None
        } else {
            Some("If these URLs are truly content-addressed public assets, short freshness causes avoidable revalidation or repeat transfer. If they are mutable or user-specific, long immutable caching would be incorrect.".into())
        },
    }
}

/// This row sums decoded sizes over a bounded sample and says so in its own
/// description ("not observed navigation transfer"), so it never carries a
/// severity that claims a measured user-facing cost: Warn is advisory and
/// Fail tops out at Medium. `performance.page_weight` keeps the escalating
/// tier because the HTML document size it grades is directly observed.
fn asset_weight_verdict(total_bytes: u64) -> (CheckStatus, Severity) {
    if total_bytes > ASSET_WEIGHT_FAIL_BYTES {
        (CheckStatus::Fail, Severity::Medium)
    } else if total_bytes > ASSET_WEIGHT_WARN_BYTES {
        (CheckStatus::Warn, Severity::Low)
    } else {
        (CheckStatus::Pass, Severity::Low)
    }
}

fn broken_images_verdict(broken_count: usize) -> (CheckStatus, Severity) {
    match broken_count {
        0 => (CheckStatus::Pass, Severity::Low),
        1..=2 => (CheckStatus::Warn, Severity::Medium),
        _ => (CheckStatus::Fail, Severity::Medium),
    }
}
pub(super) fn asset_weight_result(
    html_bytes: u64,
    collection: &AssetCollection,
    measured: &[MeasuredAsset],
) -> CheckResult {
    let page_origin = collection.page_origin.as_deref();
    // Count the largest measured candidate per responsive-image group because a
    // browser downloads one candidate, not every srcset option.
    let mut measured_count = 0usize;
    let mut unmeasured = 0usize;
    // group id -> (kind, largest measured bytes, whether that read was floored)
    let mut groups: std::collections::BTreeMap<u32, (&'static str, u64, bool)> =
        std::collections::BTreeMap::new();
    for asset in measured {
        match asset.bytes {
            Some(bytes) => {
                measured_count += 1;
                let entry = groups
                    .entry(asset.group)
                    .or_insert((asset.kind.as_str(), 0, false));
                if bytes >= entry.1 {
                    entry.1 = bytes;
                    entry.0 = asset.kind.as_str();
                    entry.2 = asset.measured_floor;
                }
            }
            None => unmeasured += 1,
        }
    }
    let mut kind_bytes: std::collections::BTreeMap<&'static str, u64> =
        std::collections::BTreeMap::new();
    let mut truncated_reads = 0usize;
    for (kind, bytes, floored) in groups.values() {
        *kind_bytes.entry(kind).or_insert(0u64) += bytes;
        if *floored {
            truncated_reads += 1;
        }
    }
    let asset_bytes: u64 = kind_bytes.values().sum();
    let total_bytes = html_bytes + asset_bytes + collection.data_uri_bytes;
    // Missing/capped reads make coverage incomplete. The sum is never called
    // a transfer floor: decoded text and the largest measured srcset candidate
    // can be larger than bytes a particular navigation actually transfers.
    let measurement_incomplete = truncated_reads > 0
        || unmeasured > 0
        || collection.skipped_unsupported > 0
        || collection.fetchable_found > collection.sampled.len();
    let (status, severity) = asset_weight_verdict(total_bytes);

    let coverage_note = if measurement_incomplete {
        " Measurement coverage is incomplete because one or more assets were capped, skipped, unavailable, or truncated."
    } else {
        ""
    };
    let summary = format!(
        "{} asset response{} measured (sampled from {} reference{}): {} sampled byte sum including the {} HTML document. This is not observed navigation transfer: text sizes are decoded/uncompressed, responsive groups use the largest measured candidate, and caching, lazy loading, media conditions, or browser selection can change what transfers.{}",
        measured_count,
        if measured_count == 1 { "" } else { "s" },
        collection.found,
        if collection.found == 1 { "" } else { "s" },
        format_bytes(total_bytes),
        format_bytes(html_bytes),
        coverage_note,
    );
    let description = match status {
        CheckStatus::Fail => format!(
            "{} The sampled sum exceeds this check's 5 MB review threshold; use a browser trace to measure the actual critical and total transfer before choosing changes.",
            summary
        ),
        CheckStatus::Warn => format!(
            "{} The sampled sum exceeds this check's 2.5 MB review threshold; use a browser trace to measure the actual critical and total transfer before choosing changes.",
            summary
        ),
        _ => summary,
    };

    let assets_json: Vec<serde_json::Value> = measured
        .iter()
        .map(|asset| {
            serde_json::json!({
                "url": evidence_url(&asset.url, page_origin),
                "kind": asset.kind.as_str(),
                "status_code": asset.status_code,
                "bytes": asset.bytes,
                "content_type": asset.content_type,
                "measured_floor": asset.measured_floor,
            })
        })
        .collect();

    CheckResult {
        check_id: "performance.asset_weight".into(),
        category: ScanCategory::Performance,
        title: match status {
            CheckStatus::Fail => "Sampled asset byte sum over 5 MB".into(),
            CheckStatus::Warn => "Sampled asset byte sum over 2.5 MB".into(),
            _ => "Sampled asset byte sum".into(),
        },
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix: if status == CheckStatus::Pass {
            None
        } else {
            Some(
                "Record a production browser trace on representative devices and cache states, then start with resources that actually dominate critical-path or total transfer. Resize/recompress images to their rendered use, reduce code that is truly shipped and executed unnecessarily, and remove unused assets only after confirming route, media, and lazy-loading behavior."
                    .into(),
            )
        },
        raw_data: Some(serde_json::json!({
            "html_bytes": html_bytes,
            "data_uri_bytes": collection.data_uri_bytes,
            "data_uri_count": collection.data_uri_count,
            "measured_asset_bytes": asset_bytes,
            "total_bytes": total_bytes,
            "by_kind": kind_bytes,
            "found": collection.found,
            "fetchable_found": collection.fetchable_found,
            "sampled": collection.sampled.len(),
            "measured": measured_count,
            "unmeasured": unmeasured,
            "truncated_reads": truncated_reads,
            "skipped_unsupported": collection.skipped_unsupported,
            "measurement_incomplete": measurement_incomplete,
            "measurement_kind": "sampled_decoded_byte_sum_not_navigation_transfer",
            "assets": assets_json,
        })),
        confidence: if status == CheckStatus::Pass && !measurement_incomplete {
            IssueConfidence::High
        } else {
            IssueConfidence::NeedsReview
        },
        confidence_reason: (status != CheckStatus::Pass || measurement_incomplete).then(|| {
            if measurement_incomplete {
                "The sampled response sizes are direct evidence, but coverage is incomplete and this is not a navigation transfer measurement; capped/skipped assets, compression, responsive selection, and cache/lazy/media behavior can change the result.".into()
            } else {
                "The sampled response sizes are direct evidence, but they are not a navigation transfer measurement: text may be compressed, responsive candidates are approximated, and cache/lazy/media behavior was not reproduced.".into()
            }
        }),
        why_it_matters: if status == CheckStatus::Pass {
            None
        } else {
            Some("Large resources can increase transfer time, parse/decode work, and metered-data use when a navigation actually downloads them. The sampled sum does not establish which bytes are critical or transferred for every visitor.".into())
        },
    }
}

pub(super) fn broken_images_result(
    measured: &[MeasuredAsset],
    page_origin: Option<&str>,
) -> CheckResult {
    let images: Vec<&MeasuredAsset> = measured
        .iter()
        .filter(|asset| asset.kind == AssetKind::Image)
        .collect();
    let image_count = images.len();
    let http_errors: Vec<&MeasuredAsset> = images
        .iter()
        .copied()
        .filter(|asset| (400..=599).contains(&asset.status_code))
        .collect();
    let non_image_types: Vec<&MeasuredAsset> = images
        .iter()
        .copied()
        .filter(|asset| {
            (200..300).contains(&asset.status_code)
                && asset
                    .content_type
                    .as_deref()
                    .is_some_and(|content_type| !content_type.starts_with("image/"))
        })
        .collect();
    let inconclusive: Vec<&MeasuredAsset> = images
        .iter()
        .copied()
        .filter(|asset| {
            asset.status_code == 0
                || !(200..300).contains(&asset.status_code)
                    && !(400..=599).contains(&asset.status_code)
        })
        .collect();
    let responded_2xx = images
        .iter()
        .filter(|asset| (200..300).contains(&asset.status_code))
        .count();

    let (status, severity) = if !http_errors.is_empty() {
        broken_images_verdict(http_errors.len())
    } else if !non_image_types.is_empty() {
        (CheckStatus::Warn, Severity::Medium)
    } else if image_count > 0 && inconclusive.len() == image_count {
        (CheckStatus::Skipped, Severity::Low)
    } else {
        (CheckStatus::Pass, Severity::Low)
    };

    let listed_errors: Vec<String> = http_errors
        .iter()
        .take(MAX_LISTED_URLS)
        .map(|asset| {
            format!(
                "{} ({})",
                evidence_url(&asset.url, page_origin),
                asset.status_code
            )
        })
        .collect();
    let listed_types: Vec<String> = non_image_types
        .iter()
        .take(MAX_LISTED_URLS)
        .map(|asset| {
            format!(
                "{} (HTTP {}; {})",
                evidence_url(&asset.url, page_origin),
                asset.status_code,
                asset
                    .content_type
                    .as_deref()
                    .unwrap_or("unknown Content-Type")
            )
        })
        .collect();
    let coverage_note = if inconclusive.is_empty() {
        String::new()
    } else {
        format!(
            " {} additional sampled candidate{} had an inconclusive network or redirect outcome.",
            inconclusive.len(),
            if inconclusive.len() == 1 { "" } else { "s" }
        )
    };
    let description = if image_count == 0 {
        "No image candidate was sampled from the fetched markup.".to_string()
    } else if !http_errors.is_empty() {
        format!(
            "{} of {} sampled image candidate{} returned an HTTP 4xx/5xx status: {}. A sampled candidate may be a fallback or srcset option that the current browser does not select.{}",
            http_errors.len(),
            image_count,
            if image_count == 1 { "" } else { "s" },
            listed_errors.join(", "),
            coverage_note,
        )
    } else if !non_image_types.is_empty() {
        format!(
            "{} of {} sampled image candidate{} returned a 2xx response with a non-image Content-Type: {}. This response mismatch needs review but does not prove browser rendering failed because selection and sniffing behavior were not reproduced.{}",
            non_image_types.len(),
            image_count,
            if image_count == 1 { "" } else { "s" },
            listed_types.join(", "),
            coverage_note,
        )
    } else if status == CheckStatus::Skipped {
        format!(
            "All {} sampled image candidate{} had an inconclusive network or redirect outcome, so availability was not graded.",
            image_count,
            if image_count == 1 { "" } else { "s" },
        )
    } else {
        format!(
            "{} of {} sampled image candidate{} returned a 2xx response. This HTTP availability probe does not prove which responsive candidate the browser selected, that the response decoded as an image, or that every page image was sampled.{}",
            responded_2xx,
            image_count,
            if image_count == 1 { "" } else { "s" },
            coverage_note,
        )
    };

    let error_json: Vec<serde_json::Value> = http_errors
        .iter()
        .map(|asset| {
            serde_json::json!({
                "url": evidence_url(&asset.url, page_origin),
                "status_code": asset.status_code,
                "srcset_candidate": asset.has_srcset,
            })
        })
        .collect();
    let non_image_json: Vec<serde_json::Value> = non_image_types
        .iter()
        .map(|asset| {
            serde_json::json!({
                "url": evidence_url(&asset.url, page_origin),
                "status_code": asset.status_code,
                "content_type": asset.content_type,
                "srcset_candidate": asset.has_srcset,
            })
        })
        .collect();
    let selection_or_coverage_uncertain = !non_image_types.is_empty()
        || !inconclusive.is_empty()
        || http_errors.iter().any(|asset| asset.has_srcset);

    CheckResult {
        check_id: "performance.broken_images".into(),
        category: ScanCategory::Performance,
        title: match status {
            CheckStatus::Warn | CheckStatus::Fail if !http_errors.is_empty() => {
                "Sampled image candidates returned HTTP errors".into()
            }
            CheckStatus::Warn => "Sampled image candidate returned non-image content".into(),
            CheckStatus::Skipped => "Sampled image availability was inconclusive".into(),
            _ => "Sampled image HTTP availability".into(),
        },
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix: if http_errors.is_empty() && non_image_types.is_empty() {
            None
        } else {
            Some(
                "Confirm whether each sampled candidate is selected at a representative viewport and request it normally without credentials. For a selected 4xx/5xx response, restore the intended file, correct the case-sensitive path or routing, or remove the stale reference. For a 2xx non-image response, fix catch-all routing and Content-Type only after confirming the intended asset. Re-test the rendered page, not only the candidate URL."
                    .into(),
            )
        },
        raw_data: Some(serde_json::json!({
            "image_assets_sampled": image_count,
            "responded_2xx": responded_2xx,
            "http_error_count": http_errors.len(),
            "http_errors": error_json,
            "non_image_content_type_count": non_image_types.len(),
            "non_image_content_types": non_image_json,
            "inconclusive_count": inconclusive.len(),
        })),
        confidence: if status == CheckStatus::Skipped || selection_or_coverage_uncertain {
            IssueConfidence::NeedsReview
        } else {
            IssueConfidence::High
        },
        confidence_reason: (status == CheckStatus::Skipped || selection_or_coverage_uncertain).then(|| "The sampled response outcomes are direct evidence, but one or more probes were inconclusive, returned non-image content, or represented a srcset candidate that may not be selected by the browser.".into()),
        why_it_matters: match (http_errors.is_empty(), non_image_types.is_empty()) {
            (false, _) => Some("If the browser selects a candidate that returns an HTTP error, the intended image may not render; responsive selection and fallbacks determine the actual visitor impact.".into()),
            (true, false) => Some("If the browser selects a candidate that returns non-image content, decoding or rendering can fail; this probe did not reproduce the browser's selected candidate or sniffing behavior.".into()),
            _ => None,
        },
    }
}

pub(super) fn heavy_images_result(
    measured: &[MeasuredAsset],
    page_origin: Option<&str>,
) -> CheckResult {
    let images_sampled = measured
        .iter()
        .filter(|asset| asset.kind == AssetKind::Image)
        .count();
    let is_jpeg_or_png = |asset: &MeasuredAsset| {
        matches!(
            asset.content_type.as_deref(),
            Some("image/jpeg" | "image/png")
        )
    };
    let heavy: Vec<&MeasuredAsset> = measured
        .iter()
        .filter(|asset| {
            asset.kind == AssetKind::Image
                && asset.bytes.is_some_and(|bytes| bytes > HEAVY_IMAGE_BYTES)
        })
        .collect();

    let status = if heavy.is_empty() {
        CheckStatus::Pass
    } else {
        CheckStatus::Warn
    };
    let severity = if heavy.is_empty() {
        Severity::Low
    } else {
        Severity::Medium
    };
    let all_have_srcset = !heavy.is_empty() && heavy.iter().all(|asset| asset.has_srcset);
    let jpeg_or_png_count = heavy.iter().filter(|asset| is_jpeg_or_png(asset)).count();

    let listed: Vec<String> = heavy
        .iter()
        .take(MAX_LISTED_URLS)
        .map(|asset| {
            let size = asset.bytes.unwrap_or(0);
            format!(
                "{} ({}{}{}{})",
                evidence_url(&asset.url, page_origin),
                if asset.measured_floor {
                    "at least "
                } else {
                    ""
                },
                format_bytes(size),
                if is_jpeg_or_png(asset) {
                    ", JPEG or PNG"
                } else {
                    ""
                },
                if asset.has_srcset {
                    ", srcset present"
                } else {
                    ", no srcset"
                },
            )
        })
        .collect();

    let description = if heavy.is_empty() {
        if images_sampled == 0 {
            "No image assets were sampled from this page.".to_string()
        } else {
            format!(
                "None of the {} sampled images measure over 300 KB.",
                images_sampled
            )
        }
    } else {
        format!(
            "{} of {} sampled images {} over 300 KB: {}.{}",
            heavy.len(),
            images_sampled,
            if heavy.len() == 1 {
                "measures"
            } else {
                "measure"
            },
            listed.join("; "),
            if jpeg_or_png_count > 0 {
                format!(
                    " {} of them arrive as JPEG or PNG. Compare supported alternatives at equivalent visual quality before deciding whether re-encoding is beneficial.",
                    jpeg_or_png_count
                )
            } else {
                String::new()
            }
        )
    };

    let heavy_json: Vec<serde_json::Value> = heavy
        .iter()
        .map(|asset| {
            serde_json::json!({
                "url": evidence_url(&asset.url, page_origin),
                "bytes": asset.bytes,
                "measured_floor": asset.measured_floor,
                "has_srcset": asset.has_srcset,
                "content_type": asset.content_type,
                "jpeg_or_png": is_jpeg_or_png(asset),
            })
        })
        .collect();

    CheckResult {
        check_id: "performance.images.heavy".into(),
        category: ScanCategory::Performance,
        title: if heavy.is_empty() {
            "Sampled image transfer size".into()
        } else {
            "Sampled image responses over 300 KB".into()
        },
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix: if heavy.is_empty() {
            None
        } else {
            Some(
                "Measure which candidate each representative viewport actually selects, then resize and recompress oversized sources to their rendered use. Choose supported formats (such as AVIF or WebP where appropriate) with correct fallbacks; for srcset, verify candidate widths and the sizes attribute match the layout."
                    .into(),
            )
        },
        raw_data: Some(serde_json::json!({
            "threshold_bytes": HEAVY_IMAGE_BYTES,
            "images_sampled": images_sampled,
            "heavy": heavy_json,
        })),
        confidence: if all_have_srcset {
            IssueConfidence::NeedsReview
        } else {
            IssueConfidence::High
        },
        confidence_reason: if all_have_srcset {
            Some(
                "Every flagged image declares srcset candidates, so the sampler may have measured the largest candidate rather than the file most browsers actually download."
                    .into(),
            )
        } else {
            None
        },
        why_it_matters: if heavy.is_empty() {
            None
        } else {
            Some("An oversized image response can dominate transferred bytes or image decode work when the browser selects it, especially for above-the-fold content. Responsive selection and caching determine the actual user impact.".into())
        },
    }
}

#[cfg(test)]
#[path = "results_tests.rs"]
mod tests;
