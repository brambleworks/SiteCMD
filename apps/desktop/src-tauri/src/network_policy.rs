//! Shared SSRF validation for scans, sitemaps, webhooks, and redirects.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

#[derive(Debug, Clone, Copy)]
pub enum UrlPolicy {
    /// The scan target a person named, and the fetches subordinate to it.
    ///
    /// SiteCMD runs as that person, on their machine, with their network, so
    /// naming an address they can already reach grants no access they did not
    /// have: this policy reaches loopback and the private networks the machine
    /// is attached to. Link-local stays refused because cloud metadata lives
    /// at `169.254.169.254` and no site anyone means to scan does, and a scan
    /// body is persisted, exported, and can be sent to an AI provider.
    ScanTarget,
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
            Self::ScanTarget | Self::Scan => true,
            Self::ExternalCallback => false,
            Self::Redirect { allow_local_dev } => allow_local_dev,
        }
    }

    /// Whether this policy may reach the private networks this machine is
    /// attached to, rather than only the machine itself. Only a target the
    /// person named, and the fetches that target leads to, may.
    fn allows_private_network(self) -> bool {
        matches!(self, Self::ScanTarget)
    }

    fn label(self) -> &'static str {
        match self {
            Self::ScanTarget | Self::Scan => "URL",
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
    /// A literal address on a private network this machine is attached to.
    /// A scan may reach it because the person named it, but it is another
    /// machine, so certificate verification stays on.
    PrivateNetwork,
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
            Some(url::Host::Ipv4(ip)) if is_private_network_ip(IpAddr::V4(ip)) => {
                Self::PrivateNetwork
            }
            Some(url::Host::Ipv6(ip)) if is_private_network_ip(IpAddr::V6(ip)) => {
                Self::PrivateNetwork
            }
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

    pub fn allows_local_dev(self) -> bool {
        matches!(self, Self::Loopback | Self::LocalhostDomain)
    }

    /// Replaces `core::localhost::is_localhost`.
    pub fn is_local_environment(self) -> bool {
        !matches!(self, Self::Public)
    }

    /// Classification that also resolves a hostname, so a name pointing into a
    /// private network (a container's service name, a machine on the LAN) is
    /// recognized as one. A scan calls this once for the target the person
    /// named, because a client must be given that origin's reach and no more.
    pub async fn classify_resolved(url: &url::Url) -> Self {
        let sync = Self::classify(url);
        let Some(url::Host::Domain(domain)) = url.host() else {
            return sync;
        };
        if sync != Self::Public {
            return sync;
        }
        let normalized = domain.trim_end_matches('.').to_ascii_lowercase();
        let port = url.port_or_known_default().unwrap_or(80);
        let Ok(addrs) = tokio::net::lookup_host((normalized.as_str(), port)).await else {
            return sync;
        };
        Self::from_resolved_addresses(sync, addrs.map(|addr| addr.ip()))
    }

    /// Private reach is earned only when every address a name answers with
    /// is private. A name that also answers publicly is a public site, and
    /// promoting it would hand that site's pages the LAN and loopback reach
    /// `ScanTarget` grants. Loopback behind a public name is DNS rebinding,
    /// refused elsewhere in this file; it must not be promoted to a reach.
    fn from_resolved_addresses(sync: Self, addrs: impl IntoIterator<Item = IpAddr>) -> Self {
        let mut private_only = false;
        for ip in addrs {
            if is_loopback_ip(ip) || !is_private_network_ip(ip) {
                return sync;
            }
            private_only = true;
        }
        if private_only {
            Self::PrivateNetwork
        } else {
            sync
        }
    }

    /// The policy for a URL this origin's page steers the scan to. A page can
    /// name any URL it likes; it never earns more reach than the origin the
    /// person asked for.
    pub fn subordinate_policy(self) -> UrlPolicy {
        match self {
            Self::PrivateNetwork => UrlPolicy::ScanTarget,
            Self::Loopback | Self::LocalhostDomain => UrlPolicy::Redirect {
                allow_local_dev: true,
            },
            Self::LocalNetworkName | Self::Public => UrlPolicy::Redirect {
                allow_local_dev: false,
            },
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

pub(crate) async fn validate_url_target(url: &url::Url, policy: UrlPolicy) -> Result<(), String> {
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
pub fn validate_page_subresource_target(url: &url::Url, policy: UrlPolicy) -> Result<(), String> {
    validate_redirect_target_nonblocking(url, policy)
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
    if policy.allows_private_network() && is_private_network_ip(ip) {
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

/// Address space a person's own machine and networks occupy: loopback, the
/// RFC 1918 private ranges, the RFC 6598 shared range that carrier-grade NAT
/// and tailnets use, and IPv6 unique-local, plus the IPv4-mapped spellings of
/// those. A named scan target may reach these.
///
/// Deliberately narrower than [`is_private_or_internal_ip`]: link-local
/// (`169.254.0.0/16`, `fe80::/10`) is excluded because cloud metadata answers
/// there, and multicast, broadcast, and the reserved blocks are excluded
/// because they are not hosts. Anything this does not name is still measured
/// against the wider deny list, so a narrow allow list cannot open a range.
pub(crate) fn is_private_network_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, ..] = ip.octets();
            ip.is_private() || ip.is_loopback() || (a == 100 && (64..=127).contains(&b))
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || is_unique_local_ipv6(ip)
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| is_private_network_ip(IpAddr::V4(mapped)))
        }
    }
}

/// Ranges that only exist inside a private, reserved, or internal network.
/// Nothing here is ever a public site, so it is the deny list every policy is
/// measured against.
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
#[path = "network_policy_tests.rs"]
mod tests;
