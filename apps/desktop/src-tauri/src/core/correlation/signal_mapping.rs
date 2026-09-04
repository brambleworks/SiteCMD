pub struct SignalMapping {
    pub source: &'static str,
    pub source_signal: &'static str,
    pub check_id: &'static str,
}

pub const SIGNAL_MAPPINGS: &[SignalMapping] = &[
    // PSI -> performance checks
    SignalMapping {
        source: "psi",
        source_signal: "render-blocking-resources",
        check_id: "performance.render_blocking",
    },
    SignalMapping {
        source: "psi",
        source_signal: "unused-css-rules",
        check_id: "performance.unused_css",
    },
    SignalMapping {
        source: "psi",
        source_signal: "unused-javascript",
        check_id: "performance.unused_javascript",
    },
    SignalMapping {
        source: "psi",
        source_signal: "modern-image-formats",
        check_id: "performance.modern_image_formats",
    },
    SignalMapping {
        source: "psi",
        source_signal: "uses-responsive-images",
        check_id: "performance.responsive_images",
    },
    SignalMapping {
        source: "psi",
        source_signal: "offscreen-images",
        check_id: "performance.lazy_load_images",
    },
    SignalMapping {
        source: "psi",
        source_signal: "uses-text-compression",
        check_id: "performance.compression",
    },
    SignalMapping {
        source: "psi",
        source_signal: "uses-long-cache-ttl",
        check_id: "performance.cache_headers",
    },
    SignalMapping {
        source: "psi",
        source_signal: "total-byte-weight",
        check_id: "performance.page_weight",
    },
    SignalMapping {
        source: "psi",
        source_signal: "field-lcp",
        check_id: "performance.lcp",
    },
    SignalMapping {
        source: "psi",
        source_signal: "field-cls",
        check_id: "performance.cls",
    },
    SignalMapping {
        source: "psi",
        source_signal: "field-inp",
        check_id: "performance.inp",
    },
    // Lab (Lighthouse) metrics from the same PSI report. Lab TBT maps to the
    // Dedicated TBT check id, NOT performance.inp: Lighthouse TBT is a lab
    // main-thread diagnostic and must not masquerade as field INP.
    SignalMapping {
        source: "psi",
        source_signal: "lab-lcp",
        check_id: "performance.lcp",
    },
    SignalMapping {
        source: "psi",
        source_signal: "lab-cls",
        check_id: "performance.cls",
    },
    SignalMapping {
        source: "psi",
        source_signal: "lab-tbt",
        check_id: "performance.tbt",
    },
    // GSC -> seo / accessibility checks
    SignalMapping {
        source: "gsc",
        source_signal: "not-indexed",
        check_id: "seo.indexing.not-indexed",
    },
    SignalMapping {
        source: "gsc",
        source_signal: "crawl-error",
        check_id: "seo.indexing.crawl-error",
    },
    SignalMapping {
        source: "gsc",
        source_signal: "blocked-by-robots",
        check_id: "seo.robots.blocked",
    },
    SignalMapping {
        source: "gsc",
        source_signal: "duplicate-no-canonical",
        check_id: "seo.canonical.missing",
    },
    SignalMapping {
        source: "gsc",
        source_signal: "canonical-mismatch",
        check_id: "seo.canonical.mismatch",
    },
    SignalMapping {
        source: "gsc",
        source_signal: "mobile-viewport",
        check_id: "seo.mobile-viewport",
    },
    SignalMapping {
        source: "gsc",
        source_signal: "touch-target-size",
        check_id: "accessibility.touch-target-size",
    },
    SignalMapping {
        source: "gsc",
        source_signal: "text-too-small",
        check_id: "accessibility.text-size",
    },
    SignalMapping {
        source: "gsc",
        source_signal: "content-wider-than-screen",
        check_id: "seo.mobile-responsive",
    },
    // Updates -> dependency / infrastructure
    SignalMapping {
        source: "updates",
        source_signal: "vulnerability",
        check_id: "dependencies.vulnerability",
    },
    SignalMapping {
        source: "updates",
        source_signal: "deprecated",
        check_id: "dependencies.deprecated",
    },
    SignalMapping {
        source: "updates",
        source_signal: "outdated-major",
        check_id: "dependencies.outdated-major",
    },
    SignalMapping {
        source: "updates",
        source_signal: "install-scripts",
        check_id: "dependencies.install-scripts",
    },
    SignalMapping {
        source: "updates",
        source_signal: "license-copyleft",
        check_id: "dependencies.license-copyleft",
    },
    SignalMapping {
        source: "updates",
        source_signal: "license-missing",
        check_id: "dependencies.license-missing",
    },
    SignalMapping {
        source: "updates",
        source_signal: "ssl-expiring",
        check_id: "infrastructure.ssl-expiring",
    },
    SignalMapping {
        source: "updates",
        source_signal: "ci-failure",
        check_id: "infrastructure.ci-failure",
    },
    // Plausible -> analytics
    SignalMapping {
        source: "plausible",
        source_signal: "traffic-drop",
        check_id: "analytics.traffic-drop",
    },
    SignalMapping {
        source: "plausible",
        source_signal: "goal-drop",
        check_id: "analytics.conversion-drop",
    },
    SignalMapping {
        source: "plausible",
        source_signal: "entry-page-anomaly",
        check_id: "analytics.landing-page-change",
    },
    // Cloudflare -> infrastructure / performance / security (DEDUPES with web_scan + psi)
    SignalMapping {
        source: "cloudflare",
        source_signal: "5xx-rate-high",
        check_id: "infrastructure.server-errors",
    },
    SignalMapping {
        source: "cloudflare",
        source_signal: "cache-hit-low",
        check_id: "performance.cache_headers",
    },
    SignalMapping {
        source: "cloudflare",
        source_signal: "origin-error",
        check_id: "infrastructure.origin-error",
    },
    SignalMapping {
        source: "cloudflare",
        source_signal: "bot-traffic-spike",
        check_id: "security.bot-traffic",
    },
    // UptimeRobot -> infrastructure / performance (DEDUPES)
    SignalMapping {
        source: "uptimerobot",
        source_signal: "monitor-down",
        check_id: "infrastructure.uptime",
    },
    SignalMapping {
        source: "uptimerobot",
        source_signal: "slow-response",
        check_id: "performance.ttfb",
    },
    SignalMapping {
        source: "uptimerobot",
        source_signal: "ssl-mismatch",
        check_id: "infrastructure.ssl-mismatch",
    },
    // Code Scan -> canonical check_ids (big dedup win with web_scan)
    SignalMapping {
        source: "code_scan",
        source_signal: "security_headers",
        check_id: "security.csp",
    },
    SignalMapping {
        source: "code_scan",
        source_signal: "env_exposure",
        check_id: "security.exposed-env",
    },
    SignalMapping {
        source: "code_scan",
        source_signal: "mixed_content",
        check_id: "security.mixed_content",
    },
    SignalMapping {
        source: "code_scan",
        source_signal: "cors_wildcard",
        check_id: "security.cors",
    },
    SignalMapping {
        source: "code_scan",
        source_signal: "cookie_flags",
        check_id: "security.cookie-flags",
    },
    SignalMapping {
        source: "code_scan",
        source_signal: "robots_config",
        check_id: "seo.robots",
    },
    SignalMapping {
        source: "code_scan",
        source_signal: "canonical_missing",
        check_id: "seo.canonical.missing",
    },
    SignalMapping {
        source: "code_scan",
        source_signal: "sitemap_missing",
        check_id: "seo.sitemap.missing",
    },
    SignalMapping {
        source: "code_scan",
        source_signal: "https_redirect",
        check_id: "security.https",
    },
    SignalMapping {
        source: "code_scan",
        source_signal: "hsts_missing",
        check_id: "security.hsts",
    },
    // Web scan -> canonical (drops `.headers.` taxonomy middle segment;
    // `https_enforcement` short-hands to `https`)
    SignalMapping {
        source: "web_scan",
        source_signal: "security.headers.csp",
        check_id: "security.csp",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "security.headers.hsts",
        check_id: "security.hsts",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "security.headers.x_frame_options",
        check_id: "security.x_frame_options",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "security.headers.x_content_type_options",
        check_id: "security.x_content_type_options",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "security.headers.referrer_policy",
        check_id: "security.referrer_policy",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "security.headers.permissions_policy",
        check_id: "security.permissions_policy",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "security.https_enforcement",
        check_id: "security.https",
    },
    // Group duplicate SEO and accessibility heading observations while keeping
    // each result as evidence. Preserve legacy aliases for stored scans.
    SignalMapping {
        source: "web_scan",
        source_signal: "seo.headings.h1",
        check_id: "accessibility.headings",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "seo.headings.hierarchy",
        check_id: "accessibility.headings",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "seo.image_alt",
        check_id: "accessibility.image_alt",
    },
    // Polish aliases share an issue group; raw ids remain separate instances.
    SignalMapping {
        source: "web_scan",
        source_signal: "polish.missing-lang",
        check_id: "accessibility.lang",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "polish.heading-hierarchy",
        check_id: "accessibility.headings",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "polish.form-accessibility",
        check_id: "accessibility.form_labels",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "polish.default-page-title",
        check_id: "seo.title",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "polish.missing-og-tags",
        check_id: "seo.open_graph",
    },
    // Fires only when the canonical link, the robots meta, and the sitemap
    // link are all absent, so `seo.canonical` has already reported the
    // missing canonical that this signal re-reports.
    SignalMapping {
        source: "web_scan",
        source_signal: "polish.no-sitemap-robots",
        check_id: "seo.canonical",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "polish.default-favicon",
        check_id: "config.favicon",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "polish.source-maps-production",
        check_id: "security.source_maps",
    },
    SignalMapping {
        source: "web_scan",
        source_signal: "polish.console-log-production",
        check_id: "config.console_logs",
    },
];

/// The canonical check id a Web Scan signal is filed under, when the table
/// renames it. Borrowed rather than owned so the score can key on it without
/// allocating for every finding.
pub fn web_scan_check_id(signal: &str) -> Option<&'static str> {
    SIGNAL_MAPPINGS
        .iter()
        .find(|m| m.source == "web_scan" && m.source_signal == signal)
        .map(|m| m.check_id)
}

#[tracing::instrument(fields(source = %source, signal = %signal))]
pub fn resolve_check_id(source: &str, signal: &str) -> String {
    // Web Scan signals are already canonical unless the table renames them.
    if source == "web_scan" {
        return web_scan_check_id(signal).unwrap_or(signal).to_string();
    }
    if let Some(m) = SIGNAL_MAPPINGS
        .iter()
        .find(|m| m.source == source && m.source_signal == signal)
    {
        return m.check_id.to_string();
    }
    format!("{source}.{signal}")
}

/// Producer signals that map to a canonical check id for one source.
pub fn source_signals_for_check_id(source: &str, check_id: &str) -> Vec<&'static str> {
    SIGNAL_MAPPINGS
        .iter()
        .filter(|mapping| mapping.source == source && mapping.check_id == check_id)
        .map(|mapping| mapping.source_signal)
        .collect()
}

/// Retired Web producer IDs retained for stored-row canonicalization.
/// Verification excludes them because no current producer can emit them.
pub const HISTORICAL_WEB_PRODUCERS: &[&str] = &["seo.image_alt"];

pub fn is_historical_web_producer(signal: &str) -> bool {
    HISTORICAL_WEB_PRODUCERS.contains(&signal)
}

/// Like `source_signals_for_check_id`, but restricted to producers that still
/// have a live emitter: the set verification may require re-observed results
/// from. Grouping and read-side aliasing must keep using the unfiltered set.
pub fn live_source_signals_for_check_id(source: &str, check_id: &str) -> Vec<&'static str> {
    source_signals_for_check_id(source, check_id)
        .into_iter()
        .filter(|signal| source != "web_scan" || !is_historical_web_producer(signal))
        .collect()
}

/// Legal canonical ids for mappings and causal-link endpoints.
pub const CANONICAL_CHECK_IDS: &[&str] = &[
    // accessibility
    "accessibility.autoplay",
    "accessibility.form_labels",
    "accessibility.headings",
    "accessibility.image_alt",
    "accessibility.landmarks",
    "accessibility.lang",
    "accessibility.link_text",
    "accessibility.skip_nav",
    "accessibility.text-size",
    "accessibility.touch-target-size",
    // analytics
    "analytics.conversion-drop",
    "analytics.landing-page-change",
    "analytics.traffic-drop",
    // compliance
    "compliance.cookie_consent",
    "compliance.form_consent",
    "compliance.privacy_policy",
    "compliance.terms",
    "compliance.trackers",
    // config
    "config.analytics",
    "config.console_logs",
    "config.custom_404",
    "config.deprecated_html",
    "config.favicon",
    "config.www_redirect",
    // dependencies
    "dependencies.deprecated",
    "dependencies.install-scripts",
    "dependencies.license-copyleft",
    "dependencies.license-missing",
    "dependencies.outdated-major",
    "dependencies.vulnerability",
    // infrastructure
    "infrastructure.ci-failure",
    "infrastructure.origin-error",
    "infrastructure.server-errors",
    "infrastructure.ssl-expiring",
    "infrastructure.ssl-mismatch",
    "infrastructure.uptime",
    // performance
    "performance.cache",
    "performance.cache_headers",
    "performance.cls",
    "performance.compression",
    "performance.dom_size",
    "performance.fonts",
    "performance.http_requests",
    "performance.images",
    "performance.images.dimensions",
    "performance.images.format",
    "performance.images.lazy",
    "performance.inp",
    "performance.lazy_load_images",
    "performance.lcp",
    "performance.modern_image_formats",
    "performance.page_weight",
    "performance.preconnect",
    "performance.redirect_chain",
    "performance.render_blocking",
    "performance.responsive_images",
    "performance.tbt",
    "performance.third_party",
    "performance.ttfb",
    "performance.unminified",
    "performance.unused_css",
    "performance.unused_javascript",
    // security (flattened - middle segment `headers` dropped per spec)
    "security.bot-traffic",
    "security.cookie-flags",
    "security.cookies",
    "security.cors",
    "security.csp",
    "security.directory_listing",
    "security.exposed-env",
    "security.exposed_files.source_secrets",
    "security.exposed_files.summary",
    "security.https",
    "security.hsts",
    "security.insecure_form",
    "security.form_action_hijack",
    "security.mixed_content",
    "security.open_redirect",
    "security.permissions_policy",
    "security.referrer_policy",
    "security.server_info.server_header",
    "security.server_info.x_powered_by",
    "security.source_maps",
    "security.sri",
    "security.ssl.chain",
    "security.ssl.expiry",
    "security.ssl.hostname",
    "security.ssl.protocol",
    "security.vibe.client_auth",
    "security.vibe.csrf",
    "security.vibe.env_exposure",
    "security.vibe.exposed_keys",
    "security.vibe.hardcoded_secrets",
    "security.x_content_type_options",
    "security.x_frame_options",
    // seo
    "seo.broken_links",
    "seo.canonical",
    "seo.canonical.mismatch",
    "seo.canonical.missing",
    "seo.duplicate_description",
    "seo.duplicate_description_across_pages",
    "seo.duplicate_meta",
    "seo.duplicate_title",
    "seo.duplicate_title_across_pages",
    "seo.headings.h1",
    "seo.headings.hierarchy",
    "seo.hreflang",
    "seo.image_alt",
    "seo.indexing.crawl-error",
    "seo.indexing.not-indexed",
    "seo.meta_description",
    "seo.mobile-responsive",
    "seo.mobile-viewport",
    "seo.noindex",
    "seo.open_graph",
    "seo.robots",
    "seo.robots.blocked",
    "seo.robots_txt",
    "seo.sitemap",
    "seo.sitemap.missing",
    "seo.structured_data",
    "seo.title",
    "seo.twitter_cards",
    "seo.url_structure",
    "seo.viewport",
];

#[cfg(test)]
#[path = "signal_mapping_tests.rs"]
mod tests;
