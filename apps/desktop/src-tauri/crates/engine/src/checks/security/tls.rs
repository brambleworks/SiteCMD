//! Portable TLS facts and certificate verdicts shared across runtime adapters.
//!
//! Expiry and hostname compare across adapters; chain and protocol compare only
//! within their trust authority and client profile.

use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

pub const EXPIRY_CHECK_ID: &str = "security.ssl.expiry";
pub const HOSTNAME_CHECK_ID: &str = "security.ssl.hostname";
pub const CHAIN_CHECK_ID: &str = "security.ssl.chain";
pub const PROTOCOL_CHECK_ID: &str = "security.ssl.protocol";

/// Every certificate sub-check id, in emission order.
pub const TLS_CHECK_IDS: &[&str] = &[
    EXPIRY_CHECK_ID,
    HOSTNAME_CHECK_ID,
    CHAIN_CHECK_ID,
    PROTOCOL_CHECK_ID,
];

/// Which trust program validated (or rejected) the chain. The programs
/// differ from each other, which is exactly why the chain sub-verdict
/// compares within an authority rather than across adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustAuthority {
    /// The bundled WebPKI root set. Desktop stopped producing this value
    /// when it switched to rustls-platform-verifier (see
    /// `PlatformVerifier`); stored scan history from before that switch may
    /// still carry `"webpki"` and remains valid to deserialize. A wasm
    /// handshake, not yet implemented, would still use this authority.
    Webpki,
    /// The operating system's own trust program, reached through
    /// rustls-platform-verifier: Keychain on macOS, CryptoAPI on Windows,
    /// and on other Unix targets (including Linux) native certificates
    /// loaded through `rustls-native-certs`, not a bundled root set. This is
    /// what desktop's sync and async TLS probes both produce today.
    PlatformVerifier,
    /// Chromium's validator, as reported by a Browser Run navigation.
    Chromium,
    /// Cloudflare Workers' public-fetch and `node:tls` trust program.
    CloudflareWorkers,
}

impl TrustAuthority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webpki => "webpki",
            Self::PlatformVerifier => "platform_verifier",
            Self::Chromium => "chromium",
            Self::CloudflareWorkers => "cloudflare_workers",
        }
    }
}

/// What the authority concluded about the served chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationResult {
    Valid,
    Invalid,
    /// The adapter could not obtain a chain verdict at all.
    Unavailable,
}

/// Chain verdict paired with its trust authority and invariant-safe detail.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TlsValidation {
    pub authority: TrustAuthority,
    pub result: ValidationResult,
    /// The authority's rejection reason, present exactly when the result is
    /// `Invalid`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl TlsValidation {
    pub fn valid(authority: TrustAuthority) -> Self {
        Self {
            authority,
            result: ValidationResult::Valid,
            detail: None,
        }
    }

    pub fn invalid(authority: TrustAuthority, detail: impl Into<String>) -> Self {
        Self {
            authority,
            result: ValidationResult::Invalid,
            detail: Some(detail.into()),
        }
    }

    pub fn unavailable(authority: TrustAuthority) -> Self {
        Self {
            authority,
            result: ValidationResult::Unavailable,
            detail: None,
        }
    }
}

/// The certificate facts every adapter supplies. Absent fields are genuinely
/// unavailable from that adapter, never guessed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TlsFacts {
    #[serde(default)]
    pub not_before: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub not_after: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub issuer: Option<String>,
    /// The subject common name plus the SAN dNSName list.
    #[serde(default)]
    pub subject_names: Vec<String>,
    /// Negotiated TLS version, e.g. "TLSv1.3".
    #[serde(default)]
    pub protocol: Option<String>,
    pub validation: TlsValidation,
    /// When the facts were captured. Re-evaluation may only grade facts
    /// younger than the freshness horizon; staler facts are inconclusive.
    pub facts_observed_at: chrono::DateTime<chrono::Utc>,
}

/// Why the transport produced no certificate facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsUnavailable {
    /// The scanned URL is not HTTPS, so there is no certificate to probe.
    NotHttps,
    /// The URL carried no host to connect to.
    NoHost,
    /// The probe never reached a TLS verdict (TCP connect, timeout, setup).
    Transport { detail: String },
    /// The probe task itself did not return a verdict.
    ProbeFailed { detail: String },
}

impl TlsUnavailable {
    fn reason(&self) -> &'static str {
        match self {
            Self::NotHttps => "non_https_url",
            Self::NoHost => "no_host_in_url",
            Self::Transport { .. } => "transport_failure",
            Self::ProbeFailed { .. } => "probe_failed",
        }
    }

    fn description(&self) -> String {
        match self {
            Self::NotHttps => "The scanned URL uses HTTP, so there is no TLS certificate on this response to probe. The HTTPS enforcement check reports the transport condition without duplicating it here.".into(),
            Self::NoHost => "Could not determine host from URL, so no TLS certificate could be requested.".into(),
            Self::Transport { detail } => format!(
                "The dedicated TLS probe could not reach the server ({}), so it produced no certificate facts. The page itself loaded over HTTPS during this scan, so this is most likely a transient network issue rather than a certificate problem. Re-scan to retry the probe.",
                detail
            ),
            Self::ProbeFailed { detail } => format!(
                "The certificate probe did not return a verdict ({}), so no certificate facts were captured. Re-scan before drawing any certificate conclusion.",
                detail
            ),
        }
    }

    fn confidence(&self) -> IssueConfidence {
        match self {
            // Not applicable is a fact, not an inconclusive measurement.
            Self::NotHttps | Self::NoHost => IssueConfidence::High,
            Self::Transport { .. } | Self::ProbeFailed { .. } => IssueConfidence::NeedsReview,
        }
    }
}

/// The certificate facts a leaf DER yields, shared by every adapter that
/// holds one (the desktop handshake and the hosted pinned TLS probe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafCertificateFacts {
    pub not_before: Option<chrono::DateTime<chrono::Utc>>,
    pub not_after: Option<chrono::DateTime<chrono::Utc>>,
    pub issuer: Option<String>,
    pub subject_names: Vec<String>,
}

/// Parse a leaf certificate's DER bytes into the facts the verdicts need.
/// Returns `None` only when the DER itself does not parse; individual fields
/// stay `None`/empty when the certificate omits them.
pub fn parse_leaf_certificate(der: &[u8]) -> Option<LeafCertificateFacts> {
    use x509_parser::prelude::*;

    let (_, cert) = X509Certificate::from_der(der).ok()?;
    let to_utc = |timestamp: i64| chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0);

    let mut subject_names: Vec<String> = cert
        .subject()
        .iter_common_name()
        .filter_map(|name| name.as_str().ok())
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect();
    if let Ok(Some(san)) = cert.subject_alternative_name() {
        for name in &san.value.general_names {
            if let GeneralName::DNSName(dns) = name {
                let dns = dns.trim().to_ascii_lowercase();
                if !dns.is_empty() && !subject_names.contains(&dns) {
                    subject_names.push(dns);
                }
            }
        }
    }

    Some(LeafCertificateFacts {
        not_before: to_utc(cert.validity().not_before.timestamp()),
        not_after: to_utc(cert.validity().not_after.timestamp()),
        issuer: Some(cert.issuer().to_string()).filter(|issuer| !issuer.is_empty()),
        subject_names,
    })
}

/// Whether a certificate name covers the host, per the usual single-label
/// wildcard rule (`*.example.com` matches `a.example.com` but neither
/// `example.com` nor `a.b.example.com`).
pub fn certificate_name_matches(name: &str, host: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    let host = host.trim().to_ascii_lowercase();
    if name.is_empty() || host.is_empty() {
        return false;
    }
    let Some(suffix) = name.strip_prefix("*.") else {
        return name == host;
    };
    // A wildcard replaces exactly one leading label, and never matches the
    // bare parent domain.
    match host.split_once('.') {
        Some((label, rest)) => !label.is_empty() && rest == suffix,
        None => false,
    }
}

fn tls_result(
    check_id: &str,
    title: String,
    description: String,
    status: CheckStatus,
    severity: Severity,
    manual_fix: Option<String>,
    raw_data: serde_json::Value,
    confidence: IssueConfidence,
    confidence_reason: Option<String>,
    why_it_matters: Option<String>,
) -> CheckResult {
    CheckResult {
        check_id: check_id.into(),
        category: ScanCategory::Security,
        title,
        description,
        status,
        severity,
        fix_prompt: None,
        manual_fix,
        raw_data: Some(raw_data),
        confidence,
        confidence_reason,
        why_it_matters,
    }
}

/// Every sub-check reports the same coverage exception when no facts exist.
/// Emitting one row per id (rather than a single combined row) keeps
/// per-check coverage accounting honest: each sub-check really was not
/// evaluated.
pub fn tls_unavailable_results(reason: &TlsUnavailable) -> Vec<CheckResult> {
    let description = reason.description();
    let confidence = reason.confidence();
    let confidence_reason = match reason {
        TlsUnavailable::NotHttps | TlsUnavailable::NoHost => None,
        TlsUnavailable::Transport { .. } => Some(
            "The probe failed before the TLS handshake produced facts; re-scan to distinguish a network blip from a real connectivity problem.".to_string(),
        ),
        TlsUnavailable::ProbeFailed { .. } => Some(
            "The certificate task did not return TLS facts; re-scan before drawing any certificate conclusion.".to_string(),
        ),
    };
    TLS_CHECK_IDS
        .iter()
        .map(|check_id| {
            tls_result(
                check_id,
                "TLS certificate facts unavailable".into(),
                description.clone(),
                CheckStatus::Skipped,
                Severity::Low,
                None,
                serde_json::json!({"reason": reason.reason(), "facts_available": false}),
                confidence,
                confidence_reason.clone(),
                None,
            )
        })
        .collect()
}

/// Grade all four sub-checks from one set of captured facts.
pub fn evaluate_tls(
    host: &str,
    facts: &TlsFacts,
    evaluation_time: chrono::DateTime<chrono::Utc>,
) -> Vec<CheckResult> {
    vec![
        evaluate_expiry(facts, evaluation_time),
        evaluate_hostname(host, facts),
        evaluate_chain(facts),
        evaluate_protocol(facts),
    ]
}

/// `security.ssl.expiry` - clock-dependent, compared across adapters.
fn evaluate_expiry(
    facts: &TlsFacts,
    evaluation_time: chrono::DateTime<chrono::Utc>,
) -> CheckResult {
    // Missing expiry data is ungraded, never assumed valid.
    let Some(not_after) = facts.not_after else {
        return tls_result(
            EXPIRY_CHECK_ID,
            "TLS certificate expiry not available".into(),
            "The TLS handshake succeeded but the certificate's expiry date was not available from this adapter, so expiry was not graded. Verify it manually (e.g. `openssl s_client -connect host:443 | openssl x509 -noout -dates`).".into(),
            CheckStatus::Skipped,
            Severity::Low,
            Some("Inspect the certificate served by the public hostname in browser developer tools and an independent TLS diagnostic. Confirm the notBefore/notAfter values, hostname coverage, and chain before deciding whether renewal or replacement is needed.".into()),
            serde_json::json!({"expiry_available": false}),
            IssueConfidence::NeedsReview,
            Some("Certificate expiry was not supplied by this adapter; manual verification required.".into()),
            None,
        );
    };

    let days_remaining = (not_after.date_naive() - evaluation_time.date_naive()).num_days();
    let (status, severity, title, description) = expiry_verdict(days_remaining);
    let why = match (&status, &severity) {
        (CheckStatus::Fail, Severity::Critical) => Some(
            "Mainstream clients reject expired certificates or present a security interstitial; HSTS can make the failure non-bypassable.".to_string(),
        ),
        (CheckStatus::Warn, Severity::High) => Some(
            "If the served certificate is not renewed or replaced before expiry, clients will begin rejecting it or presenting a security warning.".to_string(),
        ),
        _ => None,
    };
    let manual_fix = matches!(status, CheckStatus::Fail | CheckStatus::Warn).then(|| "Inspect the certificate currently served at every public edge and hostname, then confirm the provider or ACME client's scheduled renewal state (including any ACME Renewal Information window). Renew or replace it when necessary, deploy the complete chain, and verify the new certificate over IPv4 and IPv6 before expiry.".to_string());

    tls_result(
        EXPIRY_CHECK_ID,
        title.into(),
        description,
        status,
        severity,
        manual_fix,
        serde_json::json!({
            "issuer": facts.issuer.as_deref().map(crate::log_sanitizer::bounded_issue_evidence),
            "days_until_expiry": days_remaining,
            "not_before": facts.not_before.map(|value| value.to_rfc3339()),
            "not_after": not_after.to_rfc3339(),
            "facts_observed_at": facts.facts_observed_at.to_rfc3339(),
        }),
        IssueConfidence::High,
        None,
        why,
    )
}

/// Grades certificate expiry. Seven days or less warns; eight to thirty days
/// passes without claiming to observe renewal automation.
fn expiry_verdict(days_remaining: i64) -> (CheckStatus, Severity, &'static str, String) {
    if days_remaining < 0 {
        let days_ago = -days_remaining;
        (
            CheckStatus::Fail,
            Severity::Critical,
            "SSL certificate expired",
            format!(
                "SSL certificate expired {} day{} ago.",
                days_ago,
                if days_ago == 1 { "" } else { "s" }
            ),
        )
    } else if days_remaining <= 7 {
        (
            CheckStatus::Warn,
            Severity::High,
            "SSL certificate expires within 7 days",
            if days_remaining == 0 {
                "SSL certificate expires today. Confirm the provider/client renewal state now and renew or replace the served certificate before it becomes invalid.".into()
            } else {
                format!(
                    "SSL certificate expires in {} day{}. Confirm the provider/client renewal state now and renew or replace the served certificate before it becomes invalid.",
                    days_remaining,
                    if days_remaining == 1 { "" } else { "s" }
                )
            },
        )
    } else if days_remaining <= 30 {
        (
            CheckStatus::Pass,
            Severity::Low,
            "SSL/TLS certificate validity",
            format!(
                "SSL certificate is currently valid and expires in {} days. It may already be eligible or scheduled for renewal based on its lifetime, client policy, or CA-provided ARI window; this scan does not observe renewal state.",
                days_remaining
            ),
        )
    } else {
        (
            CheckStatus::Pass,
            Severity::Low,
            "SSL/TLS certificate validity",
            format!(
                "SSL certificate is valid. Expires in {} days.",
                days_remaining
            ),
        )
    }
}

/// `security.ssl.hostname` - deterministic, compared across adapters:
/// name matching is data.
fn evaluate_hostname(host: &str, facts: &TlsFacts) -> CheckResult {
    if facts.subject_names.is_empty() {
        return tls_result(
            HOSTNAME_CHECK_ID,
            "TLS certificate names not available".into(),
            "The certificate's subject and subject-alternative names were not available from this adapter, so hostname coverage was not graded.".into(),
            CheckStatus::Skipped,
            Severity::Low,
            None,
            serde_json::json!({"names_available": false}),
            IssueConfidence::NeedsReview,
            Some("Certificate names were not supplied by this adapter, so the scanned hostname could not be matched against them.".into()),
            None,
        );
    }

    let matched = facts
        .subject_names
        .iter()
        .any(|name| certificate_name_matches(name, host));
    let sample: Vec<String> = facts.subject_names.iter().take(10).cloned().collect();
    let raw_data = serde_json::json!({
        "host": host,
        "matched": matched,
        "certificate_names": sample,
        "certificate_name_count": facts.subject_names.len(),
    });

    if matched {
        return tls_result(
            HOSTNAME_CHECK_ID,
            "TLS certificate covers the scanned hostname".into(),
            format!(
                "The served certificate lists a name covering {}. This compares the scanned hostname against the certificate's subject and SAN entries; it does not evaluate chain trust or expiry, which are graded separately.",
                host
            ),
            CheckStatus::Pass,
            Severity::Low,
            None,
            raw_data,
            IssueConfidence::High,
            None,
            None,
        );
    }

    tls_result(
        HOSTNAME_CHECK_ID,
        "TLS certificate does not cover the scanned hostname".into(),
        format!(
            "None of the served certificate's {} name entr{} covers {}. Clients validating this hostname against this certificate will reject it, though a different edge, region, or SNI value can serve a different certificate.",
            facts.subject_names.len(),
            if facts.subject_names.len() == 1 { "y" } else { "ies" },
            host
        ),
        CheckStatus::Fail,
        Severity::Critical,
        Some("Issue or install a certificate whose subject alternative names cover every public hostname this site serves, including apex and www forms where both are used. Deploy it at every edge that terminates TLS for the hostname, then re-verify with the exact SNI value clients send.".into()),
        raw_data,
        IssueConfidence::High,
        None,
        Some("A certificate that does not name the hostname causes a browser security interstitial that most visitors cannot safely bypass.".into()),
    )
}

/// `security.ssl.chain` - deterministic, compared WITHIN a trust authority:
/// the platform verifier and Chromium are different trust programs, so a
/// disagreement is not necessarily a site change.
fn evaluate_chain(facts: &TlsFacts) -> CheckResult {
    let authority = facts.validation.authority;
    match facts.validation.result {
        ValidationResult::Valid => tls_result(
            CHAIN_CHECK_ID,
            "TLS chain accepted".into(),
            format!(
                "The served certificate chain was accepted by the {} trust program during this scan. Other clients use different trust stores and policies, so this is evidence for that authority rather than universal acceptance.",
                authority.as_str()
            ),
            CheckStatus::Pass,
            Severity::Low,
            None,
            serde_json::json!({"authority": authority.as_str(), "result": "valid"}),
            IssueConfidence::High,
            None,
            None,
        ),
        ValidationResult::Invalid => {
            // The constructor guarantees a detail accompanies Invalid; an
            // adapter that hand-built the struct without one still grades,
            // just without a specific condition to name.
            let raw_detail = facts.validation.detail.as_deref().unwrap_or("no reason reported");
            let definitive = certificate_validation_is_definitive(raw_detail);
            let detail = crate::log_sanitizer::bounded_issue_evidence(raw_detail);
            tls_result(
                CHAIN_CHECK_ID,
                if definitive {
                    "Certificate failed a definitive WebPKI validity check".into()
                } else {
                    "Certificate trust differs between probe paths".into()
                },
                if definitive {
                    format!(
                        "The {} trust program rejected the served certificate for a specific validity condition: {}",
                        authority.as_str(),
                        detail
                    )
                } else {
                    format!(
                        "The {} trust program rejected the served certificate ({}), while the platform-backed page request succeeded. Trust stores and enterprise roots can differ, so this result requires comparison with supported browsers and the publicly served chain.",
                        authority.as_str(),
                        detail
                    )
                },
                if definitive { CheckStatus::Fail } else { CheckStatus::Warn },
                if definitive { Severity::Critical } else { Severity::High },
                Some("Inspect the exact certificate and intermediate chain served for the hostname in supported browsers and an independent public TLS diagnostic. Correct hostname coverage, validity dates, chain order/completeness, or the authoritative trust configuration as the evidence requires; do not replace a valid private/enterprise certificate solely because a public-root probe does not trust it.".into()),
                serde_json::json!({
                    "authority": authority.as_str(),
                    "result": "invalid",
                    "error": detail,
                    "definitive": definitive,
                }),
                if definitive { IssueConfidence::High } else { IssueConfidence::NeedsReview },
                (!definitive).then(|| "The bundled trust path rejected the certificate, but the platform-backed fetch succeeded; supported client trust stores and the served chain must be compared before claiming visitor impact.".into()),
                Some(if definitive {
                    "Clients enforcing the same validity rule will reject the certificate or present a security interstitial.".into()
                } else {
                    "Some supported clients may reject a chain they cannot anchor, but this probe alone does not establish a universal browser failure.".into()
                }),
            )
        }
        ValidationResult::Unavailable => tls_result(
            CHAIN_CHECK_ID,
            "TLS chain validity not available".into(),
            format!(
                "This adapter captured certificate facts but no chain verdict from the {} trust program, so chain validity was not graded.",
                authority.as_str()
            ),
            CheckStatus::Skipped,
            Severity::Low,
            None,
            serde_json::json!({"authority": authority.as_str(), "result": "unavailable"}),
            IssueConfidence::NeedsReview,
            Some("The adapter supplied no chain-validation result, so trust could not be assessed from this scan.".into()),
            None,
        ),
    }
}

/// `security.ssl.protocol` - deterministic, compared WITHIN a TLS client
/// profile: the negotiated version is a function of the client hello, so two
/// adapters can negotiate differently against an unchanged server.
fn evaluate_protocol(facts: &TlsFacts) -> CheckResult {
    let Some(protocol) = facts.protocol.as_deref() else {
        return tls_result(
            PROTOCOL_CHECK_ID,
            "Negotiated TLS version not available".into(),
            "This adapter did not report the negotiated TLS version, so the protocol was not graded.".into(),
            CheckStatus::Skipped,
            Severity::Low,
            None,
            serde_json::json!({"protocol_available": false}),
            IssueConfidence::NeedsReview,
            Some("The adapter supplied no negotiated protocol version for this connection.".into()),
            None,
        );
    };

    let deprecated = protocol_is_deprecated(protocol);
    let raw_data = serde_json::json!({
        "protocol": protocol,
        "authority": facts.validation.authority.as_str(),
        "deprecated": deprecated,
    });

    if deprecated {
        return tls_result(
            PROTOCOL_CHECK_ID,
            format!("Connection negotiated {}", protocol),
            format!(
                "This scanner's client negotiated {} with the server. TLS 1.0 and 1.1 are deprecated (RFC 8996) and are refused by current mainstream browsers. The negotiated version depends on the client hello, so a different client may negotiate differently against the same server.",
                protocol
            ),
            CheckStatus::Fail,
            Severity::High,
            Some("Disable TLS 1.0 and 1.1 at every edge that terminates TLS for this hostname and require TLS 1.2 or later, keeping the cipher suites your supported clients need. Re-test the public endpoint afterward, including any CDN, load balancer, and origin separately.".into()),
            raw_data,
            IssueConfidence::High,
            None,
            Some("Mainstream browsers refuse deprecated TLS versions outright, so clients that cannot negotiate a newer version reach an error page instead of the site.".into()),
        );
    }

    tls_result(
        PROTOCOL_CHECK_ID,
        "Negotiated TLS version".into(),
        format!(
            "This scanner's client negotiated {} with the server. The negotiated version depends on the client hello, so this records what this client reached, not the full set of versions the server still accepts.",
            protocol
        ),
        CheckStatus::Pass,
        Severity::Low,
        None,
        raw_data,
        IssueConfidence::High,
        None,
        None,
    )
}

/// TLS versions current mainstream browsers refuse (RFC 8996).
fn protocol_is_deprecated(protocol: &str) -> bool {
    let normalized = protocol.to_ascii_lowercase().replace([' ', '_'], "");
    ["tlsv1.0", "tlsv1", "tls1.0", "tls1", "tlsv1.1", "tls1.1"]
        .iter()
        .any(|marker| normalized == *marker)
        || normalized.starts_with("ssl")
}

fn certificate_validation_is_definitive(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    [
        "expired",
        "notvalidyet",
        "not valid yet",
        "notvalidforname",
        "not valid for name",
        "revoked",
        "badsignature",
        "bad signature",
        "invalid signature",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[cfg(test)]
#[path = "tls_tests.rs"]
mod tests;
