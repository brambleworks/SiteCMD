//! Polish signals for uncustomized framework and deployment defaults.

use super::{PolishContext, PolishResult, SignalCategory, SignalWeight};
use regex::Regex;
use std::sync::LazyLock;

const CATEGORY: SignalCategory = SignalCategory::FrameworkDefaults;

const HOSTING_SUBDOMAINS: &[&str] = &[
    ".vercel.app",
    ".netlify.app",
    ".netlify.com",
    ".pages.dev", // Cloudflare Pages
    ".web.app",   // Firebase
    ".firebaseapp.com",
    ".herokuapp.com",
    ".fly.dev",
    ".railway.app",
    ".render.com",
    ".surge.sh",
    ".github.io",
    ".gitlab.io",
    ".onrender.com",
    ".deno.dev",
    ".workers.dev", // Cloudflare Workers
    ".replit.app",
    ".repl.co",
    ".glitch.me",
    ".stackblitz.io",
];

/// Known CRA default meta description
static CRA_META_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)content\s*=\s*["']Web site created using create-react-app["']"#)
        .expect("CRA meta regex")
});

/// Default root div with nothing else
static BARE_ROOT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<div\s+id\s*=\s*["'](?:root|app|__next)["']\s*>\s*</div>"#)
        .expect("bare root regex")
});

/// Flag known shared-hosting subdomains for review (LowMedium, 5).
pub fn default_deployment_subdomain(ctx: &PolishContext) -> PolishResult {
    let host = ctx.url.host_str().unwrap_or("");

    for subdomain in HOSTING_SUBDOMAINS {
        if host.ends_with(subdomain) {
            return PolishResult::fired(
                "default-deployment-subdomain",
                "Hosting on Shared Platform Subdomain",
                SignalWeight::LowMedium,
                CATEGORY,
                format!(
                    "Running on the platform's shared subdomain: {}. Fine if intentional (project pages, internal tools); a custom domain reads as launched to visitors.",
                    host
                ),
                serde_json::json!({
                    "hostname": host,
                    "platform": subdomain.trim_start_matches('.'),
                }),
            );
        }
    }

    PolishResult::clear(
        "default-deployment-subdomain",
        "Hosting on Shared Platform Subdomain",
        SignalWeight::LowMedium,
        CATEGORY,
    )
}

/// Flag two or more framework-boilerplate markers (Low, 3).
pub fn boilerplate_html(ctx: &PolishContext) -> PolishResult {
    let mut markers: Vec<String> = Vec::new();

    // CRA default meta description
    if CRA_META_RE.is_match(&ctx.html) {
        markers.push("CRA default meta description".to_string());
    }

    // Bare root/app div
    if BARE_ROOT_RE.is_match(&ctx.html) {
        markers.push("Empty root div (SPA scaffold)".to_string());
    }

    // Vite default HTML comment
    if ctx.html.contains("<!-- Vite") || ctx.html.contains("<!--vite") {
        markers.push("Vite HTML comment".to_string());
    }

    // Default noscript messages
    let lower = ctx.html_lower();
    if lower.contains("you need to enable javascript to run this app") {
        markers.push("CRA default noscript message".to_string());
    }

    // %PUBLIC_URL% placeholder (CRA build artifact)
    if ctx.html.contains("%PUBLIC_URL%") {
        markers.push("Unreplaced %PUBLIC_URL% template variable".to_string());
    }

    // Default viewport with no other meta tags
    if lower.contains(r#"content="width=device-width, initial-scale=1.0""#)
        && !lower.contains("og:title")
        && !lower.contains("description")
    {
        markers.push("Minimal viewport-only meta tags".to_string());
    }

    if markers.len() >= 2 {
        PolishResult::fired(
            "boilerplate-html",
            "Default Scaffold Markers Present",
            SignalWeight::Low,
            CATEGORY,
            format!("{} scaffold markers: {}", markers.len(), markers.join(", ")),
            serde_json::json!({ "markers": markers }),
        )
    } else {
        PolishResult::clear(
            "boilerplate-html",
            "Default Scaffold Markers Present",
            SignalWeight::Low,
            CATEGORY,
        )
    }
}

/// Detect framework-specific default error-page markers.
pub fn default_error_page(ctx: &PolishContext) -> PolishResult {
    // Generic 404 wording also appears in legitimate help and search content.
    let lower = ctx.html_lower();

    // Strict markers: copy lifted verbatim from Next.js / Rails default
    // error templates with enough context to avoid matching help text.
    let markers = [
        // Next.js client-side error component (very specific)
        "application error: a client-side exception has occurred",
        // Next.js dev-mode error overlay header
        "unhandled runtime error",
        // Rails default error page header (NOT the "looking for" phrase)
        "we're sorry, but something went wrong",
        // Express default error stack-trace marker
        "cannot get /",
    ];

    for marker in &markers {
        if lower.contains(marker) {
            return PolishResult::fired(
                "default-error-page",
                "Default Error Page",
                SignalWeight::Low,
                CATEGORY,
                "Framework default error page text detected on current page".to_string(),
                serde_json::json!({ "marker": marker }),
            );
        }
    }

    PolishResult::clear(
        "default-error-page",
        "Default Error Page",
        SignalWeight::Low,
        CATEGORY,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_url(html: &str, url: &str) -> PolishContext {
        PolishContext {
            url: url::Url::parse(url).unwrap(),
            html: html.to_string(),
            css: String::new(),
            html_lower_cache: std::sync::OnceLock::new(),
        }
    }

    fn ctx(html: &str) -> PolishContext {
        ctx_url(html, "https://example.com")
    }

    #[test]
    fn subdomain_fires_with_vercel() {
        let result = default_deployment_subdomain(&ctx_url("", "https://my-app.vercel.app"));
        assert!(result.fired, "Should fire on vercel.app subdomain");
    }

    #[test]
    fn subdomain_fires_with_netlify() {
        let result = default_deployment_subdomain(&ctx_url("", "https://cool-site.netlify.app"));
        assert!(result.fired, "Should fire on netlify.app subdomain");
    }

    #[test]
    fn subdomain_clear_with_custom_domain() {
        let result = default_deployment_subdomain(&ctx_url("", "https://myproduct.com"));
        assert!(!result.fired, "Should not fire with custom domain");
    }

    #[test]
    fn boilerplate_fires_with_cra_markers() {
        let html = r#"<html><head><meta content="Web site created using create-react-app"></head>
                       <body><noscript>You need to enable JavaScript to run this app.</noscript>
                       <div id="root"></div></body></html>"#;
        let result = boilerplate_html(&ctx(html));
        assert!(result.fired, "Should fire with CRA boilerplate markers");
    }

    #[test]
    fn boilerplate_clear_with_custom_html() {
        let html = r#"<html><head><meta name="description" content="My custom app"><meta property="og:title" content="Test"></head>
                       <body><div id="root"><main>Content</main></div></body></html>"#;
        let result = boilerplate_html(&ctx(html));
        assert!(!result.fired, "Should not fire with customized HTML");
    }

    #[test]
    fn error_page_fires_with_nextjs_runtime_error() {
        let html = "<html><body><h2>Unhandled Runtime Error</h2><p>...</p></body></html>";
        let result = default_error_page(&ctx(html));
        assert!(
            result.fired,
            "Should fire with the Next.js runtime-error overlay header"
        );
    }

    #[test]
    fn error_page_clear_with_help_text_mentioning_page_not_found() {
        let html = r#"<html><body>
            <h1>FAQ</h1>
            <p>If you get a 404 - page not found error, please contact support.</p>
            <p>The page you were looking for doesn't exist? Check our sitemap.</p>
        </body></html>"#;
        let result = default_error_page(&ctx(html));
        assert!(
            !result.fired,
            "Help text mentioning 404 / page not found must not fire"
        );
    }

    #[test]
    fn error_page_clear_with_normal_page() {
        let html = "<html><body><h1>Welcome</h1><p>Content here.</p></body></html>";
        let result = default_error_page(&ctx(html));
        assert!(!result.fired, "Should not fire with normal page content");
    }
}
