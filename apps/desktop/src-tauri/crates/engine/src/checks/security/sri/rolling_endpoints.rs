//! Known dynamic or rolling vendor endpoints that one fixed integrity hash
//! cannot pin, plus the path rules that match them. Split out of `sri.rs`
//! because the table and its reason strings are vendor data rather than
//! verdict logic; the rules and reasons are unchanged.

/// How a rolling endpoint's path is matched. Paths, not whole hosts: a vendor
/// host can also serve stable, versioned files that remain reviewable.
enum PathRule {
    /// Every path on the host is generated per request or per site.
    AnyPath,
    Exact(&'static str),
    /// The path itself or anything below it.
    Under(&'static str),
    Prefix(&'static str),
    Suffix(&'static str),
}

impl PathRule {
    fn matches(&self, path: &str) -> bool {
        match self {
            Self::AnyPath => true,
            Self::Exact(expected) => path == *expected,
            Self::Under(root) => path
                .strip_prefix(root)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/')),
            Self::Prefix(prefix) => path.starts_with(prefix),
            Self::Suffix(suffix) => path.ends_with(suffix),
        }
    }
}

struct RollingEndpoint {
    host: &'static str,
    path: PathRule,
    reason: &'static str,
}

/// Known dynamic or rolling vendor endpoints that one fixed integrity hash
/// cannot pin. Each reason cites the vendor's loading guidance, per-site
/// generation, or a versionless URL the vendor updates in place; the cache
/// lifetimes quoted are the response headers observed on 2026-09-02.
static ROLLING_ENDPOINTS: &[RollingEndpoint] = &[
    RollingEndpoint {
        host: "fonts.googleapis.com",
        path: PathRule::AnyPath,
        reason: "Google Fonts stylesheet responses can vary by request and user agent rather than identifying immutable bytes",
    },
    RollingEndpoint {
        host: "js.stripe.com",
        path: PathRule::Under("/v3"),
        reason: "Stripe.js v3 is a vendor-managed rolling endpoint whose bytes can change in place",
    },
    RollingEndpoint {
        host: "js.stripe.com",
        path: PathRule::Suffix("/stripe.js"),
        reason: "Stripe's named Stripe.js releases (for example /basil/stripe.js) are patched in place, and Stripe's docs say to always load Stripe.js directly from js.stripe.com rather than bundling or hosting it",
    },
    RollingEndpoint {
        host: "www.googletagmanager.com",
        path: PathRule::Exact("/gtm.js"),
        reason: "Google Tag Manager and gtag scripts are vendor-managed endpoints whose bytes can change in place",
    },
    RollingEndpoint {
        host: "www.googletagmanager.com",
        path: PathRule::Exact("/gtag/js"),
        reason: "Google Tag Manager and gtag scripts are vendor-managed endpoints whose bytes can change in place",
    },
    RollingEndpoint {
        host: "www.google-analytics.com",
        path: PathRule::Exact("/analytics.js"),
        reason: "Google Analytics serves this versionless script from a vendor-managed endpoint whose bytes can change in place",
    },
    RollingEndpoint {
        host: "www.google-analytics.com",
        path: PathRule::Exact("/ga.js"),
        reason: "Google Analytics serves this versionless script from a vendor-managed endpoint whose bytes can change in place",
    },
    RollingEndpoint {
        host: "www.paypal.com",
        path: PathRule::Exact("/sdk/js"),
        reason: "the PayPal JavaScript SDK response is assembled from query configuration and vendor-managed code",
    },
    RollingEndpoint {
        host: "www.paypalobjects.com",
        path: PathRule::Exact("/api/checkout.js"),
        reason: "the legacy PayPal checkout SDK is a vendor-managed rolling endpoint whose bytes can change in place",
    },
    RollingEndpoint {
        host: "plausible.io",
        path: PathRule::Prefix("/js/"),
        reason: "Plausible regenerates its tracker (the script.js variants and the per-site pa-<id>.js) as the site's configuration and tracker change, and serves the per-site script with a 60-second max-age",
    },
    RollingEndpoint {
        host: "pagead2.googlesyndication.com",
        path: PathRule::Exact("/pagead/js/adsbygoogle.js"),
        reason: "the AdSense loader is a versionless Google-managed script updated in place, and Google's ad tag guidance says never to serve its libraries from your own server",
    },
    RollingEndpoint {
        host: "securepubads.g.doubleclick.net",
        path: PathRule::Exact("/tag/js/gpt.js"),
        reason: "Google Publisher Tag is updated in place, and its best-practices guide says never to serve gpt.js or the libraries it loads from your own server",
    },
    RollingEndpoint {
        host: "www.googleadservices.com",
        path: PathRule::Exact("/pagead/conversion.js"),
        reason: "Google Ads conversion tags are versionless Google-managed scripts updated in place",
    },
    RollingEndpoint {
        host: "www.googleadservices.com",
        path: PathRule::Exact("/pagead/conversion_async.js"),
        reason: "Google Ads conversion tags are versionless Google-managed scripts updated in place",
    },
    RollingEndpoint {
        host: "www.google.com",
        path: PathRule::Exact("/recaptcha/api.js"),
        reason: "the reCAPTCHA loader is a versionless endpoint that fetches Google's current release and is served with a five-minute max-age",
    },
    RollingEndpoint {
        host: "www.google.com",
        path: PathRule::Exact("/recaptcha/enterprise.js"),
        reason: "the reCAPTCHA loader is a versionless endpoint that fetches Google's current release and is served with a five-minute max-age",
    },
    RollingEndpoint {
        host: "www.recaptcha.net",
        path: PathRule::Exact("/recaptcha/api.js"),
        reason: "the reCAPTCHA loader is a versionless endpoint that fetches Google's current release and is served with a five-minute max-age",
    },
    RollingEndpoint {
        host: "www.recaptcha.net",
        path: PathRule::Exact("/recaptcha/enterprise.js"),
        reason: "the reCAPTCHA loader is a versionless endpoint that fetches Google's current release and is served with a five-minute max-age",
    },
    RollingEndpoint {
        host: "challenges.cloudflare.com",
        path: PathRule::Exact("/turnstile/v0/api.js"),
        reason: "Cloudflare's Turnstile docs require fetching api.js from that exact URL because proxying or caching it breaks future updates",
    },
    RollingEndpoint {
        host: "static.cloudflareinsights.com",
        path: PathRule::Exact("/beacon.min.js"),
        reason: "Cloudflare Web Analytics serves its beacon from a versionless URL whose ETag is the release date, so the bytes roll with each release",
    },
    RollingEndpoint {
        host: "connect.facebook.net",
        path: PathRule::Suffix("/fbevents.js"),
        reason: "the Meta Pixel base script is a versionless locale-prefixed endpoint updated in place and served with a 20-minute max-age",
    },
    RollingEndpoint {
        host: "cdn.usefathom.com",
        path: PathRule::Exact("/script.js"),
        reason: "Fathom's tracker is a versionless endpoint served with max-age=0 and updated in place",
    },
    RollingEndpoint {
        host: "static.hotjar.com",
        path: PathRule::Prefix("/c/hotjar-"),
        reason: "Hotjar's per-site loader is generated for the site id and served with a 60-second max-age",
    },
    RollingEndpoint {
        host: "js.hcaptcha.com",
        path: PathRule::Exact("/1/api.js"),
        reason: "the hCaptcha loader is a versionless endpoint served with a five-minute max-age that fetches the current release",
    },
    RollingEndpoint {
        host: "analytics.tiktok.com",
        path: PathRule::Exact("/i18n/pixel/events.js"),
        reason: "the TikTok pixel base script is a versionless endpoint served with no-store",
    },
    RollingEndpoint {
        host: "static.ads-twitter.com",
        path: PathRule::Exact("/uwt.js"),
        reason: "the X universal website tag is a versionless endpoint served with no-cache",
    },
    RollingEndpoint {
        host: "snap.licdn.com",
        path: PathRule::Exact("/li.lms-analytics/insight.min.js"),
        reason: "the LinkedIn Insight Tag is a versionless endpoint updated in place",
    },
    RollingEndpoint {
        // Host-wide on purpose: js.sentry-cdn.com serves only per-DSN loaders,
        // so there is no pinnable artifact on it. Sentry's versioned bundles
        // live on browser.sentry-cdn.com, which stays flagged.
        host: "js.sentry-cdn.com",
        path: PathRule::AnyPath,
        reason: "Sentry's loader script is keyed by DSN public key and fetches the current SDK release, so its bytes change with each release",
    },
    RollingEndpoint {
        host: "client.crisp.chat",
        path: PathRule::Exact("/l.js"),
        reason: "Crisp's chat loader is a versionless endpoint updated in place",
    },
    RollingEndpoint {
        host: "cdn.mxpnl.com",
        path: PathRule::Exact("/libs/mixpanel-2-latest.min.js"),
        reason: "Mixpanel's -latest bundle is by name the rolling current release",
    },
    RollingEndpoint {
        host: "www.clarity.ms",
        path: PathRule::Prefix("/tag/"),
        reason: "Microsoft Clarity's loader is generated per project id and updated in place",
    },
    RollingEndpoint {
        // Effectively host-wide on purpose: every path on js.hs-scripts.com is
        // `/<portal id>.js`, generated for that portal, so there is no
        // pinnable artifact on it. HubSpot's versioned assets are served from
        // other hosts, which stay flagged.
        host: "js.hs-scripts.com",
        path: PathRule::Suffix(".js"),
        reason: "HubSpot's tracking loader is generated per portal id and updated in place",
    },
    RollingEndpoint {
        host: "cdn.segment.com",
        path: PathRule::Prefix("/analytics.js/v1/"),
        reason: "Segment's analytics.js endpoint is generated per write key and served no-store",
    },
];

fn rolling_endpoint(url: &str) -> Option<&'static RollingEndpoint> {
    let parsed = if url.starts_with("//") {
        url::Url::parse(&format!("https:{url}"))
    } else {
        url::Url::parse(url)
    }
    .ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let path = parsed.path();
    ROLLING_ENDPOINTS
        .iter()
        .find(|endpoint| endpoint.host == host && endpoint.path.matches(path))
}

/// Explain why a specific known endpoint is dynamic or rolling and therefore
/// cannot safely use one fixed integrity hash.
pub(super) fn sri_exclusion_reason(url: &str) -> Option<&'static str> {
    rolling_endpoint(url).map(|endpoint| endpoint.reason)
}

/// One example URL per rolling endpoint entry, with a fragment of the
/// reason the evidence must carry. The parent module's check-level test reads
/// it too, so it lives beside the table it documents.
#[cfg(test)]
pub(super) const ROLLING_EXAMPLES: &[(&str, &str)] = &[
    (
        "https://fonts.googleapis.com/css2?family=Inter",
        "Google Fonts",
    ),
    ("https://js.stripe.com/v3", "Stripe.js v3"),
    ("https://js.stripe.com/v3/", "Stripe.js v3"),
    ("https://js.stripe.com/basil/stripe.js", "named Stripe.js"),
    (
        "https://www.googletagmanager.com/gtm.js?id=GTM-1",
        "Google Tag Manager",
    ),
    (
        "https://www.googletagmanager.com/gtag/js?id=G-1",
        "Google Tag Manager",
    ),
    (
        "https://www.google-analytics.com/analytics.js",
        "Google Analytics",
    ),
    ("https://www.google-analytics.com/ga.js", "Google Analytics"),
    (
        "https://www.paypal.com/sdk/js?client-id=x",
        "PayPal JavaScript SDK",
    ),
    (
        "https://www.paypalobjects.com/api/checkout.js",
        "legacy PayPal",
    ),
    (
        "https://plausible.io/js/pa-EAp4w9JyHms_SbnZZLuEs.js",
        "Plausible",
    ),
    (
        "https://plausible.io/js/script.outbound-links.js",
        "Plausible",
    ),
    (
        "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=ca-pub-1",
        "AdSense",
    ),
    (
        "https://securepubads.g.doubleclick.net/tag/js/gpt.js",
        "Google Publisher Tag",
    ),
    (
        "https://www.googleadservices.com/pagead/conversion.js",
        "Google Ads conversion",
    ),
    (
        "https://www.googleadservices.com/pagead/conversion_async.js",
        "Google Ads conversion",
    ),
    (
        "https://www.google.com/recaptcha/api.js?render=explicit",
        "reCAPTCHA",
    ),
    (
        "https://www.google.com/recaptcha/enterprise.js",
        "reCAPTCHA",
    ),
    ("https://www.recaptcha.net/recaptcha/api.js", "reCAPTCHA"),
    (
        "https://www.recaptcha.net/recaptcha/enterprise.js",
        "reCAPTCHA",
    ),
    (
        "https://challenges.cloudflare.com/turnstile/v0/api.js",
        "Turnstile",
    ),
    (
        "https://static.cloudflareinsights.com/beacon.min.js",
        "Cloudflare Web Analytics",
    ),
    (
        "https://connect.facebook.net/en_US/fbevents.js",
        "Meta Pixel",
    ),
    ("https://cdn.usefathom.com/script.js", "Fathom"),
    (
        "https://static.hotjar.com/c/hotjar-123456.js?sv=6",
        "Hotjar",
    ),
    ("https://js.hcaptcha.com/1/api.js", "hCaptcha"),
    (
        "https://analytics.tiktok.com/i18n/pixel/events.js?sdkid=ABC",
        "TikTok",
    ),
    (
        "https://static.ads-twitter.com/uwt.js",
        "X universal website tag",
    ),
    (
        "https://snap.licdn.com/li.lms-analytics/insight.min.js",
        "LinkedIn Insight",
    ),
    (
        "https://js.sentry-cdn.com/0123456789abcdef.min.js",
        "Sentry",
    ),
    ("https://client.crisp.chat/l.js", "Crisp"),
    (
        "https://cdn.mxpnl.com/libs/mixpanel-2-latest.min.js",
        "Mixpanel",
    ),
    ("https://www.clarity.ms/tag/abcdefghij", "Clarity"),
    ("https://js.hs-scripts.com/2620402.js", "HubSpot"),
    (
        "https://cdn.segment.com/analytics.js/v1/WRITEKEY/analytics.min.js",
        "Segment",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rolling_endpoint_entry_has_an_example() {
        for endpoint in ROLLING_ENDPOINTS {
            let covered = ROLLING_EXAMPLES.iter().any(|(url, _)| {
                rolling_endpoint(url).is_some_and(|hit| std::ptr::eq(hit, endpoint))
            });
            assert!(
                covered,
                "rolling endpoint {} needs an example in ROLLING_EXAMPLES",
                endpoint.host
            );
        }
    }

    #[test]
    fn rolling_endpoint_hosts_match_case_insensitively_and_on_protocol_relative_urls() {
        assert!(sri_exclusion_reason("//Plausible.IO/js/script.js").is_some());
        assert!(sri_exclusion_reason("https://plausible.io/js").is_none());
        assert!(sri_exclusion_reason("https://js.stripe.com/v3-legacy/x.js").is_none());
    }
}
