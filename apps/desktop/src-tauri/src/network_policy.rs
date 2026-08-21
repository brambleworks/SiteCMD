//! Shared SSRF validation for scans, sitemaps, webhooks, and redirects.

use std::net::{IpAddr, Ipv6Addr, ToSocketAddrs};

#[derive(Debug, Clone, Copy)]
pub enum UrlPolicy {
    /// User-initiated scans may target explicit loopback dev servers.
    Scan,
    /// External callbacks/webhooks must never target local or private networks.
    ExternalCallback,
    /// Redirects inherit the local-dev allowance of the originating client.
    Redirect { allow_local_dev: bool },
}

impl UrlPolicy {
    fn allow_local_dev(self) -> bool {
        match self {
            Self::Scan => true,
            Self::ExternalCallback => false,
            Self::Redirect { allow_local_dev } => allow_local_dev,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Scan => "URL",
            Self::ExternalCallback => "External callback URL",
            Self::Redirect { .. } => "Redirect target",
        }
    }
}

pub fn validate_url_blocking(url: &str, policy: UrlPolicy) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL '{}': {}", url, e))?;
    validate_url_target_blocking(&parsed, policy)
}

pub async fn validate_url(url: &str, policy: UrlPolicy) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL '{}': {}", url, e))?;
    validate_url_target(&parsed, policy).await
}

/// Whether a validated scan origin is explicitly local and may retain the
/// scan policy's loopback exception across redirects. Public hostnames never
/// receive this exception, even if they later redirect to a loopback literal.
pub(crate) fn scan_origin_allows_local_dev(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(domain)) => {
            is_local_dev_domain(&domain.trim_end_matches('.').to_ascii_lowercase())
        }
        None => false,
    }
}

async fn validate_url_target(url: &url::Url, policy: UrlPolicy) -> Result<(), String> {
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "Unsupported URL scheme '{}'. Only http:// and https:// URLs are allowed.",
                scheme
            ))
        }
    }

    let host = url
        .host()
        .ok_or_else(|| format!("{} must include a host.", policy.label()))?;

    match host {
        url::Host::Ipv4(ip) => validate_ip_target(IpAddr::V4(ip), policy),
        url::Host::Ipv6(ip) => validate_ip_target(IpAddr::V6(ip), policy),
        url::Host::Domain(domain) => validate_domain_target_async(domain, url, policy).await,
    }
}

pub fn validate_url_target_blocking(url: &url::Url, policy: UrlPolicy) -> Result<(), String> {
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "Unsupported URL scheme '{}'. Only http:// and https:// URLs are allowed.",
                scheme
            ))
        }
    }

    let host = url
        .host()
        .ok_or_else(|| format!("{} must include a host.", policy.label()))?;

    match host {
        url::Host::Ipv4(ip) => validate_ip_target(IpAddr::V4(ip), policy),
        url::Host::Ipv6(ip) => validate_ip_target(IpAddr::V6(ip), policy),
        url::Host::Domain(domain) => validate_domain_target(domain, url, policy),
    }
}

/// Validate redirect targets without blocking DNS on the async runtime.
/// IP literals are checked here; domain addresses are checked by
/// `CachedDnsResolver` at connection time to prevent DNS rebinding.
pub fn validate_redirect_target_nonblocking(
    url: &url::Url,
    policy: UrlPolicy,
) -> Result<(), String> {
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "Unsupported URL scheme '{}'. Only http:// and https:// URLs are allowed.",
                scheme
            ))
        }
    }

    let host = url
        .host()
        .ok_or_else(|| format!("{} must include a host.", policy.label()))?;

    match host {
        url::Host::Ipv4(ip) => validate_ip_target(IpAddr::V4(ip), policy),
        url::Host::Ipv6(ip) => validate_ip_target(IpAddr::V6(ip), policy),
        url::Host::Domain(domain) => {
            let normalized = domain.trim_end_matches('.').to_ascii_lowercase();
            if normalized == "metadata.google.internal" {
                return Err("Cannot access cloud metadata endpoints.".to_string());
            }
            if is_local_dev_domain(&normalized) && !policy.allow_local_dev() {
                return Err("External callback URLs cannot target localhost.".to_string());
            }
            Ok(())
        }
    }
}

/// Validate a page-controlled subresource before connect-time DNS checks.
pub fn validate_page_subresource_target(
    url: &url::Url,
    allow_local_dev: bool,
) -> Result<(), String> {
    validate_redirect_target_nonblocking(url, UrlPolicy::Redirect { allow_local_dev })
}

fn validate_domain_target(domain: &str, url: &url::Url, policy: UrlPolicy) -> Result<(), String> {
    let normalized = domain.trim_end_matches('.').to_ascii_lowercase();
    if normalized == "metadata.google.internal" {
        return Err("Cannot access cloud metadata endpoints.".to_string());
    }
    if is_local_dev_domain(&normalized) {
        if policy.allow_local_dev() {
            return Ok(());
        }
        return Err("External callback URLs cannot target localhost.".to_string());
    }

    let port = url.port_or_known_default().unwrap_or(80);
    let resolved = (normalized.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("Could not resolve URL host '{}': {}", domain, e))?;

    let mut saw_addr = false;
    for addr in resolved {
        saw_addr = true;
        validate_resolved_domain_ip_target(domain, addr.ip(), policy)?;
    }
    if !saw_addr {
        return Err(format!("Could not resolve URL host '{}'.", domain));
    }
    Ok(())
}

async fn validate_domain_target_async(
    domain: &str,
    url: &url::Url,
    policy: UrlPolicy,
) -> Result<(), String> {
    let normalized = domain.trim_end_matches('.').to_ascii_lowercase();
    if normalized == "metadata.google.internal" {
        return Err("Cannot access cloud metadata endpoints.".to_string());
    }
    if is_local_dev_domain(&normalized) {
        if policy.allow_local_dev() {
            return Ok(());
        }
        return Err("External callback URLs cannot target localhost.".to_string());
    }

    let port = url.port_or_known_default().unwrap_or(80);
    let resolved = tokio::net::lookup_host((normalized.as_str(), port))
        .await
        .map_err(|e| format!("Could not resolve URL host '{}': {}", domain, e))?;

    let mut saw_addr = false;
    for addr in resolved {
        saw_addr = true;
        validate_resolved_domain_ip_target(domain, addr.ip(), policy)?;
    }
    if !saw_addr {
        return Err(format!("Could not resolve URL host '{}'.", domain));
    }
    Ok(())
}

fn is_local_dev_domain(domain: &str) -> bool {
    domain == "localhost" || domain.ends_with(".localhost")
}

fn validate_ip_target(ip: IpAddr, policy: UrlPolicy) -> Result<(), String> {
    if policy.allow_local_dev() && is_loopback_ip(ip) {
        return Ok(());
    }
    if is_private_or_internal_ip(ip) {
        return Err(format!(
            "Cannot access private/internal IP address '{}'.",
            ip
        ));
    }
    Ok(())
}

pub(crate) fn validate_resolved_domain_ip_target(
    domain: &str,
    ip: IpAddr,
    policy: UrlPolicy,
) -> Result<(), String> {
    let normalized = domain.trim_end_matches('.').to_ascii_lowercase();
    if is_local_dev_domain(&normalized) {
        if !policy.allow_local_dev() {
            return Err("External callback URLs cannot target localhost.".to_string());
        }
        if is_loopback_ip(ip) {
            return Ok(());
        }
        return Err(format!(
            "Cannot access non-loopback IP address '{}' through a localhost hostname.",
            ip
        ));
    }

    if is_loopback_ip(ip) {
        return Err(format!(
            "Cannot access loopback IP address '{}' through a non-localhost hostname.",
            ip
        ));
    }
    validate_ip_target(ip, policy)
}

fn is_loopback_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback(),
    }
}

fn is_private_or_internal_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified()
                // 0.0.0.0/8 "this network": only 0.0.0.0 is `is_unspecified`,
                // but some stacks route the rest of the block to loopback.
                || a == 0
                // 100.64.0.0/10 carrier-grade NAT (RFC 6598) - a common gateway
                // range that is internal, not publicly routable.
                || (a == 100 && (64..=127).contains(&b))
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || is_unique_local_ipv6(ip)
                || is_unicast_link_local_ipv6(ip)
                || ip
                    .to_ipv4_mapped()
                    .map(|mapped| is_private_or_internal_ip(IpAddr::V4(mapped)))
                    .unwrap_or(false)
        }
    }
}

fn is_unique_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_unicast_link_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

#[cfg(test)]
mod tests {
    use super::{
        scan_origin_allows_local_dev, validate_ip_target, validate_redirect_target_nonblocking,
        validate_resolved_domain_ip_target, validate_url_blocking, UrlPolicy,
    };
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn scan_validation_allows_explicit_local_dev_loopback() {
        assert!(validate_url_blocking("http://localhost:5173", UrlPolicy::Scan).is_ok());
        assert!(validate_url_blocking("http://127.0.0.1:5173", UrlPolicy::Scan).is_ok());
        assert!(validate_url_blocking("http://[::1]:5173", UrlPolicy::Scan).is_ok());
    }

    #[test]
    fn only_explicit_local_scan_origins_inherit_the_loopback_exception() {
        for url in [
            "http://localhost:5173",
            "http://app.localhost:5173",
            "http://127.0.0.2:5173",
            "http://[::1]:5173",
        ] {
            let parsed = url::Url::parse(url).expect("parse explicit local URL");
            assert!(
                scan_origin_allows_local_dev(&parsed),
                "{url} should inherit the local-dev exception"
            );
        }

        for url in [
            "https://example.com",
            "https://localhost.run",
            "https://127.0.0.1.example.com",
        ] {
            let parsed = url::Url::parse(url).expect("parse public URL");
            assert!(
                !scan_origin_allows_local_dev(&parsed),
                "{url} must not inherit the local-dev exception"
            );
        }
    }

    #[test]
    fn scan_validation_rejects_private_non_loopback_targets() {
        assert!(validate_url_blocking("http://192.168.1.10", UrlPolicy::Scan).is_err());
        assert!(validate_url_blocking("http://10.0.0.5", UrlPolicy::Scan).is_err());
        assert!(validate_url_blocking("http://169.254.169.254", UrlPolicy::Scan).is_err());
    }

    #[test]
    fn external_callback_validation_rejects_local_targets() {
        assert!(
            validate_url_blocking("http://localhost:5173", UrlPolicy::ExternalCallback).is_err()
        );
        assert!(
            validate_url_blocking("http://127.0.0.1:5173", UrlPolicy::ExternalCallback).is_err()
        );
        assert!(validate_url_blocking("http://[::1]:5173", UrlPolicy::ExternalCallback).is_err());
    }

    fn ipv4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[test]
    fn ip_target_rejects_rfc1918_under_external_callback_policy() {
        assert!(validate_ip_target(ipv4(10, 0, 0, 1), UrlPolicy::ExternalCallback).is_err());
        assert!(validate_ip_target(ipv4(172, 16, 0, 1), UrlPolicy::ExternalCallback).is_err());
        assert!(validate_ip_target(ipv4(192, 168, 1, 1), UrlPolicy::ExternalCallback).is_err());
    }

    #[test]
    fn ip_target_rejects_link_local_and_metadata_addresses() {
        // 169.254.169.254 is the cloud metadata service that DNS rebinding
        // attacks classically target.
        assert!(validate_ip_target(ipv4(169, 254, 169, 254), UrlPolicy::Scan).is_err());
        assert!(validate_ip_target(ipv4(169, 254, 0, 1), UrlPolicy::ExternalCallback).is_err());
    }

    #[test]
    fn ip_target_rejects_cgnat_and_this_network_ranges() {
        // RFC 6598 carrier-grade NAT (100.64/10) and RFC 791 "this network"
        // (0.0.0.0/8) are internal/non-routable and must be refused as targets.
        assert!(validate_ip_target(ipv4(100, 64, 0, 1), UrlPolicy::Scan).is_err());
        assert!(validate_ip_target(ipv4(100, 127, 255, 255), UrlPolicy::ExternalCallback).is_err());
        assert!(validate_ip_target(ipv4(0, 1, 2, 3), UrlPolicy::Scan).is_err());
        // Public addresses adjacent to the CGNAT block stay allowed.
        assert!(validate_ip_target(ipv4(100, 63, 0, 1), UrlPolicy::ExternalCallback).is_ok());
        assert!(validate_ip_target(ipv4(100, 128, 0, 1), UrlPolicy::ExternalCallback).is_ok());
    }

    #[test]
    fn ip_target_rejects_private_addresses_even_under_scan_policy() {
        assert!(validate_ip_target(ipv4(10, 0, 0, 5), UrlPolicy::Scan).is_err());
        assert!(validate_ip_target(ipv4(192, 168, 1, 10), UrlPolicy::Scan).is_err());
    }

    #[test]
    fn ip_target_rejects_ipv6_unique_local_and_link_local() {
        // fc00::/7 (unique local) and fe80::/10 (link local) plus loopback::1.
        assert!(validate_ip_target(
            IpAddr::V6("fc00::1".parse::<Ipv6Addr>().unwrap()),
            UrlPolicy::ExternalCallback
        )
        .is_err());
        assert!(validate_ip_target(
            IpAddr::V6("fe80::1".parse::<Ipv6Addr>().unwrap()),
            UrlPolicy::ExternalCallback
        )
        .is_err());
        assert!(
            validate_ip_target(IpAddr::V6(Ipv6Addr::LOCALHOST), UrlPolicy::ExternalCallback)
                .is_err()
        );
    }

    #[test]
    fn ip_target_rejects_ipv4_mapped_ipv6_private_addresses() {
        let mapped: Ipv6Addr = "::ffff:10.0.0.1".parse().unwrap();
        assert!(
            validate_ip_target(IpAddr::V6(mapped), UrlPolicy::ExternalCallback).is_err(),
            "v4-mapped private address must be refused so an IPv6 query path cannot bypass the v4 private-range check"
        );
    }

    #[test]
    fn ip_target_accepts_loopback_under_scan_but_rejects_under_external_callback() {
        let lo = ipv4(127, 0, 0, 1);
        assert!(validate_ip_target(lo, UrlPolicy::Scan).is_ok());
        assert!(validate_ip_target(lo, UrlPolicy::ExternalCallback).is_err());
    }

    #[test]
    fn security_regression_domain_resolution_rejects_loopback_under_scan_policy() {
        let lo = ipv4(127, 0, 0, 1);
        assert!(validate_resolved_domain_ip_target("rebind.example", lo, UrlPolicy::Scan).is_err());
    }

    #[test]
    fn local_dev_domain_resolution_must_stay_loopback() {
        let lo = ipv4(127, 0, 0, 1);
        let private = ipv4(192, 168, 1, 10);
        assert!(validate_resolved_domain_ip_target("localhost", lo, UrlPolicy::Scan).is_ok());
        assert!(validate_resolved_domain_ip_target("localhost", private, UrlPolicy::Scan).is_err());
    }

    fn redirect_nonblocking(url: &str, policy: UrlPolicy) -> Result<(), String> {
        validate_redirect_target_nonblocking(&url::Url::parse(url).expect("parse url"), policy)
    }

    #[test]
    fn redirect_policy_rejects_ip_literal_internal_targets_inline() {
        // reqwest dials IP-literal redirect targets directly, bypassing the DNS
        // resolver, so the redirect policy must reject them inline.
        let policy = UrlPolicy::Redirect {
            allow_local_dev: false,
        };
        assert!(redirect_nonblocking("http://169.254.169.254/latest/meta-data/", policy).is_err());
        assert!(redirect_nonblocking("http://10.0.0.5/admin", policy).is_err());
        assert!(redirect_nonblocking("http://[::1]/", policy).is_err());
        assert!(redirect_nonblocking("http://[::ffff:127.0.0.1]/", policy).is_err());
    }

    fn subresource(url: &str, allow_local_dev: bool) -> Result<(), String> {
        super::validate_page_subresource_target(
            &url::Url::parse(url).expect("parse url"),
            allow_local_dev,
        )
    }

    #[test]
    fn page_subresource_rejects_ip_literal_internal_targets() {
        assert!(subresource("http://169.254.169.254/latest/meta-data/", false).is_err());
        assert!(subresource("http://10.0.0.5/admin", false).is_err());
        assert!(subresource("http://192.168.1.1/reboot", false).is_err());
        assert!(subresource("http://127.0.0.1:11434/api", false).is_err());
        assert!(subresource("http://[::1]/", false).is_err());
        assert!(subresource("http://[::ffff:127.0.0.1]/", false).is_err());
    }

    #[test]
    fn page_subresource_allows_public_targets() {
        assert!(subresource("https://cdn.example.com/style.css", false).is_ok());
        assert!(subresource("https://fonts.example.org/f.css", false).is_ok());
    }

    #[test]
    fn page_subresource_allows_loopback_only_when_scanning_local_dev() {
        // Scanning a localhost dev server must still fetch its own same-origin
        // assets (allow_local_dev=true), but a public scan (false) must not.
        assert!(subresource("http://127.0.0.1:5173/app.css", true).is_ok());
        assert!(subresource("http://127.0.0.1:5173/app.css", false).is_err());
        // Even under local-dev, a private (non-loopback) LAN target stays blocked.
        assert!(subresource("http://192.168.1.10/x.css", true).is_err());
    }

    #[test]
    fn page_subresource_rejects_non_http_schemes() {
        assert!(subresource("file:///etc/passwd", false).is_err());
        assert!(subresource("ftp://internal/x", false).is_err());
    }

    #[test]
    fn redirect_policy_defers_domain_targets_without_blocking_dns() {
        let policy = UrlPolicy::Redirect {
            allow_local_dev: false,
        };
        assert!(
            redirect_nonblocking("https://this-host-does-not-resolve.invalid/", policy).is_ok()
        );
        assert!(redirect_nonblocking("https://example.com/", policy).is_ok());
        assert!(redirect_nonblocking("http://metadata.google.internal/", policy).is_err());
        assert!(redirect_nonblocking("http://localhost/", policy).is_err());
    }
}
