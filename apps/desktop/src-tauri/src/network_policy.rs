//! Shared SSRF validation for scans, sitemaps, webhooks, and redirects.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

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

/// How local a scan target is, computed once per scan and threaded through
/// the pipeline instead of re-derived from the URL at every consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalOrigin {
    /// `localhost`, `127.0.0.0/8`, `[::1]`: certificate checks are skipped
    /// and the loopback redirect exception applies.
    Loopback,
    /// A `.localhost` subdomain: loopback by RFC 6761 but resolved by the OS,
    /// so the redirect exception applies while certificate checks stay on.
    LocalhostDomain,
    /// `*.local` or `0.0.0.0`: a local environment label, no policy exception.
    LocalNetworkName,
    Public,
}

impl LocalOrigin {
    /// Two deliberate widenings relative to the predicates this replaces:
    /// every `127.0.0.0/8` literal classifies as `Loopback` (the old
    /// `is_strict_localhost` accepted only `127.0.0.1`), and a trailing-dot
    /// FQDN form (`localhost.`) normalizes the same way
    /// `is_local_dev_domain` already normalizes it elsewhere in this file,
    /// so it also classifies as `Loopback` even though `core::localhost`'s
    /// predicates compare `host_str()` without trimming the trailing dot.
    pub fn classify(url: &url::Url) -> Self {
        match url.host() {
            Some(url::Host::Ipv4(ip)) if ip.is_loopback() => Self::Loopback,
            Some(url::Host::Ipv6(ip)) if ip.is_loopback() => Self::Loopback,
            Some(url::Host::Ipv4(ip)) if ip.is_unspecified() => Self::LocalNetworkName,
            Some(url::Host::Domain(domain)) => {
                let domain = domain.trim_end_matches('.').to_ascii_lowercase();
                if domain == "localhost" {
                    Self::Loopback
                } else if domain.ends_with(".localhost") {
                    Self::LocalhostDomain
                } else if domain.ends_with(".local") {
                    Self::LocalNetworkName
                } else {
                    Self::Public
                }
            }
            _ => Self::Public,
        }
    }

    /// Replaces `core::localhost::is_strict_localhost`.
    pub fn is_strict_loopback(self) -> bool {
        matches!(self, Self::Loopback)
    }

    /// Replaces `network_policy::scan_origin_allows_local_dev`.
    pub fn allows_local_dev(self) -> bool {
        matches!(self, Self::Loopback | Self::LocalhostDomain)
    }

    /// Replaces `core::localhost::is_localhost`.
    pub fn is_local_environment(self) -> bool {
        !matches!(self, Self::Public)
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
            let [a, b, c, _] = ip.octets();
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
                // 224.0.0.0/4 multicast and 240.0.0.0/4 reserved: never a host.
                || ip.is_multicast()
                || (a & 0xf0) == 240
                // 198.18.0.0/15 benchmarking (RFC 2544) and 192.0.0.0/24 IETF
                // protocol assignments (RFC 6890).
                || (a == 198 && (b == 18 || b == 19))
                || (a == 192 && b == 0 && c == 0)
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || is_unique_local_ipv6(ip)
                || is_unicast_link_local_ipv6(ip)
                || is_site_local_ipv6(ip)
                || is_local_use_nat64(ip)
                || embedded_ipv4_addresses(ip)
                    .into_iter()
                    .flatten()
                    .any(|embedded| is_private_or_internal_ip(IpAddr::V4(embedded)))
        }
    }
}

/// IPv4 addresses carried inside transition-mechanism IPv6 prefixes. Each is
/// checked against the IPv4 policy so an IPv6 literal cannot smuggle a
/// private IPv4 target past the v4 range checks: RFC 4291 mapped and
/// compatible forms, RFC 6052 NAT64, RFC 3056 6to4, and RFC 4380 Teredo
/// (server address, plus the client address stored inverted).
fn embedded_ipv4_addresses(ip: Ipv6Addr) -> [Option<Ipv4Addr>; 5] {
    let s = ip.segments();
    let pair = |high: u16, low: u16| Ipv4Addr::from(((high as u32) << 16) | low as u32);
    let nat64 =
        (s[0] == 0x0064 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0]).then(|| pair(s[6], s[7]));
    let six_to_four = (s[0] == 0x2002).then(|| pair(s[1], s[2]));
    let teredo = s[0] == 0x2001 && s[1] == 0;
    let teredo_server = teredo.then(|| pair(s[2], s[3]));
    let teredo_client = teredo.then(|| Ipv4Addr::from(!(((s[6] as u32) << 16) | s[7] as u32)));
    [
        ip.to_ipv4(),
        nat64,
        six_to_four,
        teredo_server,
        teredo_client,
    ]
}

/// 64:ff9b:1::/48 is the local-use NAT64 prefix (RFC 8215): the embedded
/// address is site-specific, so the whole block is internal.
fn is_local_use_nat64(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 0x0001
}

fn is_unique_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_unicast_link_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

/// fec0::/10 site-local unicast (RFC 3879 deprecated it, stacks still route
/// it). It sits just outside the fe80::/10 link-local mask, so it needs its
/// own check rather than a wider mask that would also swallow fe00::/9.
fn is_site_local_ipv6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfec0
}

#[cfg(test)]
mod tests {
    use super::{
        scan_origin_allows_local_dev, validate_ip_target, validate_redirect_target_nonblocking,
        validate_resolved_domain_ip_target, validate_url_blocking, LocalOrigin, UrlPolicy,
    };
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn local_origin_matches_the_predicates_it_replaces() {
        use crate::core::localhost::{is_localhost, is_strict_localhost};
        for url in [
            "http://localhost:3000",
            "http://127.0.0.1:8080",
            "http://0.0.0.0:5173",
            "http://myapp.local",
            "http://myapp.localhost:3000",
            "http://[::1]:5173",
            "https://localhost.run",
            "https://example.com",
            "https://127.0.0.1.example.com",
        ] {
            let parsed = url::Url::parse(url).expect(url);
            let origin = LocalOrigin::classify(&parsed);
            assert_eq!(
                origin.is_local_environment(),
                is_localhost(&parsed),
                "{url}"
            );
            assert_eq!(
                origin.is_strict_loopback(),
                is_strict_localhost(&parsed),
                "{url}"
            );
            assert_eq!(
                origin.allows_local_dev(),
                scan_origin_allows_local_dev(&parsed),
                "{url}"
            );
        }
        // The one deliberate widening: every 127/8 literal is loopback.
        let wide = url::Url::parse("http://127.0.0.2:5173").unwrap();
        assert!(LocalOrigin::classify(&wide).is_strict_loopback());
        assert!(!is_strict_localhost(&wide));

        // A second deliberate widening: a trailing-dot FQDN form of
        // localhost normalizes the same way `is_local_dev_domain` (and
        // therefore `scan_origin_allows_local_dev`) already normalizes it
        // elsewhere in this file, so `LocalOrigin` treats "localhost." as
        // loopback even though the two `core::localhost` predicates compare
        // `host_str()` without trimming the trailing dot.
        let trailing_dot = url::Url::parse("http://localhost./").unwrap();
        let trailing_dot_origin = LocalOrigin::classify(&trailing_dot);
        assert!(trailing_dot_origin.is_strict_loopback());
        assert!(!is_strict_localhost(&trailing_dot));
        assert!(trailing_dot_origin.is_local_environment());
        assert!(!is_localhost(&trailing_dot));
        assert!(trailing_dot_origin.allows_local_dev());
        assert!(scan_origin_allows_local_dev(&trailing_dot));
    }

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

    // fec0::/10 sits outside the fe80::/10 link-local mask, so it needed its
    // own branch; fe00::/9 and the public 2000::/3 space must stay allowed.
    #[test]
    fn ip_target_rejects_ipv6_site_local_addresses() {
        for site_local in ["fec0::1", "feff:ffff::1", "fed0::8888"] {
            assert!(
                validate_ip_target(
                    IpAddr::V6(site_local.parse::<Ipv6Addr>().unwrap()),
                    UrlPolicy::ExternalCallback
                )
                .is_err(),
                "{site_local} is site-local and must be refused"
            );
        }
        for public in ["2606:4700::1111", "2001:4860:4860::8888"] {
            assert!(
                validate_ip_target(
                    IpAddr::V6(public.parse::<Ipv6Addr>().unwrap()),
                    UrlPolicy::ExternalCallback
                )
                .is_ok(),
                "{public} is public and must stay allowed"
            );
        }
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
    fn reserved_and_embedded_private_ranges_are_refused_with_public_neighbors_allowed() {
        let policy = UrlPolicy::ExternalCallback;
        let cases: &[(&str, bool)] = &[
            // 224.0.0.0/4 multicast and 240.0.0.0/4 reserved.
            ("224.0.0.1", false),
            ("239.255.255.255", false),
            ("223.255.255.255", true),
            ("240.0.0.1", false),
            ("255.255.255.254", false),
            // 198.18.0.0/15 benchmarking (RFC 2544).
            ("198.18.0.1", false),
            ("198.19.255.255", false),
            ("198.17.255.255", true),
            ("198.20.0.1", true),
            // 192.0.0.0/24 IETF protocol assignments (RFC 6890).
            ("192.0.0.1", false),
            ("192.0.0.255", false),
            ("192.0.1.1", true),
            // NAT64 64:ff9b::/96 carries an IPv4 address in the low 32 bits;
            // 64:ff9b:1::/48 is local-use NAT64 and always internal.
            ("64:ff9b::10.0.0.1", false),
            ("64:ff9b::a9fe:a9fe", false),
            ("64:ff9b::8.8.8.8", true),
            ("64:ff9b:1::1", false),
            // 6to4 2002::/16 carries the IPv4 address in bits 16-47.
            ("2002:0a00:0001::1", false),
            ("2002:c0a8:0101::1", false),
            ("2002:0808:0808::1", true),
            // Teredo 2001:0000::/32: server address in bits 32-63, client
            // address inverted in the low 32 bits.
            ("2001:0:0a00:1:0:0:f7f7:f7f7", false),
            ("2001:0:0808:0808:0:0:f5ff:fffe", false),
            ("2001:0:0808:0808:0:0:f7f7:f7f7", true),
            // IPv6 multicast and the deprecated IPv4-compatible form.
            ("ff02::1", false),
            ("::10.0.0.1", false),
            ("2606:4700::1111", true),
            ("1.1.1.1", true),
        ];
        for (address, allowed) in cases {
            let ip: IpAddr = address.parse().expect(address);
            assert_eq!(
                validate_ip_target(ip, policy).is_ok(),
                *allowed,
                "{address} should be {}",
                if *allowed { "allowed" } else { "refused" }
            );
        }
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
