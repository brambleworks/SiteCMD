use super::*;
use crate::checks::performance::assets::CollectedAsset;

// Build each helper asset in its own stable URL-derived group.
fn group_for(url: &str) -> u32 {
    url.bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

fn measured(
    url: &str,
    kind: AssetKind,
    status_code: u16,
    bytes: Option<u64>,
    has_srcset: bool,
) -> MeasuredAsset {
    MeasuredAsset {
        url: url.to_string(),
        kind,
        status_code,
        bytes,
        content_type: None,
        cache_control: None,
        measured_floor: false,
        has_srcset,
        group: group_for(url),
    }
}

fn collection(found: usize, fetchable_found: usize, sampled: usize) -> AssetCollection {
    AssetCollection {
        sampled: (0..sampled)
            .map(|index| CollectedAsset {
                url: url::Url::parse(&format!("https://example.com/a{}.png", index)).expect("url"),
                kind: AssetKind::Image,
                has_srcset: false,
                group: index as u32,
            })
            .collect(),
        fetchable_found,
        found,
        data_uri_count: 0,
        data_uri_bytes: 0,
        skipped_unsupported: 0,
        page_origin: Some("https://example.com".to_string()),
    }
}

#[test]
fn asset_weight_verdict_thresholds() {
    assert_eq!(
        asset_weight_verdict(1024),
        (CheckStatus::Pass, Severity::Low)
    );
    assert_eq!(
        asset_weight_verdict(ASSET_WEIGHT_WARN_BYTES),
        (CheckStatus::Pass, Severity::Low)
    );
    assert_eq!(
        asset_weight_verdict(ASSET_WEIGHT_WARN_BYTES + 1),
        (CheckStatus::Warn, Severity::Low)
    );
    assert_eq!(
        asset_weight_verdict(ASSET_WEIGHT_FAIL_BYTES),
        (CheckStatus::Warn, Severity::Low)
    );
    // A sampled, decoded byte sum with needs_review confidence must not be
    // presented at the severity reserved for measured user-facing cost.
    assert_eq!(
        asset_weight_verdict(ASSET_WEIGHT_FAIL_BYTES + 1),
        (CheckStatus::Fail, Severity::Medium)
    );
}

#[test]
fn a_sampled_byte_sum_never_grades_above_medium() {
    for bytes in [0, ASSET_WEIGHT_WARN_BYTES + 1, ASSET_WEIGHT_FAIL_BYTES + 1] {
        let (_, severity) = asset_weight_verdict(bytes);
        assert!(
            matches!(severity, Severity::Medium | Severity::Low),
            "{bytes} graded {severity:?}, but the row's own text says it is not observed navigation transfer"
        );
    }
}

#[test]
fn broken_images_verdict_scales_with_count() {
    assert_eq!(broken_images_verdict(0), (CheckStatus::Pass, Severity::Low));
    assert_eq!(
        broken_images_verdict(1),
        (CheckStatus::Warn, Severity::Medium)
    );
    assert_eq!(
        broken_images_verdict(2),
        (CheckStatus::Warn, Severity::Medium)
    );
    assert_eq!(
        broken_images_verdict(3),
        (CheckStatus::Fail, Severity::Medium)
    );
}

#[test]
fn format_bytes_uses_decimal_units_matching_the_labels() {
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(300_000), "300 KB");
    assert_eq!(format_bytes(2_500_000), "2.5 MB");
    assert_eq!(format_bytes(ASSET_WEIGHT_FAIL_BYTES), "5.0 MB");
}

#[test]
fn asset_weight_reports_exact_total_when_nothing_skipped() {
    let coll = collection(2, 2, 2);
    let assets = vec![
        measured(
            "https://example.com/a0.png",
            AssetKind::Image,
            200,
            Some(1024),
            false,
        ),
        measured(
            "https://example.com/a1.png",
            AssetKind::Image,
            200,
            Some(2048),
            false,
        ),
    ];
    let result = asset_weight_result(4096, &coll, &assets);
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(
        result
            .description
            .contains("2 asset responses measured (sampled from 2 references)"),
        "description: {}",
        result.description
    );
    assert!(
        !result.description.contains("at least"),
        "exact totals must not carry the floor qualifier: {}",
        result.description
    );
    let raw = result.raw_data.expect("raw_data");
    assert_eq!(raw["total_bytes"], 4096 + 1024 + 2048);
    assert_eq!(raw["measurement_incomplete"], false);
}

#[test]
fn asset_weight_counts_one_representative_per_srcset_group() {
    let coll = collection(4, 4, 4);
    let group = 42;
    let candidate = |url: &str, bytes: u64| MeasuredAsset {
        url: url.to_string(),
        kind: AssetKind::Image,
        status_code: 200,
        bytes: Some(bytes),
        content_type: None,
        cache_control: None,
        measured_floor: false,
        has_srcset: true,
        group,
    };
    let assets = vec![
        candidate("https://example.com/hero-400.avif", 1024 * 1024),
        candidate("https://example.com/hero-800.avif", 1536 * 1024),
        candidate("https://example.com/hero-1200.avif", 2048 * 1024),
        candidate("https://example.com/hero-1600.avif", 1800 * 1024),
    ];
    let result = asset_weight_result(0, &coll, &assets);
    let raw = result.raw_data.expect("raw_data");
    // html(0) + largest single candidate (2 MB), not the sum of all four.
    assert_eq!(raw["total_bytes"], 2048 * 1024);
}

#[test]
fn asset_weight_marks_incomplete_samples_without_calling_them_a_floor() {
    // More fetchable assets existed than were sampled.
    let coll = collection(40, 40, 2);
    let assets = vec![
        measured(
            "https://example.com/a0.png",
            AssetKind::Image,
            200,
            Some(1024),
            false,
        ),
        measured(
            "https://example.com/a1.png",
            AssetKind::Image,
            200,
            Some(2048),
            false,
        ),
    ];
    let result = asset_weight_result(4096, &coll, &assets);
    assert!(result.description.contains("incomplete"));
    assert!(!result.description.contains("at least"));
    assert_eq!(result.confidence, IssueConfidence::NeedsReview);

    // A truncated body read also makes the total a floor.
    let coll = collection(1, 1, 1);
    let mut truncated = measured(
        "https://example.com/a0.png",
        AssetKind::Image,
        200,
        Some(524_288),
        false,
    );
    truncated.measured_floor = true;
    let result = asset_weight_result(4096, &coll, &[truncated]);
    assert!(result.description.contains("incomplete"));
    assert!(!result.description.contains("at least"));
}

#[test]
fn asset_weight_fails_above_five_megabytes() {
    let coll = collection(1, 1, 1);
    let assets = vec![measured(
        "https://example.com/a0.png",
        AssetKind::Image,
        200,
        Some(6 * 1024 * 1024),
        false,
    )];
    let result = asset_weight_result(4096, &coll, &assets);
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.severity, Severity::Medium);
    assert!(result.manual_fix.is_some());
    assert!(result.why_it_matters.is_some());
}

#[test]
fn broken_images_lists_up_to_five_failing_urls() {
    let assets: Vec<MeasuredAsset> = (0..7)
        .map(|index| {
            measured(
                &format!("https://example.com/missing{}.png", index),
                AssetKind::Image,
                404,
                None,
                false,
            )
        })
        .collect();
    let result = broken_images_result(&assets, None);
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.severity, Severity::Medium);
    let listed = result.description.matches("(404)").count();
    assert_eq!(listed, 5, "description lists at most 5 URLs");
    let raw = result.raw_data.expect("raw_data");
    assert_eq!(raw["http_error_count"], 7);
}

#[test]
fn broken_images_warns_below_three_and_ignores_scripts() {
    let assets = vec![
        measured(
            "https://example.com/ok.png",
            AssetKind::Image,
            200,
            Some(10),
            false,
        ),
        measured(
            "https://example.com/gone.png",
            AssetKind::Image,
            404,
            None,
            false,
        ),
        measured(
            "https://example.com/dead.js",
            AssetKind::Script,
            404,
            None,
            false,
        ),
    ];
    let result = broken_images_result(&assets, None);
    assert_eq!(result.status, CheckStatus::Warn);
    assert!(
        !result.description.contains("dead.js"),
        "script failures must not appear in the broken-images result"
    );
}

#[test]
fn broken_images_passes_when_all_images_respond() {
    let assets = vec![measured(
        "https://example.com/ok.png",
        AssetKind::Image,
        200,
        Some(10),
        false,
    )];
    let result = broken_images_result(&assets, None);
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result.manual_fix.is_none());
}

#[test]
fn image_probe_failures_are_inconclusive_not_a_pass() {
    let assets = vec![measured(
        "https://example.com/private.png?token=secret",
        AssetKind::Image,
        0,
        None,
        false,
    )];
    let result = broken_images_result(&assets, None);
    assert_eq!(result.status, CheckStatus::Skipped);
    assert!(result.description.contains("inconclusive"));
    assert!(!result.description.contains("responded successfully"));
    let raw = result.raw_data.expect("raw data");
    assert_eq!(raw["inconclusive_count"], 1);
    assert!(!raw.to_string().contains("secret"));
}

#[test]
fn non_image_content_type_is_surfaced_without_calling_it_a_confirmed_broken_render() {
    let mut asset = measured(
        "https://example.com/catch-all.png",
        AssetKind::Image,
        200,
        Some(1024),
        false,
    );
    asset.content_type = Some("text/html".into());
    let result = broken_images_result(&[asset], None);
    assert_eq!(result.status, CheckStatus::Warn);
    assert_eq!(result.confidence, IssueConfidence::NeedsReview);
    assert!(result.description.contains("non-image Content-Type"));
    assert!(result
        .description
        .contains("does not prove browser rendering"));
}

#[test]
fn failed_srcset_candidate_downgrades_broken_image_confidence() {
    let assets = vec![measured(
        "https://example.com/hero-2400.png",
        AssetKind::Image,
        404,
        None,
        true,
    )];
    let result = broken_images_result(&assets, None);
    assert_eq!(result.status, CheckStatus::Warn);
    assert_eq!(result.confidence, IssueConfidence::NeedsReview);
    assert!(result
        .confidence_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("selected")));
}

#[test]
fn heavy_images_only_flags_images_over_threshold() {
    let assets = vec![
        measured(
            "https://example.com/exactly.png",
            AssetKind::Image,
            200,
            Some(HEAVY_IMAGE_BYTES),
            false,
        ),
        measured(
            "https://example.com/big.png",
            AssetKind::Image,
            200,
            Some(HEAVY_IMAGE_BYTES + 1),
            false,
        ),
        measured(
            "https://example.com/big.js",
            AssetKind::Script,
            200,
            Some(2 * HEAVY_IMAGE_BYTES),
            false,
        ),
    ];
    let result = heavy_images_result(&assets, None);
    assert_eq!(result.status, CheckStatus::Warn);
    let raw = result.raw_data.as_ref().expect("raw data");
    assert_eq!(raw["heavy"].as_array().map(Vec::len), Some(1));
    assert_eq!(raw["heavy"][0]["bytes"], HEAVY_IMAGE_BYTES + 1);
    assert_eq!(raw["images_sampled"], 2, "scripts are out of scope");
}

#[test]
fn heavy_images_with_srcset_everywhere_downgrade_confidence() {
    let assets = vec![measured(
        "https://example.com/hero-2x.png",
        AssetKind::Image,
        200,
        Some(800 * 1024),
        true,
    )];
    let result = heavy_images_result(&assets, None);
    assert_eq!(result.confidence, IssueConfidence::NeedsReview);
    assert!(result.confidence_reason.is_some());
    assert!(result.description.contains("srcset present"));
}

#[test]
fn heavy_plain_img_src_stays_high_confidence() {
    let assets = vec![
        measured(
            "https://example.com/hero-2x.png",
            AssetKind::Image,
            200,
            Some(800 * 1024),
            true,
        ),
        measured(
            "https://example.com/plain.png",
            AssetKind::Image,
            200,
            Some(800 * 1024),
            false,
        ),
    ];
    let result = heavy_images_result(&assets, None);
    assert_eq!(result.confidence, IssueConfidence::High);
    assert!(result.description.contains("no srcset"));
}

#[test]
fn heavy_images_notes_legacy_formats_from_measured_content_type() {
    let mut asset = measured(
        "https://example.com/photo.jpg",
        AssetKind::Image,
        200,
        Some(900 * 1024),
        false,
    );
    asset.content_type = Some("image/jpeg".into());
    let result = heavy_images_result(&[asset], None);
    assert!(result.description.contains("JPEG or PNG"));
    assert!(!result.description.contains("30-50%"));
    let raw = result.raw_data.expect("raw_data");
    assert_eq!(raw["heavy"][0]["jpeg_or_png"], true);
}

#[test]
fn persisted_asset_evidence_redacts_queries_fragments_and_sensitive_path_tokens() {
    let coll = collection(1, 1, 1);
    let url = "https://cdn.example.com/account/reset/short-token?signature=secret#fragment";
    let mut asset = measured(url, AssetKind::Image, 404, None, false);
    asset.content_type = Some("image/png".into());

    for result in [
        asset_weight_result(0, &coll, std::slice::from_ref(&asset)),
        broken_images_result(std::slice::from_ref(&asset), None),
        heavy_images_result(std::slice::from_ref(&asset), None),
        asset_caching_result(std::slice::from_ref(&asset), None),
    ] {
        let serialized = serde_json::to_string(&result).expect("serialize result");
        assert!(!serialized.contains("short-token"), "{serialized}");
        assert!(!serialized.contains("signature"), "{serialized}");
        assert!(!serialized.contains("secret"), "{serialized}");
        assert!(!serialized.contains("fragment"), "{serialized}");
        if serialized.contains("cdn.example.com") {
            assert!(
                serialized.contains("/account/reset/[redacted]"),
                "{serialized}"
            );
        }
    }
}

#[test]
fn persisted_asset_evidence_retains_an_actionable_ordinary_path() {
    let asset = measured(
        "https://cdn.example.com/images/missing-logo.png?signature=secret#fragment",
        AssetKind::Image,
        404,
        None,
        false,
    );
    let result = broken_images_result(&[asset], None);
    let serialized = serde_json::to_string(&result).expect("serialize result");
    assert!(
        serialized.contains("https://cdn.example.com/images/missing-logo.png"),
        "{serialized}"
    );
    assert!(!serialized.contains("signature"), "{serialized}");
    assert!(!serialized.contains("secret"), "{serialized}");
    assert!(!serialized.contains("fragment"), "{serialized}");
}

#[test]
fn all_results_pass_when_no_assets_sampled() {
    let coll = collection(0, 0, 0);
    let weight = asset_weight_result(2048, &coll, &[]);
    let broken = broken_images_result(&[], None);
    let heavy = heavy_images_result(&[], None);
    let caching = asset_caching_result(&[], None);
    assert_eq!(weight.status, CheckStatus::Pass);
    assert_eq!(broken.status, CheckStatus::Pass);
    assert_eq!(heavy.status, CheckStatus::Pass);
    assert_eq!(caching.status, CheckStatus::Pass);
    assert_eq!(weight.check_id, "performance.asset_weight");
    assert_eq!(broken.check_id, "performance.broken_images");
    assert_eq!(heavy.check_id, "performance.images.heavy");
    assert_eq!(caching.check_id, "performance.asset_caching");
}

fn cached(url: &str, cache_control: Option<&str>) -> MeasuredAsset {
    MeasuredAsset {
        url: url.to_string(),
        kind: AssetKind::Script,
        status_code: 200,
        bytes: Some(1024),
        content_type: None,
        cache_control: cache_control.map(str::to_string),
        measured_floor: false,
        has_srcset: false,
        group: group_for(url),
    }
}

#[test]
fn fingerprinted_asset_without_durable_cache_warns() {
    // A build-hashed filename served with a short/absent cache is the precise
    // caching miss: the URL is immutable but browsers keep re-checking it.
    let assets = vec![
        cached(
            "https://example.com/assets/index-DkJ8s0aB.js",
            Some("no-cache"),
        ),
        cached("https://example.com/assets/vendor.4f3a2b1c.css", None),
    ];
    let result = asset_caching_result(&assets, Some("https://example.com"));
    assert_eq!(result.status, CheckStatus::Warn);
    assert_eq!(result.severity, Severity::Low);
    assert_eq!(result.confidence, IssueConfidence::NeedsReview);
    assert!(
        result.description.contains("look content-hashed"),
        "description must hedge the hash claim: {}",
        result.description
    );
    assert!(!result
        .description
        .contains("should only ever serve one file"));
    let raw = result.raw_data.expect("raw_data");
    assert_eq!(raw["weak_count"], 2);
}

#[test]
fn single_weak_cached_asset_uses_singular_grammar() {
    let assets = vec![
        cached(
            "https://example.com/assets/index-DkJ8s0aB.js",
            Some("no-cache"),
        ),
        cached(
            "https://example.com/assets/vendor.4f3a2b1c.css",
            Some("public, max-age=31536000, immutable"),
        ),
    ];
    let result = asset_caching_result(&assets, Some("https://example.com"));
    assert!(
        result
            .description
            .contains("1 of 2 sampled same-site assets with fingerprint-like filenames is served"),
        "singular verb expected: {}",
        result.description
    );
}

#[test]
fn third_party_fingerprinted_assets_are_counted_but_never_graded() {
    // plausible.io sets its own cache headers; the scanned site cannot change
    // them, so a short max-age there is not the site's caching miss.
    let assets = vec![
        cached(
            "https://plausible.io/js/pa-XfHGJa14uHVX9qMr7Gz_g.js",
            Some("public, max-age=60, no-transform"),
        ),
        cached(
            "https://example.com/assets/index-DkJ8s0aB.js",
            Some("public, max-age=31536000, immutable"),
        ),
    ];
    let result = asset_caching_result(&assets, Some("https://example.com"));
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.description);
    assert_eq!(result.confidence, IssueConfidence::High);
    assert!(
        result.description.contains(
            "1 sampled third-party asset with a fingerprint-like filename was not graded"
        ),
        "{}",
        result.description
    );
    let raw = result.raw_data.expect("raw_data");
    assert_eq!(raw["fingerprinted_sampled"], 2);
    assert_eq!(raw["third_party_sampled"], 1);
    assert_eq!(raw["weak_count"], 0);
    assert!(!raw.to_string().contains("plausible.io"));
}

#[test]
fn only_third_party_fingerprints_report_nothing_graded() {
    let assets = vec![cached(
        "https://plausible.io/js/pa-XfHGJa14uHVX9qMr7Gz_g.js",
        Some("public, max-age=60, no-transform"),
    )];
    let result = asset_caching_result(&assets, Some("https://example.com"));
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(
        result
            .description
            .starts_with("No sampled same-site asset filename matched"),
        "{}",
        result.description
    );
    assert_eq!(result.raw_data.expect("raw_data")["third_party_sampled"], 1);
}

#[test]
fn the_sites_own_cdn_subdomain_is_same_site_and_graded() {
    let assets = vec![cached(
        "https://cdn.example.com/assets/index-DkJ8s0aB.js",
        Some("no-cache"),
    )];
    let result = asset_caching_result(&assets, Some("https://www.example.com"));
    assert_eq!(result.status, CheckStatus::Warn, "{}", result.description);
    let raw = result.raw_data.expect("raw_data");
    assert_eq!(raw["third_party_sampled"], 0);
    assert_eq!(raw["weak_count"], 1);
}

#[test]
fn same_site_needs_the_registrable_domain_not_just_the_public_suffix() {
    assert!(is_same_site(
        "https://static.example.co.uk/a-DkJ8s0aB.js",
        Some("https://www.example.co.uk")
    ));
    assert!(!is_same_site(
        "https://other.co.uk/a-DkJ8s0aB.js",
        Some("https://www.example.co.uk")
    ));
    assert!(!is_same_site(
        "https://example.com.evil.net/a-DkJ8s0aB.js",
        Some("https://example.com")
    ));
    assert!(is_same_site(
        "http://localhost:5173/a-DkJ8s0aB.js",
        Some("http://localhost:5173")
    ));
    assert!(!is_same_site("https://example.com/a-DkJ8s0aB.js", None));
}

#[test]
fn single_heavy_image_uses_singular_grammar() {
    let assets = vec![
        measured(
            "https://example.com/big.png",
            AssetKind::Image,
            200,
            Some(HEAVY_IMAGE_BYTES + 1),
            false,
        ),
        measured(
            "https://example.com/small.png",
            AssetKind::Image,
            200,
            Some(1024),
            false,
        ),
    ];
    let result = heavy_images_result(&assets, None);
    assert!(
        result
            .description
            .contains("1 of 2 sampled images measures over 300 KB"),
        "singular verb expected: {}",
        result.description
    );
}

#[test]
fn asset_weight_copy_labels_sizes_as_uncompressed() {
    // The sampler measures decompressed bytes for compressed text assets,
    // so the copy must not present the total as network transfer.
    let coll = collection(1, 1, 1);
    let assets = vec![measured(
        "https://example.com/app.js",
        AssetKind::Script,
        200,
        Some(1024),
        false,
    )];
    let result = asset_weight_result(2048, &coll, &assets);
    assert!(
        result.description.contains("decoded/uncompressed"),
        "description must label sizes honestly: {}",
        result.description
    );
}

#[test]
fn durable_cache_and_unversioned_assets_do_not_warn() {
    // A hashed asset with a year-long immutable cache is correct; an
    // unversioned filename is out of scope (it may legitimately revalidate).
    let assets = vec![
        cached(
            "https://example.com/assets/index-DkJ8s0aB.js",
            Some("public, max-age=31536000, immutable"),
        ),
        cached("https://example.com/js/app.js", Some("no-cache")),
        cached("https://example.com/css/styles.css", None),
    ];
    let result = asset_caching_result(&assets, Some("https://example.com"));
    assert_eq!(
        result.status,
        CheckStatus::Pass,
        "description: {}",
        result.description
    );
    assert!(
        result
            .description
            .starts_with("All 1 sampled same-site assets"),
        "{}",
        result.description
    );
}

#[test]
fn heavy_image_evidence_keeps_asset_filenames_but_not_opaque_path_tokens() {
    let own = measured(
        "https://sitecmd.com/images/screenshots/problem/dashboard-health-score-before-fix-2026.png",
        AssetKind::Image,
        200,
        Some(507_000),
        false,
    );
    // A filename ending in an image extension is build output wherever it is
    // served from, and the fix needs it verbatim to find the file.
    let foreign = measured(
        "https://cdn.example.com/images/screenshots/problem/dashboard-health-score-before-fix-2026.png",
        AssetKind::Image,
        200,
        Some(507_000),
        false,
    );
    // An extension-less long segment on a foreign host can be a signed path
    // token, so it stays redacted.
    let signed = measured(
        "https://cdn.example.com/image/upload/s--abcdef0123456789abcdef0123456789--/hero.png",
        AssetKind::Image,
        200,
        Some(507_000),
        false,
    );
    let result = heavy_images_result(&[own, foreign, signed], Some("https://sitecmd.com"));
    assert!(
        result.description.contains("https://sitecmd.com/images/screenshots/problem/dashboard-health-score-before-fix-2026.png (507 KB"),
        "{}",
        result.description
    );
    assert!(
        result.description.contains("https://cdn.example.com/images/screenshots/problem/dashboard-health-score-before-fix-2026.png (507 KB"),
        "{}",
        result.description
    );
    assert!(
        result
            .description
            .contains("https://cdn.example.com/image/upload/[redacted]/hero.png (507 KB"),
        "{}",
        result.description
    );
}

#[test]
fn asset_weight_result_keeps_asset_filenames_but_strips_opaque_segments_off_origin() {
    // A hashed bundle name is build output; the fix cannot find the file
    // without it, so it survives with or without a page origin.
    let bundle = "https://sitecmd.com/assets/dashboard-health-score-a1b2c3d4e5f67890.js";
    // An extension-less long segment is opaque, and off the scanned origin it
    // can be a signed path token.
    let opaque = "https://sitecmd.com/deliver/a1b2c3d4e5f678901234567890abcdef1234/main";
    let assets = vec![
        measured(bundle, AssetKind::Script, 200, Some(1024), false),
        measured(opaque, AssetKind::Script, 200, Some(1024), false),
    ];

    let mut coll = collection(1, 1, 1);
    coll.page_origin = Some("https://sitecmd.com".to_string());
    let same_origin = asset_weight_result(0, &coll, &assets);
    let serialized = serde_json::to_string(&same_origin).expect("serialize result");
    assert!(serialized.contains(bundle), "{serialized}");
    assert!(serialized.contains(opaque), "{serialized}");

    coll.page_origin = None;
    let no_origin = asset_weight_result(0, &coll, &assets);
    let serialized = serde_json::to_string(&no_origin).expect("serialize result");
    assert!(serialized.contains(bundle), "{serialized}");
    assert!(!serialized.contains(opaque), "{serialized}");
    assert!(
        serialized.contains("https://sitecmd.com/deliver/[redacted]/main"),
        "{serialized}"
    );
}

#[test]
fn broken_images_result_keeps_asset_filenames_but_strips_opaque_segments_off_origin() {
    let bundle = "https://sitecmd.com/assets/dashboard-health-score-a1b2c3d4e5f67890.js";
    let opaque = "https://sitecmd.com/deliver/a1b2c3d4e5f678901234567890abcdef1234/main";

    let same_origin = broken_images_result(
        &[
            measured(bundle, AssetKind::Image, 404, None, false),
            measured(opaque, AssetKind::Image, 404, None, false),
        ],
        Some("https://sitecmd.com"),
    );
    assert!(
        same_origin.description.contains(bundle) && same_origin.description.contains(opaque),
        "{}",
        same_origin.description
    );

    let no_origin = broken_images_result(
        &[
            measured(bundle, AssetKind::Image, 404, None, false),
            measured(opaque, AssetKind::Image, 404, None, false),
        ],
        None,
    );
    assert!(
        no_origin.description.contains(bundle),
        "a hashed bundle filename is what the fix greps for: {}",
        no_origin.description
    );
    assert!(
        no_origin
            .description
            .contains("https://sitecmd.com/deliver/[redacted]/main"),
        "{}",
        no_origin.description
    );
    assert!(
        !no_origin.description.contains(opaque),
        "{}",
        no_origin.description
    );
}

#[test]
fn asset_caching_result_keeps_the_sites_own_path_and_grades_nothing_without_a_page_origin() {
    let url = "https://sitecmd.com/assets/dashboard-health-score-a1b2c3d4e5f67890.js";

    let same_origin = asset_caching_result(
        &[cached(url, Some("no-cache"))],
        Some("https://sitecmd.com"),
    );
    assert_eq!(same_origin.status, CheckStatus::Warn);
    assert!(
        same_origin.description.contains(url),
        "{}",
        same_origin.description
    );

    // Without a page origin, same-site membership is unknown rather than
    // empty: the check reports what it could not establish instead of passing
    // the site, and persists no foreign path token either way.
    let no_origin = asset_caching_result(&[cached(url, Some("no-cache"))], None);
    assert_eq!(
        no_origin.status,
        CheckStatus::Skipped,
        "{}",
        no_origin.description
    );
    let raw = no_origin.raw_data.expect("raw_data");
    assert_eq!(raw["reason"], "no_page_origin");
    assert_eq!(raw["fingerprinted_sampled"], 1);
    assert!(raw.get("weak").is_none(), "nothing was graded: {raw}");
    assert!(raw.get("weak_count").is_none(), "nothing was graded: {raw}");
    assert!(
        !no_origin.description.contains("sitecmd.com"),
        "{}",
        no_origin.description
    );
}

#[test]
fn without_a_page_origin_an_empty_fingerprint_sample_still_reports_its_verdict() {
    // Nothing matched the filename pattern, which is true whether or not the
    // assets could have been attributed, so this one is not a skip.
    let result = asset_caching_result(
        &[cached("https://cdn.example/app.js", Some("no-cache"))],
        None,
    );
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.description);
    assert!(
        result
            .description
            .starts_with("No sampled static-asset filename matched"),
        "{}",
        result.description
    );
}

#[test]
fn fingerprint_detection_ignores_version_words() {
    // Version-ish names must not read as content hashes (would false-positive
    // on every un-fingerprinted vendored library).
    assert!(!looks_fingerprinted("https://example.com/js/bootstrap4.js"));
    assert!(!looks_fingerprinted("https://example.com/js/dashboard.js"));
    assert!(!looks_fingerprinted("https://example.com/js/html5shiv.js"));
    // Real build hashes (hex and Vite-style base) are detected.
    assert!(looks_fingerprinted(
        "https://example.com/assets/index-DkJ8s0aB.js"
    ));
    assert!(looks_fingerprinted(
        "https://example.com/assets/main.4f3a2b1c.css"
    ));
    // A query string must not defeat the filename check.
    assert!(looks_fingerprinted(
        "https://example.com/assets/main.4f3a2b1c.css?v=2"
    ));
}
